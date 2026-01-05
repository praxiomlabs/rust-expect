//! SSH session management.
//!
//! This module provides SSH session handling with actual russh integration
//! when the `ssh` feature is enabled.

use std::sync::Arc;
use std::time::Duration;

use super::auth::{AuthMethod, HostKeyVerification, SshCredentials};
#[cfg(feature = "ssh")]
use crate::error::SshError;

/// SSH session configuration.
#[derive(Debug, Clone)]
pub struct SshConfig {
    /// Host to connect to.
    pub host: String,
    /// Port (default 22).
    pub port: u16,
    /// Credentials.
    pub credentials: SshCredentials,
    /// Connection timeout.
    pub connect_timeout: Duration,
    /// Host key verification policy.
    pub host_key_verification: HostKeyVerification,
    /// Enable compression.
    pub compression: bool,
    /// TCP keepalive interval.
    pub tcp_keepalive: Option<Duration>,
}

impl Default for SshConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 22,
            credentials: SshCredentials::default(),
            connect_timeout: Duration::from_secs(30),
            host_key_verification: HostKeyVerification::default(),
            compression: false,
            tcp_keepalive: Some(Duration::from_secs(60)),
        }
    }
}

impl SshConfig {
    /// Create new config for a host.
    #[must_use]
    pub fn new(host: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            ..Default::default()
        }
    }

    /// Set port.
    #[must_use]
    pub const fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Set credentials.
    #[must_use]
    pub fn credentials(mut self, credentials: SshCredentials) -> Self {
        self.credentials = credentials;
        self
    }

    /// Set username.
    #[must_use]
    pub fn username(mut self, username: impl Into<String>) -> Self {
        self.credentials.username = username.into();
        self
    }

    /// Set connect timeout.
    #[must_use]
    pub const fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Set host key verification.
    #[must_use]
    pub const fn host_key_verification(mut self, policy: HostKeyVerification) -> Self {
        self.host_key_verification = policy;
        self
    }

    /// Enable compression.
    #[must_use]
    pub const fn with_compression(mut self) -> Self {
        self.compression = true;
        self
    }

    /// Get the address string.
    #[must_use]
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// SSH session state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshSessionState {
    /// Not connected.
    Disconnected,
    /// Connecting.
    Connecting,
    /// Authenticating.
    Authenticating,
    /// Connected and ready.
    Connected,
    /// Error state.
    Error,
}

// ============================================================================
// russh integration (when ssh feature is enabled)
// ============================================================================

#[cfg(feature = "ssh")]
mod russh_impl {
    use std::path::Path;

    use russh::client;
    use russh::keys::{PrivateKey, PrivateKeyWithHashAlg, PublicKey};

    use super::{Arc, AuthMethod, HostKeyVerification, SshCredentials, SshError};

    /// Client handler for russh that manages host key verification.
    pub struct SshClientHandler {
        /// Host key verification policy.
        pub host_key_verification: HostKeyVerification,
        /// The host we're connecting to.
        pub host: String,
        /// The port we're connecting to.
        pub port: u16,
    }

    impl client::Handler for SshClientHandler {
        type Error = russh::Error;

        /// Verify the server's host key.
        async fn check_server_key(
            &mut self,
            server_public_key: &PublicKey,
        ) -> Result<bool, Self::Error> {
            match self.host_key_verification {
                #[cfg(feature = "insecure-skip-verify")]
                HostKeyVerification::AcceptAll => {
                    tracing::warn!(
                        host = %self.host,
                        "Accepting server key without verification (INSECURE)"
                    );
                    Ok(true)
                }
                HostKeyVerification::RejectUnknown => {
                    tracing::debug!(
                        host = %self.host,
                        key = ?server_public_key,
                        "Rejecting unknown host key"
                    );
                    Ok(false)
                }
                HostKeyVerification::KnownHosts => {
                    // Check against known_hosts file using russh-keys
                    check_known_hosts(&self.host, self.port, server_public_key)
                }
                HostKeyVerification::Tofu => {
                    // Trust on first use - accept and save to known_hosts
                    handle_tofu(&self.host, self.port, server_public_key)
                }
            }
        }
    }

