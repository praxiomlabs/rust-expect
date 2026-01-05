//! Command and argument validation.
//!
//! This module provides validation functions for commands and arguments
//! to prevent security issues such as command injection.

use crate::error::{ExpectError, SpawnError};

/// Characters that are potentially dangerous in shell contexts.
///
/// These characters can be used for command injection or have special meanings
/// in shell environments. While the library uses `execve` directly (not through a shell),
/// validating these helps prevent issues when commands are logged, displayed, or
/// when users accidentally pass shell-interpreted strings.
pub const SHELL_METACHARACTERS: &[char] = &[
    ';', '&', '|', '`', '$', '(', ')', '{', '}', '[', ']', '<', '>', '!', '*', '?', '#', '~', '\\',
    '"', '\'', '\n', '\r',
];

/// Validation options for command arguments.
#[derive(Debug, Clone, Default)]
pub struct ValidationOptions {
    /// Whether to reject null bytes.
    pub reject_null_bytes: bool,
    /// Whether to reject shell metacharacters.
    pub reject_shell_metacharacters: bool,
    /// Whether to reject empty strings.
    pub reject_empty: bool,
}

impl ValidationOptions {
    /// Create strict validation options (rejects null bytes and empty strings).
    #[must_use]
    pub const fn strict() -> Self {
        Self {
            reject_null_bytes: true,
            reject_shell_metacharacters: false,
            reject_empty: true,
        }
    }

    /// Create paranoid validation options (rejects all potentially dangerous characters).
    #[must_use]
    pub const fn paranoid() -> Self {
        Self {
            reject_null_bytes: true,
            reject_shell_metacharacters: true,
            reject_empty: true,
        }
    }

    /// Create permissive validation options (only rejects null bytes).
    #[must_use]
    pub const fn permissive() -> Self {
        Self {
            reject_null_bytes: true,
            reject_shell_metacharacters: false,
            reject_empty: false,
        }
    }
}

/// Check if a string contains null bytes.
#[must_use]
pub fn contains_null_byte(s: &str) -> bool {
    s.contains('\0')
}

/// Check if a string contains shell metacharacters.
#[must_use]
pub fn contains_shell_metachar(s: &str) -> bool {
    s.chars().any(|c| SHELL_METACHARACTERS.contains(&c))
}

/// Find the first shell metacharacter in a string.
#[must_use]
pub fn find_shell_metachar(s: &str) -> Option<char> {
    s.chars().find(|c| SHELL_METACHARACTERS.contains(c))
}

/// Validate a command string.
///
/// # Arguments
///
/// * `command` - The command to validate
/// * `options` - Validation options
///
/// # Returns
///
/// Returns `Ok(())` if the command is valid, or an error describing why it's invalid.
pub fn validate_command(command: &str, options: &ValidationOptions) -> crate::error::Result<()> {
    if options.reject_empty && command.is_empty() {
        return Err(ExpectError::Spawn(SpawnError::InvalidArgument {
            kind: "command".to_string(),
            value: String::new(),
            reason: "command cannot be empty".to_string(),
        }));
    }

    if options.reject_null_bytes && contains_null_byte(command) {
        return Err(ExpectError::Spawn(SpawnError::InvalidArgument {
            kind: "command".to_string(),
            value: command.to_string(),
            reason: "command contains null byte".to_string(),
        }));
    }

    if options.reject_shell_metacharacters
        && let Some(c) = find_shell_metachar(command)
    {
        return Err(ExpectError::Spawn(SpawnError::InvalidArgument {
            kind: "command".to_string(),
            value: command.to_string(),
            reason: format!("command contains shell metacharacter '{c}'"),
        }));
    }

    Ok(())
}

/// Validate a command argument.
///
/// # Arguments
///
/// * `arg` - The argument to validate
/// * `options` - Validation options
///
/// # Returns
///
/// Returns `Ok(())` if the argument is valid, or an error describing why it's invalid.
pub fn validate_argument(arg: &str, options: &ValidationOptions) -> crate::error::Result<()> {
    if options.reject_null_bytes && contains_null_byte(arg) {
        return Err(ExpectError::Spawn(SpawnError::InvalidArgument {
            kind: "argument".to_string(),
            value: arg.to_string(),
            reason: "argument contains null byte".to_string(),
        }));
    }

    if options.reject_shell_metacharacters
        && let Some(c) = find_shell_metachar(arg)
    {
        return Err(ExpectError::Spawn(SpawnError::InvalidArgument {
            kind: "argument".to_string(),
            value: arg.to_string(),
            reason: format!("argument contains shell metacharacter '{c}'"),
        }));
    }

    Ok(())
}

/// Validate a command and all its arguments.
///
/// # Arguments
///
/// * `command` - The command to validate
/// * `args` - The arguments to validate
/// * `options` - Validation options
///
/// # Returns
///
/// Returns `Ok(())` if all inputs are valid, or an error describing the first invalid input.
pub fn validate_command_with_args<I, S>(
    command: &str,
    args: I,
    options: &ValidationOptions,
) -> crate::error::Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    validate_command(command, options)?;

    for arg in args {
        validate_argument(arg.as_ref(), options)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_byte_detection() {
        assert!(contains_null_byte("hello\0world"));
        assert!(!contains_null_byte("hello world"));
    }

    #[test]
    fn test_shell_metachar_detection() {
        assert!(contains_shell_metachar("echo; rm -rf"));
        assert!(contains_shell_metachar("$(whoami)"));
        assert!(contains_shell_metachar("hello | world"));
        assert!(!contains_shell_metachar("hello_world"));
        assert!(!contains_shell_metachar("/usr/bin/test"));
    }

    #[test]
    fn test_validate_command_null_byte() {
        let opts = ValidationOptions::strict();
        assert!(validate_command("test\0cmd", &opts).is_err());
        assert!(validate_command("test_cmd", &opts).is_ok());
    }

    #[test]
    fn test_validate_command_empty() {
        let strict = ValidationOptions::strict();
        let permissive = ValidationOptions::permissive();

        assert!(validate_command("", &strict).is_err());
        assert!(validate_command("", &permissive).is_ok());
    }

    #[test]
    fn test_validate_command_metachar() {
        let paranoid = ValidationOptions::paranoid();
        let strict = ValidationOptions::strict();

        assert!(validate_command("echo; rm", &paranoid).is_err());
        assert!(validate_command("echo; rm", &strict).is_ok());
    }

    #[test]
    fn test_validate_argument() {
        let opts = ValidationOptions::strict();
        assert!(validate_argument("normal_arg", &opts).is_ok());
        assert!(validate_argument("--flag", &opts).is_ok());
        assert!(validate_argument("arg\0value", &opts).is_err());
    }

    #[test]
    fn test_validate_command_with_args() {
        let opts = ValidationOptions::strict();

        assert!(validate_command_with_args("/bin/echo", ["hello", "world"], &opts).is_ok());
        assert!(validate_command_with_args("/bin/echo", ["hello\0world"], &opts).is_err());
    }
}
