//! PTY backend for local process spawning.
//!
//! This module provides the PTY backend that uses the rust-pty crate
//! to spawn local processes with pseudo-terminal support.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::backend::ChildExit;
use crate::config::SessionConfig;
use crate::error::{ExpectError, Result, SpawnError};
use crate::types::ProcessExitStatus;

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
    pub const fn set_pid(&mut self, pid: u32) {
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
#[non_exhaustive]
pub struct PtyConfig {
    /// Terminal dimensions (cols, rows).
    pub dimensions: (u16, u16),
    /// Whether to use a login shell.
    pub login_shell: bool,
    /// Environment variable handling.
    pub env_mode: EnvMode,
    /// Environment variables to apply per `env_mode` (overlay for `Extend`,
    /// the full set for `Clear`, ignored for `Inherit`).
    pub env: std::collections::HashMap<String, String>,
    /// Working directory for the spawned child. `None` inherits the parent's
    /// current directory.
    pub working_directory: Option<std::path::PathBuf>,
}

impl Default for PtyConfig {
    fn default() -> Self {
        Self {
            dimensions: (80, 24),
            login_shell: false,
            env_mode: EnvMode::Inherit,
            env: std::collections::HashMap::new(),
            working_directory: None,
        }
    }
}

impl From<&SessionConfig> for PtyConfig {
    fn from(config: &SessionConfig) -> Self {
        Self {
            dimensions: config.dimensions,
            login_shell: false,
            env_mode: match (config.inherit_env, config.env.is_empty()) {
                (false, _) => EnvMode::Clear,
                (true, true) => EnvMode::Inherit,
                (true, false) => EnvMode::Extend,
            },
            env: config.env.clone(),
            working_directory: config.working_dir.clone(),
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
    pub const fn set_dimensions(&mut self, cols: u16, rows: u16) {
        self.config.dimensions = (cols, rows);
    }

    /// Spawn a command.
    ///
    /// The Unix implementation spawns via `tokio::process::Command` (through
    /// rust-pty's `UnixPtySystem`); the only work between fork and exec is the
    /// async-signal-safe `setsid` + `TIOCSCTTY` in rust-pty's `pre_exec` hook,
    /// so it is safe under a multi-threaded Tokio runtime (the default
    /// `#[tokio::main]`). Environment and working-directory setup happen in the
    /// parent before spawning.
    ///
    /// # Errors
    ///
    /// Returns an error if PTY allocation or process spawning fails.
    #[cfg(unix)]
    pub async fn spawn(&self, command: &str, args: &[String]) -> Result<PtyHandle> {
        use rust_pty::{PtySystem, UnixPtySystem};

        // Preserve the `InvalidWorkingDir` contract: rust-pty surfaces a missing
        // working directory as a generic spawn failure, so validate it up front
        // for a clear, specific error.
        if let Some(dir) = &self.config.working_directory
            && !dir.is_dir()
        {
            return Err(ExpectError::Spawn(SpawnError::InvalidWorkingDir {
                path: dir.display().to_string(),
            }));
        }

        // Build env per env_mode (mirrors the Windows branch):
        // - Inherit (no overrides): env: None (rust-pty inherits the parent env).
        // - Inherit/Extend (with overrides): parent env + overrides (ours win).
        // - Clear: only our overrides (parent env discarded).
        let built_env: Option<std::collections::HashMap<std::ffi::OsString, std::ffi::OsString>> =
            match self.config.env_mode {
                EnvMode::Inherit if self.config.env.is_empty() => None,
                EnvMode::Inherit | EnvMode::Extend => {
                    let mut m: std::collections::HashMap<_, _> = std::env::vars_os().collect();
                    for (k, v) in &self.config.env {
                        m.insert(std::ffi::OsString::from(k), std::ffi::OsString::from(v));
                    }
                    Some(m)
                }
                EnvMode::Clear => Some(
                    self.config
                        .env
                        .iter()
                        .map(|(k, v)| (std::ffi::OsString::from(k), std::ffi::OsString::from(v)))
                        .collect(),
                ),
            };

        let pty_config = rust_pty::PtyConfig {
            window_size: self.config.dimensions,
            env: match self.config.env_mode {
                EnvMode::Clear if self.config.env.is_empty() => {
                    Some(std::collections::HashMap::new())
                }
                _ => built_env,
            },
            working_directory: self.config.working_directory.clone(),
            ..Default::default()
        };

        let (master, child) =
            UnixPtySystem::spawn(command, args.iter().map(String::as_str), &pty_config)
                .await
                .map_err(|e| {
                    ExpectError::Spawn(SpawnError::PtyAllocation {
                        reason: format!("Unix PTY spawn failed: {e}"),
                    })
                })?;

        Ok(PtyHandle {
            master,
            child,
            dimensions: self.config.dimensions,
        })
    }

    /// Spawn a command on Windows using ConPTY.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - ConPTY is not available (Windows version too old)
    /// - PTY allocation fails
    /// - Process spawning fails
    #[cfg(windows)]
    pub async fn spawn(&self, command: &str, args: &[String]) -> Result<WindowsPtyHandle> {
        use rust_pty::{PtySystem, WindowsPtySystem};

        // Build env per env_mode:
        // - Inherit: env: None (rust-pty inherits parent env), but if we also
        //   have overrides, we need to inherit + overlay → build a full map.
        // - Clear:   env: Some(our overrides) — parent env discarded.
        // - Extend:  env: Some(parent + our overrides), parent first so ours win.
        let built_env: Option<std::collections::HashMap<std::ffi::OsString, std::ffi::OsString>> =
            match self.config.env_mode {
                EnvMode::Inherit if self.config.env.is_empty() => None,
                EnvMode::Inherit | EnvMode::Extend => {
                    let mut m: std::collections::HashMap<_, _> = std::env::vars_os().collect();
                    for (k, v) in &self.config.env {
                        m.insert(std::ffi::OsString::from(k), std::ffi::OsString::from(v));
                    }
                    Some(m)
                }
                EnvMode::Clear => Some(
                    self.config
                        .env
                        .iter()
                        .map(|(k, v)| (std::ffi::OsString::from(k), std::ffi::OsString::from(v)))
                        .collect(),
                ),
            };

        // Create configuration for rust-pty
        let pty_config = rust_pty::PtyConfig {
            window_size: self.config.dimensions,
            env: match self.config.env_mode {
                EnvMode::Clear if self.config.env.is_empty() => {
                    Some(std::collections::HashMap::new())
                }
                _ => built_env,
            },
            working_directory: self.config.working_directory.clone(),
            ..Default::default()
        };

        // Spawn using rust-pty's Windows implementation
        let (master, child) =
            WindowsPtySystem::spawn(command, args.iter().map(|s| s.as_str()), &pty_config)
                .await
                .map_err(|e| {
                    ExpectError::Spawn(SpawnError::PtyAllocation {
                        reason: format!("Windows ConPTY spawn failed: {e}"),
                    })
                })?;

        Ok(WindowsPtyHandle {
            master,
            child,
            dimensions: self.config.dimensions,
        })
    }
}

impl Default for PtySpawner {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle to a spawned PTY process (Unix).
#[cfg(unix)]
#[derive(Debug)]
pub struct PtyHandle {
    /// The PTY master from rust-pty.
    pub(crate) master: rust_pty::UnixPtyMaster,
    /// The child process handle.
    pub(crate) child: rust_pty::UnixPtyChild,
    /// Terminal dimensions (cols, rows).
    dimensions: (u16, u16),
}

/// Handle to a spawned PTY process (Windows).
#[cfg(windows)]
pub struct WindowsPtyHandle {
    /// The PTY master from rust-pty.
    pub(crate) master: rust_pty::WindowsPtyMaster,
    /// The child process handle.
    pub(crate) child: rust_pty::WindowsPtyChild,
    /// Terminal dimensions (cols, rows).
    dimensions: (u16, u16),
}

#[cfg(windows)]
impl std::fmt::Debug for WindowsPtyHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowsPtyHandle")
            .field("dimensions", &self.dimensions)
            .finish_non_exhaustive()
    }
}

#[cfg(unix)]
impl PtyHandle {
    /// Get the process ID.
    #[must_use]
    pub const fn pid(&self) -> u32 {
        self.child.pid()
    }

    /// Get the terminal dimensions.
    #[must_use]
    pub const fn dimensions(&self) -> (u16, u16) {
        self.dimensions
    }

    /// Resize the terminal.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        use rust_pty::{PtyMaster, WindowSize};
        self.master
            .resize(WindowSize::new(cols, rows))
            .map_err(|e| ExpectError::Io(io::Error::other(format!("resize failed: {e}"))))?;
        self.dimensions = (cols, rows);
        Ok(())
    }

    // NB: no `signal`/`kill` here. The unguarded low-level signal path was
    // removed for the PID-reuse guard (S1); signal a child through
    // `Session`/`SyncSession`, whose `AsyncPty::signal` performs the
    // authoritative reap-before-kill check.
}

#[cfg(windows)]
impl WindowsPtyHandle {
    /// Get the process ID.
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.child.pid()
    }

    /// Get the terminal dimensions.
    #[must_use]
    pub const fn dimensions(&self) -> (u16, u16) {
        self.dimensions
    }

    /// Resize the terminal.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        use rust_pty::{PtyMaster, WindowSize};
        let size = WindowSize::new(cols, rows);
        self.master
            .resize(size)
            .map_err(|e| ExpectError::Io(io::Error::other(format!("resize failed: {e}"))))?;
        self.dimensions = (cols, rows);
        Ok(())
    }

    /// Check if the child process is still running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.child.is_running()
    }

    /// Kill the process.
    pub fn kill(&mut self) -> Result<()> {
        self.child
            .kill()
            .map_err(|e| ExpectError::Io(io::Error::other(format!("kill failed: {e}"))))
    }
}

