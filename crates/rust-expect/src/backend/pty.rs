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

/// Apply `env_mode` plus the user-supplied overrides to the calling
/// process's environment.
///
/// **Must only be called in a child process after `fork`** — it mutates
/// global `environ` state via `setenv`/`clearenv`/`unsetenv`, which is
/// safe only because the child is single-threaded at this point (between
/// fork and exec).
///
/// - `Inherit`: leave the inherited parent env in place; just apply overrides.
/// - `Clear`:   wipe environ (Linux: `clearenv`; elsewhere: walk + `unsetenv`)
///   then apply overrides.
/// - `Extend`:  same as Inherit semantically; overrides overwrite existing.
#[cfg(unix)]
#[allow(unsafe_code)]
unsafe fn apply_env_in_child(
    env_mode: EnvMode,
    env_pairs: &[(std::ffi::CString, std::ffi::CString)],
) {
    // SAFETY: caller (this function's doc-comment contract) guarantees we are
    // executing post-fork, pre-exec in a child process, which is single-threaded.
    // Mutating `environ` via clearenv/setenv/unsetenv is therefore race-free.
    unsafe {
        match env_mode {
            EnvMode::Inherit | EnvMode::Extend => {}
            EnvMode::Clear => {
                #[cfg(target_os = "linux")]
                {
                    libc::clearenv();
                }
                #[cfg(not(target_os = "linux"))]
                {
                    // Collect every existing key into owned CStrings BEFORE we
                    // start calling unsetenv. unsetenv mutates the global
                    // `environ` array — entries shift, the array can be
                    // reallocated — so iterating it concurrently with
                    // mutation is fragile and libc-dependent. Snapshotting
                    // first sidesteps the issue entirely, and the keys can
                    // be of arbitrary length without truncation.
                    // Edition 2024 requires extern blocks declaring foreign
                    // statics to be wrapped in `unsafe extern`.
                    unsafe extern "C" {
                        static mut environ: *mut *mut libc::c_char;
                    }
                    let mut names: Vec<std::ffi::CString> = Vec::new();
                    if !environ.is_null() {
                        let mut p = environ;
                        while !(*p).is_null() {
                            let entry = *p;
                            // Find the '=' separator (or NUL if malformed).
                            let mut len = 0usize;
                            while *entry.add(len) != 0 && *entry.add(len) != b'=' as libc::c_char {
                                len += 1;
                            }
                            if len > 0 {
                                let bytes = std::slice::from_raw_parts(entry.cast::<u8>(), len);
                                if let Ok(c) = std::ffi::CString::new(bytes) {
                                    names.push(c);
                                }
                            }
                            p = p.add(1);
                        }
                    }
                    for name in &names {
                        libc::unsetenv(name.as_ptr());
                    }
                }
            }
        }
        for (k, v) in env_pairs {
            libc::setenv(k.as_ptr(), v.as_ptr(), 1);
        }
    }
}

/// Validate environment-variable overrides and convert them to pairs of
/// `CString` that can be safely applied between fork and exec on Unix.
///
/// `setenv` allocates, so the canonical safety model after `fork` is to
/// only use async-signal-safe functions. We do still call `setenv` in the
/// child — this codebase forks before any tokio worker threads exist, so
/// allocator state is single-threaded and the call is sound in practice.
/// Pre-building these `CString`s here means we don't have to allocate in
/// the child on the keys or values themselves.
#[cfg(unix)]
fn build_env_cstrings(
    env: &std::collections::HashMap<String, String>,
) -> Result<Vec<(std::ffi::CString, std::ffi::CString)>> {
    use std::ffi::CString;

    let mut pairs: Vec<(CString, CString)> = Vec::with_capacity(env.len());
    for (k, v) in env {
        if k.contains('=') {
            return Err(ExpectError::Spawn(SpawnError::InvalidArgument {
                kind: "env key".to_string(),
                value: k.clone(),
                reason: "env key contains '='".to_string(),
            }));
        }
        let key = CString::new(k.as_str()).map_err(|_| {
            ExpectError::Spawn(SpawnError::InvalidArgument {
                kind: "env key".to_string(),
                value: k.clone(),
                reason: "env key contains null byte".to_string(),
            })
        })?;
        let val = CString::new(v.as_str()).map_err(|_| {
            ExpectError::Spawn(SpawnError::InvalidArgument {
                kind: "env value".to_string(),
                value: v.clone(),
                reason: "env value contains null byte".to_string(),
            })
        })?;
        pairs.push((key, val));
    }
    Ok(pairs)
}

