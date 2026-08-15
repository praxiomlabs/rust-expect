//! PTY backend for local process spawning.
//!
//! This module provides the PTY backend that uses the rust-pty crate
//! to spawn local processes with pseudo-terminal support.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::backend::{ChildExit, ProcessControl, ProcessHandle, Resizable};
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

    /// Spawn a command on Windows using `ConPTY`.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `ConPTY` is not available (Windows version too old)
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
        let (master, child) = WindowsPtySystem::spawn(
            command,
            args.iter().map(std::string::String::as_str),
            &pty_config,
        )
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
    /// Control over the child, shared with the owning session.
    ///
    /// The transport keeps a handle rather than the child itself so that
    /// killing or signalling never contends with a parked read. It still needs
    /// the handle because `poll_write` must turn a write to an exited child
    /// into `BrokenPipe`.
    process: ProcessHandle,
    /// Process ID.
    pid: u32,
    /// Terminal dimensions.
    dimensions: (u16, u16),
}

/// Control over a Unix child process, separated from the PTY master that
/// carries its I/O.
#[cfg(unix)]
pub struct PtyProcess {
    /// The child process handle from rust-pty.
    child: rust_pty::UnixPtyChild,
    /// Process ID.
    pid: u32,
    /// Whether the caller has given up ownership, so dropping this handle
    /// leaves the child running. See [`ProcessControl::detach`].
    detached: bool,
}

#[cfg(unix)]
impl PtyProcess {
    /// Non-blocking reap.
    ///
    /// Returns `Some(status)` once the child has exited (rust-pty caches the
    /// status), or `None` while it is still running or its status cannot be
    /// determined.
    fn try_wait(&mut self) -> Option<ProcessExitStatus> {
        match self.child.try_wait() {
            Ok(Some(rust_pty::ExitStatus::Exited(code))) => Some(ProcessExitStatus::Exited(code)),
            Ok(Some(rust_pty::ExitStatus::Signaled(sig))) => Some(ProcessExitStatus::Signaled(sig)),
            Ok(None) | Err(_) => None,
        }
    }

    /// Whether the child leads its own process group, and can therefore be
    /// signalled as one.
    ///
    /// Checked rather than assumed. A child spawned with neither `new_session`
    /// nor `controlling_terminal` inherits *our* process group, and `killpg` on
    /// its pid would then signal this process and everything beside it. Both
    /// options default to true, so in practice the child is its own leader —
    /// but "in practice" is not a guard.
    ///
    /// Queried per call rather than cached at construction: `setsid` runs in the
    /// child after fork, so there is no moment at spawn when the answer is
    /// reliably known, and a child is free to move itself later.
    #[allow(unsafe_code)]
    fn leads_its_group(&self) -> bool {
        // SAFETY: a read-only query; `getpgid` returns -1 for an unknown pid,
        // which simply reads as "not a leader".
        let pgid = unsafe { libc::getpgid(self.pid as i32) };
        pgid == self.pid as i32
    }

    /// Deliver `signal` to the child's process group if it leads one, and to
    /// the child alone otherwise. Returns the raw syscall result.
    #[allow(unsafe_code)]
    fn deliver(&self, signal: i32) -> i32 {
        if self.leads_its_group() {
            // SAFETY: pid is a live process that leads its own group, checked
            // immediately above.
            unsafe { libc::killpg(self.pid as i32, signal) }
        } else {
            // SAFETY: pid is a valid process ID from the spawned child.
            unsafe { libc::kill(self.pid as i32, signal) }
        }
    }

    /// Whether the child has been observed to exit, by any exit status
    /// rust-pty can report.
    ///
    /// Deliberately broader than `try_wait().is_some()`: this is the guard
    /// `poll_write` uses, and a write to a child that exited in a form
    /// [`Self::try_wait`] declines to map must still fail rather than
    /// disappear into a dead PTY.
    fn has_exited(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(Some(_)))
    }
}

#[cfg(unix)]
impl ProcessControl for PtyProcess {
    fn pid(&self) -> Option<u32> {
        Some(self.pid)
    }

