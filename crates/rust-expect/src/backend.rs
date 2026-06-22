//! Backend module for different transport implementations.
//!
//! This module provides various backends for session communication,
//! including PTY for local processes and SSH for remote connections.

mod pty;

// Export AsyncPty and PtyHandle for Unix platforms
#[cfg(unix)]
pub use pty::{AsyncPty, PtyHandle};
pub use pty::{EnvMode, PtyConfig, PtySpawner, PtyTransport};
// Export WindowsAsyncPty and WindowsPtyHandle for Windows platforms
#[cfg(windows)]
pub use pty::{WindowsAsyncPty, WindowsPtyHandle};

// SSH backend is conditionally compiled
#[cfg(feature = "ssh")]
pub mod ssh;

/// Reaping/liveness probe for a transport that wraps a child process.
///
/// `Session::wait`/`wait_timeout` use this to report the child's *real* exit
/// status after EOF, rather than [`ProcessExitStatus::Unknown`]. Transports
/// that are not backed by a local child process (SSH channels, mock streams)
/// rely on the default implementation, which reports `None` (status unknowable)
/// and so leaves the session reporting `Unknown` — exactly the prior behavior.
///
/// [`ProcessExitStatus::Unknown`]: crate::types::ProcessExitStatus::Unknown
pub trait ChildExit {
    /// Non-blocking reap.
    ///
    /// Returns `Some(status)` once the child has exited and been reaped (the
    /// status is cached, so repeated calls keep returning it), or `None` while
    /// the child is still running or its status cannot be determined.
    fn try_exit_status(&mut self) -> Option<crate::types::ProcessExitStatus> {
        None
    }
}

/// Trait for session backends.
pub trait Backend {
    /// The transport type produced by this backend.
    type Transport;

    /// Check if the backend is available.
    fn is_available(&self) -> bool;

    /// Get the backend name.
    fn name(&self) -> &'static str;
}

/// Available backend types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendType {
    /// Local PTY backend.
    Pty,
    /// SSH backend for remote connections.
    Ssh,
    /// Mock backend for testing.
    Mock,
}

impl BackendType {
    /// Check if this backend is available.
    #[must_use]
    pub const fn is_available(self) -> bool {
        match self {
            Self::Pty => cfg!(unix) || cfg!(windows),
            Self::Ssh => cfg!(feature = "ssh"),
            Self::Mock => cfg!(feature = "mock"),
        }
    }

    /// Get the backend name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Pty => "pty",
            Self::Ssh => "ssh",
            Self::Mock => "mock",
        }
    }
}
