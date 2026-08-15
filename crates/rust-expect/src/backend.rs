//! Backend module for different transport implementations.
//!
//! This module provides various backends for session communication,
//! including PTY for local processes and SSH for remote connections.

mod pty;

// Export AsyncPty and PtyHandle for Unix platforms
#[cfg(unix)]
pub use pty::{AsyncPty, PtyHandle, PtyProcess};
pub use pty::{EnvMode, PtyConfig, PtySpawner, PtyTransport};
// Export WindowsAsyncPty and WindowsPtyHandle for Windows platforms
#[cfg(windows)]
pub use pty::{WindowsAsyncPty, WindowsPtyHandle, WindowsPtyProcess};

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

/// Terminal-resize capability, independent of process control.
///
/// Resizing acts on the transport's terminal (the PTY master), not on the
/// child, which is why it is a separate capability from [`ProcessControl`] —
/// an SSH channel can be resizable without any local process to signal.
pub trait Resizable {
    /// Resize the terminal to `cols` × `rows`.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying resize operation fails.
    fn resize(&mut self, cols: u16, rows: u16) -> Result<(), crate::error::ExpectError>;

    /// The transport's current dimensions as `(cols, rows)`.
    fn dimensions(&self) -> (u16, u16);
}

/// Control over the child process behind a session, held separately from the
/// transport that carries its I/O.
///
/// Keeping these operations off the transport is what lets a session be killed
/// while a read is parked. Every method here is a short, non-blocking syscall,
/// so implementations are held behind a plain [`std::sync::Mutex`] rather than
/// an async one, and no implementation may block or await.
pub trait ProcessControl: Send {
    /// The child's process id, if the backend has one.
    fn pid(&self) -> Option<u32>;

    /// Send a signal to the child.
    ///
    /// The default implementation reports the operation as unsupported, which
    /// is correct for every backend without Unix signal semantics.
    ///
    /// # Errors
    ///
    /// Returns [`ExpectError::Unsupported`] if the backend has no signals,
    /// [`ExpectError::SessionClosed`] if the child has already exited, or an
    /// I/O error if delivery fails.
    ///
    /// [`ExpectError::Unsupported`]: crate::error::ExpectError::Unsupported
    /// [`ExpectError::SessionClosed`]: crate::error::ExpectError::SessionClosed
    fn signal(&mut self, signal: i32) -> Result<(), crate::error::ExpectError> {
        let _ = signal;
        Err(crate::error::ExpectError::Unsupported {
            operation: "signal",
        })
    }

    /// Kill the child.
    ///
    /// # Errors
    ///
    /// Returns an error if the child cannot be killed.
    fn kill(&mut self) -> Result<(), crate::error::ExpectError>;

    /// Whether the child is still running.
    ///
    /// Takes `&mut self` because the honest answer requires a non-blocking
    /// reap. Unlike the pre-capability `Session::is_running`, this cannot
    /// report a guess: there is no lock to fail to acquire.
    fn is_running(&mut self) -> bool;

    /// Non-blocking reap, as [`ChildExit::try_exit_status`].
    ///
    /// The child lives behind this handle, so backends whose transport also
    /// implements [`ChildExit`] delegate here rather than keeping a second
    /// child reference.
    fn try_exit_status(&mut self) -> Option<crate::types::ProcessExitStatus> {
        None
    }

    /// Whether the child has been observed to exit, by any status the backend
    /// can report.
    ///
    /// Deliberately distinct from `!is_running()`: this is the guard a
    /// transport's `poll_write` uses, and it must answer "has it gone" even for
    /// exit forms [`Self::try_exit_status`] declines to map. Backends with no
    /// such gap can leave the default.
    fn has_exited(&mut self) -> bool {
        !self.is_running()
    }

    /// Give up ownership of the child, so dropping this handle leaves it
    /// running.
    ///
    /// One-way. Backends that never owned the child in the first place — SSH
    /// channels, mocks, plain streams — leave the default, which does nothing
    /// because there is nothing to give up.
    fn detach(&mut self) {}
}

/// A cloneable handle to a session's [`ProcessControl`].
///
/// Shared between the session and, where a backend needs it, the transport —
/// `AsyncPty::poll_write` consults it to turn a write to an exited child into
/// `BrokenPipe`. The inner lock is only ever held for the duration of one
/// syscall and never across an await, so contention here does not park a task.
#[derive(Clone)]
pub struct ProcessHandle(std::sync::Arc<std::sync::Mutex<dyn ProcessControl + Send>>);

impl ProcessHandle {
    /// Wrap a [`ProcessControl`] implementation in a shareable handle.
    pub fn new<P: ProcessControl + Send + 'static>(control: P) -> Self {
        Self(std::sync::Arc::new(std::sync::Mutex::new(control)))
    }

    /// Run `f` against the control implementation.
    ///
    /// Recovers from lock poisoning rather than propagating it: a panic in one
    /// control call must not make a session permanently unkillable. This
    /// mirrors the screen mutex's recovery in `session::handle`.
    pub fn with<R>(&self, f: impl FnOnce(&mut (dyn ProcessControl + Send)) -> R) -> R {
        let mut guard = match self.0.lock() {
            Ok(g) => g,
            Err(poison) => {
                tracing::warn!("process-control mutex was poisoned; recovering inner state");
                poison.into_inner()
            }
        };
        f(&mut *guard)
    }
}

impl std::fmt::Debug for ProcessHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessHandle")
            .field("pid", &self.with(|c| c.pid()))
            .finish_non_exhaustive()
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
