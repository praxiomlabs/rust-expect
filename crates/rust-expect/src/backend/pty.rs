//! PTY backend for local process spawning.
//!
//! This module provides the PTY backend that uses the rust-pty crate
//! to spawn local processes with pseudo-terminal support.

use crate::config::SessionConfig;
use crate::error::{ExpectError, Result, SpawnError};
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// A PTY-based transport for local process communication.
pub struct PtyTransport {
    /// The PTY reader half.
    reader: Box<dyn AsyncRead + Unpin + Send>,
    /// The PTY writer half.
    writer: Box<dyn AsyncWrite + Unpin + Send>,
    /// Process ID.
    pid: Option<u32>,
}

impl PtyTransport {
    /// Create a new PTY transport from reader and writer.
    pub fn new<R, W>(reader: R, writer: W) -> Self
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        Self {
            reader: Box::new(reader),
            writer: Box::new(writer),
            pid: None,
        }
    }

    /// Set the process ID.
    pub fn set_pid(&mut self, pid: u32) {
        self.pid = Some(pid);
    }

    /// Get the process ID.
    #[must_use]
    pub const fn pid(&self) -> Option<u32> {
        self.pid
    }
}

impl AsyncRead for PtyTransport {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.reader).poll_read(cx, buf)
    }
}

impl AsyncWrite for PtyTransport {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.writer).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.writer).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.writer).poll_shutdown(cx)
    }
}

/// Configuration for PTY spawning.
#[derive(Debug, Clone)]
pub struct PtyConfig {
    /// Terminal dimensions (cols, rows).
    pub dimensions: (u16, u16),
    /// Whether to use a login shell.
    pub login_shell: bool,
    /// Environment variable handling.
    pub env_mode: EnvMode,
}

impl Default for PtyConfig {
    fn default() -> Self {
        Self {
            dimensions: (80, 24),
            login_shell: false,
            env_mode: EnvMode::Inherit,
        }
    }
}

impl From<&SessionConfig> for PtyConfig {
    fn from(config: &SessionConfig) -> Self {
        Self {
            dimensions: config.dimensions,
            login_shell: false,
            env_mode: if config.env.is_empty() {
                EnvMode::Inherit
            } else {
                EnvMode::Extend
            },
        }
    }
}

/// Environment variable handling mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvMode {
    /// Inherit all environment variables from parent.
    Inherit,
    /// Clear environment and only use specified variables.
    Clear,
    /// Inherit and extend with specified variables.
    Extend,
}

/// Spawner for PTY sessions.
pub struct PtySpawner {
    config: PtyConfig,
}