/// Async wrapper around a PTY file descriptor for use with Tokio.
///
/// This provides `AsyncRead` and `AsyncWrite` implementations that
/// integrate with the Tokio runtime.
#[cfg(unix)]
pub struct AsyncPty {
    /// The underlying Unix PTY master from rust-pty.
    master: rust_pty::UnixPtyMaster,
    /// The child process handle.
    child: rust_pty::UnixPtyChild,
    /// Process ID.
    pid: u32,
    /// Terminal dimensions.
    dimensions: (u16, u16),
}

#[cfg(unix)]
impl AsyncPty {
    /// Create a new async PTY wrapper from a `PtyHandle`.
    ///
    /// Takes ownership of the `PtyHandle`'s file descriptor.
    ///
    /// # Errors
    ///
    /// Returns an error if the `AsyncFd` cannot be created.
    pub fn from_handle(handle: PtyHandle) -> io::Result<Self> {
        let pid = handle.child.pid();
        let dimensions = handle.dimensions;
        Ok(Self {
            master: handle.master,
            child: handle.child,
            pid,
            dimensions,
        })
    }

    /// Non-blocking reap of the child process.
    ///
    /// Returns `Some(status)` once the child has exited (rust-pty caches the
    /// status), or `None` while it is still running or its status cannot be
    /// determined.
    pub fn try_wait(&mut self) -> Option<ProcessExitStatus> {
        match self.child.try_wait() {
            Ok(Some(rust_pty::ExitStatus::Exited(code))) => Some(ProcessExitStatus::Exited(code)),
            Ok(Some(rust_pty::ExitStatus::Signaled(sig))) => Some(ProcessExitStatus::Signaled(sig)),
            Ok(None) | Err(_) => None,
        }
    }

