//! Builder patterns for test setup.

use crate::config::SessionConfig;
use std::collections::HashMap;
use std::time::Duration;

/// Builder for creating test expect configurations.
#[derive(Debug, Clone, Default)]
pub struct ExpectTestBuilder {
    timeout: Option<Duration>,
    env: HashMap<String, String>,
    dimensions: Option<(u16, u16)>,
}

impl ExpectTestBuilder {
    /// Create a new test builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the default timeout.
    #[must_use]
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set a fast timeout for quick tests.
    #[must_use]
    pub const fn fast_timeout(self) -> Self {
        self.timeout(Duration::from_millis(100))
    }

    /// Set a slow timeout for slower tests.
    #[must_use]
    pub const fn slow_timeout(self) -> Self {
        self.timeout(Duration::from_secs(30))
    }

    /// Add an environment variable.
    #[must_use]
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Set terminal dimensions.
    #[must_use]
    pub const fn dimensions(mut self, width: u16, height: u16) -> Self {
        self.dimensions = Some((width, height));
        self
    }

    /// Set minimal environment (no inherited env).
    #[must_use]
    pub fn minimal_env(self) -> Self {
        self.env("TERM", "dumb")
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", "/tmp")
    }

    /// Build the session configuration.
    #[must_use]
    pub fn build(self) -> SessionConfig {
        let mut config = SessionConfig::default();

        if let Some(timeout) = self.timeout {
            config.timeout.default = timeout;
        }

        for (key, value) in self.env {
            config.env.insert(key, value);
        }

        if let Some(dims) = self.dimensions {
            config.dimensions = dims;
        }

        config
    }
}

/// Builder for creating test sessions.
#[derive(Debug, Clone, Default)]
pub struct SessionTestBuilder {
    command: Option<String>,
    args: Vec<String>,
    env: HashMap<String, String>,
    cwd: Option<std::path::PathBuf>,
    timeout: Option<Duration>,
}

impl SessionTestBuilder {
    /// Create a new session builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the command to run.
    #[must_use]
    pub fn command(mut self, cmd: impl Into<String>) -> Self {
        self.command = Some(cmd.into());
        self
    }

    /// Use a shell command.
    #[must_use]
    pub fn shell(self) -> Self {
        self.command("sh").arg("-c")
    }

    /// Use bash as the shell.
    #[must_use]
    pub fn bash(self) -> Self {
        self.command("bash").arg("-c")
    }

    /// Add an argument.
    #[must_use]
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Add multiple arguments.
    #[must_use]
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args
            .extend(args.into_iter().map(std::convert::Into::into));
        self
    }

    /// Add an environment variable.
    #[must_use]
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Set the working directory.
    #[must_use]
    pub fn cwd(mut self, dir: impl Into<std::path::PathBuf>) -> Self {
        self.cwd = Some(dir.into());
        self
    }

    /// Set the timeout.
    #[must_use]
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Get the command.
    #[must_use]
    pub fn get_command(&self) -> Option<&str> {
        self.command.as_deref()
    }

    /// Get the arguments.
    #[must_use]
    pub fn get_args(&self) -> &[String] {
        &self.args
    }

    /// Get the environment.
    #[must_use]
    pub const fn get_env(&self) -> &HashMap<String, String> {
        &self.env
    }

    /// Get the working directory.
    #[must_use]
    pub fn get_cwd(&self) -> Option<&std::path::Path> {
        self.cwd.as_deref()
    }

    /// Get the timeout.
    #[must_use]
    pub const fn get_timeout(&self) -> Option<Duration> {
        self.timeout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expect_test_builder() {
        let config = ExpectTestBuilder::new()
            .timeout(Duration::from_secs(5))
            .dimensions(80, 24)
            .env("TERM", "dumb")
            .build();

        assert_eq!(config.timeout.default, Duration::from_secs(5));
        assert_eq!(config.dimensions, (80, 24));
        assert_eq!(config.env.get("TERM"), Some(&"dumb".to_string()));
    }

    #[test]
    fn session_test_builder() {
        let builder = SessionTestBuilder::new()
            .command("cat")
            .arg("-n")
            .env("TERM", "dumb")
            .timeout(Duration::from_secs(10));

        assert_eq!(builder.get_command(), Some("cat"));
        assert_eq!(builder.get_args(), &["-n"]);
        assert!(builder.get_env().contains_key("TERM"));
        assert_eq!(builder.get_timeout(), Some(Duration::from_secs(10)));
    }
}