impl PtySpawner {
    /// Create a new PTY spawner with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: PtyConfig::default(),
        }
    }

    /// Create a new PTY spawner with custom configuration.
    #[must_use]
    pub const fn with_config(config: PtyConfig) -> Self {
        Self { config }
    }

    /// Set the terminal dimensions.
    pub fn set_dimensions(&mut self, cols: u16, rows: u16) {
        self.config.dimensions = (cols, rows);
    }

    /// Spawn a command.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The command or arguments contain null bytes
    /// - PTY allocation fails
    /// - Fork fails
    /// - Exec fails (child exits with code 1)
    #[cfg(unix)]
    pub async fn spawn(&self, command: &str, args: &[String]) -> Result<PtyHandle> {
        use std::ffi::CString;

        // Validate and create CStrings BEFORE forking so we can return proper errors
        let cmd_cstring = CString::new(command).map_err(|_| {
            ExpectError::Spawn(SpawnError::InvalidArgument {
                kind: "command".to_string(),
                value: command.to_string(),
                reason: "command contains null byte".to_string(),
            })
        })?;

        let mut argv_cstrings: Vec<CString> = Vec::with_capacity(args.len() + 1);
        argv_cstrings.push(cmd_cstring.clone());

        for (idx, arg) in args.iter().enumerate() {
            let arg_cstring = CString::new(arg.as_str()).map_err(|_| {
                ExpectError::Spawn(SpawnError::InvalidArgument {
                    kind: format!("argument[{}]", idx),
                    value: arg.clone(),
                    reason: "argument contains null byte".to_string(),
                })
            })?;
            argv_cstrings.push(arg_cstring);
        }

        // Create PTY pair
        // SAFETY: openpty() is called with valid pointers to stack-allocated integers.
        // The null pointers for name, termp, and winp are explicitly allowed per POSIX.
        // We check the return value and handle errors appropriately.
        let pty_result = unsafe {
            let mut master: libc::c_int = 0;
            let mut slave: libc::c_int = 0;

            // Open PTY
            if libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            ) != 0
            {
                return Err(ExpectError::Spawn(SpawnError::PtyAllocation {
                    reason: "Failed to open PTY".to_string(),
                }));
            }

            (master, slave)
        };

        let (master_fd, slave_fd) = pty_result;

        // Fork the process
        // SAFETY: fork() is safe to call at this point as we have no threads running
        // that could hold locks. The child process will immediately set up its
        // environment and exec into the target program.
        let pid = unsafe { libc::fork() };

        match pid {
            -1 => Err(ExpectError::Spawn(SpawnError::Io(
                io::Error::last_os_error(),
            ))),
            0 => {
                // Child process
                // SAFETY: This runs in the forked child process only. We:
                // - Close the master fd (not needed in child)
                // - Create a new session with setsid()
                // - Set the slave as the controlling terminal via TIOCSCTTY
                // - Redirect stdin/stdout/stderr to the slave pty
                // - Close the original slave fd if it's not 0, 1, or 2
                // - Execute the target command (never returns on success)
                // - Exit with code 1 if exec fails
                // All file descriptors are valid and owned by this process.
                unsafe {
                    libc::close(master_fd);
                    libc::setsid();
                    libc::ioctl(slave_fd, libc::TIOCSCTTY, 0);

                    libc::dup2(slave_fd, 0);
                    libc::dup2(slave_fd, 1);
                    libc::dup2(slave_fd, 2);

                    if slave_fd > 2 {
                        libc::close(slave_fd);
                    }

                    // Use pre-validated CStrings (validated before fork)
                    let argv_ptrs: Vec<*const libc::c_char> = argv_cstrings
                        .iter()
                        .map(|s| s.as_ptr())
                        .chain(std::iter::once(std::ptr::null()))
                        .collect();

                    libc::execvp(cmd_cstring.as_ptr(), argv_ptrs.as_ptr());
                    libc::_exit(1);
                }
            }
            child_pid => {
                // Parent process
                // SAFETY: slave_fd is a valid file descriptor obtained from openpty().
                // The parent doesn't need the slave end; only the child uses it.
                unsafe {
                    libc::close(slave_fd);
                }

                // Set non-blocking
                // SAFETY: master_fd is a valid file descriptor from openpty().
                // F_GETFL and F_SETFL with O_NONBLOCK are standard operations
                // that don't violate any safety invariants.
                unsafe {
                    let flags = libc::fcntl(master_fd, libc::F_GETFL);
                    libc::fcntl(master_fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
                }

                Ok(PtyHandle {
                    master_fd,
                    pid: child_pid as u32,
                    dimensions: self.config.dimensions,
                })
            }
        }
    }

    /// Spawn a command (Windows placeholder).
    #[cfg(windows)]
    pub async fn spawn(&self, _command: &str, _args: &[String]) -> Result<PtyHandle> {
        Err(ExpectError::Spawn(SpawnError::PtyAllocation {
            reason: "PTY not yet implemented for Windows".to_string(),
        }))
    }
}

impl Default for PtySpawner {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle to a spawned PTY process.
#[derive(Debug)]
pub struct PtyHandle {
    /// Master PTY file descriptor.
    #[cfg(unix)]
    master_fd: i32,
    /// Process ID.
    pid: u32,
    /// Terminal dimensions (cols, rows).
    dimensions: (u16, u16),
}

impl PtyHandle {
    /// Get the process ID.
    #[must_use]
    pub const fn pid(&self) -> u32 {
        self.pid
    }

    /// Get the terminal dimensions.
    #[must_use]
    pub const fn dimensions(&self) -> (u16, u16) {
        self.dimensions
    }

