//! Core traits for PTY abstraction.
//!
//! This module defines the primary traits used by the rust-pty crate:
//!
//! - [`PtyMaster`]: The master side of a PTY (for reading/writing to the terminal).
//! - [`PtyChild`]: Handle for the spawned child process.
//! - [`PtySystem`]: Factory for creating PTY sessions.

use std::future::Future;
use std::pin::Pin;

use tokio::io::{AsyncRead, AsyncWrite};

use crate::config::{PtyConfig, PtySignal, WindowSize};
use crate::error::Result;

/// The master side of a pseudo-terminal.
///
/// This trait represents the controller end of a PTY pair. It provides
/// async read/write access to the terminal and methods for controlling
/// the PTY (resizing, closing, etc.).
///
/// # Platform Behavior
///
/// - **Unix**: Wraps a file descriptor for the master PTY.
/// - **Windows**: Wraps `ConPTY` input/output pipes.
pub trait PtyMaster: AsyncRead + AsyncWrite + Send + Sync + Unpin {
    /// Resize the PTY to the given window size.
    ///
    /// This sends a window size change notification to the child process
    /// (SIGWINCH on Unix, `ConPTY` resize on Windows).
    fn resize(&self, size: WindowSize) -> Result<()>;

    /// Get the current window size.
    fn window_size(&self) -> Result<WindowSize>;

    /// Close the master side of the PTY.
    ///
    /// This signals EOF to the child process. After calling this method,
    /// reads will return EOF and writes will fail.
    fn close(&mut self) -> Result<()>;

    /// Check if the PTY is still open.
    fn is_open(&self) -> bool;

    /// Get the raw file descriptor (Unix) or handle (Windows).
    ///
    /// # Safety
    ///
    /// The returned value is platform-specific and should only be used
    /// for low-level operations that understand the platform semantics.
    #[cfg(unix)]
    fn as_raw_fd(&self) -> std::os::unix::io::RawFd;

    /// Get the raw handle (Windows only).
    #[cfg(windows)]
    fn as_raw_handle(&self) -> std::os::windows::io::RawHandle;
}

/// Handle for a child process spawned in a PTY.
///
/// This trait provides methods for monitoring and controlling the child
/// process. It's separate from [`PtyMaster`] to allow independent lifetime
/// management of the PTY and the process.
pub trait PtyChild: Send + Sync {
    /// Get the process ID of the child.
    fn pid(&self) -> u32;

    /// Check if the child process is still running.
    fn is_running(&self) -> bool;

    /// Wait for the child process to exit.
    ///
    /// Returns the exit status when the process terminates.
    fn wait(&mut self) -> Pin<Box<dyn Future<Output = Result<ExitStatus>> + Send + '_>>;

    /// Try to get the exit status without blocking.
    ///
    /// Returns `None` if the process is still running.
    fn try_wait(&mut self) -> Result<Option<ExitStatus>>;

    /// Send a signal to the child process.
    fn signal(&self, signal: PtySignal) -> Result<()>;

    /// Kill the child process.
    ///
    /// This sends SIGKILL on Unix or calls `TerminateProcess` on Windows.
    fn kill(&mut self) -> Result<()>;
}

/// Exit status of a child process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitStatus {
    /// The process exited normally with the given exit code.
    Exited(i32),

    /// The process was terminated by a signal (Unix only).
    #[cfg(unix)]
    Signaled(i32),

    /// The process was terminated (Windows).
    /// The exit code may not be meaningful.
    #[cfg(windows)]
    Terminated(u32),
}

impl ExitStatus {
    /// Check if the process exited successfully (exit code 0).
    #[must_use]
    pub const fn success(&self) -> bool {
        matches!(self, Self::Exited(0))
    }

    /// Get the exit code, if available.
    #[must_use]
    pub const fn code(&self) -> Option<i32> {
        match self {
            Self::Exited(code) => Some(*code),
            #[cfg(unix)]
            Self::Signaled(_) => None,
            #[cfg(windows)]
            Self::Terminated(code) => Some(*code as i32),
        }
    }

    /// Get the signal number that terminated the process (Unix only).
    #[cfg(unix)]
    #[must_use]
    pub const fn signal(&self) -> Option<i32> {
        match self {
            Self::Signaled(sig) => Some(*sig),
            _ => None,
        }
    }
}

impl std::fmt::Display for ExitStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exited(code) => write!(f, "exited with code {code}"),
            #[cfg(unix)]
            Self::Signaled(sig) => write!(f, "terminated by signal {sig}"),
            #[cfg(windows)]
            Self::Terminated(code) => write!(f, "terminated with code {code}"),
        }
    }
}

/// Factory trait for creating PTY sessions.
///
/// This trait provides the main entry point for spawning processes in a PTY.
/// Platform-specific implementations handle the details of PTY creation.
pub trait PtySystem: Send + Sync {
    /// The master PTY type for this platform.
    type Master: PtyMaster;
    /// The child process type for this platform.
    type Child: PtyChild;

    /// Spawn a new process in a PTY.
    ///
    /// # Arguments
    ///
    /// * `program` - The program to execute.
    /// * `args` - Command-line arguments (not including the program name).
    /// * `config` - PTY configuration.
    ///
    /// # Returns
    ///
    /// A tuple of the master PTY and child process handle.
    fn spawn<S, I>(
        program: S,
        args: I,
        config: &PtyConfig,
    ) -> impl Future<Output = Result<(Self::Master, Self::Child)>> + Send
    where
        S: AsRef<std::ffi::OsStr> + Send,
        I: IntoIterator + Send,
        I::Item: AsRef<std::ffi::OsStr>;

    /// Spawn a shell in a PTY using the default configuration.
    ///
    /// On Unix, this uses the user's shell from the SHELL environment variable
    /// or falls back to `/bin/sh`. On Windows, this uses `cmd.exe`.
    #[must_use]
    fn spawn_shell(
        config: &PtyConfig,
    ) -> impl Future<Output = Result<(Self::Master, Self::Child)>> + Send {
        async move {
            #[cfg(unix)]
            let shell =
                std::env::var_os("SHELL").unwrap_or_else(|| std::ffi::OsString::from("/bin/sh"));
            #[cfg(windows)]
            let shell = std::ffi::OsString::from("cmd.exe");

            Self::spawn(&shell, std::iter::empty::<&str>(), config).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_status_success() {
        let status = ExitStatus::Exited(0);
        assert!(status.success());
        assert_eq!(status.code(), Some(0));
    }

    #[test]
    fn exit_status_failure() {
        let status = ExitStatus::Exited(1);
        assert!(!status.success());
        assert_eq!(status.code(), Some(1));
    }

    #[cfg(unix)]
    #[test]
    fn exit_status_signaled() {
        let status = ExitStatus::Signaled(9);
        assert!(!status.success());
        assert_eq!(status.code(), None);
        assert_eq!(status.signal(), Some(9));
    }
}
