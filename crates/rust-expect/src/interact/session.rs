//! Interactive session with pattern hooks.
//!
//! This module provides the interactive session functionality with pattern-based
//! callbacks. When patterns match in the output, registered callbacks are triggered.
//!
//! # Example
//!
//! ```ignore
//! use rust_expect::Session;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), rust_expect::ExpectError> {
//!     let mut session = Session::spawn("/bin/bash", &[]).await?;
//!
//!     session.interact()
//!         .on_output("password:", |ctx| {
//!             println!("Password prompt detected!");
//!             ctx.send("secret\n")
//!         })
//!         .on_output("logout", |_| {
//!             InteractAction::Stop
//!         })
//!         .start()
//!         .await?;
//!
//!     Ok(())
//! }
//! ```

use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};

use super::hooks::{HookManager, InteractionEvent};
use super::mode::InteractionMode;
use super::terminal::TerminalSize;
use crate::error::{ExpectError, Result};
use crate::expect::Pattern;
use crate::session::transport::SharedTransport;

/// Action to take after a pattern match in interactive mode.
#[derive(Debug, Clone)]
pub enum InteractAction {
    /// Continue interaction.
    Continue,
    /// Send data to the session.
    Send(Vec<u8>),
    /// Stop the interaction.
    Stop,
    /// Stop with an error.
    Error(String),
}

impl InteractAction {
    /// Create a send action from a string.
    pub fn send(s: impl Into<String>) -> Self {
        Self::Send(s.into().into_bytes())
    }

    /// Create a send action from bytes.
    pub fn send_bytes(data: impl Into<Vec<u8>>) -> Self {
        Self::Send(data.into())
    }
}

/// Context passed to pattern hook callbacks.
pub struct InteractContext<'a> {
    /// The matched text.
    pub matched: &'a str,
    /// Text before the match.
    pub before: &'a str,
    /// Text after the match.
    pub after: &'a str,
    /// The full buffer contents.
    pub buffer: &'a str,
    /// The pattern index that matched.
    pub pattern_index: usize,
}

impl InteractContext<'_> {
    /// Create a send action for convenience.
    pub fn send(&self, data: impl Into<String>) -> InteractAction {
        InteractAction::send(data)
    }

    /// Create a send action with the platform's default line ending.
    ///
    /// This previously hardcoded `\n`, which `ConPTY` discards, so the action could
    /// never submit a line on Windows. It uses the platform default rather than the
    /// session's configured [`LineEnding`](crate::LineEnding) because
    /// `InteractContext` carries only match data and has no access to the config;
    /// threading it through would mean adding a public field.
    pub fn send_line(&self, data: impl Into<String>) -> InteractAction {
        let mut s = data.into();
        s.push_str(crate::LineEnding::default().as_str());
        InteractAction::send(s)
    }
}

