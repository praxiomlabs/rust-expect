//! Error types for rust-expect.
//!
//! This module defines all error types used throughout the library.
//! Errors are designed to be informative, providing context about what went wrong
//! and including relevant data for debugging (e.g., buffer contents on timeout).

use std::process::ExitStatus;
use std::time::Duration;

use thiserror::Error;

/// Maximum length of buffer content to display in error messages.
const MAX_BUFFER_DISPLAY: usize = 500;

/// Context lines to show before/after truncation point.
const CONTEXT_LINES: usize = 3;

/// Format buffer content for display, truncating if necessary.
fn format_buffer_snippet(buffer: &str) -> String {
    if buffer.is_empty() {
        return "(empty buffer)".to_string();
    }

    let buffer_len = buffer.len();

    if buffer_len <= MAX_BUFFER_DISPLAY {
        // Small buffer, show everything with visual markers
        return format!(
            "┌─ buffer ({} bytes) ──────────────────────\n│ {}\n└────────────────────────────────────────",
            buffer_len,
            buffer.lines().collect::<Vec<_>>().join("\n│ ")
        );
    }

    // Large buffer - show tail with context
    let lines: Vec<&str> = buffer.lines().collect();
    let total_lines = lines.len();

    if total_lines <= CONTEXT_LINES * 2 {
        // Few lines, show all
        return format!(
            "┌─ buffer ({} bytes, {} lines) ─────────────\n│ {}\n└────────────────────────────────────────",
            buffer_len,
            total_lines,
            lines.join("\n│ ")
        );
    }

    // Show last N lines with truncation indicator
    let tail_lines = &lines[lines.len().saturating_sub(CONTEXT_LINES * 2)..];
    let hidden = total_lines - tail_lines.len();

    format!(
        "┌─ buffer ({} bytes, {} lines) ─────────────\n│ ... ({} lines hidden)\n│ {}\n└────────────────────────────────────────",
        buffer_len,
        total_lines,
        hidden,
        tail_lines.join("\n│ ")
    )
}

/// Format a timeout error message with enhanced context.
fn format_timeout_error(duration: Duration, pattern: &str, buffer: &str) -> String {
    let buffer_snippet = format_buffer_snippet(buffer);

    format!(
        "timeout after {duration:?} waiting for pattern\n\
         \n\
         Pattern: '{pattern}'\n\
         \n\
         {buffer_snippet}\n\
         \n\
         Tip: The pattern was not found in the output. Check that:\n\
         - The expected text actually appears in the output\n\
         - The pattern is correct (regex special chars may need escaping)\n\
         - The timeout duration is sufficient"
    )
}

/// Format a pattern not found error message.
fn format_pattern_not_found_error(pattern: &str, buffer: &str) -> String {
    let buffer_snippet = format_buffer_snippet(buffer);

    format!(
        "pattern not found before EOF\n\
         \n\
         Pattern: '{pattern}'\n\
         \n\
         {buffer_snippet}\n\
         \n\
         Tip: The process closed before the pattern was found."
    )
}

/// Format a process exited error message.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn format_process_exited_error(exit_status: &ExitStatus, buffer: &str) -> String {
    let buffer_snippet = format_buffer_snippet(buffer);

    format!(
        "process exited unexpectedly with {exit_status:?}\n\
         \n\
         {buffer_snippet}"
    )
}

/// Format an EOF error message.
fn format_eof_error(buffer: &str) -> String {
    let buffer_snippet = format_buffer_snippet(buffer);

    format!(
        "end of file reached unexpectedly\n\
         \n\
         {buffer_snippet}"
    )
}