    /// Resize the terminal.
    #[cfg(unix)]
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        let winsize = libc::winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        // SAFETY: master_fd is a valid PTY file descriptor stored in self.
        // TIOCSWINSZ is a valid ioctl command for PTYs that sets the window size.
        // winsize is a valid pointer to a properly initialized struct on the stack.
        let result = unsafe { libc::ioctl(self.master_fd, libc::TIOCSWINSZ, &winsize) };

        if result != 0 {
            Err(ExpectError::Io(io::Error::last_os_error()))
        } else {
            self.dimensions = (cols, rows);
            Ok(())
        }
    }

    /// Resize the terminal (Windows placeholder).
    #[cfg(windows)]
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        self.dimensions = (cols, rows);
        Ok(())
    }

    /// Wait for the process to exit.
    #[cfg(unix)]
    pub fn wait(&self) -> Result<i32> {
        let mut status: libc::c_int = 0;
        // SAFETY: self.pid is a valid process ID from fork().
        // status is a valid pointer to a stack-allocated integer.
        // The options argument (0) means blocking wait, which is valid.
        let result = unsafe { libc::waitpid(self.pid as i32, &mut status, 0) };

        if result == -1 {
            Err(ExpectError::Io(io::Error::last_os_error()))
        } else if libc::WIFEXITED(status) {
            Ok(libc::WEXITSTATUS(status))
        } else if libc::WIFSIGNALED(status) {
            Ok(128 + libc::WTERMSIG(status))
        } else {
            Ok(-1)
        }
    }

    /// Wait for the process to exit (Windows placeholder).
    #[cfg(windows)]
    pub fn wait(&self) -> Result<i32> {
        Ok(0)
    }

    /// Send a signal to the process.
    #[cfg(unix)]
    pub fn signal(&self, signal: i32) -> Result<()> {
        // SAFETY: self.pid is a valid process ID from fork().
        // The signal is passed from the caller and must be a valid signal number.
        // kill() is safe to call with any PID; it returns an error for invalid PIDs.
        let result = unsafe { libc::kill(self.pid as i32, signal) };
        if result != 0 {
            Err(ExpectError::Io(io::Error::last_os_error()))
        } else {
            Ok(())
        }
    }

    /// Send a signal to the process (Windows placeholder).
    #[cfg(windows)]
    pub fn signal(&self, _signal: i32) -> Result<()> {
        Ok(())
    }

    /// Kill the process.
    #[cfg(unix)]
    pub fn kill(&self) -> Result<()> {
        self.signal(libc::SIGKILL)
    }

    /// Kill the process (Windows placeholder).
    #[cfg(windows)]
    pub fn kill(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(unix)]
impl Drop for PtyHandle {
    fn drop(&mut self) {
        // Close the master fd
        // SAFETY: master_fd is a valid file descriptor obtained from openpty()
        // and stored in this struct. It has not been closed elsewhere as we own it.
        // Closing in Drop ensures the fd is released when the handle is dropped.
        unsafe {
            libc::close(self.master_fd);
        }
    }
}

/// Async wrapper around a PTY file descriptor for use with Tokio.
///
/// This provides `AsyncRead` and `AsyncWrite` implementations that
/// integrate with the Tokio runtime.
#[cfg(unix)]
pub struct AsyncPty {
    /// The async file descriptor wrapper.
    inner: tokio::io::unix::AsyncFd<std::os::unix::io::RawFd>,
    /// Process ID.
    pid: u32,
    /// Terminal dimensions.
    dimensions: (u16, u16),
}

#[cfg(unix)]
impl AsyncPty {
    /// Create a new async PTY wrapper from a PtyHandle.
    ///
    /// Takes ownership of the PtyHandle's file descriptor.
    ///
    /// # Errors
    ///
    /// Returns an error if the AsyncFd cannot be created.
    pub fn from_handle(handle: PtyHandle) -> io::Result<Self> {
        let fd = handle.master_fd;
        let pid = handle.pid;
        let dimensions = handle.dimensions;

        // Prevent the original handle from closing the fd
        std::mem::forget(handle);

        let inner = tokio::io::unix::AsyncFd::new(fd)?;
        Ok(Self {
            inner,
            pid,
            dimensions,
        })
    }

    /// Get the process ID.
    #[must_use]
    pub const fn pid(&self) -> u32 {
        self.pid
    }

