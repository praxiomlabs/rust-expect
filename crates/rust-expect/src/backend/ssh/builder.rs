//! SSH session builder.

use super::auth::{AuthMethod, HostKeyVerification, SshCredentials};
use super::session::{SshConfig, SshSession};
use std::path::PathBuf;
use std::time::Duration;

/// Builder for SSH sessions.
#[derive(Debug, Default)]
pub struct SshSessionBuilder {
    host: Option<String>,
    port: u16,
    username: Option<String>,
    auth_methods: Vec<AuthMethod>,
    connect_timeout: Duration,
    host_key_verification: HostKeyVerification,
    compression: bool,
    tcp_keepalive: Option<Duration>,
}

impl SshSessionBuilder {
    /// Create a new builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            port: 22,
            connect_timeout: Duration::from_secs(30),
            host_key_verification: HostKeyVerification::KnownHosts,
            tcp_keepalive: Some(Duration::from_secs(60)),
            ..Default::default()
        }
    }

    /// Set the host.
    #[must_use]
    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.host = Some(host.into());
        self
    }

    /// Set the port.
    #[must_use]
    pub const fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Set the username.
    #[must_use]
    pub fn username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    /// Add password authentication.
    #[must_use]
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.auth_methods.push(AuthMethod::Password(password.into()));
        self
    }

    /// Add public key authentication.
    #[must_use]
    pub fn private_key(mut self, path: impl Into<PathBuf>) -> Self {
        self.auth_methods.push(AuthMethod::PublicKey {
            private_key: path.into(),
            passphrase: None,
        });
        self
    }

    /// Add public key with passphrase.
    #[must_use]
    pub fn private_key_with_passphrase(
        mut self,
        path: impl Into<PathBuf>,
        passphrase: impl Into<String>,
    ) -> Self {
        self.auth_methods.push(AuthMethod::PublicKey {
            private_key: path.into(),
            passphrase: Some(passphrase.into()),
        });
        self
    }

    /// Enable SSH agent authentication.
    #[must_use]
    pub fn agent(mut self) -> Self {
        self.auth_methods.push(AuthMethod::Agent);
        self
    }

    /// Set connection timeout.
    #[must_use]
    pub const fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Set host key verification policy.
    #[must_use]
    pub const fn host_key_verification(mut self, policy: HostKeyVerification) -> Self {
        self.host_key_verification = policy;
        self
    }

    /// Accept all host keys without verification.
    ///
    /// # Security Warning
    ///
    /// **DANGEROUS:** This disables SSH host key verification entirely, allowing
    /// man-in-the-middle attacks. Only use this in controlled testing environments.
    ///
    /// This method is only available when the `insecure-skip-verify` feature is enabled.
    #[cfg(feature = "insecure-skip-verify")]
    #[must_use]
    pub const fn accept_all_keys(mut self) -> Self {
        self.host_key_verification = HostKeyVerification::AcceptAll;
        self
    }

    /// Enable compression.
    #[must_use]
    pub const fn compression(mut self, enabled: bool) -> Self {
        self.compression = enabled;
        self
    }

    /// Set TCP keepalive.
    #[must_use]
    pub const fn tcp_keepalive(mut self, interval: Option<Duration>) -> Self {
        self.tcp_keepalive = interval;
        self
    }

    /// Build the session.
    pub fn build(self) -> crate::error::Result<SshSession> {
        let host = self.host.ok_or_else(|| {
            crate::error::ExpectError::config("SSH host is required")
        })?;

        let username = self.username.unwrap_or_else(|| {
            std::env::var("USER")
                .or_else(|_| std::env::var("USERNAME"))
                .unwrap_or_else(|_| "root".to_string())
        });

        let mut credentials = SshCredentials::new(username);
        for method in self.auth_methods {
            credentials = credentials.with_auth(method);
        }

        // Add default auth if none specified
        if credentials.auth_methods.is_empty() {
            credentials = credentials.with_defaults();
        }

        let config = SshConfig {
            host,
            port: self.port,
            credentials,
            connect_timeout: self.connect_timeout,
            host_key_verification: self.host_key_verification,
            compression: self.compression,
            tcp_keepalive: self.tcp_keepalive,
        };

        Ok(SshSession::new(config))
    }

    /// Build and connect.
    pub fn connect(self) -> crate::error::Result<SshSession> {
        let mut session = self.build()?;
        session.connect()?;
        Ok(session)
    }
}

/// Create an SSH session from a URI-like string.
///
/// Format: `[user@]host[:port]`
#[must_use] pub fn parse_ssh_target(target: &str) -> (Option<String>, String, u16) {
    let (user_part, rest) = if let Some(at_pos) = target.find('@') {
        (Some(target[..at_pos].to_string()), &target[at_pos + 1..])
    } else {
        (None, target)
    };

    let (host, port) = if let Some(colon_pos) = rest.rfind(':') {
        let port_str = &rest[colon_pos + 1..];
        if let Ok(port) = port_str.parse() {
            (rest[..colon_pos].to_string(), port)
        } else {
            (rest.to_string(), 22)
        }
    } else {
        (rest.to_string(), 22)
    };

    (user_part, host, port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_basic() {
        let session = SshSessionBuilder::new()
            .host("example.com")
            .username("user")
            .password("pass")
            .build()
            .unwrap();

        assert_eq!(session.config().host, "example.com");
        assert_eq!(session.config().credentials.username, "user");
    }

    #[test]
    fn parse_target_full() {
        let (user, host, port) = parse_ssh_target("admin@server.com:2222");
        assert_eq!(user, Some("admin".to_string()));
        assert_eq!(host, "server.com");
        assert_eq!(port, 2222);
    }

    #[test]
    fn parse_target_simple() {
        let (user, host, port) = parse_ssh_target("server.com");
        assert_eq!(user, None);
        assert_eq!(host, "server.com");
        assert_eq!(port, 22);
    }
}
