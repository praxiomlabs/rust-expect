//! Session handle for interacting with spawned processes.
//!
//! This module provides the main `Session` type that users interact with
//! to control spawned processes, send input, and expect output.

use crate::config::SessionConfig;
use crate::dialog::{Dialog, DialogExecutor, DialogResult};
use crate::error::{ExpectError, Result};
use crate::expect::{ExpectState, MatchResult, Matcher, Pattern, PatternManager, PatternSet};
use crate::interact::InteractBuilder;
use crate::types::{ControlChar, Dimensions, Match, ProcessExitStatus, SessionId, SessionState};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

#[cfg(unix)]
use crate::backend::{AsyncPty, PtyConfig, PtySpawner};

#[cfg(windows)]
use crate::backend::{PtyConfig, PtySpawner, WindowsAsyncPty};

/// A session handle for interacting with a spawned process.
///
/// The session provides methods to send input, expect patterns in output,
/// and manage the lifecycle of the process.
pub struct Session<T: AsyncReadExt + AsyncWriteExt + Unpin + Send> {
    /// The underlying transport (PTY, SSH channel, etc.).
    transport: Arc<Mutex<T>>,
    /// Session configuration.
    config: SessionConfig,
    /// Pattern matcher.
    matcher: Matcher,
    /// Pattern manager for before/after patterns.
    pattern_manager: PatternManager,
    /// Current session state.
    state: SessionState,
    /// Unique session identifier.
    id: SessionId,
    /// EOF flag.
    eof: bool,
}

impl<T: AsyncReadExt + AsyncWriteExt + Unpin + Send> Session<T> {
    /// Create a new session with the given transport.
    pub fn new(transport: T, config: SessionConfig) -> Self {
        let buffer_size = config.buffer.max_size;
        Self {
            transport: Arc::new(Mutex::new(transport)),
            config,
            matcher: Matcher::new(buffer_size),
            pattern_manager: PatternManager::new(),
            state: SessionState::Starting,
            id: SessionId::new(),
            eof: false,
        }
    }

    /// Get the session ID.
    #[must_use]
    pub const fn id(&self) -> &SessionId {
        &self.id
    }

    /// Get the current session state.
    #[must_use]
    pub const fn state(&self) -> SessionState {
        self.state
    }

    /// Get the session configuration.
    #[must_use]
    pub const fn config(&self) -> &SessionConfig {
        &self.config
    }

    /// Check if EOF has been detected.
    #[must_use]
    pub const fn is_eof(&self) -> bool {
        self.eof
    }

    /// Get the current buffer contents.
    #[must_use]
    pub fn buffer(&mut self) -> String {
        self.matcher.buffer_str()
    }

    /// Clear the buffer.
    pub fn clear_buffer(&mut self) {
        self.matcher.clear();
    }

    /// Get the pattern manager for before/after patterns.
    #[must_use]
    pub const fn pattern_manager(&self) -> &PatternManager {
        &self.pattern_manager
    }

    /// Get mutable access to the pattern manager.
    pub fn pattern_manager_mut(&mut self) -> &mut PatternManager {
        &mut self.pattern_manager
    }

    /// Set the session state.
    pub fn set_state(&mut self, state: SessionState) {
        self.state = state;
    }

    /// Send bytes to the process.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub async fn send(&mut self, data: &[u8]) -> Result<()> {
        if matches!(self.state, SessionState::Closed | SessionState::Exited(_)) {
            return Err(ExpectError::SessionClosed);
        }

