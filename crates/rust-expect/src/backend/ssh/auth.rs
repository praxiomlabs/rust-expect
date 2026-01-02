//! SSH authentication methods.

use std::path::PathBuf;

/// SSH authentication method.
#[derive(Debug, Clone)]
pub enum AuthMethod {
    /// Password authentication.
    Password(String),
    /// Public key authentication.
    PublicKey {
        /// Private key path.
        private_key: PathBuf,
        /// Passphrase for the key (if encrypted).
        passphrase: Option<String>,
    },
    /// SSH agent authentication.
    Agent,
    /// Keyboard-interactive authentication.
    KeyboardInteractive,
    /// No authentication (for tunnels).
    None,
}

impl AuthMethod {
    /// Create password auth.
    #[must_use]
    pub fn password(password: impl Into<String>) -> Self {
        Self::Password(password.into())
    }

    /// Create public key auth.
    #[must_use]
    pub fn public_key(private_key: impl Into<PathBuf>) -> Self {
        Self::PublicKey {
            private_key: private_key.into(),
            passphrase: None,
        }
    }

    /// Create public key auth with passphrase.
    #[must_use]
    pub fn public_key_with_passphrase(
        private_key: impl Into<PathBuf>,
        passphrase: impl Into<String>,
    ) -> Self {
        Self::PublicKey {
            private_key: private_key.into(),
            passphrase: Some(passphrase.into()),
        }
    }

    /// Create agent auth.
    #[must_use]
    pub const fn agent() -> Self {
        Self::Agent
    }

    /// Check if this is password auth.
    #[must_use]
    pub const fn is_password(&self) -> bool {
        matches!(self, Self::Password(_))
    }

    /// Check if this is public key auth.
    #[must_use]
    pub const fn is_public_key(&self) -> bool {
        matches!(self, Self::PublicKey { .. })
    }
}

/// SSH credentials.
#[derive(Debug, Clone)]
pub struct SshCredentials {
    /// Username.
    pub username: String,
    /// Authentication methods to try (in order).
    pub auth_methods: Vec<AuthMethod>,
}

impl SshCredentials {
    /// Create new credentials.
    #[must_use]
    pub fn new(username: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            auth_methods: Vec::new(),
        }
    }

    /// Add an authentication method.
    #[must_use]
    pub fn with_auth(mut self, method: AuthMethod) -> Self {
        self.auth_methods.push(method);
        self
    }

    /// Add password authentication.
    #[must_use]
    pub fn with_password(self, password: impl Into<String>) -> Self {
        self.with_auth(AuthMethod::password(password))
    }

    /// Add public key authentication.
    #[must_use]
    pub fn with_key(self, private_key: impl Into<PathBuf>) -> Self {
        self.with_auth(AuthMethod::public_key(private_key))
    }

    /// Add agent authentication.
    #[must_use]
    pub fn with_agent(self) -> Self {
        self.with_auth(AuthMethod::Agent)
    }

    /// Create with default authentication (agent, then default keys).
    #[must_use]
    pub fn with_defaults(self) -> Self {
        let home = std::env::var("HOME").unwrap_or_default();
        self.with_agent()
            .with_key(format!("{home}/.ssh/id_ed25519"))
            .with_key(format!("{home}/.ssh/id_rsa"))
    }
}

impl Default for SshCredentials {
    fn default() -> Self {
        let username = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "root".to_string());
        Self::new(username)
    }
}

/// Host key verification policy.
///
/// # Security
///
/// The default policy is `KnownHosts`, which checks the server's key against
/// the user's `known_hosts` file. This is the recommended setting for production use.
///
/// The `AcceptAll` variant is only available when the `insecure-skip-verify` feature
/// is enabled. Using it in production environments enables MITM attacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum HostKeyVerification {
    /// Accept all keys without verification.
    ///
    /// # Security Warning
    ///
    /// **DANGEROUS:** This disables SSH host key verification entirely, allowing
    /// man-in-the-middle attacks. Only use this in controlled testing environments
    /// where you trust the network completely.
    ///
    /// This variant is only available when the `insecure-skip-verify` feature is enabled.
    #[cfg(feature = "insecure-skip-verify")]
    AcceptAll,
    /// Reject unknown keys.
    RejectUnknown,
    /// Check against `known_hosts` file.
    KnownHosts,
    /// Accept on first use, then verify (Trust On First Use).
    Tofu,
}

impl Default for HostKeyVerification {
    fn default() -> Self {
        Self::KnownHosts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_method_password() {
        let auth = AuthMethod::password("secret");
        assert!(auth.is_password());
        assert!(!auth.is_public_key());
    }

    #[test]
    fn credentials_builder() {
        let creds = SshCredentials::new("user")
            .with_password("pass")
            .with_agent();

        assert_eq!(creds.username, "user");
        assert_eq!(creds.auth_methods.len(), 2);
    }
}