    /// Check a server key against the `known_hosts` file.
    #[allow(clippy::unnecessary_wraps)]
    fn check_known_hosts(
        host: &str,
        port: u16,
        server_public_key: &PublicKey,
    ) -> Result<bool, russh::Error> {
        let known_hosts_path = get_known_hosts_path();

        if !known_hosts_path.exists() {
            tracing::warn!(
                host = %host,
                path = %known_hosts_path.display(),
                "known_hosts file not found, rejecting key"
            );
            return Ok(false);
        }

        // Read and parse the known_hosts file
        let contents = match std::fs::read_to_string(&known_hosts_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    host = %host,
                    error = %e,
                    "Failed to read known_hosts file"
                );
                return Ok(false);
            }
        };

        // Build the host pattern to search for
        // Standard SSH uses "host" for port 22, "[host]:port" for non-standard ports
        let host_pattern = if port == 22 {
            host.to_string()
        } else {
            format!("[{host}]:{port}")
        };

        // Parse each line looking for matching host entries
        for line in contents.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Skip markers like @cert-authority and @revoked for now
            if line.starts_with('@') {
                continue;
            }

            // Parse: hostnames keytype base64key [comment]
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                continue;
            }

            let hostnames = parts[0];
            let key_type = parts[1];
            let key_data = parts[2];

            // Check if this line matches our host
            let host_matches = hostnames.split(',').any(|h| {
                let h = h.trim();
                h == host || h == host_pattern || h == format!("{host},*") || h == "*"
            });

            if !host_matches {
                continue;
            }

            // Try to parse and compare the key
            if let Some(stored_key) = parse_known_hosts_key(key_type, key_data) {
                if keys_match(&stored_key, server_public_key) {
                    tracing::debug!(
                        host = %host,
                        "Host key verified against known_hosts"
                    );
                    return Ok(true);
                }
                // Key mismatch - potential MITM attack!
                tracing::error!(
                    host = %host,
                    "HOST KEY MISMATCH! Possible man-in-the-middle attack!"
                );
                return Ok(false);
            }
        }

        // Host not found in known_hosts
        tracing::warn!(
            host = %host,
            "Host not found in known_hosts file"
        );
        Ok(false)
    }

    /// Parse a public key from `known_hosts` format.
    fn parse_known_hosts_key(key_type: &str, key_data: &str) -> Option<PublicKey> {
        // The key_type should match what's in the decoded data
        // Common types: ssh-rsa, ssh-ed25519, ecdsa-sha2-nistp256, etc.
        match key_type {
            "ssh-ed25519"
            | "ssh-rsa"
            | "ecdsa-sha2-nistp256"
            | "ecdsa-sha2-nistp384"
            | "ecdsa-sha2-nistp521" => {
                // Try to parse using russh-keys (takes base64 directly)
                russh::keys::parse_public_key_base64(key_data).ok()
            }
            _ => {
                tracing::debug!(key_type = %key_type, "Unknown key type in known_hosts");
                None
            }
        }
    }

    /// Compare two public keys for equality.
    fn keys_match(stored: &PublicKey, server: &PublicKey) -> bool {
        // Compare the key fingerprints using SHA-256 (standard for OpenSSH)
        use russh::keys::HashAlg;
        stored.fingerprint(HashAlg::Sha256) == server.fingerprint(HashAlg::Sha256)
    }

    /// Handle Trust On First Use - accept and save the key.
    #[allow(clippy::unnecessary_wraps)]
    fn handle_tofu(
        host: &str,
        port: u16,
        server_public_key: &PublicKey,
    ) -> Result<bool, russh::Error> {
        let known_hosts_path = get_known_hosts_path();

        // Create .ssh directory if it doesn't exist
        if let Some(parent) = known_hosts_path.parent() {
            if !parent.exists() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    tracing::warn!(
                        error = %e,
                        "Failed to create .ssh directory, accepting key without saving"
                    );
                    return Ok(true);
                }
                // Set proper permissions on Unix
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ =
                        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
                }
            }
        }

        // Format the host entry
        let host_entry = if port == 22 {
            host.to_string()
        } else {
            format!("[{host}]:{port}")
        };

        // Get the key in OpenSSH format
        let key_str = format_public_key_openssh(server_public_key);

        // Append to known_hosts
        let line = format!("{host_entry} {key_str}\n");

        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&known_hosts_path)
        {
            Ok(mut file) => {
                use std::io::Write;
                if let Err(e) = file.write_all(line.as_bytes()) {
                    tracing::warn!(
                        error = %e,
                        "Failed to write to known_hosts, accepting key without saving"
                    );
                } else {
                    tracing::info!(
                        host = %host,
                        path = %known_hosts_path.display(),
                        "Added host key to known_hosts (TOFU)"
                    );

                    // Set proper permissions on Unix
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let _ = std::fs::set_permissions(
                            &known_hosts_path,
                            std::fs::Permissions::from_mode(0o644),
                        );
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Failed to open known_hosts for writing, accepting key without saving"
                );
            }
        }

        Ok(true)
    }

    /// Format a public key in OpenSSH format for `known_hosts`.
    fn format_public_key_openssh(key: &PublicKey) -> String {
        // Use the built-in to_openssh method which returns "key_type base64_data [comment]"
        // For known_hosts we strip the comment portion
        key.to_openssh()
            .unwrap_or_else(|_| format!("{} <encoding-error>", key.algorithm().as_str()))
            .split_whitespace()
            .take(2)
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Get the default `known_hosts` path.
    fn get_known_hosts_path() -> std::path::PathBuf {
        // Check for custom path in environment
        if let Ok(path) = std::env::var("SSH_KNOWN_HOSTS") {
            return std::path::PathBuf::from(path);
        }

        // Use standard location
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        std::path::PathBuf::from(home)
            .join(".ssh")
            .join("known_hosts")
    }

    /// Load a private key from a file.
    ///
    /// Supports both unencrypted and encrypted private keys. For encrypted keys,
    /// provide the passphrase to decrypt the key.
    ///
    /// # Supported Formats
    ///
    /// - OpenSSH format (both encrypted and unencrypted)
    /// - PKCS#8 format (both encrypted and unencrypted)
    /// - PEM format
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Load an unencrypted key
    /// let key = load_private_key(Path::new("~/.ssh/id_ed25519"), None).await?;
    ///
    /// // Load an encrypted key with passphrase
    /// let key = load_private_key(
    ///     Path::new("~/.ssh/id_rsa"),
    ///     Some("my_passphrase"),
    /// ).await?;
    /// ```
    pub async fn load_private_key(
        path: &Path,
        passphrase: Option<&str>,
    ) -> crate::error::Result<Arc<PrivateKey>> {
        let key_data = tokio::fs::read(path).await.map_err(|e| {
            crate::error::ExpectError::Ssh(SshError::Authentication {
                user: String::new(),
                reason: format!("Failed to read key file {}: {}", path.display(), e),
            })
        })?;

        // Convert bytes to string for decode_secret_key
        let key_str = String::from_utf8(key_data).map_err(|e| {
            crate::error::ExpectError::Ssh(SshError::Authentication {
                user: String::new(),
                reason: format!("Key file {} is not valid UTF-8: {}", path.display(), e),
            })
        })?;

        // Use russh::keys::decode_secret_key which handles both encrypted and unencrypted keys
        let key = russh::keys::decode_secret_key(&key_str, passphrase).map_err(|e| {
            let reason = if passphrase.is_none() && e.to_string().contains("encrypted") {
                format!(
                    "Key {} appears to be encrypted but no passphrase was provided. \
                     Use AuthMethod::public_key_with_passphrase() to specify a passphrase.",
                    path.display()
                )
            } else {
                format!("Failed to decode key {}: {}", path.display(), e)
            };
            crate::error::ExpectError::Ssh(SshError::Authentication {
                user: String::new(),
                reason,
            })
        })?;

        Ok(Arc::new(key))
    }

    /// Authenticate using the configured methods.
    #[allow(clippy::too_many_lines)]
    pub async fn authenticate(
        handle: &mut client::Handle<SshClientHandler>,
        credentials: &SshCredentials,
    ) -> crate::error::Result<bool> {
        let username = &credentials.username;

        for method in &credentials.auth_methods {
            match method {
                AuthMethod::Password(password) => {
                    tracing::debug!(user = %username, "Attempting password authentication");
                    match handle.authenticate_password(username, password).await {
                        Ok(auth_result) if auth_result.success() => {
                            tracing::info!(user = %username, "Password authentication successful");
                            return Ok(true);
                        }
                        Ok(_) => {
                            tracing::debug!(user = %username, "Password authentication failed");
                        }
                        Err(e) => {
                            tracing::debug!(
                                user = %username,
                                error = %e,
                                "Password authentication error"
                            );
                        }
                    }
                }
                AuthMethod::PublicKey {
                    private_key,
                    passphrase,
                } => {
                    tracing::debug!(
                        user = %username,
                        key = %private_key.display(),
                        "Attempting public key authentication"
                    );

                    match load_private_key(private_key, passphrase.as_deref()).await {
                        Ok(key) => {
                            // Get the best supported RSA hash algorithm if applicable
                            // best_supported_rsa_hash returns Result<Option<Option<HashAlg>>, _>
                            let rsa_hash = handle
                                .best_supported_rsa_hash()
                                .await
                                .ok()
                                .flatten()
                                .flatten();
                            let key_with_hash = PrivateKeyWithHashAlg::new(key, rsa_hash);

                            match handle.authenticate_publickey(username, key_with_hash).await {
                                Ok(auth_result) if auth_result.success() => {
                                    tracing::info!(
                                        user = %username,
                                        "Public key authentication successful"
                                    );
                                    return Ok(true);
                                }
                                Ok(_) => {
                                    tracing::debug!(
                                        user = %username,
                                        "Public key authentication failed"
                                    );
                                }
                                Err(e) => {
                                    tracing::debug!(
                                        user = %username,
                                        error = %e,
                                        "Public key authentication error"
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::debug!(
                                user = %username,
                                key = %private_key.display(),
                                error = %e,
                                "Failed to load private key"
                            );
                        }
                    }
                }
                AuthMethod::Agent => {
                    tracing::debug!(user = %username, "Attempting SSH agent authentication");

                    // Connect to the SSH agent
                    #[cfg(unix)]
                    match russh::keys::agent::client::AgentClient::connect_env().await {
                        Ok(mut agent) => {
                            // Get list of keys from agent
                            match agent.request_identities().await {
                                Ok(keys) => {
                                    tracing::debug!(
                                        user = %username,
                                        key_count = keys.len(),
                                        "Found keys in SSH agent"
                                    );

                                    // Try each key from the agent
                                    for key in keys {
                                        // Get the best supported RSA hash algorithm if applicable
                                        let rsa_hash = handle
                                            .best_supported_rsa_hash()
                                            .await
                                            .ok()
                                            .flatten()
                                            .flatten();

                                        match handle
                                            .authenticate_publickey_with(
                                                username,
                                                key.clone(),
                                                rsa_hash,
                                                &mut agent,
                                            )
                                            .await
                                        {
                                            Ok(auth_result) if auth_result.success() => {
                                                tracing::info!(
                                                    user = %username,
                                                    key_type = %key.algorithm().as_str(),
                                                    "SSH agent authentication successful"
                                                );
                                                return Ok(true);
                                            }
                                            Ok(_) => {
                                                tracing::debug!(
                                                    user = %username,
                                                    key_type = %key.algorithm().as_str(),
                                                    "SSH agent key rejected, trying next"
                                                );
                                            }
                                            Err(e) => {
                                                tracing::debug!(
                                                    user = %username,
                                                    error = %e,
                                                    "SSH agent authentication error"
                                                );
                                            }
                                        }
                                    }
                                    tracing::debug!(
                                        user = %username,
                                        "All SSH agent keys exhausted"
                                    );
                                }
                                Err(e) => {
                                    tracing::debug!(
                                        user = %username,
                                        error = %e,
                                        "Failed to get identities from SSH agent"
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::debug!(
                                user = %username,
                                error = %e,
                                "Failed to connect to SSH agent"
                            );
                        }
                    }

                    // Windows: Try Pageant first, then OpenSSH agent via named pipe
                    #[cfg(windows)]
                    {
                        // Try Pageant first (PuTTY SSH agent)
                        // Note: connect_pageant() returns AgentClient directly (not Result).
                        // Errors are detected when calling request_identities().
                        tracing::debug!(user = %username, "Trying Pageant SSH agent");
                        let mut agent =
                            russh::keys::agent::client::AgentClient::connect_pageant().await;
                        match agent.request_identities().await {
                            Ok(keys) => {
                                tracing::debug!(
                                    user = %username,
                                    key_count = keys.len(),
                                    "Found keys in Pageant"
                                );

                                for key in keys {
                                    let rsa_hash = handle
                                        .best_supported_rsa_hash()
                                        .await
                                        .ok()
                                        .flatten()
                                        .flatten();

                                    match handle
                                        .authenticate_publickey_with(
                                            username,
                                            key.clone(),
                                            rsa_hash,
                                            &mut agent,
                                        )
                                        .await
                                    {
                                        Ok(auth_result) if auth_result.success() => {
                                            tracing::info!(
                                                user = %username,
                                                key_type = %key.algorithm().as_str(),
                                                "Pageant authentication successful"
                                            );
                                            return Ok(true);
                                        }
                                        Ok(_) => {
                                            tracing::debug!(
                                                user = %username,
                                                key_type = %key.algorithm().as_str(),
                                                "Pageant key rejected, trying next"
                                            );
                                        }
                                        Err(e) => {
                                            tracing::debug!(
                                                user = %username,
                                                error = %e,
                                                "Pageant authentication error"
                                            );
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::debug!(
                                    user = %username,
                                    error = %e,
                                    "Pageant not available or failed to get identities, trying OpenSSH agent"
                                );
                            }
                        }

                        // Try Windows OpenSSH agent via named pipe
                        const OPENSSH_AGENT_PIPE: &str = r"\\.\pipe\openssh-ssh-agent";
                        tracing::debug!(
                            user = %username,
                            pipe = OPENSSH_AGENT_PIPE,
                            "Trying OpenSSH agent via named pipe"
                        );

                        match russh::keys::agent::client::AgentClient::connect_named_pipe(
                            OPENSSH_AGENT_PIPE,
                        )
                        .await
                        {
                            Ok(mut agent) => match agent.request_identities().await {
                                Ok(keys) => {
                                    tracing::debug!(
                                        user = %username,
                                        key_count = keys.len(),
                                        "Found keys in OpenSSH agent"
                                    );

                                    for key in keys {
                                        let rsa_hash = handle
                                            .best_supported_rsa_hash()
                                            .await
                                            .ok()
                                            .flatten()
                                            .flatten();

                                        match handle
                                            .authenticate_publickey_with(
                                                username,
                                                key.clone(),
                                                rsa_hash,
                                                &mut agent,
                                            )
                                            .await
                                        {
                                            Ok(auth_result) if auth_result.success() => {
                                                tracing::info!(
                                                    user = %username,
                                                    key_type = %key.algorithm().as_str(),
                                                    "OpenSSH agent authentication successful"
                                                );
                                                return Ok(true);
                                            }
                                            Ok(_) => {
                                                tracing::debug!(
                                                    user = %username,
                                                    key_type = %key.algorithm().as_str(),
                                                    "OpenSSH agent key rejected, trying next"
                                                );
                                            }
                                            Err(e) => {
                                                tracing::debug!(
                                                    user = %username,
                                                    error = %e,
                                                    "OpenSSH agent authentication error"
                                                );
                                            }
                                        }
                                    }
                                    tracing::debug!(
                                        user = %username,
                                        "All OpenSSH agent keys exhausted"
                                    );
                                }
                                Err(e) => {
                                    tracing::debug!(
                                        user = %username,
                                        error = %e,
                                        "Failed to get identities from OpenSSH agent"
                                    );
                                }
                            },
                            Err(e) => {
                                tracing::debug!(
                                    user = %username,
                                    error = %e,
                                    "Failed to connect to OpenSSH agent"
                                );
                            }
                        }
                    }

                    #[cfg(not(any(unix, windows)))]
                    {
                        tracing::debug!(
                            user = %username,
                            "SSH agent authentication not supported on this platform"
                        );
                    }
                }
                AuthMethod::KeyboardInteractive { responses } => {
                    tracing::debug!(
                        user = %username,
                        response_count = responses.len(),
                        "Attempting keyboard-interactive authentication"
                    );

                    // Start the keyboard-interactive authentication
                    match handle
                        .authenticate_keyboard_interactive_start(username.clone(), None)
                        .await
                    {
                        Ok(auth_response) => {
                            use russh::client::KeyboardInteractiveAuthResponse;

                            let mut current_response = auth_response;
                            let mut response_index = 0;

                            // Loop to handle multiple rounds of prompts
                            loop {
                                match current_response {
                                    KeyboardInteractiveAuthResponse::Success => {
                                        tracing::info!(
                                            user = %username,
                                            "Keyboard-interactive authentication successful"
                                        );
                                        return Ok(true);
                                    }
                                    KeyboardInteractiveAuthResponse::Failure {
                                        remaining_methods,
                                        partial_success,
                                    } => {
                                        tracing::debug!(
                                            user = %username,
                                            partial_success = partial_success,
                                            remaining = ?remaining_methods,
                                            "Keyboard-interactive authentication failed"
                                        );
                                        break; // Try next auth method
                                    }
                                    KeyboardInteractiveAuthResponse::InfoRequest {
                                        name,
                                        instructions,
                                        prompts,
                                    } => {
                                        tracing::debug!(
                                            user = %username,
                                            name = %name,
                                            instructions = %instructions,
                                            prompt_count = prompts.len(),
                                            "Received keyboard-interactive prompts"
                                        );

                                        // Build responses for the prompts
                                        let mut prompt_responses =
                                            Vec::with_capacity(prompts.len());
                                        for prompt in &prompts {
                                            let response = if response_index < responses.len() {
                                                responses[response_index].clone()
                                            } else {
                                                tracing::warn!(
                                                    user = %username,
                                                    prompt = %prompt.prompt,
                                                    "No response available for prompt, using empty string"
                                                );
                                                String::new()
                                            };
                                            prompt_responses.push(response);
                                            response_index += 1;
                                        }

                                        // Send responses
                                        match handle
                                            .authenticate_keyboard_interactive_respond(
                                                prompt_responses,
                                            )
                                            .await
                                        {
                                            Ok(next_response) => {
                                                current_response = next_response;
                                            }
                                            Err(e) => {
                                                tracing::debug!(
                                                    user = %username,
                                                    error = %e,
                                                    "Keyboard-interactive response error"
                                                );
                                                break; // Try next auth method
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::debug!(
                                user = %username,
                                error = %e,
                                "Keyboard-interactive start error"
                            );
                        }
                    }
                }
                AuthMethod::None => {
                    tracing::debug!(user = %username, "Attempting none authentication");
                    match handle.authenticate_none(username).await {
                        Ok(auth_result) if auth_result.success() => {
                            tracing::info!(user = %username, "None authentication successful");
                            return Ok(true);
                        }
                        Ok(_) => {
                            tracing::debug!(user = %username, "None authentication failed");
                        }
                        Err(e) => {
                            tracing::debug!(
                                user = %username,
                                error = %e,
                                "None authentication error"
                            );
                        }
                    }
                }
            }
        }

        Err(crate::error::ExpectError::Ssh(SshError::Authentication {
            user: username.clone(),
            reason: "All authentication methods exhausted".to_string(),
        }))
    }
}

/// SSH session with russh integration.
///
/// When the `ssh` feature is enabled, this provides a full SSH client
/// implementation using the russh library.
#[cfg(feature = "ssh")]
pub struct SshSession {
    /// Configuration.
    config: SshConfig,
    /// Current state.
    state: SshSessionState,
    /// The russh client handle (when connected).
    handle: Option<russh::client::Handle<russh_impl::SshClientHandler>>,
}

#[cfg(feature = "ssh")]
impl std::fmt::Debug for SshSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SshSession")
            .field("config", &self.config)
            .field("state", &self.state)
            .field("connected", &self.handle.is_some())
            .finish()
    }
}

#[cfg(feature = "ssh")]
impl SshSession {
    /// Create a new session.
    #[must_use]
    pub const fn new(config: SshConfig) -> Self {
        Self {
            config,
            state: SshSessionState::Disconnected,
            handle: None,
        }
    }

    /// Get configuration.
    #[must_use]
    pub const fn config(&self) -> &SshConfig {
        &self.config
    }

    /// Get current state.
    #[must_use]
    pub const fn state(&self) -> SshSessionState {
        self.state
    }

    /// Check if connected.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.state == SshSessionState::Connected && self.handle.is_some()
    }

    /// Connect to the SSH server asynchronously.
    ///
    /// This establishes a TCP connection, performs the SSH handshake,
    /// and authenticates using the configured credentials.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The TCP connection fails
    /// - Host key verification fails
    /// - All authentication methods are exhausted
    pub async fn connect_async(&mut self) -> crate::error::Result<()> {
        self.state = SshSessionState::Connecting;

        // Create the russh client config
        let ssh_config = Arc::new(russh::client::Config::default());

        // Create the handler
        let handler = russh_impl::SshClientHandler {
            host_key_verification: self.config.host_key_verification,
            host: self.config.host.clone(),
            port: self.config.port,
        };

        // Connect to the server
        let addr = (self.config.host.as_str(), self.config.port);
        tracing::info!(
            host = %self.config.host,
            port = %self.config.port,
            "Connecting to SSH server"
        );

        let mut handle = tokio::time::timeout(
            self.config.connect_timeout,
            russh::client::connect(ssh_config, addr, handler),
        )
        .await
        .map_err(|_| {
            self.state = SshSessionState::Error;
            crate::error::ExpectError::Ssh(SshError::Timeout {
                duration: self.config.connect_timeout,
            })
        })?
        .map_err(|e| {
            self.state = SshSessionState::Error;
            crate::error::ExpectError::Ssh(SshError::Connection {
                host: self.config.host.clone(),
                port: self.config.port,
                reason: e.to_string(),
            })
        })?;

        // Authenticate
        self.state = SshSessionState::Authenticating;
        tracing::debug!(
            user = %self.config.credentials.username,
            "Authenticating with SSH server"
        );

        russh_impl::authenticate(&mut handle, &self.config.credentials).await?;

        // Success!
        self.state = SshSessionState::Connected;
        self.handle = Some(handle);

        tracing::info!(
            host = %self.config.host,
            user = %self.config.credentials.username,
            "SSH connection established"
        );

        Ok(())
    }

    /// Connect synchronously by blocking on the async connection.
    ///
    /// This is a convenience method that uses the current tokio runtime
    /// to block on the async connection. Prefer `connect_async` when possible.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection or authentication fails.
    ///
    /// # Panics
    ///
    /// Panics if called outside of a tokio runtime context.
    pub fn connect(&mut self) -> crate::error::Result<()> {
        // Try to get the current runtime handle
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            // We're in an async context, use block_in_place
            tokio::task::block_in_place(|| handle.block_on(self.connect_async()))
        } else {
            // No runtime, create a temporary one
            let rt = tokio::runtime::Runtime::new().map_err(|e| {
                crate::error::ExpectError::Ssh(SshError::Session {
                    reason: format!("Failed to create runtime: {e}"),
                })
            })?;
            rt.block_on(self.connect_async())
        }
    }

    /// Disconnect from the SSH server.
    pub fn disconnect(&mut self) {
        if let Some(handle) = self.handle.take() {
            // Attempt graceful disconnect
            let _ = tokio::runtime::Handle::try_current().map(|rt| {
                tokio::task::block_in_place(|| {
                    rt.block_on(async {
                        let _ = handle
                            .disconnect(russh::Disconnect::ByApplication, "", "en")
                            .await;
                    });
                });
            });
        }
        self.state = SshSessionState::Disconnected;
    }

    /// Get a reference to the russh handle.
    ///
    /// This is useful for advanced operations like opening channels.
    #[must_use]
    pub const fn handle(&self) -> Option<&russh::client::Handle<russh_impl::SshClientHandler>> {
        self.handle.as_ref()
    }

    /// Get a mutable reference to the russh handle.
    pub fn handle_mut(
        &mut self,
    ) -> Option<&mut russh::client::Handle<russh_impl::SshClientHandler>> {
        self.handle.as_mut()
    }

    /// Open a session channel.
    ///
    /// This opens a new SSH channel that can be used for executing commands
    /// or starting an interactive shell.
    ///
    /// # Errors
    ///
    /// Returns an error if the session is not connected or channel opening fails.
    pub async fn open_channel(
        &mut self,
    ) -> crate::error::Result<russh::Channel<russh::client::Msg>> {
        let handle = self.handle.as_mut().ok_or_else(|| {
            crate::error::ExpectError::Ssh(SshError::Session {
                reason: "Not connected".to_string(),
            })
        })?;

        let channel = handle.channel_open_session().await.map_err(|e| {
            crate::error::ExpectError::Ssh(SshError::Channel {
                reason: e.to_string(),
            })
        })?;

        Ok(channel)
    }

    /// Open an interactive shell session with a PTY.
    ///
    /// This is a convenience method that opens a channel, requests a PTY,
    /// and starts a shell, returning a stream that implements `AsyncRead` and `AsyncWrite`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use rust_expect::backend::ssh::{SshSession, SshConfig, SshCredentials};
    /// use tokio::io::{AsyncReadExt, AsyncWriteExt};
    ///
    /// let config = SshConfig::new("example.com")
    ///     .username("user")
    ///     .credentials(SshCredentials::new("user").with_password("pass"));
    ///
    /// let mut session = SshSession::new(config);
    /// session.connect_async().await?;
    ///
    /// let mut shell = session.shell().await?;
    /// shell.write_all(b"ls -la\n").await?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the session is not connected, channel opening fails,
    /// PTY request fails, or shell request fails.
    pub async fn shell(&mut self) -> crate::error::Result<super::channel::SshChannelStream> {
        self.shell_with_config(super::channel::ChannelConfig::default())
            .await
    }

    /// Open an interactive shell session with custom configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the session is not connected, channel opening fails,
    /// PTY request fails (if enabled), or shell request fails.
    pub async fn shell_with_config(
        &mut self,
        config: super::channel::ChannelConfig,
    ) -> crate::error::Result<super::channel::SshChannelStream> {
        let channel = self.open_channel().await?;
        let mut stream = super::channel::SshChannelStream::new(channel, config);

        // Request PTY if configured
        if stream.config().pty {
            stream.request_pty().await?;
        }

        // Request shell
        stream.request_shell().await?;

        Ok(stream)
    }

    /// Execute a command and return a stream for reading output.
    ///
    /// This opens a channel, optionally requests a PTY, and executes the specified command.
    /// The returned stream can be used to read command output and write stdin.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut exec = session.exec("uname -a").await?;
    /// let mut output = String::new();
    /// exec.read_to_string(&mut output).await?;
    /// println!("Output: {}", output);
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the session is not connected or command execution fails.
    pub async fn exec(
        &mut self,
        command: &str,
    ) -> crate::error::Result<super::channel::SshChannelStream> {
        self.exec_with_config(command, super::channel::ChannelConfig::default().no_pty())
            .await
    }

    /// Execute a command with custom configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the session is not connected or command execution fails.
    pub async fn exec_with_config(
        &mut self,
        command: &str,
        config: super::channel::ChannelConfig,
    ) -> crate::error::Result<super::channel::SshChannelStream> {
        let channel = self.open_channel().await?;
        let mut stream = super::channel::SshChannelStream::new(channel, config);

        // Request PTY if configured
        if stream.config().pty {
            stream.request_pty().await?;
        }

        // Execute command
        stream.exec(command).await?;

        Ok(stream)
    }
}

#[cfg(feature = "ssh")]
impl Drop for SshSession {
    fn drop(&mut self) {
        self.disconnect();
    }
}

// ============================================================================
// Stub implementation (when ssh feature is disabled)
// ============================================================================

/// SSH session stub for when the `ssh` feature is disabled.
///
/// This provides API compatibility but operations will fail at runtime.
#[cfg(not(feature = "ssh"))]
#[derive(Debug)]
pub struct SshSession {
    /// Configuration.
    config: SshConfig,
    /// Current state.
    state: SshSessionState,
}

#[cfg(not(feature = "ssh"))]
impl SshSession {
    /// Create a new session.
    #[must_use]
    pub const fn new(config: SshConfig) -> Self {
        Self {
            config,
            state: SshSessionState::Disconnected,
        }
    }

    /// Get configuration.
    #[must_use]
    pub const fn config(&self) -> &SshConfig {
        &self.config
    }

    /// Get current state.
    #[must_use]
    pub const fn state(&self) -> SshSessionState {
        self.state
    }

    /// Check if connected.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.state == SshSessionState::Connected
    }

    /// Connect (stub - always succeeds for API compatibility in tests).
    pub fn connect(&mut self) -> crate::error::Result<()> {
        self.state = SshSessionState::Connected;
        Ok(())
    }

    /// Disconnect.
    pub fn disconnect(&mut self) {
        self.state = SshSessionState::Disconnected;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_config_builder() {
        let config = SshConfig::new("example.com")
            .port(2222)
            .username("admin")
            .with_compression();

        assert_eq!(config.host, "example.com");
        assert_eq!(config.port, 2222);
        assert!(config.compression);
        assert_eq!(config.address(), "example.com:2222");
    }

    #[test]
    fn ssh_session_state() {
        let mut session = SshSession::new(SshConfig::new("host"));
        assert_eq!(session.state(), SshSessionState::Disconnected);

        // Note: In the stub implementation, connect() succeeds
        // In the real implementation, it would fail without a server
        #[cfg(not(feature = "ssh"))]
        {
            session.connect().unwrap();
            assert!(session.is_connected());
        }

        session.disconnect();
        assert!(!session.is_connected());
    }

    #[test]
    fn ssh_config_defaults() {
        let config = SshConfig::default();
        assert_eq!(config.port, 22);
        assert_eq!(config.connect_timeout, Duration::from_secs(30));
        assert!(!config.compression);
        assert_eq!(
            config.host_key_verification,
            HostKeyVerification::KnownHosts
        );
    }
}