/// Validate the configured working directory and convert it to a `CString`
/// that can be safely passed to `chdir` between fork and exec on Unix.
///
/// The directory's existence is checked here so a bad path yields a clean
/// `InvalidWorkingDir` error instead of an opaque child exit, and the
/// allocation happens pre-fork because allocating in the child is unsound.
#[cfg(unix)]
fn build_cwd_cstring(
    working_directory: Option<&std::path::PathBuf>,
) -> Result<Option<std::ffi::CString>> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let Some(path) = working_directory else {
        return Ok(None);
    };
    if !path.is_dir() {
        return Err(ExpectError::Spawn(SpawnError::InvalidWorkingDir {
            path: path.display().to_string(),
        }));
    }
    let cstring = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
        ExpectError::Spawn(SpawnError::InvalidWorkingDir {
            path: path.display().to_string(),
        })
    })?;
    Ok(Some(cstring))
}

/// Allocate a PTY master/slave pair, retrying briefly on failure.
///
/// macOS caps the system-wide PTY count at `kern.tty.ptmx_max` (511 by
/// default) — far below Linux's dynamic `/dev/pts` allocation. Under heavy
/// concurrent spawning (a parallel test suite, or an app driving many sessions
/// at once) `openpty` can momentarily fail — with `ENXIO` ("Device not
/// configured"), the BSD PTY-exhaustion code — even though a slot frees
/// moments later as other sessions are torn down. We therefore retry with a
/// short bounded backoff before giving up, which is what turns intermittent
/// `PtyAllocation` failures on macOS into reliable spawns.
///
/// The retry fires on **any** `openpty` failure rather than matching a
/// specific errno. Our call always passes fixed, valid arguments (null
/// name/termp/winp), so the only realistic failure is resource exhaustion;
/// retrying unconditionally is simpler and more robust than enumerating
/// errnos, and the bound guarantees a genuinely permanent failure still
/// surfaces promptly — carrying the raw OS error for diagnosis rather than the
/// previous opaque "Failed to open PTY".
#[cfg(unix)]
#[allow(unsafe_code)]
async fn open_pty_pair_with_retry() -> Result<(libc::c_int, libc::c_int)> {
    // ~10 attempts with a short linear backoff bounds the worst-case added
    // latency (~90ms) on a genuinely exhausted system while smoothing over
    // sub-millisecond transient spikes under concurrent spawning.
    const ATTEMPTS: u32 = 10;

    let mut last_err = io::Error::other("openpty was never attempted");

    for attempt in 0..ATTEMPTS {
        let mut master: libc::c_int = 0;
        let mut slave: libc::c_int = 0;

        // SAFETY: openpty() is called with valid pointers to stack-allocated
        // integers. The null pointers for name, termp, and winp are explicitly
        // allowed per POSIX. We check the return value below.
        let rc = unsafe {
            libc::openpty(
                &raw mut master,
                &raw mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };

        if rc == 0 {
            return Ok((master, slave));
        }

        last_err = io::Error::last_os_error();
        if attempt + 1 < ATTEMPTS {
            // Non-blocking backoff so other sessions can release their PTYs.
            tokio::time::sleep(std::time::Duration::from_millis(2 * u64::from(attempt + 1))).await;
        }
    }

    Err(ExpectError::Spawn(SpawnError::PtyAllocation {
        reason: format!(
            "openpty failed after {ATTEMPTS} attempts \
             (likely PTY-table exhaustion; on macOS see kern.tty.ptmx_max): {last_err}"
        ),
    }))
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
    /// # Runtime requirement (Unix)
    ///
    /// The Unix implementation forks and then calls `setenv` / `unsetenv` /
    /// `clearenv` between fork and exec to apply the configured env mode.
    /// Those libc functions are **not** async-signal-safe — they allocate
    /// — so the post-fork window in the child must run on a single thread
    /// for the call to be sound. In this crate that is true because
    /// callers reach `spawn` directly from a fresh `tokio::main` or
    /// equivalent before any background thread has captured the
    /// allocator lock at the fork point.
    ///
    /// **If you embed this crate in a host that pre-spawns worker
    /// threads (for example, a multi-threaded scheduler that's already
    /// running by the time you call `Session::spawn`)**, the assumption
    /// breaks: another thread may hold the allocator lock at the moment
    /// of `fork`, and the child can deadlock or corrupt heap state on
    /// the first `setenv` call. In that environment, prefer a
    /// `posix_spawn`-based spawner or a pre-fork sentinel-pipe helper.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The command or arguments contain null bytes
    /// - PTY allocation fails
    /// - Fork fails
    /// - Exec fails (child exits with code 1)
    #[cfg(unix)]
    #[allow(unsafe_code)]
    #[allow(clippy::unused_async)]
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
                    kind: format!("argument[{idx}]"),
                    value: arg.clone(),
                    reason: "argument contains null byte".to_string(),
                })
            })?;
            argv_cstrings.push(arg_cstring);
        }

        // Validate env entries before fork so we can return a clean error.
        let env_pairs = build_env_cstrings(&self.config.env)?;
        let env_mode = self.config.env_mode;

        // Validate the working directory and build its CString before forking;
        // chdir(2) is async-signal-safe but the CString allocation is not.
        let workdir_cstring = build_cwd_cstring(self.config.working_directory.as_ref())?;

        // Create PTY pair, retrying briefly on transient PTY-table exhaustion.
        // See `open_pty_pair_with_retry` for the macOS `ptmx_max` rationale.
        let (master_fd, slave_fd) = open_pty_pair_with_retry().await?;

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
                    // Widen TIOCSCTTY to c_ulong for macOS compatibility (u32 -> u64).
                    libc::ioctl(slave_fd, libc::c_ulong::from(libc::TIOCSCTTY), 0);

                    libc::dup2(slave_fd, 0);
                    libc::dup2(slave_fd, 1);
                    libc::dup2(slave_fd, 2);

                    if slave_fd > 2 {
                        libc::close(slave_fd);
                    }

                    // Change to the configured working directory before exec.
                    if let Some(ref cwd) = workdir_cstring
                        && libc::chdir(cwd.as_ptr()) != 0
                    {
                        libc::_exit(1);
                    }

                    // Apply env_mode + overrides before exec.
                    apply_env_in_child(env_mode, &env_pairs);

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
    /// Master PTY file descriptor.
    master_fd: i32,
    /// Process ID.
    pid: u32,
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
        self.pid
    }

    /// Get the terminal dimensions.
    #[must_use]
    pub const fn dimensions(&self) -> (u16, u16) {
        self.dimensions
    }

    /// Resize the terminal.
    #[allow(unsafe_code)]
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
        // Cast to c_ulong for macOS compatibility (u32 -> u64).
        let result =
            unsafe { libc::ioctl(self.master_fd, libc::TIOCSWINSZ as libc::c_ulong, &winsize) };

        if result != 0 {
            Err(ExpectError::Io(io::Error::last_os_error()))
        } else {
            self.dimensions = (cols, rows);
            Ok(())
        }
    }

    /// Wait for the process to exit.
    #[allow(unsafe_code)]
    pub fn wait(&self) -> Result<i32> {
        let mut status: libc::c_int = 0;
        // SAFETY: self.pid is a valid process ID from fork().
        // status is a valid pointer to a stack-allocated integer.
        // The options argument (0) means blocking wait, which is valid.
        let result = unsafe { libc::waitpid(self.pid as i32, &raw mut status, 0) };

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

#[cfg(unix)]
impl Drop for PtyHandle {
    #[allow(unsafe_code)]
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
    /// Cached exit status, set once the child has been reaped.
    ///
    /// `waitpid` is not idempotent — a second reap of an already-collected
    /// child fails with `ECHILD` — so the first observed status is cached here
    /// and returned by all subsequent `try_wait`/`is_running` calls.
    exit_status: Option<ProcessExitStatus>,
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
            exit_status: None,
        })
    }

    /// Non-blocking reap of the child process.
    ///
    /// Returns `Some(status)` if the child has exited (caching it for future
    /// calls), or `None` while it is still running. A child reaped elsewhere
    /// (`ECHILD`) is reported as exited with [`ProcessExitStatus::Unknown`],
    /// since its real code is no longer recoverable.
    #[allow(unsafe_code)]
    pub fn try_wait(&mut self) -> Option<ProcessExitStatus> {
        if let Some(status) = self.exit_status {
            return Some(status);
        }

        let mut raw: libc::c_int = 0;
        loop {
            // SAFETY: self.pid is a valid PID from fork(); &raw mut raw is a
            // valid out-pointer; WNOHANG makes this a non-blocking query.
            let result = unsafe { libc::waitpid(self.pid as i32, &raw mut raw, libc::WNOHANG) };

            if result == 0 {
                // Child still running.
                return None;
            }

            if result == -1 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::Interrupted {
                    continue; // EINTR — retry the syscall.
                }
                // ECHILD (already reaped) or another error: the child is gone
                // but its real status is unrecoverable.
                let status = ProcessExitStatus::Unknown;
                self.exit_status = Some(status);
                return Some(status);
            }

            let status = decode_wait_status(raw);
            self.exit_status = Some(status);
            return Some(status);
        }
    }

    /// Check whether the child process is still running.
    ///
    /// Non-blocking: performs a `waitpid(WNOHANG)` peek, so it reports the truth
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
    #[allow(unsafe_code)]
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        let winsize = libc::winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        // SAFETY: The fd is valid and TIOCSWINSZ is a valid ioctl for PTYs.
        // Cast to c_ulong for macOS compatibility (u32 -> u64).
        let result = unsafe {
            libc::ioctl(
                *self.inner.get_ref(),
                libc::TIOCSWINSZ as libc::c_ulong,
                &winsize,
            )
        };

        if result != 0 {
            Err(ExpectError::Io(io::Error::last_os_error()))
        } else {
            self.dimensions = (cols, rows);
            Ok(())
        }
    }

    /// Send a signal to the child process.
    ///
    /// Guards against PID reuse: if the child has already exited (and possibly
    /// been reaped, freeing its PID for the OS to recycle), this returns
    /// [`ExpectError::SessionClosed`] rather than risk `libc::kill` landing on
    /// an unrelated process. A raw `ESRCH` from `kill` maps to the same. Other
    /// delivery failures (e.g. `EPERM`) are surfaced unchanged as
    /// [`ExpectError::Io`].
    #[allow(unsafe_code)]
    pub fn signal(&mut self, signal: i32) -> Result<()> {
        // Authoritative pre-kill check. `try_wait` reaps-and-caches; on this
        // path `AsyncPty` is the only in-crate reaper, so `None` means the PID
        // is still ours between here and the `kill` below. (This does not
        // defend against the user reaping the same child out-of-band.)
        if self.try_wait().is_some() {
            return Err(ExpectError::SessionClosed);
        }
        // SAFETY: pid is a valid process ID from fork().
        let result = unsafe { libc::kill(self.pid as i32, signal) };
        if result == 0 {
            Ok(())
        } else {
            let err = io::Error::last_os_error();
            // Child exited between the guard and the kill (e.g. reaped
            // out-of-band): treat as already closed rather than a raw error.
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
    #[allow(unsafe_code)]
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
                libc::read(
                    fd,
                    unfilled.as_mut_ptr().cast::<libc::c_void>(),
                    unfilled.len(),
                )
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
    #[allow(unsafe_code)]
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        // A dead child's PTY master buffers writes on Linux; surface exit as BrokenPipe.
        if self.as_mut().get_mut().try_wait().is_some() {
            return Poll::Ready(Err(io::ErrorKind::BrokenPipe.into()));
        }
        loop {
            let mut guard = match self.inner.poll_write_ready(cx) {
                Poll::Ready(Ok(guard)) => guard,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            };

            let fd = *self.inner.get_ref();

            // SAFETY: fd is a valid file descriptor, buf is a valid buffer.
            let result = unsafe { libc::write(fd, buf.as_ptr().cast::<libc::c_void>(), buf.len()) };

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
    #[allow(unsafe_code)]
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
            .field("exit_status", &self.exit_status)
            .finish()
    }
}

#[cfg(unix)]
impl ChildExit for AsyncPty {
    fn try_exit_status(&mut self) -> Option<ProcessExitStatus> {
        self.try_wait()
    }
}

/// Decode a raw `waitpid` status word into a [`ProcessExitStatus`].
///
/// Distinguishes a normal exit (`WIFEXITED` → `Exited(code)`) from termination
/// by a signal (`WIFSIGNALED` → `Signaled(sig)`), preserving the raw signal
/// number rather than collapsing it into `128 + sig`.
#[cfg(unix)]
const fn decode_wait_status(raw: libc::c_int) -> ProcessExitStatus {
    if libc::WIFEXITED(raw) {
        ProcessExitStatus::Exited(libc::WEXITSTATUS(raw))
    } else if libc::WIFSIGNALED(raw) {
        ProcessExitStatus::Signaled(libc::WTERMSIG(raw))
    } else {
        // Stopped/continued: the child has not actually terminated.
        ProcessExitStatus::Unknown
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
    #[allow(unsafe_code)]
    async fn open_pty_pair_with_retry_allocates_valid_fds() {
        // Happy path: allocation succeeds and yields two distinct valid fds.
        let (master, slave) = open_pty_pair_with_retry().await.expect("openpty");
        assert!(master >= 0 && slave >= 0 && master != slave);
        // SAFETY: both fds were just returned by openpty and are owned here;
        // closing them releases the allocated PTY pair.
        unsafe {
            libc::close(master);
            libc::close(slave);
        }
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
