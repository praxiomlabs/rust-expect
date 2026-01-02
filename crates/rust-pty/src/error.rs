//! Error types for the rust-pty crate.
//!
//! This module provides a unified error type [`PtyError`] that covers all
//! possible failure modes when working with pseudo-terminals.

use std::io;

/// The error type for PTY operations.
///
/// This enum represents all possible errors that can occur when creating,
/// using, or managing pseudo-terminals across different platforms.
#[derive(Debug, thiserror::Error)]
pub enum PtyError {
    /// Failed to create a new PTY.
    #[error("failed to create PTY: {0}")]
    Create(#[source] io::Error),

    /// Failed to spawn a child process.
    #[error("failed to spawn process: {0}")]
    Spawn(#[source] io::Error),

    /// An I/O error occurred during PTY operations.
    #[error("PTY I/O error: {0}")]
    Io(#[from] io::Error),

    /// Failed to set terminal attributes.
    #[error("failed to set terminal attributes: {0}")]
    SetAttributes(#[source] io::Error),

    /// Failed to get terminal attributes.
    #[error("failed to get terminal attributes: {0}")]
    GetAttributes(#[source] io::Error),

    /// Failed to resize the PTY.
    #[error("failed to resize PTY: {0}")]
    Resize(#[source] io::Error),

    /// The PTY has been closed.
    #[error("PTY has been closed")]
    Closed,

    /// The child process has exited.
    #[error("child process exited with status: {0}")]
    ProcessExited(i32),

    /// The child process was killed by a signal.
    #[cfg(unix)]
    #[error("child process killed by signal: {0}")]
    ProcessSignaled(i32),

    /// Failed to send a signal to the child process.
    #[error("failed to send signal: {0}")]
    Signal(#[source] io::Error),

    /// Failed to wait for the child process.
    #[error("failed to wait for child: {0}")]
    Wait(#[source] io::Error),

    /// Invalid window size specified.
    #[error("invalid window size: {width}x{height}")]
    InvalidWindowSize {
        /// The requested width.
        width: u16,
        /// The requested height.
        height: u16,
    },

    /// The operation timed out.
    #[error("operation timed out")]
    Timeout,

    /// Platform-specific error on Unix.
    #[cfg(unix)]
    #[error("Unix error: {message}")]
    Unix {
        /// Description of the error.
        message: String,
        /// The underlying errno value.
        errno: i32,
    },

    /// Platform-specific error on Windows.
    #[cfg(windows)]
    #[error("Windows error: {message} (code: {code})")]
    Windows {
        /// Description of the error.
        message: String,
        /// The Windows error code.
        code: u32,
    },

    /// ConPTY is not available (Windows version too old).
    #[cfg(windows)]
    #[error("ConPTY is not available on this Windows version")]
    ConPtyNotAvailable,
}

/// A specialized Result type for PTY operations.
pub type Result<T> = std::result::Result<T, PtyError>;

#[cfg(unix)]
impl From<rustix::io::Errno> for PtyError {
    fn from(errno: rustix::io::Errno) -> Self {
        Self::Io(io::Error::from_raw_os_error(errno.raw_os_error()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        let err = PtyError::Closed;
        assert_eq!(err.to_string(), "PTY has been closed");
    }

    #[test]
    fn error_from_io() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "not found");
        let pty_err: PtyError = io_err.into();
        assert!(matches!(pty_err, PtyError::Io(_)));
    }
}
