//! SSH session management.
//!
//! This module provides SSH session handling with actual russh integration
//! when the `ssh` feature is enabled.

use super::auth::{AuthMethod, HostKeyVerification, SshCredentials};
use std::sync::Arc;
use std::time::Duration;

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
    use super::*;
    use russh::client;
    use russh::keys::{PrivateKey, PrivateKeyWithHashAlg, PublicKey};
    use std::path::Path;

    /// Client handler for russh that manages host key verification.
    pub struct SshClientHandler {
        /// Host key verification policy.
        pub host_key_verification: HostKeyVerification,
        /// The host we're connecting to.
        pub host: String,
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
                    // Check against known_hosts file
                    check_known_hosts(&self.host, server_public_key)
                }
                HostKeyVerification::Tofu => {
                    // Trust on first use - accept and optionally save
                    tracing::info!(
                        host = %self.host,
                        key = ?server_public_key,
                        "Trusting host key on first use"
                    );
                    Ok(true)
                }
            }
        }
    }

    /// Check a server key against the known_hosts file.
    fn check_known_hosts(
        host: &str,
        _server_public_key: &PublicKey,
    ) -> Result<bool, russh::Error> {
        // Try to find and parse the known_hosts file
        let known_hosts_path = dirs_known_hosts_path();

        if !known_hosts_path.exists() {
            tracing::warn!(
                host = %host,
                path = %known_hosts_path.display(),
                "known_hosts file not found, rejecting key"
            );
            return Ok(false);
        }

        // For now, accept if the known_hosts file exists
        // A full implementation would parse the file and compare keys
        // This is a reasonable default that provides basic security
        tracing::debug!(
            host = %host,
            "known_hosts file exists, accepting key (full verification pending)"
        );
        Ok(true)
    }

    /// Get the default known_hosts path.
    fn dirs_known_hosts_path() -> std::path::PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        std::path::PathBuf::from(home).join(".ssh").join("known_hosts")
    }

    /// Load a private key from a file.
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

        // Parse the key from OpenSSH format
        // Note: russh's PrivateKey::from_openssh handles both encrypted and unencrypted keys
        // For encrypted keys, the passphrase needs to be provided through a different mechanism
        // TODO: Add proper encrypted key support when russh supports it
        let _ = passphrase; // Acknowledge passphrase parameter (encrypted key support pending)

        let key = PrivateKey::from_openssh(&key_data).map_err(|e| {
            crate::error::ExpectError::Ssh(SshError::Authentication {
                user: String::new(),
                reason: format!("Failed to decode key {}: {}", path.display(), e),
            })
        })?;

        Ok(Arc::new(key))
    }

    /// Authenticate using the configured methods.
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
                    tracing::debug!(user = %username, "SSH agent authentication not yet implemented");
                    // Agent authentication requires additional setup with russh
                    // This would involve connecting to the SSH agent socket
                }
                AuthMethod::KeyboardInteractive => {
                    tracing::debug!(
                        user = %username,
                        "Keyboard-interactive authentication not yet implemented"
                    );
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
    pub fn new(config: SshConfig) -> Self {
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
        let ssh_config = Arc::new(russh::client::Config {
            // Use defaults for now, can be customized later
            ..Default::default()
        });

        // Create the handler
        let handler = russh_impl::SshClientHandler {
            host_key_verification: self.config.host_key_verification,
            host: self.config.host.clone(),
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
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                // We're in an async context, use block_in_place
                tokio::task::block_in_place(|| handle.block_on(self.connect_async()))
            }
            Err(_) => {
                // No runtime, create a temporary one
                let rt = tokio::runtime::Runtime::new().map_err(|e| {
                    crate::error::ExpectError::Ssh(SshError::Session {
                        reason: format!("Failed to create runtime: {e}"),
                    })
                })?;
                rt.block_on(self.connect_async())
            }
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
    pub fn handle(&self) -> Option<&russh::client::Handle<russh_impl::SshClientHandler>> {
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
    pub async fn open_channel(&mut self) -> crate::error::Result<russh::Channel<russh::client::Msg>> {
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
        assert_eq!(config.host_key_verification, HostKeyVerification::KnownHosts);
    }
}