    /// Get the terminal dimensions.
    #[must_use]
    pub const fn dimensions(&self) -> (u16, u16) {
        self.dimensions
    }

    /// Resize the terminal.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        let winsize = libc::winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        // SAFETY: The fd is valid and TIOCSWINSZ is a valid ioctl for PTYs.
        let result = unsafe { libc::ioctl(*self.inner.get_ref(), libc::TIOCSWINSZ, &winsize) };

        if result != 0 {
            Err(ExpectError::Io(io::Error::last_os_error()))
        } else {
            self.dimensions = (cols, rows);
            Ok(())
        }
    }

    /// Send a signal to the child process.
    pub fn signal(&self, signal: i32) -> Result<()> {
        // SAFETY: pid is a valid process ID from fork().
        let result = unsafe { libc::kill(self.pid as i32, signal) };
        if result != 0 {
            Err(ExpectError::Io(io::Error::last_os_error()))
        } else {
            Ok(())
        }
    }

    /// Kill the child process.
    pub fn kill(&self) -> Result<()> {
        self.signal(libc::SIGKILL)
    }
}

#[cfg(unix)]
impl AsyncRead for AsyncPty {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        loop {
            let mut guard = match self.inner.poll_read_ready(cx) {
                Poll::Ready(Ok(guard)) => guard,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            };

            let fd = *self.inner.get_ref();
            let unfilled = buf.initialize_unfilled();

            // SAFETY: fd is a valid file descriptor, unfilled is a valid buffer.
            let result = unsafe {
                libc::read(fd, unfilled.as_mut_ptr() as *mut libc::c_void, unfilled.len())
            };

            if result >= 0 {
                buf.advance(result as usize);
                return Poll::Ready(Ok(()));
            }

            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::WouldBlock {
                guard.clear_ready();
                continue;
            }
            return Poll::Ready(Err(err));
        }
    }
}

#[cfg(unix)]
impl AsyncWrite for AsyncPty {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        loop {
            let mut guard = match self.inner.poll_write_ready(cx) {
                Poll::Ready(Ok(guard)) => guard,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            };

            let fd = *self.inner.get_ref();

            // SAFETY: fd is a valid file descriptor, buf is a valid buffer.
            let result =
                unsafe { libc::write(fd, buf.as_ptr() as *const libc::c_void, buf.len()) };

            if result >= 0 {
                return Poll::Ready(Ok(result as usize));
            }

            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::WouldBlock {
                guard.clear_ready();
                continue;
            }
            return Poll::Ready(Err(err));
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // PTY doesn't need explicit flushing
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // Shutdown is handled by Drop
        Poll::Ready(Ok(()))
    }
}

#[cfg(unix)]
impl Drop for AsyncPty {
    fn drop(&mut self) {
        // SAFETY: The fd is valid and owned by us.
        unsafe {
            libc::close(*self.inner.get_ref());
        }
    }
}

#[cfg(unix)]
impl std::fmt::Debug for AsyncPty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncPty")
            .field("fd", self.inner.get_ref())
            .field("pid", &self.pid)
            .field("dimensions", &self.dimensions)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pty_config_default() {
        let config = PtyConfig::default();
        assert_eq!(config.dimensions.0, 80);
        assert_eq!(config.dimensions.1, 24);
        assert_eq!(config.env_mode, EnvMode::Inherit);
    }

    #[test]
    fn pty_config_from_session() {
        let mut session_config = SessionConfig::default();
        session_config.dimensions = (120, 40);

        let pty_config = PtyConfig::from(&session_config);
        assert_eq!(pty_config.dimensions.0, 120);
        assert_eq!(pty_config.dimensions.1, 40);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn spawn_rejects_null_byte_in_command() {
        let spawner = PtySpawner::new();
        let result = spawner.spawn("test\0command", &[]).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(
            err_str.contains("null byte"),
            "Expected error about null byte, got: {err_str}"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn spawn_rejects_null_byte_in_args() {
        let spawner = PtySpawner::new();
        let result = spawner
            .spawn("/bin/echo", &["hello\0world".to_string()])
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(
            err_str.contains("null byte"),
            "Expected error about null byte, got: {err_str}"
        );
    }
}
