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
    ///
    /// This authentication method is commonly used by SSH servers that delegate
    /// authentication to PAM or other backends. Many servers that appear to support
    /// password authentication actually use keyboard-interactive behind the scenes.
    ///
    /// The `responses` field contains pre-defined answers to server prompts.
    /// For simple password-based keyboard-interactive (the most common case),
    /// provide a single response containing the password.
    ///
    /// # Example
    ///
    /// ```
    /// use rust_expect::backend::ssh::AuthMethod;
    ///
    /// // For password-like keyboard-interactive (most common)
    /// let auth = AuthMethod::keyboard_interactive(vec!["my_password".to_string()]);
    ///
    /// // For multi-factor authentication with multiple prompts
    /// let auth = AuthMethod::keyboard_interactive(vec![
    ///     "password".to_string(),
    ///     "123456".to_string(), // OTP code
    /// ]);
    /// ```
    KeyboardInteractive {
        /// Pre-defined responses to server prompts.
        ///
        /// These responses are provided in order as the server sends prompts.
        /// If there are more prompts than responses, empty strings are used.
        /// If there are more responses than prompts, extra responses are ignored.
        responses: Vec<String>,
    },
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

    /// Create keyboard-interactive auth with responses.
    ///
    /// This is useful for servers that use keyboard-interactive as a wrapper
    /// for password authentication (common with PAM-based servers) or for
    /// multi-factor authentication with known prompts.
    ///
    /// # Example
    ///
    /// ```
    /// use rust_expect::backend::ssh::AuthMethod;
    ///
    /// // Simple password-like keyboard-interactive
    /// let auth = AuthMethod::keyboard_interactive(vec!["my_password".to_string()]);
    ///
    /// // Multi-factor with password and OTP
    /// let auth = AuthMethod::keyboard_interactive(vec![
    ///     "password".to_string(),
    ///     "123456".to_string(),
    /// ]);
    /// ```
    #[must_use]
    pub const fn keyboard_interactive(responses: Vec<String>) -> Self {
        Self::KeyboardInteractive { responses }
    }

    /// Create keyboard-interactive auth with a single password response.
    ///
    /// This is a convenience method for the common case where keyboard-interactive
    /// is used as a wrapper for password authentication.
    ///
    /// # Example
    ///
    /// ```
    /// use rust_expect::backend::ssh::AuthMethod;
    ///
    /// let auth = AuthMethod::keyboard_interactive_password("my_password");
    /// ```
    #[must_use]
    pub fn keyboard_interactive_password(password: impl Into<String>) -> Self {
        Self::KeyboardInteractive {
            responses: vec![password.into()],
        }
    }

    /// Check if this is keyboard-interactive auth.
    #[must_use]
    pub const fn is_keyboard_interactive(&self) -> bool {
        matches!(self, Self::KeyboardInteractive { .. })
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

    /// Add public key authentication with passphrase.
    #[must_use]
    pub fn with_key_passphrase(
        self,
        private_key: impl Into<PathBuf>,
        passphrase: impl Into<String>,
    ) -> Self {
        self.with_auth(AuthMethod::public_key_with_passphrase(
            private_key,
            passphrase,
        ))
    }

    /// Add agent authentication.
    #[must_use]
    pub fn with_agent(self) -> Self {
        self.with_auth(AuthMethod::Agent)
    }

    /// Add keyboard-interactive authentication with a password response.
    ///
    /// This is useful for servers that use keyboard-interactive as a wrapper
    /// for password authentication (common with PAM-based SSH servers).
    #[must_use]
    pub fn with_keyboard_interactive(self, password: impl Into<String>) -> Self {
        self.with_auth(AuthMethod::keyboard_interactive_password(password))
    }

    /// Add keyboard-interactive authentication with multiple responses.
    ///
    /// This is useful for multi-factor authentication where the server
    /// may prompt for multiple pieces of information.
    #[must_use]
    pub fn with_keyboard_interactive_responses(self, responses: Vec<String>) -> Self {
        self.with_auth(AuthMethod::keyboard_interactive(responses))
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
#[derive(Default)]
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
    #[default]
    KnownHosts,
    /// Accept on first use, then verify (Trust On First Use).
    Tofu,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_method_password() {
        let auth = AuthMethod::password("secret");
        assert!(auth.is_password());
        assert!(!auth.is_public_key());
        assert!(!auth.is_keyboard_interactive());
    }

    #[test]
    fn auth_method_keyboard_interactive() {
        // Single response (password-like)
        let auth = AuthMethod::keyboard_interactive_password("secret");
        assert!(auth.is_keyboard_interactive());
        assert!(!auth.is_password());
        assert!(!auth.is_public_key());

        if let AuthMethod::KeyboardInteractive { responses } = auth {
            assert_eq!(responses.len(), 1);
            assert_eq!(responses[0], "secret");
        } else {
            panic!("Expected KeyboardInteractive variant");
        }
    }

    #[test]
    fn auth_method_keyboard_interactive_multi_response() {
        // Multiple responses (MFA)
        let auth =
            AuthMethod::keyboard_interactive(vec!["password".to_string(), "123456".to_string()]);
        assert!(auth.is_keyboard_interactive());

        if let AuthMethod::KeyboardInteractive { responses } = auth {
            assert_eq!(responses.len(), 2);
            assert_eq!(responses[0], "password");
            assert_eq!(responses[1], "123456");
        } else {
            panic!("Expected KeyboardInteractive variant");
        }
    }

    #[test]
    fn credentials_builder() {
        let creds = SshCredentials::new("user")
            .with_password("pass")
            .with_agent();

        assert_eq!(creds.username, "user");
        assert_eq!(creds.auth_methods.len(), 2);
    }

    #[test]
    fn credentials_keyboard_interactive() {
        let creds = SshCredentials::new("user").with_keyboard_interactive("password");

        assert_eq!(creds.username, "user");
        assert_eq!(creds.auth_methods.len(), 1);
        assert!(creds.auth_methods[0].is_keyboard_interactive());
    }

    #[test]
    fn credentials_keyboard_interactive_multi_response() {
        let creds = SshCredentials::new("user").with_keyboard_interactive_responses(vec![
            "password".to_string(),
            "otp_code".to_string(),
        ]);

        assert_eq!(creds.username, "user");
        assert_eq!(creds.auth_methods.len(), 1);

        if let AuthMethod::KeyboardInteractive { responses } = &creds.auth_methods[0] {
            assert_eq!(responses.len(), 2);
        } else {
            panic!("Expected KeyboardInteractive variant");
        }
    }

    #[test]
    fn credentials_multiple_auth_methods() {
        // Test combining keyboard-interactive with other methods
        let creds = SshCredentials::new("user")
            .with_agent()
            .with_keyboard_interactive("password")
            .with_password("fallback");

        assert_eq!(creds.auth_methods.len(), 3);
        assert!(matches!(creds.auth_methods[0], AuthMethod::Agent));
        assert!(creds.auth_methods[1].is_keyboard_interactive());
        assert!(creds.auth_methods[2].is_password());
    }
}
