//! Environment-based configuration.

use std::collections::HashMap;
use std::time::Duration;

/// Environment configuration prefix.
pub const DEFAULT_PREFIX: &str = "EXPECT";

/// Environment variable reader.
#[derive(Debug, Clone)]
pub struct EnvConfig {
    /// Prefix for environment variables.
    prefix: String,
    /// Cached values.
    cache: HashMap<String, String>,
}

impl Default for EnvConfig {
    fn default() -> Self {
        Self::new(DEFAULT_PREFIX)
    }
}

impl EnvConfig {
    /// Create a new environment config reader.
    #[must_use]
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            cache: HashMap::new(),
        }
    }

    /// Create without a prefix.
    #[must_use]
    pub fn no_prefix() -> Self {
        Self {
            prefix: String::new(),
            cache: HashMap::new(),
        }
    }

    /// Build the full environment variable name.
    fn var_name(&self, name: &str) -> String {
        if self.prefix.is_empty() {
            name.to_uppercase()
        } else {
            format!("{}_{}", self.prefix, name.to_uppercase())
        }
    }

    /// Get a string value.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<String> {
        let var_name = self.var_name(name);
        std::env::var(&var_name).ok()
    }

    /// Get a string value with default.
    #[must_use]
    pub fn get_or(&self, name: &str, default: &str) -> String {
        self.get(name).unwrap_or_else(|| default.to_string())
    }

    /// Get a parsed value.
    #[must_use]
    pub fn parse<T: std::str::FromStr>(&self, name: &str) -> Option<T> {
        self.get(name).and_then(|v| v.parse().ok())
    }

    /// Get a parsed value with default.
    #[must_use]
    pub fn parse_or<T: std::str::FromStr>(&self, name: &str, default: T) -> T {
        self.parse(name).unwrap_or(default)
    }

    /// Get a boolean value.
    #[must_use]
    pub fn bool(&self, name: &str) -> Option<bool> {
        self.get(name).map(|v| {
            matches!(
                v.to_lowercase().as_str(),
                "1" | "true" | "yes" | "on" | "enabled"
            )
        })
    }

    /// Get a boolean with default.
    #[must_use]
    pub fn bool_or(&self, name: &str, default: bool) -> bool {
        self.bool(name).unwrap_or(default)
    }

    /// Get a duration in seconds.
    #[must_use]
    pub fn duration_secs(&self, name: &str) -> Option<Duration> {
        self.parse::<u64>(name).map(Duration::from_secs)
    }

    /// Get a duration in milliseconds.
    #[must_use]
    pub fn duration_millis(&self, name: &str) -> Option<Duration> {
        self.parse::<u64>(name).map(Duration::from_millis)
    }

    /// Get all environment variables with the prefix.
    #[must_use]
    pub fn all(&self) -> HashMap<String, String> {
        let mut result = HashMap::new();
        let prefix_upper = format!("{}_", self.prefix.to_uppercase());

        for (key, value) in std::env::vars() {
            if self.prefix.is_empty() || key.starts_with(&prefix_upper) {
                let key = if self.prefix.is_empty() {
                    key
                } else {
                    key[prefix_upper.len()..].to_string()
                };
                result.insert(key, value);
            }
        }

        result
    }

    /// Check if a variable is set.
    #[must_use]
    pub fn is_set(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// Set a value (for testing).
    pub fn set(&mut self, name: &str, value: impl Into<String>) {
        let var_name = self.var_name(name);
        std::env::set_var(&var_name, value.into());
    }

    /// Unset a value (for testing).
    pub fn unset(&mut self, name: &str) {
        let var_name = self.var_name(name);
        std::env::remove_var(&var_name);
    }
}

/// Common environment variables.
pub mod vars {
    /// Default timeout.
    pub const TIMEOUT: &str = "TIMEOUT";
    /// Debug mode.
    pub const DEBUG: &str = "DEBUG";
    /// Log level.
    pub const LOG_LEVEL: &str = "LOG_LEVEL";
    /// Shell to use.
    pub const SHELL: &str = "SHELL";
    /// Terminal type.
    pub const TERM: &str = "TERM";
    /// Terminal columns.
    pub const COLUMNS: &str = "COLUMNS";
    /// Terminal lines.
    pub const LINES: &str = "LINES";
    /// Home directory.
    pub const HOME: &str = "HOME";
    /// User name.
    pub const USER: &str = "USER";
    /// Path.
    pub const PATH: &str = "PATH";
}

/// Get a standard environment variable.
#[must_use]
pub fn get_env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// Get the current user.
#[must_use]
pub fn current_user() -> Option<String> {
    get_env("USER").or_else(|| get_env("USERNAME"))
}

/// Get the home directory.
#[must_use]
pub fn home_dir() -> Option<std::path::PathBuf> {
    get_env("HOME")
        .or_else(|| get_env("USERPROFILE"))
        .map(std::path::PathBuf::from)
}

/// Get the current shell.
#[must_use]
pub fn current_shell() -> Option<String> {
    get_env("SHELL")
}

/// Get the terminal type.
#[must_use]
pub fn term_type() -> Option<String> {
    get_env("TERM")
}

/// Build environment map for a subprocess.
#[must_use]
pub fn subprocess_env() -> HashMap<String, String> {
    let mut env = HashMap::new();

    // Pass through important variables
    for var in ["PATH", "HOME", "USER", "TERM", "LANG", "LC_ALL", "SHELL"] {
        if let Some(value) = get_env(var) {
            env.insert(var.to_string(), value);
        }
    }

    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_config_prefix() {
        let config = EnvConfig::new("TEST");
        assert_eq!(config.var_name("foo"), "TEST_FOO");
        assert_eq!(config.var_name("bar_baz"), "TEST_BAR_BAZ");
    }

    #[test]
    fn env_config_no_prefix() {
        let config = EnvConfig::no_prefix();
        assert_eq!(config.var_name("foo"), "FOO");
    }

    #[test]
    fn env_bool_parsing() {
        let mut config = EnvConfig::new("TEST");
        config.set("ENABLED", "true");
        config.set("DISABLED", "false");

        assert_eq!(config.bool("ENABLED"), Some(true));
        assert_eq!(config.bool("DISABLED"), Some(false));

        config.unset("ENABLED");
        config.unset("DISABLED");
    }

    #[test]
    fn subprocess_env_has_path() {
        let env = subprocess_env();
        // PATH should usually be set
        if std::env::var("PATH").is_ok() {
            assert!(env.contains_key("PATH"));
        }
    }
}