        let mut transport = self.transport.lock().await;
        transport
            .write_all(data)
            .await
            .map_err(|e| ExpectError::io_context("writing to process", e))?;
        transport
            .flush()
            .await
            .map_err(|e| ExpectError::io_context("flushing process output", e))?;
        Ok(())
    }

    /// Send a string to the process.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub async fn send_str(&mut self, s: &str) -> Result<()> {
        self.send(s.as_bytes()).await
    }

    /// Send a line to the process (appends newline based on config).
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub async fn send_line(&mut self, line: &str) -> Result<()> {
        let line_ending = self.config.line_ending.as_str();
        let data = format!("{line}{line_ending}");
        self.send(data.as_bytes()).await
    }

    /// Send a control character to the process.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub async fn send_control(&mut self, ctrl: ControlChar) -> Result<()> {
        self.send(&[ctrl.as_byte()]).await
    }

    /// Expect a pattern in the output.
    ///
    /// Blocks until the pattern is matched, EOF is detected, or timeout occurs.
    ///
    /// # Errors
    ///
    /// Returns an error on timeout, EOF (if not expected), or I/O error.
    pub async fn expect(&mut self, pattern: impl Into<Pattern>) -> Result<Match> {
        let patterns = PatternSet::from_patterns(vec![pattern.into()]);
        self.expect_any(&patterns).await
    }

    /// Expect any of the given patterns.
    ///
    /// # Errors
    ///
    /// Returns an error on timeout, EOF (if not expected), or I/O error.
    pub async fn expect_any(&mut self, patterns: &PatternSet) -> Result<Match> {
        let timeout = self.matcher.get_timeout(patterns);
        let state = ExpectState::new(patterns.clone(), timeout);

        loop {
            // Check before patterns first
            if let Some((_, action)) = self.pattern_manager.check_before(&self.matcher.buffer_str()) {
                match action {
                    crate::expect::HandlerAction::Continue => {}
                    crate::expect::HandlerAction::Return(s) => {
                        return Ok(Match::new(0, s, String::new(), self.matcher.buffer_str()));
                    }
                    crate::expect::HandlerAction::Abort(msg) => {
                        return Err(ExpectError::PatternNotFound {
                            pattern: msg,
                            buffer: self.matcher.buffer_str(),
                        });
                    }
                    crate::expect::HandlerAction::Respond(s) => {
                        self.send_str(&s).await?;
                    }
                }
            }

            // Check for pattern match
            if let Some(result) = self.matcher.try_match_any(patterns) {
                return Ok(self.matcher.consume_match(&result));
            }

            // Check for timeout
            if state.is_timed_out() {
                return Err(ExpectError::Timeout {
                    duration: timeout,
                    pattern: patterns
                        .iter()
                        .next()
                        .map(|p| p.pattern.as_str().to_string())
                        .unwrap_or_default(),
                    buffer: self.matcher.buffer_str(),
                });
            }

            // Check for EOF
            if self.eof {
                if state.expects_eof() {
                    return Ok(Match::new(0, String::new(), self.matcher.buffer_str(), String::new()));
                }
                return Err(ExpectError::Eof {
                    buffer: self.matcher.buffer_str(),
                });
            }

            // Read more data
            self.read_with_timeout(state.remaining_time()).await?;
        }
    }

    /// Expect with a specific timeout.
    ///
    /// # Errors
    ///
    /// Returns an error on timeout, EOF, or I/O error.
    pub async fn expect_timeout(&mut self, pattern: impl Into<Pattern>, timeout: Duration) -> Result<Match> {
        let pattern = pattern.into();
        let mut patterns = PatternSet::new();
        patterns.add(pattern).add(Pattern::timeout(timeout));
        self.expect_any(&patterns).await
    }

    /// Read data from the transport with timeout.
    async fn read_with_timeout(&mut self, timeout: Duration) -> Result<usize> {
        let mut buf = [0u8; 4096];
        let mut transport = self.transport.lock().await;

        match tokio::time::timeout(timeout, transport.read(&mut buf)).await {
            Ok(Ok(0)) => {
                self.eof = true;
                Ok(0)
            }
            Ok(Ok(n)) => {
                self.matcher.append(&buf[..n]);
                Ok(n)
            }
            Ok(Err(e)) => Err(ExpectError::io_context("reading from process", e)),
            Err(_) => {
                // Timeout, but not an error - caller will handle
                Ok(0)
            }
        }
    }

    /// Wait for the process to exit.
    ///
    /// # Errors
    ///
    /// Returns an error if waiting fails.
    pub async fn wait(&mut self) -> Result<ProcessExitStatus> {
        // Read until EOF
        while !self.eof {
            if self.read_with_timeout(Duration::from_millis(100)).await? == 0 && !self.eof {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }

        // Return unknown status - actual status depends on backend
        self.state = SessionState::Exited(ProcessExitStatus::Unknown);
        Ok(ProcessExitStatus::Unknown)
    }

    /// Check if a pattern matches immediately without blocking.
    #[must_use]
    pub fn check(&mut self, pattern: &Pattern) -> Option<MatchResult> {
        self.matcher.try_match(pattern)
    }

    /// Get the underlying transport.
    ///
    /// Use with caution as direct access bypasses session management.
    #[must_use]
    pub const fn transport(&self) -> &Arc<Mutex<T>> {
        &self.transport
    }

    /// Start an interactive session with pattern hooks.
    ///
    /// This returns a builder that allows you to configure pattern-based
    /// callbacks that fire when patterns match in the output or input.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use rust_expect::{Session, InteractAction};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), rust_expect::ExpectError> {
    ///     let mut session = Session::spawn("/bin/bash", &[]).await?;
    ///
    ///     session.interact()
    ///         .on_output("password:", |ctx| {
    ///             ctx.send("my_password\n")
    ///         })
    ///         .on_output("logout", |_| {
    ///             InteractAction::Stop
    ///         })
    ///         .start()
    ///         .await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    #[must_use] pub fn interact(&self) -> InteractBuilder<'_, T>
    where
        T: 'static,
    {
        InteractBuilder::new(&self.transport)
    }

    /// Run a dialog on this session.
    ///
    /// A dialog is a predefined sequence of expect/send operations.
    /// This method executes the dialog and returns the result.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use rust_expect::{Session, Dialog, DialogStep};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), rust_expect::ExpectError> {
    ///     let mut session = Session::spawn("/bin/bash", &[]).await?;
    ///
    ///     let dialog = Dialog::named("shell_test")
    ///         .step(DialogStep::new("prompt")
    ///             .with_expect("$")
    ///             .with_send("echo hello\n"))
    ///         .step(DialogStep::new("verify")
    ///             .with_expect("hello"));
    ///
    ///     let result = session.run_dialog(&dialog).await?;
    ///     assert!(result.success);
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if I/O fails. Step-level timeouts are reported
    /// in the `DialogResult` rather than as errors.
    pub async fn run_dialog(&mut self, dialog: &Dialog) -> Result<DialogResult> {
        let executor = DialogExecutor::default();
        executor.execute(self, dialog).await
    }

    /// Run a dialog with a custom executor.
    ///
    /// This allows customizing the executor settings (max steps, default timeout).
    ///
    /// # Errors
    ///
    /// Returns an error if I/O fails.
    pub async fn run_dialog_with(
        &mut self,
        dialog: &Dialog,
        executor: &DialogExecutor,
    ) -> Result<DialogResult> {
        executor.execute(self, dialog).await
    }

    /// Expect end-of-file (process termination).
    ///
    /// This is a convenience method for waiting until the process terminates
    /// and closes its output stream.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use rust_expect::Session;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), rust_expect::ExpectError> {
    ///     let mut session = Session::spawn("echo", &["hello"]).await?;
    ///     session.expect("hello").await?;
    ///     session.expect_eof().await?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the session times out before EOF or an I/O error occurs.
    pub async fn expect_eof(&mut self) -> Result<Match> {
        self.expect(Pattern::eof()).await
    }

    /// Expect end-of-file with a specific timeout.
    ///
    /// # Errors
    ///
    /// Returns an error if the session times out before EOF or an I/O error occurs.
    pub async fn expect_eof_timeout(&mut self, timeout: Duration) -> Result<Match> {
        let mut patterns = PatternSet::new();
        patterns.add(Pattern::eof()).add(Pattern::timeout(timeout));
        self.expect_any(&patterns).await
    }
}