/// The main error type for rust-expect operations.
#[derive(Debug, Error)]
pub enum ExpectError {
    /// Failed to spawn a process.
    #[error("failed to spawn process: {0}")]
    Spawn(#[from] SpawnError),

    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// An I/O error occurred with additional context.
    #[error("{context}: {source}")]
    IoWithContext {
        /// What operation was being performed.
        context: String,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Timeout waiting for pattern match.
    #[error("{}", format_timeout_error(*duration, pattern, buffer))]
    Timeout {
        /// The timeout duration that elapsed.
        duration: Duration,
        /// The pattern that was being searched for.
        pattern: String,
        /// Buffer contents at the time of timeout.
        buffer: String,
    },

    /// Pattern was not found before EOF.
    #[error("{}", format_pattern_not_found_error(pattern, buffer))]
    PatternNotFound {
        /// The pattern that was being searched for.
        pattern: String,
        /// Buffer contents when EOF was reached.
        buffer: String,
    },

    /// Process exited unexpectedly.
    #[error("{}", format_process_exited_error(exit_status, buffer))]
    ProcessExited {
        /// The exit status of the process.
        exit_status: ExitStatus,
        /// Buffer contents when process exited.
        buffer: String,
    },

    /// End of file reached.
    #[error("{}", format_eof_error(buffer))]
    Eof {
        /// Buffer contents when EOF was reached.
        buffer: String,
    },

    /// Invalid pattern specification.
    #[error("invalid pattern: {message}")]
    InvalidPattern {
        /// Description of what's wrong with the pattern.
        message: String,
    },

    /// Invalid regex pattern.
    #[error("invalid regex pattern: {0}")]
    Regex(#[from] regex::Error),

    /// Session is closed.
    #[error("session is closed")]
    SessionClosed,

    /// Session not found.
    #[error("session with id {id} not found")]
    SessionNotFound {
        /// The session ID that was not found.
        id: usize,
    },

    /// No sessions available for operation.
    #[error("no sessions available for operation")]
    NoSessions,

    /// Error in multi-session operation.
    #[error("multi-session error in session {session_id}: {error}")]
    MultiSessionError {
        /// The session that encountered the error.
        session_id: usize,
        /// The underlying error.
        error: Box<ExpectError>,
    },

    /// Session is not in interact mode.
    #[error("session is not in interact mode")]
    NotInteracting,

    /// Buffer overflow.
    #[error("buffer overflow: maximum size of {max_size} bytes exceeded")]
    BufferOverflow {
        /// The maximum buffer size that was exceeded.
        max_size: usize,
    },

    /// Encoding error.
    #[error("encoding error: {message}")]
    Encoding {
        /// Description of the encoding error.
        message: String,
    },

    /// SSH connection error.
    #[cfg(feature = "ssh")]
    #[error("SSH error: {0}")]
    Ssh(#[from] SshError),

    /// Configuration error.
    #[error("configuration error: {message}")]
    Config {
        /// Description of the configuration error.
        message: String,
    },

    /// Signal error (Unix only).
    #[cfg(unix)]
    #[error("signal error: {message}")]
    Signal {
        /// Description of the signal error.
        message: String,
    },
}

/// Errors related to process spawning.
#[derive(Debug, Error)]
pub enum SpawnError {
    /// Command not found.
    #[error("command not found: {command}")]
    CommandNotFound {
        /// The command that was not found.
        command: String,
    },

    /// Permission denied.
    #[error("permission denied: {path}")]
    PermissionDenied {
        /// The path that could not be accessed.
        path: String,
    },

    /// PTY allocation failed.
    #[error("failed to allocate PTY: {reason}")]
    PtyAllocation {
        /// The reason for the failure.
        reason: String,
    },

    /// Failed to set up terminal.
    #[error("failed to set up terminal: {reason}")]
    TerminalSetup {
        /// The reason for the failure.
        reason: String,
    },

    /// Environment variable error.
    #[error("invalid environment variable: {name}")]
    InvalidEnv {
        /// The name of the invalid environment variable.
        name: String,
    },

    /// Working directory error.
    #[error("invalid working directory: {path}")]
    InvalidWorkingDir {
        /// The invalid working directory path.
        path: String,
    },

    /// General I/O error during spawn.
    #[error("I/O error during spawn: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid command or argument.
    #[error("invalid {kind}: {reason}")]
    InvalidArgument {
        /// The kind of invalid input (e.g., "command", "argument").
        kind: String,
        /// The value that was invalid.
        value: String,
        /// The reason it's invalid.
        reason: String,
    },
}

/// Errors related to SSH connections.
#[cfg(feature = "ssh")]
#[derive(Debug, Error)]
pub enum SshError {
    /// Connection failed.
    #[error("failed to connect to {host}:{port}: {reason}")]
    Connection {
        /// The host that could not be connected to.
        host: String,
        /// The port that was used.
        port: u16,
        /// The reason for the failure.
        reason: String,
    },

    /// Authentication failed.
    #[error("authentication failed for user '{user}': {reason}")]
    Authentication {
        /// The user that failed to authenticate.
        user: String,
        /// The reason for the failure.
        reason: String,
    },

    /// Host key verification failed.
    #[error("host key verification failed for {host}: {reason}")]
    HostKeyVerification {
        /// The host whose key verification failed.
        host: String,
        /// The reason for the failure.
        reason: String,
    },

    /// Channel error.
    #[error("SSH channel error: {reason}")]
    Channel {
        /// The reason for the channel error.
        reason: String,
    },

    /// Session error.
    #[error("SSH session error: {reason}")]
    Session {
        /// The reason for the session error.
        reason: String,
    },

    /// Timeout during SSH operation.
    #[error("SSH operation timed out after {duration:?}")]
    Timeout {
        /// The duration that elapsed.
        duration: Duration,
    },
}

/// Result type alias for rust-expect operations.
pub type Result<T> = std::result::Result<T, ExpectError>;

impl ExpectError {
    /// Create a timeout error with the given details.
    pub fn timeout(
        duration: Duration,
        pattern: impl Into<String>,
        buffer: impl Into<String>,
    ) -> Self {
        Self::Timeout {
            duration,
            pattern: pattern.into(),
            buffer: buffer.into(),
        }
    }

    /// Create a pattern not found error.
    pub fn pattern_not_found(pattern: impl Into<String>, buffer: impl Into<String>) -> Self {
        Self::PatternNotFound {
            pattern: pattern.into(),
            buffer: buffer.into(),
        }
    }

    /// Create a process exited error.
    pub fn process_exited(exit_status: ExitStatus, buffer: impl Into<String>) -> Self {
        Self::ProcessExited {
            exit_status,
            buffer: buffer.into(),
        }
    }

    /// Create an EOF error.
    pub fn eof(buffer: impl Into<String>) -> Self {
        Self::Eof {
            buffer: buffer.into(),
        }
    }

    /// Create an invalid pattern error.
    pub fn invalid_pattern(message: impl Into<String>) -> Self {
        Self::InvalidPattern {
            message: message.into(),
        }
    }

    /// Create a buffer overflow error.
    #[must_use]
    pub const fn buffer_overflow(max_size: usize) -> Self {
        Self::BufferOverflow { max_size }
    }

    /// Create an encoding error.
    pub fn encoding(message: impl Into<String>) -> Self {
        Self::Encoding {
            message: message.into(),
        }
    }

    /// Create a configuration error.
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
        }
    }

    /// Create an I/O error with context.
    pub fn io_context(context: impl Into<String>, source: std::io::Error) -> Self {
        Self::IoWithContext {
            context: context.into(),
            source,
        }
    }

    /// Wrap an I/O result with context.
    pub fn with_io_context<T>(result: std::io::Result<T>, context: impl Into<String>) -> Result<T> {
        result.map_err(|e| Self::io_context(context, e))
    }

    /// Check if this is a timeout error.
    #[must_use]
    pub const fn is_timeout(&self) -> bool {
        matches!(self, Self::Timeout { .. })
    }

    /// Check if this is an EOF error.
    #[must_use]
    pub const fn is_eof(&self) -> bool {
        matches!(self, Self::Eof { .. } | Self::ProcessExited { .. })
    }

    /// Get the buffer contents if this error contains them.
    #[must_use]
    pub fn buffer(&self) -> Option<&str> {
        match self {
            Self::Timeout { buffer, .. }
            | Self::PatternNotFound { buffer, .. }
            | Self::ProcessExited { buffer, .. }
            | Self::Eof { buffer, .. } => Some(buffer),
            _ => None,
        }
    }
}

impl SpawnError {
    /// Create a command not found error.
    pub fn command_not_found(command: impl Into<String>) -> Self {
        Self::CommandNotFound {
            command: command.into(),
        }
    }

    /// Create a permission denied error.
    pub fn permission_denied(path: impl Into<String>) -> Self {
        Self::PermissionDenied { path: path.into() }
    }

    /// Create a PTY allocation error.
    pub fn pty_allocation(reason: impl Into<String>) -> Self {
        Self::PtyAllocation {
            reason: reason.into(),
        }
    }

    /// Create a terminal setup error.
    pub fn terminal_setup(reason: impl Into<String>) -> Self {
        Self::TerminalSetup {
            reason: reason.into(),
        }
    }

    /// Create an invalid environment variable error.
    pub fn invalid_env(name: impl Into<String>) -> Self {
        Self::InvalidEnv { name: name.into() }
    }

    /// Create an invalid working directory error.
    pub fn invalid_working_dir(path: impl Into<String>) -> Self {
        Self::InvalidWorkingDir { path: path.into() }
    }
}

#[cfg(feature = "ssh")]
impl SshError {
    /// Create a connection error.
    pub fn connection(host: impl Into<String>, port: u16, reason: impl Into<String>) -> Self {
        Self::Connection {
            host: host.into(),
            port,
            reason: reason.into(),
        }
    }

    /// Create an authentication error.
    pub fn authentication(user: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Authentication {
            user: user.into(),
            reason: reason.into(),
        }
    }

    /// Create a host key verification error.
    pub fn host_key_verification(host: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::HostKeyVerification {
            host: host.into(),
            reason: reason.into(),
        }
    }

    /// Create a channel error.
    pub fn channel(reason: impl Into<String>) -> Self {
        Self::Channel {
            reason: reason.into(),
        }
    }

    /// Create a session error.
    pub fn session(reason: impl Into<String>) -> Self {
        Self::Session {
            reason: reason.into(),
        }
    }

    /// Create a timeout error.
    #[must_use]
    pub const fn timeout(duration: Duration) -> Self {
        Self::Timeout { duration }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        let err = ExpectError::timeout(
            Duration::from_secs(5),
            "password:",
            "Enter username: admin\n",
        );
        let msg = err.to_string();
        assert!(msg.contains("timeout"));
        assert!(msg.contains("password:"));
        assert!(msg.contains("admin"));
        // Check for enhanced formatting
        assert!(msg.contains("Pattern:"));
        assert!(msg.contains("buffer"));
    }

    #[test]
    fn error_display_with_tips() {
        let err = ExpectError::timeout(Duration::from_secs(5), "password:", "output here\n");
        let msg = err.to_string();
        // Check that tips are included
        assert!(msg.contains("Tip:"));
    }

    #[test]
    fn error_display_empty_buffer() {
        let err = ExpectError::eof("");
        let msg = err.to_string();
        assert!(msg.contains("empty buffer"));
    }

    #[test]
    fn error_display_large_buffer_truncation() {
        // Create a large buffer (> 500 bytes, > 6 lines)
        let large_buffer: String = (0..50).fold(String::new(), |mut acc, i| {
            use std::fmt::Write;
            let _ = writeln!(acc, "Line {i}: Some content here");
            acc
        });

        let err = ExpectError::timeout(Duration::from_secs(1), "pattern", &large_buffer);
        let msg = err.to_string();

        // Should contain truncation indicator
        assert!(msg.contains("lines hidden"));
        // Should show line count
        assert!(msg.contains("lines)"));
    }

    #[test]
    fn error_is_timeout() {
        let timeout = ExpectError::timeout(Duration::from_secs(1), "test", "buffer");
        assert!(timeout.is_timeout());

        let eof = ExpectError::eof("buffer");
        assert!(!eof.is_timeout());
    }

    #[test]
    fn error_buffer() {
        let err = ExpectError::timeout(Duration::from_secs(1), "test", "the buffer");
        assert_eq!(err.buffer(), Some("the buffer"));

        let io_err = ExpectError::Io(std::io::Error::other("test"));
        assert!(io_err.buffer().is_none());
    }

    #[test]
    fn spawn_error_display() {
        let err = SpawnError::command_not_found("/usr/bin/nonexistent");
        assert!(err.to_string().contains("nonexistent"));
    }

    #[test]
    fn format_buffer_snippet_empty() {
        let result = format_buffer_snippet("");
        assert_eq!(result, "(empty buffer)");
    }

    #[test]
    fn format_buffer_snippet_small() {
        let result = format_buffer_snippet("hello\nworld");
        assert!(result.contains("hello"));
        assert!(result.contains("world"));
        assert!(result.contains("bytes"));
    }

    #[test]
    fn pattern_not_found_error() {
        let err = ExpectError::pattern_not_found("prompt>", "some output");
        let msg = err.to_string();
        assert!(msg.contains("prompt>"));
        assert!(msg.contains("some output"));
        assert!(msg.contains("EOF"));
    }

    #[test]
    fn eof_error() {
        let err = ExpectError::eof("final output");
        let msg = err.to_string();
        assert!(msg.contains("end of file"));
        assert!(msg.contains("final output"));
    }

    #[test]
    fn io_with_context_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = ExpectError::io_context("reading config file", io_err);
        let msg = err.to_string();
        assert!(msg.contains("reading config file"));
        assert!(msg.contains("file not found"));
    }

    #[test]
    fn with_io_context_helper() {
        let result: std::io::Result<()> = Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "access denied",
        ));
        let err = ExpectError::with_io_context(result, "writing to log file").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("writing to log file"));
        assert!(msg.contains("access denied"));
    }

    #[test]
    fn with_io_context_success() {
        let result: std::io::Result<i32> = Ok(42);
        let value = ExpectError::with_io_context(result, "some operation").unwrap();
        assert_eq!(value, 42);
    }
}