    /// Check whether the child process is still running.
    ///
    /// Non-blocking: reaps through tokio's child handle, so it reports the truth
    /// immediately after the child exits. Mirrors `WindowsAsyncPty::is_running`.
    pub fn is_running(&mut self) -> bool {
        self.try_wait().is_none()
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
        use rust_pty::{PtyMaster, WindowSize};
        self.master
            .resize(WindowSize::new(cols, rows))
            .map_err(|e| ExpectError::Io(io::Error::other(format!("resize failed: {e}"))))?;
        self.dimensions = (cols, rows);
        Ok(())
    }

    /// Send a signal to the child process.
    ///
    /// Guards against PID reuse (S1): if the child has already exited (and
    /// possibly been reaped, freeing its PID for the OS to recycle), this
    /// returns [`ExpectError::SessionClosed`] rather than risk `libc::kill`
    /// landing on an unrelated process. A raw `ESRCH` from `kill` maps to the
    /// same. Other delivery failures (e.g. `EPERM`) surface unchanged as
    /// [`ExpectError::Io`]. A raw `libc::kill` is used (rather than
    /// `rust_pty::PtyChild::signal`) to preserve arbitrary-signal support and
    /// keep the authoritative guard at this layer.
    #[allow(unsafe_code)]
    pub fn signal(&mut self, signal: i32) -> Result<()> {
        // Authoritative pre-kill reap check via tokio's child handle.
        if self.try_wait().is_some() {
            return Err(ExpectError::SessionClosed);
        }
        // SAFETY: pid is a valid process ID from the spawned child.
        let result = unsafe { libc::kill(self.pid as i32, signal) };
        if result == 0 {
            Ok(())
        } else {
            let err = io::Error::last_os_error();
            // Child exited between the guard and the kill: treat as already
            // closed rather than a raw error.
            if err.raw_os_error() == Some(libc::ESRCH) {
                Err(ExpectError::SessionClosed)
            } else {
                Err(ExpectError::Io(err))
            }
        }
    }

    /// Kill the child process.
    pub fn kill(&mut self) -> Result<()> {
        self.signal(libc::SIGKILL)
    }
}