impl<T: AsyncReadExt + AsyncWriteExt + Unpin + Send> std::fmt::Debug for Session<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("id", &self.id)
            .field("state", &self.state)
            .field("eof", &self.eof)
            .finish_non_exhaustive()
    }
}

// Unix-specific spawn implementation
#[cfg(unix)]
impl Session<AsyncPty> {
    /// Spawn a new process with the given command.
    ///
    /// This creates a new PTY, forks a child process, and returns a Session
    /// connected to the child's terminal.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use rust_expect::Session;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), rust_expect::ExpectError> {
    ///     let mut session = Session::spawn("/bin/bash", &[]).await?;
    ///     session.expect("$").await?;
    ///     session.send_line("echo hello").await?;
    ///     session.expect("hello").await?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The command contains null bytes
    /// - PTY allocation fails
    /// - Fork fails
    /// - The command cannot be executed
    pub async fn spawn(command: &str, args: &[&str]) -> Result<Self> {
        Self::spawn_with_config(command, args, SessionConfig::default()).await
    }

    /// Spawn a new process with custom configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if spawning fails.
    pub async fn spawn_with_config(
        command: &str,
        args: &[&str],
        config: SessionConfig,
    ) -> Result<Self> {
        let pty_config = PtyConfig::from(&config);
        let spawner = PtySpawner::with_config(pty_config);

        // Convert &[&str] to Vec<String> for the spawner
        let args_owned: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();

        // Spawn the process
        let handle = spawner.spawn(command, &args_owned).await?;

        // Wrap in AsyncPty for async I/O
        let async_pty = AsyncPty::from_handle(handle)
            .map_err(|e| ExpectError::io_context("creating async PTY wrapper", e))?;

        // Create the session
        let mut session = Self::new(async_pty, config);
        session.state = SessionState::Running;

        Ok(session)
    }