    /// Send a signal to the child process.
    ///
    /// Delivered to the child's **process group**, which is the behaviour of the
    /// terminal this session is: pressing Ctrl-C at a terminal signals the
    /// foreground process group, not one process. Without it a shell's
    /// background jobs survive `kill()` — measured, in
    /// `tests/unix_process_ownership.rs`. Falls back to signalling the child
    /// alone when it does not lead a group of its own — a child spawned with
    /// neither `new_session` nor `controlling_terminal` inherits this process's
    /// group, and signalling that group would hit us.
    ///
    /// Guards against PID reuse (S1): if the child has already exited (and
    /// possibly been reaped, freeing its PID for the OS to recycle), this
    /// returns [`ExpectError::SessionClosed`] rather than risk the signal
    /// landing on an unrelated process. A raw `ESRCH` maps to the same. Other
    /// delivery failures (e.g. `EPERM`) surface unchanged as
    /// [`ExpectError::Io`]. Raw syscalls are used (rather than
    /// `rust_pty::PtyChild::signal`) to preserve arbitrary-signal support and
    /// keep the authoritative guard at this layer.
    fn signal(&mut self, signal: i32) -> Result<()> {
        // Authoritative pre-kill reap check via tokio's child handle.
        if self.try_wait().is_some() {
            return Err(ExpectError::SessionClosed);
        }
        let result = self.deliver(signal);
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

    fn kill(&mut self) -> Result<()> {
        self.signal(libc::SIGKILL)
    }

    fn is_running(&mut self) -> bool {
        self.try_wait().is_none()
    }

    fn try_exit_status(&mut self) -> Option<ProcessExitStatus> {
        self.try_wait()
    }

    fn has_exited(&mut self) -> bool {
        Self::has_exited(self)
    }

    fn detach(&mut self) {
        self.detached = true;
    }
}

/// The child dies with the last handle to it.
///
/// Closing the PTY master already hangs up the child's controlling terminal,
/// and that `SIGHUP` cleans up an ordinary child on its own — but only an
/// ordinary one. A child that ignores `SIGHUP` outlived its session entirely,
/// with nothing left holding it and nothing that would ever reap it (measured
/// in `tests/unix_process_ownership.rs`). Relying on the hangup also meant the
/// only cleanup in the crate was a side effect of field drop order that nothing
/// stated or tested, and that any handle outliving the session would remove.
///
/// This runs when the last `ProcessHandle` goes, so a session and a transport
/// that share one keep the child alive until both are gone.
///
/// `SIGKILL`, not a gentler signal followed by a wait: `Drop` cannot await, and
/// a blocking grace period inside it would stall whatever thread is dropping
/// the session. Callers who want the child asked nicely first have
/// `Session::shutdown`, which can wait. Callers who want the child to outlive
/// the session have `Session::detach`.
#[cfg(unix)]
impl Drop for PtyProcess {
    fn drop(&mut self) {
        if self.detached || self.try_wait().is_some() {
            return;
        }
        // Best effort by construction: nothing here can report a failure to
        // anyone, and a child that is already gone is the common case.
        let _ = self.deliver(libc::SIGKILL);
    }
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
            process: ProcessHandle::new(PtyProcess {
                child: handle.child,
                pid,
                detached: false,
            }),
            pid,
            dimensions,
        })
    }

    /// A cloneable handle to this PTY's child process.
    ///
    /// The session takes one of these at spawn time so that process control
    /// does not have to go through the transport lock.
    #[must_use]
    pub fn process_handle(&self) -> ProcessHandle {
        self.process.clone()
    }

    /// Non-blocking reap of the child process.
    ///
    /// Returns `Some(status)` once the child has exited (rust-pty caches the
    /// status), or `None` while it is still running or its status cannot be
    /// determined.
    pub fn try_wait(&mut self) -> Option<ProcessExitStatus> {
        self.process.with(|c| c.try_exit_status())
    }

    /// Check whether the child process is still running.
    ///
    /// Non-blocking: reaps through tokio's child handle, so it reports the truth
    /// immediately after the child exits. Mirrors `WindowsAsyncPty::is_running`.
    pub fn is_running(&mut self) -> bool {
        self.process.with(|c| c.is_running())
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
    /// Delegates to [`PtyProcess`], which owns the child and the PID-reuse
    /// guard. Prefer `Session::signal`, which reaches the same handle without
    /// touching the transport at all.
    ///
    /// # Errors
    ///
    /// Returns [`ExpectError::SessionClosed`] if the child has already exited,
    /// or an I/O error if delivery fails.
    pub fn signal(&mut self, signal: i32) -> Result<()> {
        self.process.with(|c| c.signal(signal))
    }

    /// Kill the child process.
    ///
    /// # Errors
    ///
    /// Returns an error if the child cannot be killed.
    pub fn kill(&mut self) -> Result<()> {
        self.process.with(|c| c.kill())
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
        if self.process.with(|c| c.has_exited()) {
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

#[cfg(unix)]
impl Resizable for AsyncPty {
    fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        Self::resize(self, cols, rows)
    }

    fn dimensions(&self) -> (u16, u16) {
        self.dimensions
    }
}

/// Async wrapper around Windows `ConPTY` for use with Tokio.
///
/// This wraps the rust-pty `WindowsPtyMaster` and provides the same interface
/// as the Unix `AsyncPty` for consistent cross-platform Session usage.
#[cfg(windows)]
pub struct WindowsAsyncPty {
    /// The underlying Windows PTY master.
    master: rust_pty::WindowsPtyMaster,
    /// Control over the child, shared with the owning session.
    ///
    /// As on Unix, the transport keeps a handle rather than the child so that
    /// killing never contends with a parked read, while `poll_write` can still
    /// turn a write to an exited child into `BrokenPipe`.
    process: ProcessHandle,
    /// Process ID.
    pid: u32,
    /// Terminal dimensions.
    dimensions: (u16, u16),
}

/// Control over a Windows `ConPTY` child process, separated from the master
/// that carries its I/O.
#[cfg(windows)]
pub struct WindowsPtyProcess {
    /// The child process handle from rust-pty.
    child: rust_pty::WindowsPtyChild,
    /// Process ID.
    pid: u32,
}

#[cfg(windows)]
impl ProcessControl for WindowsPtyProcess {
    fn pid(&self) -> Option<u32> {
        Some(self.pid)
    }

    // `signal` is left at the trait default: Windows has no signals to send,
    // so it reports ExpectError::Unsupported rather than silently doing
    // nothing. This matches the pre-split API, where `Session<WindowsAsyncPty>`
    // had no `signal` method at all.

    fn kill(&mut self) -> Result<()> {
        self.child
            .kill()
            .map_err(|e| ExpectError::Io(io::Error::other(format!("kill failed: {e}"))))
    }

    fn is_running(&mut self) -> bool {
        self.child.is_running()
    }

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

    fn has_exited(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(Some(_)))
    }
}

#[cfg(windows)]
impl WindowsAsyncPty {
    /// Create a new Windows async PTY wrapper from a `WindowsPtyHandle`.
    ///
    /// Takes ownership of the handle.
    #[must_use]
    pub fn from_handle(handle: WindowsPtyHandle) -> Self {
        let pid = handle.child.pid();
        let dimensions = handle.dimensions;
        Self {
            master: handle.master,
            process: ProcessHandle::new(WindowsPtyProcess {
                child: handle.child,
                pid,
            }),
            pid,
            dimensions,
        }
    }

    /// A cloneable handle to this PTY's child process.
    ///
    /// The session takes one of these at spawn time so that process control
    /// does not have to go through the transport lock.
    #[must_use]
    pub fn process_handle(&self) -> ProcessHandle {
        self.process.clone()
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
        self.process.with(|c| c.is_running())
    }

    /// Kill the child process.
    ///
    /// # Errors
    ///
    /// Returns an error if the child cannot be killed.
    pub fn kill(&mut self) -> Result<()> {
        self.process.with(|c| c.kill())
    }
}

#[cfg(windows)]
impl ChildExit for WindowsAsyncPty {
    fn try_exit_status(&mut self) -> Option<ProcessExitStatus> {
        self.process.with(|c| c.try_exit_status())
    }
}

#[cfg(windows)]
impl Resizable for WindowsAsyncPty {
    fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        Self::resize(self, cols, rows)
    }

    fn dimensions(&self) -> (u16, u16) {
        self.dimensions
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
        if self.process.with(|c| c.has_exited()) {
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
