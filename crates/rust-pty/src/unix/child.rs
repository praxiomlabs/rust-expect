//! Unix child process management for PTY.
//!
//! This module provides child process spawning and management for Unix PTY
//! sessions, handling fork, exec, and process lifecycle.

use std::ffi::OsStr;
use std::future::Future;
use std::io;
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};
use std::pin::Pin;
use std::process::ExitStatus as StdExitStatus;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use rustix::process::{kill_process, Pid, Signal, WaitStatus};
use tokio::process::Child as TokioChild;
use tokio::sync::Mutex;

use crate::config::{PtyConfig, PtySignal};
use crate::error::{PtyError, Result};
use crate::traits::{ExitStatus, PtyChild};

/// Unix child process handle.
///
/// This struct manages a child process spawned in a PTY, providing methods
/// for monitoring its state and sending signals.
pub struct UnixPtyChild {
    /// The underlying tokio child process (if using Command-based spawn).
    child: Arc<Mutex<Option<TokioChild>>>,
    /// The process ID.
    pid: u32,
    /// Whether the process is still running.
    running: Arc<AtomicBool>,
    /// Cached exit status.
    exit_status: Arc<Mutex<Option<ExitStatus>>>,
}

impl std::fmt::Debug for UnixPtyChild {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnixPtyChild")
            .field("pid", &self.pid)
            .field("running", &self.running.load(Ordering::SeqCst))
            .finish()
    }
}

impl UnixPtyChild {
    /// Create a new child process handle.
    #[must_use] pub fn new(child: TokioChild) -> Self {
        let pid = child.id().expect("child should have pid");
        Self {
            child: Arc::new(Mutex::new(Some(child))),
            pid,
            running: Arc::new(AtomicBool::new(true)),
            exit_status: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a child handle from just a PID (for fork-based spawning).
    #[must_use] pub fn from_pid(pid: u32) -> Self {
        Self {
            child: Arc::new(Mutex::new(None)),
            pid,
            running: Arc::new(AtomicBool::new(true)),
            exit_status: Arc::new(Mutex::new(None)),
        }
    }

    /// Get the process ID.
    #[must_use]
    pub const fn pid(&self) -> u32 {
        self.pid
    }

    /// Check if the process is still running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Wait for the child process to exit.
    pub async fn wait(&mut self) -> Result<ExitStatus> {
        // Check cached status
        {
            let status = self.exit_status.lock().await;
            if let Some(s) = *status {
                return Ok(s);
            }
        }

        // Try to wait using tokio child if available
        let mut child_guard = self.child.lock().await;
        if let Some(ref mut child) = *child_guard {
            let status = child.wait().await.map_err(PtyError::Wait)?;
            let exit_status = convert_exit_status(status);

            self.running.store(false, Ordering::SeqCst);
            *self.exit_status.lock().await = Some(exit_status);

            return Ok(exit_status);
        }

        // Fall back to waitpid for fork-based spawn
        drop(child_guard);
        self.wait_pid().await
    }

    /// Wait using waitpid system call.
    async fn wait_pid(&mut self) -> Result<ExitStatus> {
        use rustix::process::{waitpid, WaitOptions};

        let pid = Pid::from_raw(self.pid as i32)
            .ok_or_else(|| PtyError::Wait(io::Error::new(io::ErrorKind::InvalidInput, "invalid pid")))?;

        // Use blocking waitpid in a spawn_blocking context
        let result = tokio::task::spawn_blocking(move || {
            waitpid(Some(pid), WaitOptions::empty())
        })
        .await
        .map_err(|e| PtyError::Wait(io::Error::new(io::ErrorKind::Other, e)))?;

        match result {
            Ok(Some((_pid, wait_status))) => {
                let exit_status = convert_wait_status(wait_status);

                self.running.store(false, Ordering::SeqCst);
                *self.exit_status.lock().await = Some(exit_status);

                Ok(exit_status)
            }
            Ok(None) => {
                // Process still running, shouldn't happen with default options
                Err(PtyError::Wait(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "process still running",
                )))
            }
            Err(e) => Err(PtyError::Wait(io::Error::from_raw_os_error(e.raw_os_error()))),
        }
    }

    /// Try to get the exit status without blocking.
    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>> {
        use rustix::process::{waitpid, WaitOptions};

        // Check cached status first
        if let Ok(guard) = self.exit_status.try_lock() {
            if let Some(s) = *guard {
                return Ok(Some(s));
            }
        }

        let pid = Pid::from_raw(self.pid as i32)
            .ok_or_else(|| PtyError::Wait(io::Error::new(io::ErrorKind::InvalidInput, "invalid pid")))?;

        match waitpid(Some(pid), WaitOptions::NOHANG) {
            Ok(Some((_pid, wait_status))) => {
                let exit_status = convert_wait_status(wait_status);

                self.running.store(false, Ordering::SeqCst);
                if let Ok(mut guard) = self.exit_status.try_lock() {
                    *guard = Some(exit_status);
                }

                Ok(Some(exit_status))
            }
            Ok(None) => Ok(None), // Still running
            Err(e) => Err(PtyError::Wait(io::Error::from_raw_os_error(e.raw_os_error()))),
        }
    }

    /// Send a signal to the child process.
    pub fn signal(&self, signal: PtySignal) -> Result<()> {
        if !self.is_running() {
            return Err(PtyError::ProcessExited(0));
        }

        let sig_num = signal
            .as_unix_signal()
            .ok_or_else(|| PtyError::Signal(io::Error::new(io::ErrorKind::Unsupported, "unsupported signal")))?;

        let pid = Pid::from_raw(self.pid as i32)
            .ok_or_else(|| PtyError::Signal(io::Error::new(io::ErrorKind::InvalidInput, "invalid pid")))?;

        let signal = Signal::from_named_raw(sig_num)
            .ok_or_else(|| PtyError::Signal(io::Error::new(io::ErrorKind::InvalidInput, "invalid signal")))?;

        kill_process(pid, signal)
            .map_err(|e| PtyError::Signal(io::Error::from_raw_os_error(e.raw_os_error())))
    }

    /// Kill the child process (SIGKILL).
    pub fn kill(&mut self) -> Result<()> {
        self.signal(PtySignal::Kill)
    }
}