    /// Get the child process ID.
    #[must_use]
    pub fn pid(&self) -> u32 {
        // We need to access the inner transport's pid
        // For now, use the blocking lock since we know it's not contended
        // during a sync call like this
        if let Ok(transport) = self.transport.try_lock() {
            transport.pid()
        } else {
            0
        }
    }

    /// Resize the terminal.
    ///
    /// # Errors
    ///
    /// Returns an error if the resize ioctl fails.
    pub async fn resize_pty(&mut self, cols: u16, rows: u16) -> Result<()> {
        let mut transport = self.transport.lock().await;
        transport.resize(cols, rows)
    }

    /// Send a signal to the child process.
    ///
    /// # Errors
    ///
    /// Returns an error if sending the signal fails.
    pub fn signal(&self, signal: i32) -> Result<()> {
        if let Ok(transport) = self.transport.try_lock() {
            transport.signal(signal)
        } else {
            Err(ExpectError::io_context(
                "sending signal to process",
                std::io::Error::new(std::io::ErrorKind::WouldBlock, "transport is locked"),
            ))
        }
    }

    /// Kill the child process.
    ///
    /// # Errors
    ///
    /// Returns an error if killing the process fails.
    pub fn kill(&self) -> Result<()> {
        if let Ok(transport) = self.transport.try_lock() {
            transport.kill()
        } else {
            Err(ExpectError::io_context(
                "killing process",
                std::io::Error::new(std::io::ErrorKind::WouldBlock, "transport is locked"),
            ))
        }
    }
}

// Windows-specific spawn implementation
#[cfg(windows)]
impl Session<WindowsAsyncPty> {
    /// Spawn a new process with the given command.
    ///
    /// This creates a new PTY using Windows ConPTY, spawns a child process,
    /// and returns a Session connected to the child's terminal.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use rust_expect::Session;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), rust_expect::ExpectError> {
    ///     let mut session = Session::spawn("cmd.exe", &[]).await?;
    ///     session.expect(">").await?;
    ///     session.send_line("echo hello").await?;
    ///     session.expect("hello").await?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - ConPTY is not available (Windows version too old)
    /// - PTY allocation fails
    /// - The command cannot be executed
    pub async fn spawn(command: &str, args: &[&str]) -> Result<Self> {
        Self::spawn_with_config(command, args, SessionConfig::default()).await
    }

    /// Spawn a new process with custom configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if spawning fails.
    pub async fn spawn_with_config(
        command: &str,
        args: &[&str],
        config: SessionConfig,
    ) -> Result<Self> {
        let pty_config = PtyConfig::from(&config);
        let spawner = PtySpawner::with_config(pty_config);

        // Convert &[&str] to Vec<String> for the spawner
        let args_owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();

        // Spawn the process
        let handle = spawner.spawn(command, &args_owned).await?;

        // Wrap in WindowsAsyncPty for async I/O
        let async_pty = WindowsAsyncPty::from_handle(handle);

        // Create the session
        let mut session = Session::new(async_pty, config);
        session.state = SessionState::Running;

        Ok(session)
    }

    /// Get the child process ID.
    #[must_use]
    pub fn pid(&self) -> u32 {
        if let Ok(transport) = self.transport.try_lock() {
            transport.pid()
        } else {
            0
        }
    }

    /// Resize the terminal.
    ///
    /// # Errors
    ///
    /// Returns an error if the resize operation fails.
    pub async fn resize_pty(&mut self, cols: u16, rows: u16) -> Result<()> {
        let mut transport = self.transport.lock().await;
        transport.resize(cols, rows)
    }

    /// Check if the child process is still running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        if let Ok(transport) = self.transport.try_lock() {
            transport.is_running()
        } else {
            true // Assume running if we can't check
        }
    }

    /// Kill the child process.
    ///
    /// # Errors
    ///
    /// Returns an error if killing the process fails.
    pub fn kill(&self) -> Result<()> {
        if let Ok(mut transport) = self.transport.try_lock() {
            transport.kill()
        } else {
            Err(ExpectError::io_context(
                "killing process",
                std::io::Error::new(std::io::ErrorKind::WouldBlock, "transport is locked"),
            ))
        }
    }
}

/// Extension trait for session operations.
pub trait SessionExt {
    /// Send and expect in one call.
    fn send_expect(
        &mut self,
        send: &str,
        expect: impl Into<Pattern>,
    ) -> impl std::future::Future<Output = Result<Match>> + Send;

    /// Resize the terminal.
    fn resize(&mut self, dimensions: Dimensions) -> impl std::future::Future<Output = Result<()>> + Send;
}