#[cfg(unix)]
impl AsyncRead for AsyncPty {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.master).poll_read(cx, buf)
    }
}

#[cfg(unix)]
impl AsyncWrite for AsyncPty {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        // A dead child's PTY master buffers writes; surface exit as BrokenPipe.
        if matches!(self.child.try_wait(), Ok(Some(_))) {
            return Poll::Ready(Err(io::ErrorKind::BrokenPipe.into()));
        }
        Pin::new(&mut self.master).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.master).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.master).poll_shutdown(cx)
    }
}

#[cfg(unix)]
impl std::fmt::Debug for AsyncPty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncPty")
            .field("pid", &self.pid)
            .field("dimensions", &self.dimensions)
            .finish_non_exhaustive()
    }
}

#[cfg(unix)]
impl ChildExit for AsyncPty {
    fn try_exit_status(&mut self) -> Option<ProcessExitStatus> {
        self.try_wait()
    }
}

/// Async wrapper around Windows ConPTY for use with Tokio.
///
/// This wraps the rust-pty WindowsPtyMaster and provides the same interface
/// as the Unix AsyncPty for consistent cross-platform Session usage.
#[cfg(windows)]
pub struct WindowsAsyncPty {
    /// The underlying Windows PTY master.
    master: rust_pty::WindowsPtyMaster,
    /// The child process handle.
    child: rust_pty::WindowsPtyChild,
    /// Process ID.
    pid: u32,
    /// Terminal dimensions.
    dimensions: (u16, u16),
}

#[cfg(windows)]
impl WindowsAsyncPty {
    /// Create a new Windows async PTY wrapper from a WindowsPtyHandle.
    ///
    /// Takes ownership of the handle.
    pub fn from_handle(handle: WindowsPtyHandle) -> Self {
        let pid = handle.child.pid();
        let dimensions = handle.dimensions;
        Self {
            master: handle.master,
            child: handle.child,
            pid,
            dimensions,
        }
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
        use rust_pty::{PtyMaster, WindowSize};
        let size = WindowSize::new(cols, rows);
        self.master
            .resize(size)
            .map_err(|e| ExpectError::Io(io::Error::other(format!("resize failed: {e}"))))?;
        self.dimensions = (cols, rows);
        Ok(())
    }

    /// Check if the child process is still running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.child.is_running()
    }

    /// Kill the child process.
    pub fn kill(&mut self) -> Result<()> {
        self.child
            .kill()
            .map_err(|e| ExpectError::Io(io::Error::other(format!("kill failed: {e}"))))
    }
}

#[cfg(windows)]
impl ChildExit for WindowsAsyncPty {
    fn try_exit_status(&mut self) -> Option<ProcessExitStatus> {
        // WindowsPtyChild::try_wait peeks GetExitCodeProcess without blocking and
        // returns the real status once the child has exited. The exit watcher
        // installed by rust-pty guarantees EOF is delivered to readers, so by the
        // time Session::wait reaches here the child has typically already exited.
        match self.child.try_wait() {
            Ok(Some(rust_pty::ExitStatus::Exited(code))) => Some(ProcessExitStatus::Exited(code)),
            // Windows reports every exit as `Terminated(exit_code)`; the code is the real exit code.
            Ok(Some(rust_pty::ExitStatus::Terminated(code))) => {
                Some(ProcessExitStatus::Exited(code as i32))
            }
            // Still running, or status unrecoverable.
            Ok(None) | Err(_) => None,
        }
    }
}

#[cfg(windows)]
impl AsyncRead for WindowsAsyncPty {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // Delegate to the underlying WindowsPtyMaster which implements AsyncRead
        Pin::new(&mut self.master).poll_read(cx, buf)
    }
}

#[cfg(windows)]
impl AsyncWrite for WindowsAsyncPty {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        // Mirror the Unix guard: a write after the ConPTY child exits must surface closure.
        if matches!(self.child.try_wait(), Ok(Some(_))) {
            return Poll::Ready(Err(io::ErrorKind::BrokenPipe.into()));
        }
        Pin::new(&mut self.master).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.master).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.master).poll_shutdown(cx)
    }
}

#[cfg(windows)]
impl std::fmt::Debug for WindowsAsyncPty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowsAsyncPty")
            .field("pid", &self.pid)
            .field("dimensions", &self.dimensions)
            .finish_non_exhaustive()
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
        let session_config = SessionConfig {
            dimensions: (120, 40),
            ..Default::default()
        };

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
            err_str.contains("nul byte"),
            "Expected error about a nul byte, got: {err_str}"
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
            err_str.contains("nul byte"),
            "Expected error about a nul byte, got: {err_str}"
        );
    }
}