impl PtyChild for UnixPtyChild {
    fn pid(&self) -> u32 {
        Self::pid(self)
    }

    fn is_running(&self) -> bool {
        Self::is_running(self)
    }

    fn wait(&mut self) -> Pin<Box<dyn Future<Output = Result<ExitStatus>> + Send + '_>> {
        Box::pin(Self::wait(self))
    }

    fn try_wait(&mut self) -> Result<Option<ExitStatus>> {
        Self::try_wait(self)
    }

    fn signal(&self, signal: PtySignal) -> Result<()> {
        Self::signal(self, signal)
    }

    fn kill(&mut self) -> Result<()> {
        Self::kill(self)
    }
}

/// Convert rustix `WaitStatus` to our `ExitStatus`.
fn convert_wait_status(status: WaitStatus) -> ExitStatus {
    if status.exited() {
        // Get exit code
        let code = status.exit_status().unwrap_or(0);
        ExitStatus::Exited(code)
    } else if status.signaled() {
        // Get terminating signal - it's already an i32
        let signal = status.terminating_signal().unwrap_or(0);
        ExitStatus::Signaled(signal)
    } else {
        // Stopped or continued - process not actually exited
        ExitStatus::Exited(-1)
    }
}

/// Convert `std::process::ExitStatus` to our `ExitStatus`.
fn convert_exit_status(status: StdExitStatus) -> ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(code) = status.code() {
            ExitStatus::Exited(code)
        } else if let Some(signal) = status.signal() {
            ExitStatus::Signaled(signal)
        } else {
            ExitStatus::Exited(-1)
        }
    }

    #[cfg(not(unix))]
    {
        ExitStatus::Exited(status.code().unwrap_or(-1))
    }
}

/// Spawn a child process in a PTY.
///
/// This sets up the child's stdin/stdout/stderr to use the slave PTY
/// and executes the specified program.
pub async fn spawn_child<S, I>(
    slave_fd: OwnedFd,
    program: S,
    args: I,
    config: &PtyConfig,
) -> Result<UnixPtyChild>
where
    S: AsRef<OsStr>,
    I: IntoIterator,
    I::Item: AsRef<OsStr>,
{
    use std::process::Stdio;
    use tokio::process::Command;

    // Convert to raw fd for dup2
    let slave_raw = slave_fd.as_raw_fd();

    // Build environment
    let env = config.effective_env();

    // Build command
    let mut cmd = Command::new(program.as_ref());
    cmd.args(args);
    cmd.env_clear();
    cmd.envs(env);

    if let Some(ref dir) = config.working_directory {
        cmd.current_dir(dir);
    }

    // Set up stdio to use the slave PTY
    // SAFETY: We're duplicating a valid fd
    unsafe {
        cmd.stdin(Stdio::from_raw_fd(libc::dup(slave_raw)));
        cmd.stdout(Stdio::from_raw_fd(libc::dup(slave_raw)));
        cmd.stderr(Stdio::from_raw_fd(libc::dup(slave_raw)));
    }

    // Configure process
    if config.new_session {
        cmd.process_group(0);
    }

    // Pre-exec hook to set up controlling terminal
    #[cfg(unix)]
    if config.controlling_terminal {
        // SAFETY: These are async-signal-safe operations
        unsafe {
            cmd.pre_exec(move || {
                // Create new session
                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }

                // Set controlling terminal
                if libc::ioctl(slave_raw, libc::TIOCSCTTY, 0) == -1 {
                    return Err(io::Error::last_os_error());
                }

                Ok(())
            });
        }
    }

    let child = cmd.spawn().map_err(PtyError::Spawn)?;

    Ok(UnixPtyChild::new(child))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn child_from_pid() {
        let child = UnixPtyChild::from_pid(1234);
        assert_eq!(child.pid(), 1234);
        assert!(child.is_running());
    }
}