/// Type alias for pattern hook callbacks.
pub type PatternHook = Box<dyn Fn(&InteractContext<'_>) -> InteractAction + Send + Sync>;

/// Context passed to resize hook callbacks.
#[derive(Debug, Clone, Copy)]
pub struct ResizeContext {
    /// New terminal size.
    pub size: TerminalSize,
    /// Previous terminal size (if known).
    pub previous: Option<TerminalSize>,
}

/// Type alias for resize hook callbacks.
pub type ResizeHook = Box<dyn Fn(&ResizeContext) -> InteractAction + Send + Sync>;

/// Output pattern hook registration.
struct OutputPatternHook {
    pattern: Pattern,
    callback: PatternHook,
}

/// Input pattern hook registration.
struct InputPatternHook {
    pattern: Pattern,
    callback: PatternHook,
}

/// Builder for configuring interactive sessions.
pub struct InteractBuilder<'a, T>
where
    T: AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static,
{
    /// Reference to the session's transport handle.
    transport: &'a SharedTransport<T>,
    /// Output pattern hooks.
    output_hooks: Vec<OutputPatternHook>,
    /// Input pattern hooks.
    input_hooks: Vec<InputPatternHook>,
    /// Resize hook.
    resize_hook: Option<ResizeHook>,
    /// Byte-level hook manager.
    hook_manager: HookManager,
    /// Interaction mode configuration.
    mode: InteractionMode,
    /// Buffer for accumulating output.
    buffer_size: usize,
    /// Escape string to exit interact mode.
    escape_sequence: Option<Vec<u8>>,
    /// Default timeout for the interaction.
    timeout: Option<Duration>,
    /// Session-registered observers to fire on every chunk read during
    /// the interact loop, in addition to the expect-driven taps. Required
    /// so attached screens and transcript recorders don't go stale while
    /// `interact()` is the active read-driver.
    output_taps: Vec<crate::session::EventSubscriber>,
}

impl<'a, T> InteractBuilder<'a, T>
where
    T: AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static,
{
    /// Create a new interact builder.
    pub(crate) fn new(
        transport: &'a SharedTransport<T>,
        output_taps: Vec<crate::session::EventSubscriber>,
    ) -> Self {
        Self {
            transport,
            output_hooks: Vec::new(),
            input_hooks: Vec::new(),
            resize_hook: None,
            hook_manager: HookManager::new(),
            mode: InteractionMode::default(),
            buffer_size: 8192,
            escape_sequence: Some(vec![0x1d]), // Ctrl+] by default
            timeout: None,
            output_taps,
        }
    }

    /// Register a pattern hook for output.
    ///
    /// When the output matches the pattern, the callback is invoked.
    ///
    /// # Example
    ///
    /// ```ignore
    /// session.interact()
    ///     .on_output("password:", |ctx| {
    ///         ctx.send("my_password\n")
    ///     })
    ///     .start()
    ///     .await?;
    /// ```
    #[must_use]
    pub fn on_output<F>(mut self, pattern: impl Into<Pattern>, callback: F) -> Self
    where
        F: Fn(&InteractContext<'_>) -> InteractAction + Send + Sync + 'static,
    {
        self.output_hooks.push(OutputPatternHook {
            pattern: pattern.into(),
            callback: Box::new(callback),
        });
        self
    }

    /// Register a pattern hook for input.
    ///
    /// When the input matches the pattern, the callback is invoked.
    #[must_use]
    pub fn on_input<F>(mut self, pattern: impl Into<Pattern>, callback: F) -> Self
    where
        F: Fn(&InteractContext<'_>) -> InteractAction + Send + Sync + 'static,
    {
        self.input_hooks.push(InputPatternHook {
            pattern: pattern.into(),
            callback: Box::new(callback),
        });
        self
    }

    /// Register a hook for terminal resize events.
    ///
    /// On Unix systems, this is triggered by SIGWINCH. The callback receives
    /// the new terminal size and can optionally return an action.
    ///
    /// # Example
    ///
    /// ```ignore
    /// session.interact()
    ///     .on_resize(|ctx| {
    ///         println!("Terminal resized to {}x{}", ctx.size.cols, ctx.size.rows);
    ///         InteractAction::Continue
    ///     })
    ///     .start()
    ///     .await?;
    /// ```
    ///
    /// # Platform Support
    ///
    /// - **Unix**: Resize events are detected via SIGWINCH signal handling.
    /// - **Windows**: Resize detection is not currently supported; the callback
    ///   will not be invoked.
    #[must_use]
    pub fn on_resize<F>(mut self, callback: F) -> Self
    where
        F: Fn(&ResizeContext) -> InteractAction + Send + Sync + 'static,
    {
        self.resize_hook = Some(Box::new(callback));
        self
    }

    /// Set the interaction mode.
    #[must_use]
    pub const fn with_mode(mut self, mode: InteractionMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set the escape sequence to exit interact mode.
    ///
    /// Default is Ctrl+] (0x1d).
    #[must_use]
    pub fn with_escape(mut self, escape: impl Into<Vec<u8>>) -> Self {
        self.escape_sequence = Some(escape.into());
        self
    }

    /// Disable the escape sequence (interact runs until pattern stops it).
    #[must_use]
    pub fn no_escape(mut self) -> Self {
        self.escape_sequence = None;
        self
    }

    /// Set a timeout for the interaction.
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set the output buffer size.
    #[must_use]
    pub const fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    /// Add a byte-level input hook.
    #[must_use]
    pub fn with_input_hook<F>(mut self, hook: F) -> Self
    where
        F: Fn(&[u8]) -> Vec<u8> + Send + Sync + 'static,
    {
        self.hook_manager.add_input_hook(hook);
        self
    }

    /// Add a byte-level output hook.
    #[must_use]
    pub fn with_output_hook<F>(mut self, hook: F) -> Self
    where
        F: Fn(&[u8]) -> Vec<u8> + Send + Sync + 'static,
    {
        self.hook_manager.add_output_hook(hook);
        self
    }

    /// Start the interactive session.
    ///
    /// This runs the interaction loop, reading from stdin and the session,
    /// checking patterns, and invoking callbacks when matches occur.
    ///
    /// The interaction continues until:
    /// - A pattern callback returns `InteractAction::Stop`
    /// - The escape sequence is detected
    /// - A timeout occurs (if configured)
    /// - EOF is reached on the session
    ///
    /// # Errors
    ///
    /// Returns an error if I/O fails or a pattern callback returns an error.
    pub async fn start(self) -> Result<InteractResult> {
        self.start_with_io(tokio::io::stdin(), tokio::io::stdout())
            .await
    }

    /// Start the interaction against an arbitrary terminal pair.
    ///
    /// [`start`](Self::start) is this with the process's own stdin and stdout.
    /// Tests use it to drive the loop without a terminal.
    pub(crate) async fn start_with_io(
        self,
        input: impl AsyncRead + Unpin + Send,
        output: impl AsyncWrite + Unpin + Send,
    ) -> Result<InteractResult> {
        let mut runner = InteractRunner::new(
            self.transport.clone(),
            self.output_hooks,
            self.input_hooks,
            self.resize_hook,
            self.hook_manager,
            self.mode,
            self.buffer_size,
            self.escape_sequence,
            self.timeout,
            self.output_taps,
        );
        runner.run(input, output).await
    }
}

/// Result of an interactive session.
#[derive(Debug, Clone)]
pub struct InteractResult {
    /// How the interaction ended.
    pub reason: InteractEndReason,
    /// Final buffer contents.
    pub buffer: String,
}

/// Reason the interaction ended.
#[derive(Debug, Clone)]
pub enum InteractEndReason {
    /// A pattern callback returned Stop.
    PatternStop {
        /// Index of the pattern that stopped interaction.
        pattern_index: usize,
    },
    /// Escape sequence was detected.
    Escape,
    /// Timeout occurred.
    Timeout,
    /// EOF was reached on the session.
    Eof,
    /// An error occurred in a pattern callback.
    Error(String),
}

/// Wait until `deadline`, or forever if there is none.
///
/// The interaction's deadline has to be something the loop can park on. A
/// check at the top of the loop only runs once some other branch has completed,
/// which leaves the timeout at the mercy of whether anything else is happening.
async fn wait_until(deadline: Option<tokio::time::Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

/// Internal runner for the interaction loop.
struct InteractRunner<T>
where
    T: AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static,
{
    transport: SharedTransport<T>,
    output_hooks: Vec<OutputPatternHook>,
    input_hooks: Vec<InputPatternHook>,
    /// Resize hook - used on Unix via SIGWINCH signal handling.
    /// On Windows, terminal resize events aren't currently supported.
    #[cfg_attr(windows, allow(dead_code))]
    resize_hook: Option<ResizeHook>,
    hook_manager: HookManager,
    mode: InteractionMode,
    buffer: String,
    buffer_size: usize,
    escape_sequence: Option<Vec<u8>>,
    /// Session-registered observers fired on every chunk so attached
    /// screens and transcript recorders keep updating during `interact()`.
    output_taps: Vec<crate::session::EventSubscriber>,
    timeout: Option<Duration>,
    /// Current terminal size - tracked for resize delta detection on Unix.
    /// On Windows, terminal resize events aren't currently supported.
    #[cfg_attr(windows, allow(dead_code))]
    current_size: Option<TerminalSize>,
}

impl<T> InteractRunner<T>
where
    T: AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static,
{
    #[allow(clippy::too_many_arguments)]
    fn new(
        transport: SharedTransport<T>,
        output_hooks: Vec<OutputPatternHook>,
        input_hooks: Vec<InputPatternHook>,
        resize_hook: Option<ResizeHook>,
        hook_manager: HookManager,
        mode: InteractionMode,
        buffer_size: usize,
        escape_sequence: Option<Vec<u8>>,
        timeout: Option<Duration>,
        output_taps: Vec<crate::session::EventSubscriber>,
    ) -> Self {
        // Get initial terminal size
        let current_size = super::terminal::Terminal::size().ok();

        Self {
            transport,
            output_hooks,
            input_hooks,
            resize_hook,
            hook_manager,
            mode,
            buffer: String::with_capacity(buffer_size),
            buffer_size,
            escape_sequence,
            timeout,
            current_size,
            output_taps,
        }
    }

    /// Fire every session observer with an `Output` event, wrapping each in
    /// `catch_unwind` so a panicking observer can't take down the runner.
    /// Matches the contract of `Session::read_with_timeout`.
    fn fire_taps(&self, chunk: &[u8]) {
        crate::session::events::emit_output(&self.output_taps, chunk);
    }

    /// Run the interaction loop over `input`/`output`.
    ///
    /// The terminal ends of the loop are parameters rather than direct calls to
    /// `tokio::io::stdin`/`stdout` so the loop can be driven by a test. Nothing
    /// else constructs an `InteractRunner`, so before this the loop could not be
    /// exercised at all — which is why the defects below survived.
    async fn run(
        &mut self,
        input: impl AsyncRead + Unpin + Send,
        output: impl AsyncWrite + Unpin + Send,
    ) -> Result<InteractResult> {
        #[cfg(unix)]
        {
            self.run_with_signals(input, output).await
        }
        #[cfg(not(unix))]
        {
            self.run_without_signals(input, output).await
        }
    }

    /// Run the interaction loop with Unix signal handling (SIGWINCH).
    #[cfg(unix)]
    #[allow(clippy::significant_drop_tightening)]
    async fn run_with_signals(
        &mut self,
        input: impl AsyncRead + Unpin + Send,
        mut output: impl AsyncWrite + Unpin + Send,
    ) -> Result<InteractResult> {
        self.hook_manager.notify(&InteractionEvent::Started);

        let mut stdin = BufReader::new(input);
        let mut input_buf = [0u8; 1024];
        let mut output_buf = [0u8; 4096];
        let mut escape_buf: Vec<u8> = Vec::new();

        let deadline = self.timeout.map(|t| tokio::time::Instant::now() + t);

        // The terminal's input is a branch of the loop, not a guarantee. It can
        // be closed before the interaction starts — `interact()` with stdin
        // redirected from `/dev/null` or from an exhausted pipe — or close
        // partway through. Reading a closed input returns EOF instantly, so a
        // loop that keeps polling it never parks: measured at 3.2 million reads
        // in 300ms before this flag existed.
        let mut input_open = true;

        // Set up SIGWINCH signal handler
        let mut sigwinch =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::window_change())
                .map_err(ExpectError::Io)?;

        loop {
            // Check timeout
            if let Some(deadline) = deadline
                && tokio::time::Instant::now() >= deadline
            {
                self.hook_manager.notify(&InteractionEvent::Ended);
                return Ok(InteractResult {
                    reason: InteractEndReason::Timeout,
                    buffer: self.buffer.clone(),
                });
            }

            let read_timeout = self.mode.read_timeout;
            // A clone, not a guard: `SharedTransport` takes the lock inside
            // each poll, so parking here on a quiet child holds nothing. The
            // previous `lock().await` was held for the whole `select!` —
            // including while waiting on stdin — which is what made process
            // control unreachable for the length of an interactive session.
            let mut transport = self.transport.clone();

            tokio::select! {
                // End the interaction at its deadline. Without this branch the
                // timeout is only observed between other completions, so a
                // quiet session with a closed terminal input never reaches it.
                () = wait_until(deadline) => {
                    self.hook_manager.notify(&InteractionEvent::Ended);
                    return Ok(InteractResult {
                        reason: InteractEndReason::Timeout,
                        buffer: self.buffer.clone(),
                    });
                }

                // Handle SIGWINCH (window resize)
                _ = sigwinch.recv() => {
                    if let Some(result) = self.handle_resize().await? {
                        return Ok(result);
                    }
                }

                // Read from session output
                result = transport.read(&mut output_buf) => {
                    match result {
                        Ok(0) => {
                            self.hook_manager.notify(&InteractionEvent::Ended);
                            return Ok(InteractResult {
                                reason: InteractEndReason::Eof,
                                buffer: self.buffer.clone(),
                            });
                        }
                        Ok(n) => {
                            if let Some(result) =
                                self.handle_output(&output_buf[..n], &mut output).await?
                            {
                                return Ok(result);
                            }
                        }
                        Err(e) => {
                            self.hook_manager.notify(&InteractionEvent::Ended);
                            return Err(ExpectError::Io(e));
                        }
                    }
                }

                // Read from the terminal (user input)
                result = tokio::time::timeout(read_timeout, stdin.read(&mut input_buf)), if input_open => {

                    if let Ok(Ok(n)) = result {
                        if n == 0 {
                            // The terminal's input closed. The child is still
                            // running, so the interaction continues — but this
                            // branch has nothing left to report and polling it
                            // again would spin.
                            input_open = false;
                            continue;
                        }

                        if let Some(result) = self
                            .handle_terminal_input(&input_buf[..n], &mut escape_buf)
                            .await?
                        {
                            return Ok(result);
                        }
                    }
                }
            }
        }
    }

    /// Run the interaction loop without signal handling (non-Unix platforms).
    #[cfg(not(unix))]
    #[allow(clippy::significant_drop_tightening)]
    async fn run_without_signals(
        &mut self,
        input: impl AsyncRead + Unpin + Send,
        mut output: impl AsyncWrite + Unpin + Send,
    ) -> Result<InteractResult> {
        self.hook_manager.notify(&InteractionEvent::Started);

        let mut stdin = BufReader::new(input);
        let mut input_buf = [0u8; 1024];
        let mut output_buf = [0u8; 4096];
        let mut escape_buf: Vec<u8> = Vec::new();

        let deadline = self.timeout.map(|t| tokio::time::Instant::now() + t);

        // See the Unix loop: a closed terminal input reads EOF instantly, so a
        // branch that retries on EOF spins the loop.
        let mut input_open = true;

        loop {
            // Check timeout
            if let Some(deadline) = deadline
                && tokio::time::Instant::now() >= deadline
            {
                self.hook_manager.notify(&InteractionEvent::Ended);
                return Ok(InteractResult {
                    reason: InteractEndReason::Timeout,
                    buffer: self.buffer.clone(),
                });
            }

            let read_timeout = self.mode.read_timeout;
            // A clone, not a guard: `SharedTransport` takes the lock inside
            // each poll, so parking here on a quiet child holds nothing. The
            // previous `lock().await` was held for the whole `select!` —
            // including while waiting on stdin — which is what made process
            // control unreachable for the length of an interactive session.
            let mut transport = self.transport.clone();

            tokio::select! {
                // End the interaction at its deadline. See the Unix loop.
                () = wait_until(deadline) => {
                    self.hook_manager.notify(&InteractionEvent::Ended);
                    return Ok(InteractResult {
                        reason: InteractEndReason::Timeout,
                        buffer: self.buffer.clone(),
                    });
                }

                // Read from session output
                result = transport.read(&mut output_buf) => {
                    match result {
                        Ok(0) => {
                            self.hook_manager.notify(&InteractionEvent::Ended);
                            return Ok(InteractResult {
                                reason: InteractEndReason::Eof,
                                buffer: self.buffer.clone(),
                            });
                        }
                        Ok(n) => {
                            if let Some(result) =
                                self.handle_output(&output_buf[..n], &mut output).await?
                            {
                                return Ok(result);
                            }
                        }
                        Err(e) => {
                            self.hook_manager.notify(&InteractionEvent::Ended);
                            return Err(ExpectError::Io(e));
                        }
                    }
                }

                // Read from the terminal (user input)
                result = tokio::time::timeout(read_timeout, stdin.read(&mut input_buf)), if input_open => {

                    if let Ok(Ok(n)) = result {
                        if n == 0 {
                            // The terminal's input closed. The child is still
                            // running, so the interaction continues — but this
                            // branch has nothing left to report and polling it
                            // again would spin.
                            input_open = false;
                            continue;
                        }

                        if let Some(result) = self
                            .handle_terminal_input(&input_buf[..n], &mut escape_buf)
                            .await?
                        {
                            return Ok(result);
                        }
                    }
                }
            }
        }
    }

    /// Handle one chunk of child output: observers, hooks, the terminal, the
    /// match buffer, then the output patterns.
    ///
    /// Returns `Some` if a pattern ended the interaction. Shared by the Unix and
    /// non-Unix loops, which otherwise carried this verbatim twice.
    async fn handle_output(
        &mut self,
        data: &[u8],
        output: &mut (impl AsyncWrite + Unpin + Send),
    ) -> Result<Option<InteractResult>> {
        // Fire session-registered output taps on the raw chunk before any
        // hook-manager rewriting, so taps see exactly what the PTY emitted.
        self.fire_taps(data);
        let processed = self.hook_manager.process_output(data.to_vec());

        self.hook_manager
            .notify(&InteractionEvent::Output(processed.clone()));

        let _ = output.write_all(&processed).await;
        let _ = output.flush().await;

        // Append to buffer for pattern matching
        if let Ok(s) = std::str::from_utf8(&processed) {
            self.buffer.push_str(s);
            // Trim buffer if too large
            if self.buffer.len() > self.buffer_size {
                let start = self.buffer.len() - self.buffer_size;
                self.buffer = self.buffer[start..].to_string();
            }
        }

        self.check_output_patterns().await
    }

    /// Handle one chunk of terminal input: the escape sequence, the input
    /// hooks, the input patterns, then the child.
    ///
    /// Returns `Some` if the escape sequence or a pattern ended the
    /// interaction. Shared by the Unix and non-Unix loops.
    async fn handle_terminal_input(
        &self,
        data: &[u8],
        escape_buf: &mut Vec<u8>,
    ) -> Result<Option<InteractResult>> {
        if let Some(ref esc) = self.escape_sequence {
            escape_buf.extend_from_slice(data);
            if escape_buf.ends_with(esc) {
                self.hook_manager.notify(&InteractionEvent::ExitRequested);
                self.hook_manager.notify(&InteractionEvent::Ended);
                return Ok(Some(InteractResult {
                    reason: InteractEndReason::Escape,
                    buffer: self.buffer.clone(),
                }));
            }
            // Keep only last N bytes where N is escape length
            if escape_buf.len() > esc.len() {
                let excess = escape_buf.len() - esc.len();
                escape_buf.drain(..excess);
            }
        }

        let processed = self.hook_manager.process_input(data.to_vec());

        self.hook_manager
            .notify(&InteractionEvent::Input(processed.clone()));

        if let Some(result) = self.check_input_patterns(&processed).await? {
            return Ok(Some(result));
        }

        let mut transport = self.transport.clone();
        transport
            .write_all(&processed)
            .await
            .map_err(ExpectError::Io)?;
        transport.flush().await.map_err(ExpectError::Io)?;
        Ok(None)
    }

    #[allow(clippy::significant_drop_tightening)]
    async fn check_output_patterns(&mut self) -> Result<Option<InteractResult>> {
        for (index, hook) in self.output_hooks.iter().enumerate() {
            if let Some(m) = hook.pattern.matches(&self.buffer) {
                let matched = &self.buffer[m.start..m.end];
                let before = &self.buffer[..m.start];
                let after = &self.buffer[m.end..];

                let ctx = InteractContext {
                    matched,
                    before,
                    after,
                    buffer: &self.buffer,
                    pattern_index: index,
                };

                match (hook.callback)(&ctx) {
                    InteractAction::Continue => {
                        // Clear the matched portion to avoid re-triggering
                        self.buffer = after.to_string();
                    }
                    InteractAction::Send(data) => {
                        let mut transport = self.transport.clone();
                        transport.write_all(&data).await.map_err(ExpectError::Io)?;
                        transport.flush().await.map_err(ExpectError::Io)?;
                        // Clear matched portion
                        self.buffer = after.to_string();
                    }
                    InteractAction::Stop => {
                        self.hook_manager.notify(&InteractionEvent::Ended);
                        return Ok(Some(InteractResult {
                            reason: InteractEndReason::PatternStop {
                                pattern_index: index,
                            },
                            buffer: self.buffer.clone(),
                        }));
                    }
                    InteractAction::Error(msg) => {
                        self.hook_manager.notify(&InteractionEvent::Ended);
                        return Ok(Some(InteractResult {
                            reason: InteractEndReason::Error(msg),
                            buffer: self.buffer.clone(),
                        }));
                    }
                }
            }
        }
        Ok(None)
    }

    #[allow(clippy::significant_drop_tightening)]
    async fn check_input_patterns(&self, input: &[u8]) -> Result<Option<InteractResult>> {
        let input_str = String::from_utf8_lossy(input);

        for (index, hook) in self.input_hooks.iter().enumerate() {
            if let Some(m) = hook.pattern.matches(&input_str) {
                let matched = &input_str[m.start..m.end];
                let before = &input_str[..m.start];
                let after = &input_str[m.end..];

                let ctx = InteractContext {
                    matched,
                    before,
                    after,
                    buffer: &input_str,
                    pattern_index: index,
                };

                match (hook.callback)(&ctx) {
                    InteractAction::Continue => {}
                    InteractAction::Send(data) => {
                        let mut transport = self.transport.clone();
                        transport.write_all(&data).await.map_err(ExpectError::Io)?;
                        transport.flush().await.map_err(ExpectError::Io)?;
                    }
                    InteractAction::Stop => {
                        return Ok(Some(InteractResult {
                            reason: InteractEndReason::PatternStop {
                                pattern_index: index,
                            },
                            buffer: self.buffer.clone(),
                        }));
                    }
                    InteractAction::Error(msg) => {
                        return Ok(Some(InteractResult {
                            reason: InteractEndReason::Error(msg),
                            buffer: self.buffer.clone(),
                        }));
                    }
                }
            }
        }
        Ok(None)
    }

    /// Handle a window resize event.
    ///
    /// This is called on Unix when SIGWINCH is received. On Windows, terminal
    /// resize events aren't currently supported via signals.
    #[cfg_attr(windows, allow(dead_code))]
    #[allow(clippy::significant_drop_tightening)]
    async fn handle_resize(&mut self) -> Result<Option<InteractResult>> {
        // Get the new terminal size
        let Ok(new_size) = super::terminal::Terminal::size() else {
            return Ok(None); // Ignore if we can't get size
        };

        // Build the context with previous size
        let ctx = ResizeContext {
            size: new_size,
            previous: self.current_size,
        };

        // Notify via hook manager
        self.hook_manager.notify(&InteractionEvent::Resize {
            cols: new_size.cols,
            rows: new_size.rows,
        });

        // Update our tracked size
        self.current_size = Some(new_size);

        // Call the user's resize hook if registered
        if let Some(ref hook) = self.resize_hook {
            match hook(&ctx) {
                InteractAction::Continue => {}
                InteractAction::Send(data) => {
                    let mut transport = self.transport.clone();
                    transport.write_all(&data).await.map_err(ExpectError::Io)?;
                    transport.flush().await.map_err(ExpectError::Io)?;
                }
                InteractAction::Stop => {
                    self.hook_manager.notify(&InteractionEvent::Ended);
                    return Ok(Some(InteractResult {
                        reason: InteractEndReason::PatternStop { pattern_index: 0 },
                        buffer: self.buffer.clone(),
                    }));
                }
                InteractAction::Error(msg) => {
                    self.hook_manager.notify(&InteractionEvent::Ended);
                    return Ok(Some(InteractResult {
                        reason: InteractEndReason::Error(msg),
                        buffer: self.buffer.clone(),
                    }));
                }
            }
        }

        Ok(None)
    }
}

#[cfg(all(test, feature = "mock"))]
mod tests {
    use std::future::Future;
    use std::io;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Context, Poll};
    use std::time::Duration;

    use tokio::io::ReadBuf;

    use super::*;
    use crate::mock::MockTransport;
    use crate::session::transport::SharedTransport;

    /// A terminal input that is permanently at end-of-file, counting every read.
    ///
    /// A loop that treats "the user's terminal closed" as a reason to retry
    /// reads this once per iteration, so the count measures how many times the
    /// loop went round.
    struct ClosedInput(Arc<AtomicUsize>);

    impl AsyncRead for ClosedInput {
        fn poll_read(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            // Returning with the buffer unfilled is EOF.
            Poll::Ready(Ok(()))
        }
    }

    /// A shared transport over a mock, plus a handle for queueing output and
    /// reading back what the interaction sent.
    fn transport(output: &str) -> (SharedTransport<MockTransport>, MockTransport) {
        let mock = MockTransport::new();
        let handle = mock.clone();
        handle.queue_output_str(output);
        (SharedTransport::new(mock), handle)
    }

    /// Drive an interaction under a hard outer bound.
    ///
    /// An interaction that fails to end on its own should fail the test, not
    /// hang the suite — which is what happened when the loop's deadline branch
    /// was mutated away.
    async fn bounded(interaction: impl Future<Output = Result<InteractResult>>) -> InteractResult {
        tokio::time::timeout(Duration::from_secs(5), interaction)
            .await
            .expect("the interaction never ended on its own")
            .expect("the interaction should end cleanly")
    }

    #[tokio::test]
    async fn output_pattern_can_stop_the_interaction() {
        let (shared, _handle) = transport("login: ");
        let result = bounded(
            InteractBuilder::new(&shared, Vec::new())
                .no_escape()
                .with_timeout(Duration::from_secs(2))
                .on_output("login:", |_| InteractAction::Stop)
                .start_with_io(&b""[..], Vec::new()),
        )
        .await;

        assert!(
            matches!(
                result.reason,
                InteractEndReason::PatternStop { pattern_index: 0 }
            ),
            "expected a pattern stop, got {:?}",
            result.reason
        );
    }

    #[tokio::test]
    async fn an_output_hook_can_answer_the_child() {
        let (shared, handle) = transport("password: ");
        let result = bounded(
            InteractBuilder::new(&shared, Vec::new())
                .no_escape()
                .with_timeout(Duration::from_millis(300))
                .on_output("password:", |ctx| ctx.send("hunter2\n"))
                .start_with_io(&b""[..], Vec::new()),
        )
        .await;

        assert_eq!(handle.take_input_str(), "hunter2\n");
        assert!(
            matches!(result.reason, InteractEndReason::Timeout),
            "expected a timeout, got {:?}",
            result.reason
        );
    }

    #[tokio::test]
    async fn typed_input_reaches_the_child() {
        let (shared, handle) = transport("");
        let _ = bounded(
            InteractBuilder::new(&shared, Vec::new())
                .no_escape()
                .with_timeout(Duration::from_millis(300))
                .start_with_io(&b"whoami\n"[..], Vec::new()),
        )
        .await;

        assert_eq!(handle.take_input_str(), "whoami\n");
    }

    #[tokio::test]
    async fn child_output_reaches_the_terminal() {
        let (shared, _handle) = transport("hello world");
        let mut terminal = Vec::new();
        let _ = bounded(
            InteractBuilder::new(&shared, Vec::new())
                .no_escape()
                .with_timeout(Duration::from_millis(300))
                .start_with_io(&b""[..], &mut terminal),
        )
        .await;

        assert_eq!(String::from_utf8_lossy(&terminal), "hello world");
    }

    #[tokio::test]
    async fn eof_from_the_child_ends_the_interaction() {
        let (shared, handle) = transport("bye\n");
        handle.signal_eof();
        let result = bounded(
            InteractBuilder::new(&shared, Vec::new())
                .no_escape()
                .with_timeout(Duration::from_secs(2))
                .start_with_io(&b""[..], Vec::new()),
        )
        .await;

        assert!(
            matches!(result.reason, InteractEndReason::Eof),
            "expected EOF, got {:?}",
            result.reason
        );
    }

    #[tokio::test]
    async fn the_escape_sequence_ends_the_interaction() {
        let (shared, _handle) = transport("");
        let result = bounded(
            InteractBuilder::new(&shared, Vec::new())
                .with_escape(vec![0x1d])
                .with_timeout(Duration::from_secs(2))
                .start_with_io(&b"\x1d"[..], Vec::new()),
        )
        .await;

        assert!(
            matches!(result.reason, InteractEndReason::Escape),
            "expected an escape, got {:?}",
            result.reason
        );
    }

    /// A terminal whose input has closed is not a reason to spin. `interact()`
    /// under `< /dev/null`, or with a pipe whose writer has gone, reads EOF
    /// instantly on every pass, so a loop that retries never parks. Measured at
    /// 3,192,359 reads in 300ms before the fix.
    #[tokio::test]
    async fn a_closed_terminal_input_is_read_once_not_spun_on() {
        let reads = Arc::new(AtomicUsize::new(0));
        let (shared, _handle) = transport("");
        let _ = bounded(
            InteractBuilder::new(&shared, Vec::new())
                .no_escape()
                .with_timeout(Duration::from_millis(300))
                .start_with_io(ClosedInput(Arc::clone(&reads)), Vec::new()),
        )
        .await;

        let count = reads.load(Ordering::Relaxed);
        assert!(
            count <= 5,
            "closed terminal input was read {count} times in 300ms; the loop is spinning"
        );
    }

    /// The deadline has to be part of what the loop waits on. Once a closed
    /// terminal input stops driving iterations, nothing else wakes a quiet
    /// session, so the configured timeout is the only thing that can end it.
    #[tokio::test]
    async fn a_quiet_interaction_still_times_out() {
        let (shared, _handle) = transport("");
        let started = std::time::Instant::now();
        let result = bounded(
            InteractBuilder::new(&shared, Vec::new())
                .no_escape()
                .with_timeout(Duration::from_millis(300))
                .start_with_io(ClosedInput(Arc::new(AtomicUsize::new(0))), Vec::new()),
        )
        .await;

        assert!(
            matches!(result.reason, InteractEndReason::Timeout),
            "expected a timeout, got {:?}",
            result.reason
        );
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "the timeout took {:?}, far past its 300ms deadline",
            started.elapsed()
        );
    }
}
