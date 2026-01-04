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

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

use crate::error::{ExpectError, Result};
use crate::expect::Pattern;

use super::hooks::{HookManager, InteractionEvent};
use super::mode::InteractionMode;
use super::terminal::TerminalSize;

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

    /// Create a send action with line ending.
    pub fn send_line(&self, data: impl Into<String>) -> InteractAction {
        let mut s = data.into();
        s.push('\n');
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
    /// Reference to the transport.
    transport: &'a Arc<Mutex<T>>,
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
}

impl<'a, T> InteractBuilder<'a, T>
where
    T: AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static,
{
    /// Create a new interact builder.
    pub(crate) fn new(transport: &'a Arc<Mutex<T>>) -> Self {
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
    pub fn on_resize<F>(mut self, callback: F) -> Self
    where
        F: Fn(&ResizeContext) -> InteractAction + Send + Sync + 'static,
    {
        self.resize_hook = Some(Box::new(callback));
        self
    }

    /// Set the interaction mode.
    #[must_use] pub const fn with_mode(mut self, mode: InteractionMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set the escape sequence to exit interact mode.
    ///
    /// Default is Ctrl+] (0x1d).
    pub fn with_escape(mut self, escape: impl Into<Vec<u8>>) -> Self {
        self.escape_sequence = Some(escape.into());
        self
    }

    /// Disable the escape sequence (interact runs until pattern stops it).
    #[must_use] pub fn no_escape(mut self) -> Self {
        self.escape_sequence = None;
        self
    }

    /// Set a timeout for the interaction.
    #[must_use] pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set the output buffer size.
    #[must_use] pub const fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    /// Add a byte-level input hook.
    pub fn with_input_hook<F>(mut self, hook: F) -> Self
    where
        F: Fn(&[u8]) -> Vec<u8> + Send + Sync + 'static,
    {
        self.hook_manager.add_input_hook(hook);
        self
    }

    /// Add a byte-level output hook.
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
        let mut runner = InteractRunner::new(
            Arc::clone(self.transport),
            self.output_hooks,
            self.input_hooks,
            self.resize_hook,
            self.hook_manager,
            self.mode,
            self.buffer_size,
            self.escape_sequence,
            self.timeout,
        );
        runner.run().await
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

/// Internal runner for the interaction loop.
struct InteractRunner<T>
where
    T: AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static,
{
    transport: Arc<Mutex<T>>,
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
    fn new(
        transport: Arc<Mutex<T>>,
        output_hooks: Vec<OutputPatternHook>,
        input_hooks: Vec<InputPatternHook>,
        resize_hook: Option<ResizeHook>,
        hook_manager: HookManager,
        mode: InteractionMode,
        buffer_size: usize,
        escape_sequence: Option<Vec<u8>>,
        timeout: Option<Duration>,
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
        }
    }

    async fn run(&mut self) -> Result<InteractResult> {
        #[cfg(unix)]
        {
            self.run_with_signals().await
        }
        #[cfg(not(unix))]
        {
            self.run_without_signals().await
        }
    }

    /// Run the interaction loop with Unix signal handling (SIGWINCH).
    #[cfg(unix)]
    async fn run_with_signals(&mut self) -> Result<InteractResult> {
        use tokio::io::{stdin, stdout, BufReader};

        self.hook_manager.notify(InteractionEvent::Started);

        let mut stdin = BufReader::new(stdin());
        let mut input_buf = [0u8; 1024];
        let mut output_buf = [0u8; 4096];
        let mut escape_buf: Vec<u8> = Vec::new();

        let deadline = self.timeout.map(|t| std::time::Instant::now() + t);

        // Set up SIGWINCH signal handler
        let mut sigwinch = tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::window_change(),
        )
        .map_err(ExpectError::Io)?;

        loop {
            // Check timeout
            if let Some(deadline) = deadline {
                if std::time::Instant::now() >= deadline {
                    self.hook_manager.notify(InteractionEvent::Ended);
                    return Ok(InteractResult {
                        reason: InteractEndReason::Timeout,
                        buffer: self.buffer.clone(),
                    });
                }
            }

            let read_timeout = self.mode.read_timeout;
            let mut transport = self.transport.lock().await;

            tokio::select! {
                // Handle SIGWINCH (window resize)
                _ = sigwinch.recv() => {
                    drop(transport); // Release lock before processing

                    if let Some(result) = self.handle_resize().await? {
                        return Ok(result);
                    }
                }

                // Read from session output
                result = transport.read(&mut output_buf) => {
                    drop(transport); // Release lock before processing
                    match result {
                        Ok(0) => {
                            self.hook_manager.notify(InteractionEvent::Ended);
                            return Ok(InteractResult {
                                reason: InteractEndReason::Eof,
                                buffer: self.buffer.clone(),
                            });
                        }
                        Ok(n) => {
                            let data = &output_buf[..n];
                            let processed = self.hook_manager.process_output(data.to_vec());

                            self.hook_manager.notify(InteractionEvent::Output(processed.clone()));

                            // Write to stdout
                            let mut stdout = stdout();
                            let _ = stdout.write_all(&processed).await;
                            let _ = stdout.flush().await;

                            // Append to buffer for pattern matching
                            if let Ok(s) = std::str::from_utf8(&processed) {
                                self.buffer.push_str(s);
                                // Trim buffer if too large
                                if self.buffer.len() > self.buffer_size {
                                    let start = self.buffer.len() - self.buffer_size;
                                    self.buffer = self.buffer[start..].to_string();
                                }
                            }

                            // Check output patterns
                            if let Some(result) = self.check_output_patterns().await? {
                                return Ok(result);
                            }
                        }
                        Err(e) => {
                            self.hook_manager.notify(InteractionEvent::Ended);
                            return Err(ExpectError::Io(e));
                        }
                    }
                }

                // Read from stdin (user input)
                result = tokio::time::timeout(read_timeout, stdin.read(&mut input_buf)) => {
                    drop(transport); // Release lock

                    if let Ok(Ok(n)) = result {
                        if n == 0 {
                            continue;
                        }

                        let data = &input_buf[..n];

                        // Check for escape sequence
                        if let Some(ref esc) = self.escape_sequence {
                            escape_buf.extend_from_slice(data);
                            if escape_buf.ends_with(esc) {
                                self.hook_manager.notify(InteractionEvent::ExitRequested);
                                self.hook_manager.notify(InteractionEvent::Ended);
                                return Ok(InteractResult {
                                    reason: InteractEndReason::Escape,
                                    buffer: self.buffer.clone(),
                                });
                            }
                            // Keep only last N bytes where N is escape length
                            if escape_buf.len() > esc.len() {
                                escape_buf = escape_buf[escape_buf.len() - esc.len()..].to_vec();
                            }
                        }

                        // Process through input hooks
                        let processed = self.hook_manager.process_input(data.to_vec());

                        self.hook_manager.notify(InteractionEvent::Input(processed.clone()));

                        // Check input patterns
                        if let Some(result) = self.check_input_patterns(&processed).await? {
                            return Ok(result);
                        }

                        // Send to session
                        let mut transport = self.transport.lock().await;
                        transport.write_all(&processed).await.map_err(ExpectError::Io)?;
                        transport.flush().await.map_err(ExpectError::Io)?;
                    }
                }
            }
        }
    }

    /// Run the interaction loop without signal handling (non-Unix platforms).
    #[cfg(not(unix))]
    async fn run_without_signals(&mut self) -> Result<InteractResult> {
        use tokio::io::{stdin, stdout, BufReader};

        self.hook_manager.notify(InteractionEvent::Started);

        let mut stdin = BufReader::new(stdin());
        let mut input_buf = [0u8; 1024];
        let mut output_buf = [0u8; 4096];
        let mut escape_buf: Vec<u8> = Vec::new();

        let deadline = self.timeout.map(|t| std::time::Instant::now() + t);

        loop {
            // Check timeout
            if let Some(deadline) = deadline {
                if std::time::Instant::now() >= deadline {
                    self.hook_manager.notify(InteractionEvent::Ended);
                    return Ok(InteractResult {
                        reason: InteractEndReason::Timeout,
                        buffer: self.buffer.clone(),
                    });
                }
            }

            let read_timeout = self.mode.read_timeout;
            let mut transport = self.transport.lock().await;

            tokio::select! {
                // Read from session output
                result = transport.read(&mut output_buf) => {
                    drop(transport); // Release lock before processing
                    match result {
                        Ok(0) => {
                            self.hook_manager.notify(InteractionEvent::Ended);
                            return Ok(InteractResult {
                                reason: InteractEndReason::Eof,
                                buffer: self.buffer.clone(),
                            });
                        }
                        Ok(n) => {
                            let data = &output_buf[..n];
                            let processed = self.hook_manager.process_output(data.to_vec());

                            self.hook_manager.notify(InteractionEvent::Output(processed.clone()));

                            // Write to stdout
                            let mut stdout = stdout();
                            let _ = stdout.write_all(&processed).await;
                            let _ = stdout.flush().await;

                            // Append to buffer for pattern matching
                            if let Ok(s) = std::str::from_utf8(&processed) {
                                self.buffer.push_str(s);
                                // Trim buffer if too large
                                if self.buffer.len() > self.buffer_size {
                                    let start = self.buffer.len() - self.buffer_size;
                                    self.buffer = self.buffer[start..].to_string();
                                }
                            }

                            // Check output patterns
                            if let Some(result) = self.check_output_patterns().await? {
                                return Ok(result);
                            }
                        }
                        Err(e) => {
                            self.hook_manager.notify(InteractionEvent::Ended);
                            return Err(ExpectError::Io(e));
                        }
                    }
                }

                // Read from stdin (user input)
                result = tokio::time::timeout(read_timeout, stdin.read(&mut input_buf)) => {
                    drop(transport); // Release lock

                    if let Ok(Ok(n)) = result {
                        if n == 0 {
                            continue;
                        }

                        let data = &input_buf[..n];

                        // Check for escape sequence
                        if let Some(ref esc) = self.escape_sequence {
                            escape_buf.extend_from_slice(data);
                            if escape_buf.ends_with(esc) {
                                self.hook_manager.notify(InteractionEvent::ExitRequested);
                                self.hook_manager.notify(InteractionEvent::Ended);
                                return Ok(InteractResult {
                                    reason: InteractEndReason::Escape,
                                    buffer: self.buffer.clone(),
                                });
                            }
                            // Keep only last N bytes where N is escape length
                            if escape_buf.len() > esc.len() {
                                escape_buf = escape_buf[escape_buf.len() - esc.len()..].to_vec();
                            }
                        }

                        // Process through input hooks
                        let processed = self.hook_manager.process_input(data.to_vec());

                        self.hook_manager.notify(InteractionEvent::Input(processed.clone()));

                        // Check input patterns
                        if let Some(result) = self.check_input_patterns(&processed).await? {
                            return Ok(result);
                        }

                        // Send to session
                        let mut transport = self.transport.lock().await;
                        transport.write_all(&processed).await.map_err(ExpectError::Io)?;
                        transport.flush().await.map_err(ExpectError::Io)?;
                    }
                }
            }
        }
    }

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
                        let mut transport = self.transport.lock().await;
                        transport.write_all(&data).await.map_err(ExpectError::Io)?;
                        transport.flush().await.map_err(ExpectError::Io)?;
                        // Clear matched portion
                        self.buffer = after.to_string();
                    }
                    InteractAction::Stop => {
                        self.hook_manager.notify(InteractionEvent::Ended);
                        return Ok(Some(InteractResult {
                            reason: InteractEndReason::PatternStop { pattern_index: index },
                            buffer: self.buffer.clone(),
                        }));
                    }
                    InteractAction::Error(msg) => {
                        self.hook_manager.notify(InteractionEvent::Ended);
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
                        let mut transport = self.transport.lock().await;
                        transport.write_all(&data).await.map_err(ExpectError::Io)?;
                        transport.flush().await.map_err(ExpectError::Io)?;
                    }
                    InteractAction::Stop => {
                        return Ok(Some(InteractResult {
                            reason: InteractEndReason::PatternStop { pattern_index: index },
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
    async fn handle_resize(&mut self) -> Result<Option<InteractResult>> {
        // Get the new terminal size
        let new_size = match super::terminal::Terminal::size() {
            Ok(size) => size,
            Err(_) => return Ok(None), // Ignore if we can't get size
        };

        // Build the context with previous size
        let ctx = ResizeContext {
            size: new_size,
            previous: self.current_size,
        };

        // Notify via hook manager
        self.hook_manager.notify(InteractionEvent::Resize {
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
                    let mut transport = self.transport.lock().await;
                    transport.write_all(&data).await.map_err(ExpectError::Io)?;
                    transport.flush().await.map_err(ExpectError::Io)?;
                }
                InteractAction::Stop => {
                    self.hook_manager.notify(InteractionEvent::Ended);
                    return Ok(Some(InteractResult {
                        reason: InteractEndReason::PatternStop { pattern_index: 0 },
                        buffer: self.buffer.clone(),
                    }));
                }
                InteractAction::Error(msg) => {
                    self.hook_manager.notify(InteractionEvent::Ended);
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
