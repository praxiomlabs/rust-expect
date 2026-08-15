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
use crate::error::Result;
use crate::expect::Pattern;
use crate::session::Session;

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
    /// The session being interacted with. The interaction is its read driver
    /// for the duration, which is why this is a borrow of the session itself
    /// rather than a copy of its transport handle.
    session: &'a mut Session<T>,
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
    /// Escape string to exit interact mode.
    escape_sequence: Option<Vec<u8>>,
    /// Default timeout for the interaction.
    timeout: Option<Duration>,
}

impl<'a, T> InteractBuilder<'a, T>
where
    T: AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static,
{
    /// Create a new interact builder.
    pub(crate) fn new(session: &'a mut Session<T>) -> Self {
        Self {
            session,
            output_hooks: Vec::new(),
            input_hooks: Vec::new(),
            resize_hook: None,
            hook_manager: HookManager::new(),
            mode: InteractionMode::default(),
            escape_sequence: Some(vec![0x1d]), // Ctrl+] by default
            timeout: None,
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
            self.session,
            self.output_hooks,
            self.input_hooks,
            self.resize_hook,
            self.hook_manager,
            self.mode,
            self.escape_sequence,
            self.timeout,
        );
        runner.run(input, output).await
    }
}

/// Result of an interactive session.
#[derive(Debug, Clone)]
pub struct InteractResult {
    /// How the interaction ended.
    pub reason: InteractEndReason,
    /// The session's buffer when the interaction ended.
    ///
    /// This is the session's own buffer, not a copy the interaction kept
    /// beside it, so it includes anything read before the interaction started
    /// and excludes text an output hook has already consumed.
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
struct InteractRunner<'a, T>
where
    T: AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static,
{
    session: &'a mut Session<T>,
    output_hooks: Vec<OutputPatternHook>,
    input_hooks: Vec<InputPatternHook>,
    /// Resize hook - used on Unix via SIGWINCH signal handling.
    /// On Windows, terminal resize events aren't currently supported.
    #[cfg_attr(windows, allow(dead_code))]
    resize_hook: Option<ResizeHook>,
    hook_manager: HookManager,
    mode: InteractionMode,
    escape_sequence: Option<Vec<u8>>,
    timeout: Option<Duration>,
    /// Current terminal size - tracked for resize delta detection on Unix.
    /// On Windows, terminal resize events aren't currently supported.
    #[cfg_attr(windows, allow(dead_code))]
    current_size: Option<TerminalSize>,
}

impl<'a, T> InteractRunner<'a, T>
where
    T: AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static,
{
    #[allow(clippy::too_many_arguments)]
    fn new(
        session: &'a mut Session<T>,
        output_hooks: Vec<OutputPatternHook>,
        input_hooks: Vec<InputPatternHook>,
        resize_hook: Option<ResizeHook>,
        hook_manager: HookManager,
        mode: InteractionMode,
        escape_sequence: Option<Vec<u8>>,
        timeout: Option<Duration>,
    ) -> Self {
        // Get initial terminal size
        let current_size = super::terminal::Terminal::size().ok();

        Self {
            session,
            output_hooks,
            input_hooks,
            resize_hook,
            hook_manager,
            mode,
            escape_sequence,
            timeout,
            current_size,
        }
    }

    /// What the interaction has seen so far: the session's buffer, which is
    /// now the only one. The runner used to keep a private `String` beside it.
    fn buffer(&mut self) -> String {
        self.session.buffer()
    }

    /// Write to the child through the session, so the bytes reach subscribers
    /// as `SessionEvent::Input` and are refused once the child is gone.
    async fn send(&mut self, data: &[u8]) -> Result<()> {
        self.session.send(data).await
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
        // Bracketed here rather than in the loops so every exit path — pattern
        // stop, escape, timeout, EOF, or an I/O error — leaves the state.
        self.session.begin_interacting();
        #[cfg(unix)]
        let outcome = self.run_with_signals(input, output).await;
        #[cfg(not(unix))]
        let outcome = self.run_without_signals(input, output).await;
        self.session.end_interacting();
        outcome
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
                .map_err(crate::error::ExpectError::Io)?;

        loop {
            // Check timeout
            if let Some(deadline) = deadline
                && tokio::time::Instant::now() >= deadline
            {
                self.hook_manager.notify(&InteractionEvent::Ended);
                let buffer = self.buffer();
                return Ok(InteractResult {
                    reason: InteractEndReason::Timeout,
                    buffer,
                });
            }

            let read_timeout = self.mode.read_timeout;
            // A clone, not a guard: `SharedTransport` takes the lock inside
            // each poll, so parking here on a quiet child holds nothing. The
            // previous `lock().await` was held for the whole `select!` —
            // including while waiting on stdin — which is what made process
            // control unreachable for the length of an interactive session.
            //
            // The bytes it reads go straight into the session via
            // `absorb_read`, so this is the session's read, not a second one.
            let mut transport = self.session.transport_handle();

            tokio::select! {
                // End the interaction at its deadline. Without this branch the
                // timeout is only observed between other completions, so a
                // quiet session with a closed terminal input never reaches it.
                () = wait_until(deadline) => {
                    self.hook_manager.notify(&InteractionEvent::Ended);
                    let buffer = self.buffer();
                    return Ok(InteractResult {
                        reason: InteractEndReason::Timeout,
                        buffer,
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
                    // Everything read here enters the session: the matcher, the
                    // event stream, and the state machine. EOF and a failed read
                    // are the session's EOF and the session's failure, not just
                    // this loop's.
                    let read = self.session.absorb_read(result, &output_buf);
                    match read {
                        Ok(0) => {
                            self.hook_manager.notify(&InteractionEvent::Ended);
                            let buffer = self.buffer();
                            return Ok(InteractResult {
                                reason: InteractEndReason::Eof,
                                buffer,
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
                            return Err(e);
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
                let buffer = self.buffer();
                return Ok(InteractResult {
                    reason: InteractEndReason::Timeout,
                    buffer,
                });
            }

            let read_timeout = self.mode.read_timeout;
            // A clone, not a guard: `SharedTransport` takes the lock inside
            // each poll, so parking here on a quiet child holds nothing. The
            // previous `lock().await` was held for the whole `select!` —
            // including while waiting on stdin — which is what made process
            // control unreachable for the length of an interactive session.
            //
            // The bytes it reads go straight into the session via
            // `absorb_read`, so this is the session's read, not a second one.
            let mut transport = self.session.transport_handle();

            tokio::select! {
                // End the interaction at its deadline. See the Unix loop.
                () = wait_until(deadline) => {
                    self.hook_manager.notify(&InteractionEvent::Ended);
                    let buffer = self.buffer();
                    return Ok(InteractResult {
                        reason: InteractEndReason::Timeout,
                        buffer,
                    });
                }

                // Read from session output
                result = transport.read(&mut output_buf) => {
                    // Everything read here enters the session: the matcher, the
                    // event stream, and the state machine. EOF and a failed read
                    // are the session's EOF and the session's failure, not just
                    // this loop's.
                    let read = self.session.absorb_read(result, &output_buf);
                    match read {
                        Ok(0) => {
                            self.hook_manager.notify(&InteractionEvent::Ended);
                            let buffer = self.buffer();
                            return Ok(InteractResult {
                                reason: InteractEndReason::Eof,
                                buffer,
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
                            return Err(e);
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
    /// The chunk is already in the session's buffer — `absorb_read` put it
    /// there before this is called. What remains is the terminal's copy, which
    /// the output hooks may rewrite, and the pattern check.
    ///
    /// Output hooks no longer rewrite what the patterns see. They rewrite what
    /// the *terminal* sees, and the session buffers what the child actually
    /// sent, so a screen or transcript attached to the session records the real
    /// output rather than a display transform of it.
    async fn handle_output(
        &mut self,
        data: &[u8],
        output: &mut (impl AsyncWrite + Unpin + Send),
    ) -> Result<Option<InteractResult>> {
        let processed = self.hook_manager.process_output(data.to_vec());

        self.hook_manager
            .notify(&InteractionEvent::Output(processed.clone()));

        let _ = output.write_all(&processed).await;
        let _ = output.flush().await;

        self.check_output_patterns().await
    }

    /// Handle one chunk of terminal input: the escape sequence, the input
    /// hooks, the input patterns, then the child.
    ///
    /// Returns `Some` if the escape sequence or a pattern ended the
    /// interaction. Shared by the Unix and non-Unix loops.
    async fn handle_terminal_input(
        &mut self,
        data: &[u8],
        escape_buf: &mut Vec<u8>,
    ) -> Result<Option<InteractResult>> {
        if let Some(esc) = self.escape_sequence.clone() {
            escape_buf.extend_from_slice(data);
            if escape_buf.ends_with(&esc) {
                self.hook_manager.notify(&InteractionEvent::ExitRequested);
                self.hook_manager.notify(&InteractionEvent::Ended);
                let buffer = self.buffer();
                return Ok(Some(InteractResult {
                    reason: InteractEndReason::Escape,
                    buffer,
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

        self.send(&processed).await?;
        Ok(None)
    }

    /// Check the output hooks against the session's buffer.
    ///
    /// Matching goes through the session's matcher, which tracks decoded text
    /// against raw bytes (so an invalid sequence cannot shift the offsets) and
    /// bounds the buffer by discarding whole bytes rather than slicing a
    /// `String` at an arbitrary index.
    async fn check_output_patterns(&mut self) -> Result<Option<InteractResult>> {
        for index in 0..self.output_hooks.len() {
            let Some(found) = self
                .session
                .matcher_mut()
                .try_match(&self.output_hooks[index].pattern)
            else {
                continue;
            };

            // Consume through the match whatever the hook decides. The hook has
            // handled it, so it must not fire again on the next chunk — and now
            // that this is the session's own buffer, a later `expect()` must not
            // match it a second time either.
            let m = self.session.matcher_mut().consume_match(&found);
            let buffer = format!("{}{}{}", m.before, m.matched, m.after);

            let action = {
                let ctx = InteractContext {
                    matched: &m.matched,
                    before: &m.before,
                    after: &m.after,
                    buffer: &buffer,
                    pattern_index: index,
                };
                (self.output_hooks[index].callback)(&ctx)
            };

            match action {
                InteractAction::Continue => {}
                InteractAction::Send(data) => self.send(&data).await?,
                InteractAction::Stop => {
                    self.hook_manager.notify(&InteractionEvent::Ended);
                    return Ok(Some(InteractResult {
                        reason: InteractEndReason::PatternStop {
                            pattern_index: index,
                        },
                        buffer,
                    }));
                }
                InteractAction::Error(msg) => {
                    self.hook_manager.notify(&InteractionEvent::Ended);
                    return Ok(Some(InteractResult {
                        reason: InteractEndReason::Error(msg),
                        buffer,
                    }));
                }
            }
        }
        Ok(None)
    }

    /// Check the input hooks against one chunk of what the user typed.
    ///
    /// Unlike the output hooks these match the chunk itself, not a buffer:
    /// input arrives as discrete keystrokes with no accumulated context.
    async fn check_input_patterns(&mut self, input: &[u8]) -> Result<Option<InteractResult>> {
        let input_str = String::from_utf8_lossy(input).into_owned();

        for index in 0..self.input_hooks.len() {
            let Some(m) = self.input_hooks[index].pattern.matches(&input_str) else {
                continue;
            };

            let action = {
                let ctx = InteractContext {
                    matched: &input_str[m.start..m.end],
                    before: &input_str[..m.start],
                    after: &input_str[m.end..],
                    buffer: &input_str,
                    pattern_index: index,
                };
                (self.input_hooks[index].callback)(&ctx)
            };

            match action {
                InteractAction::Continue => {}
                InteractAction::Send(data) => self.send(&data).await?,
                InteractAction::Stop => {
                    let buffer = self.buffer();
                    return Ok(Some(InteractResult {
                        reason: InteractEndReason::PatternStop {
                            pattern_index: index,
                        },
                        buffer,
                    }));
                }
                InteractAction::Error(msg) => {
                    let buffer = self.buffer();
                    return Ok(Some(InteractResult {
                        reason: InteractEndReason::Error(msg),
                        buffer,
                    }));
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
        let action = self.resize_hook.as_ref().map(|hook| hook(&ctx));
        match action {
            None | Some(InteractAction::Continue) => {}
            Some(InteractAction::Send(data)) => self.send(&data).await?,
            Some(InteractAction::Stop) => {
                self.hook_manager.notify(&InteractionEvent::Ended);
                let buffer = self.buffer();
                return Ok(Some(InteractResult {
                    reason: InteractEndReason::PatternStop { pattern_index: 0 },
                    buffer,
                }));
            }
            Some(InteractAction::Error(msg)) => {
                self.hook_manager.notify(&InteractionEvent::Ended);
                let buffer = self.buffer();
                return Ok(Some(InteractResult {
                    reason: InteractEndReason::Error(msg),
                    buffer,
                }));
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
    use crate::config::SessionConfig;
    use crate::mock::MockTransport;
    use crate::session::Session;

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

    /// A session over a mock transport pre-loaded with `output`, plus a handle
    /// for queueing more and reading back what the interaction sent.
    fn session(output: &str) -> (Session<MockTransport>, MockTransport) {
        let mock = MockTransport::new();
        let handle = mock.clone();
        handle.queue_output_str(output);
        (Session::new(mock, SessionConfig::default()), handle)
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
        let (mut session, _handle) = session("login: ");
        let result = bounded(
            session
                .interact()
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
        let (mut session, handle) = session("password: ");
        let result = bounded(
            session
                .interact()
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
        let (mut session, handle) = session("");
        let _ = bounded(
            session
                .interact()
                .no_escape()
                .with_timeout(Duration::from_millis(300))
                .start_with_io(&b"whoami\n"[..], Vec::new()),
        )
        .await;

        assert_eq!(handle.take_input_str(), "whoami\n");
    }

    #[tokio::test]
    async fn child_output_reaches_the_terminal() {
        let (mut session, _handle) = session("hello world");
        let mut terminal = Vec::new();
        let _ = bounded(
            session
                .interact()
                .no_escape()
                .with_timeout(Duration::from_millis(300))
                .start_with_io(&b""[..], &mut terminal),
        )
        .await;

        assert_eq!(String::from_utf8_lossy(&terminal), "hello world");
    }

    #[tokio::test]
    async fn eof_from_the_child_ends_the_interaction() {
        let (mut session, handle) = session("bye\n");
        handle.signal_eof();
        let result = bounded(
            session
                .interact()
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
        let (mut session, _handle) = session("");
        let result = bounded(
            session
                .interact()
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
        let (mut session, _handle) = session("");
        let _ = bounded(
            session
                .interact()
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
        let (mut session, _handle) = session("");
        let started = std::time::Instant::now();
        let result = bounded(
            session
                .interact()
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

    /// AR-017. A PTY read ends wherever the kernel buffer does, so a chunk
    /// routinely stops partway through a multibyte character. Dropping such a
    /// chunk loses it from what the interaction's patterns match against, and
    /// the loss is invisible — the bytes still reach the terminal.
    #[tokio::test]
    async fn a_chunk_split_mid_character_is_not_dropped() {
        // "caf" plus the first byte of 'é'; the rest arrives in the next read.
        let (mut session, handle) = session("");
        handle.queue_output(&[b'c', b'a', b'f', 0xC3]);

        let queue = handle.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            queue.queue_output(&[0xA9, b' ', b'r', b'e', b'a', b'd', b'y']);
        });

        let result = bounded(
            session
                .interact()
                .no_escape()
                .with_timeout(Duration::from_millis(600))
                .on_output("café ready", |_| InteractAction::Stop)
                .start_with_io(&b""[..], Vec::new()),
        )
        .await;

        assert!(
            matches!(
                result.reason,
                InteractEndReason::PatternStop { pattern_index: 0 }
            ),
            "the split chunk was dropped, so the pattern never fired; got {:?}",
            result.reason
        );
    }

    /// AR-016. Trimming the interaction buffer used to slice a `String` at a
    /// raw byte offset, which panics when the cut lands inside a character.
    /// Three 3000-byte bursts cross the 8192-byte bound at byte 808 of a
    /// three-byte sequence.
    #[tokio::test]
    async fn trimming_a_multibyte_stream_does_not_panic() {
        let (mut session, handle) = session("");

        let queue = handle.clone();
        tokio::spawn(async move {
            let burst = "€".repeat(1000);
            for _ in 0..3 {
                tokio::time::sleep(Duration::from_millis(30)).await;
                queue.queue_output_str(&burst);
            }
        });

        let result = bounded(
            session
                .interact()
                .no_escape()
                .with_timeout(Duration::from_millis(400))
                .start_with_io(&b""[..], Vec::new()),
        )
        .await;

        assert!(
            matches!(result.reason, InteractEndReason::Timeout),
            "expected a timeout, got {:?}",
            result.reason
        );
    }

    /// AR-003. The session is not a bystander while `interact()` runs: output
    /// the interaction reads lands in the session's own buffer, so a caller
    /// that interacts and then expects sees what happened in between.
    #[tokio::test]
    async fn output_read_during_the_interaction_reaches_the_session() {
        let (mut session, _handle) = session("build finished\n");
        let _ = bounded(
            session
                .interact()
                .no_escape()
                .with_timeout(Duration::from_millis(300))
                .start_with_io(&b""[..], Vec::new()),
        )
        .await;

        assert!(
            session.buffer().contains("build finished"),
            "the session buffer missed the interaction's output: {:?}",
            session.buffer()
        );
    }

    /// AR-003/AR-004. EOF observed by the interaction is EOF for the session.
    #[tokio::test]
    async fn eof_during_the_interaction_reaches_the_session_state() {
        let (mut session, handle) = session("done\n");
        handle.signal_eof();
        let _ = bounded(
            session
                .interact()
                .no_escape()
                .with_timeout(Duration::from_secs(2))
                .start_with_io(&b""[..], Vec::new()),
        )
        .await;

        assert!(
            session.is_eof(),
            "the session did not see the EOF the interaction ended on; state = {:?}",
            session.state()
        );
    }
}
