//! Session handle for interacting with spawned processes.
//!
//! This module provides the main `Session` type that users interact with
//! to control spawned processes, send input, and expect output.

use std::sync::Arc;
#[cfg(feature = "screen")]
use std::sync::{Mutex as StdMutex, MutexGuard};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

use crate::backend::ChildExit;
#[cfg(unix)]
use crate::backend::{AsyncPty, PtyConfig, PtySpawner};
#[cfg(windows)]
use crate::backend::{PtyConfig, PtySpawner, WindowsAsyncPty};
use crate::config::SessionConfig;
use crate::dialog::{Dialog, DialogExecutor, DialogResult};
use crate::error::{ExpectError, Result};
use crate::expect::{ExpectState, MatchResult, Matcher, Pattern, PatternManager, PatternSet};
use crate::interact::InteractBuilder;
#[cfg(feature = "screen")]
use crate::screen::Screen;
use crate::types::{ControlChar, Dimensions, Match, ProcessExitStatus, SessionId, SessionState};

/// Callback invoked for every chunk of bytes read from the transport.
///
/// Taps observe the raw byte stream as it arrives, after it is appended to the
/// matcher buffer but before any pattern matching is performed. They are the
/// foundation for screen emulation, transcript recording, and other features
/// that need to see output as it happens.
pub type OutputTap = Arc<dyn Fn(&[u8]) + Send + Sync>;

/// Opaque handle identifying a registered output tap. Returned by
/// [`Session::add_output_tap`] and accepted by
/// [`Session::remove_output_tap`].
///
/// Backed by `u64`. The id space is large enough that wraparound is not
/// reachable in practice; the implementation uses a non-wrapping `+= 1`
/// so a hypothetical exhaustion would surface as a loud panic instead of
/// silently colliding with a still-registered tap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TapId(u64);

impl std::fmt::Display for TapId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tap#{}", self.0)
    }
}

/// Lock the screen mutex, recovering from poisoning.
///
/// A user-supplied tap (or `Screen::process` panicking on a malformed parse
/// path) can poison the screen mutex. Silently returning a default on
/// poisoning makes screen-aware expects look like they always-miss, which
/// is a confusing failure mode. Recovering via `into_inner` lets the call
/// continue against the actual screen state — the screen contents are
/// still valid; only the lock was tainted.
#[cfg(feature = "screen")]
fn lock_screen(screen: &Arc<StdMutex<Screen>>) -> MutexGuard<'_, Screen> {
    match screen.lock() {
        Ok(g) => g,
        Err(poison) => {
            tracing::warn!("screen mutex was poisoned; recovering inner state");
            poison.into_inner()
        }
    }
}

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
    /// Output taps invoked on every chunk of bytes read from the transport,
    /// stored as (id, callback) so they can be removed individually.
    output_taps: Vec<(TapId, OutputTap)>,
    /// Monotonic counter for assigning new `TapId`s.
    next_tap_id: u64,
    /// Attached virtual terminal screen, fed from an output tap.
    #[cfg(feature = "screen")]
    screen: Option<Arc<StdMutex<Screen>>>,
    /// Tap id used to feed the attached screen, so `detach_screen` can
    /// remove only that tap and leave user-registered taps in place.
    #[cfg(feature = "screen")]
    screen_tap_id: Option<TapId>,
    /// Poll interval used by the screen-aware expect helpers
    /// (`expect_screen_contains`, `wait_screen_not_contains`,
    /// `wait_screen_stable`). 50 ms by default.
    #[cfg(feature = "screen")]
    screen_poll_interval: Duration,
}

/// Whether a write error means the child/peer is gone (the PTY slave end
/// closed, or the pipe broke) as opposed to a transient or unexpected failure.
///
/// After the slave closes, a write to the PTY master fails with `EIO` on Unix —
/// which `std` reports as an uncategorized kind, so we match the raw code — and
/// with `BrokenPipe` on Windows.
fn write_error_means_closed(err: &std::io::Error) -> bool {
    use std::io::ErrorKind;
    if matches!(
        err.kind(),
        ErrorKind::BrokenPipe | ErrorKind::ConnectionReset
    ) {
        return true;
    }
    #[cfg(unix)]
    if err.raw_os_error() == Some(libc::EIO) {
        return true;
    }
    false
}

impl<T: AsyncReadExt + AsyncWriteExt + Unpin + Send> Session<T> {
    /// Create a new session with the given transport.
    pub fn new(transport: T, config: SessionConfig) -> Self {
        let buffer_size = config.buffer.max_size;
        let mut matcher = Matcher::new(buffer_size);
        matcher.set_default_timeout(config.timeout.default);
        Self {
            transport: Arc::new(Mutex::new(transport)),
            config,
            matcher,
            pattern_manager: PatternManager::new(),
            state: SessionState::Starting,
            id: SessionId::new(),
            eof: false,
            output_taps: Vec::new(),
            next_tap_id: 0,
            #[cfg(feature = "screen")]
            screen: None,
            #[cfg(feature = "screen")]
            screen_tap_id: None,
            #[cfg(feature = "screen")]
            screen_poll_interval: Duration::from_millis(50),
        }
    }

    /// Set the polling interval used by the screen-aware expect helpers.
    ///
    /// Affects `expect_screen_contains`, `wait_screen_not_contains`, and
    /// `wait_screen_stable`. Smaller values reduce match latency at the
    /// cost of CPU; larger values do the opposite. Default is 50 ms.
    ///
    /// Available with the `screen` feature.
    #[cfg(feature = "screen")]
    pub const fn set_screen_poll_interval(&mut self, interval: Duration) {
        self.screen_poll_interval = interval;
    }

    /// Get the current screen-poll interval. Default 50 ms.
    ///
    /// Available with the `screen` feature.
    #[cfg(feature = "screen")]
    #[must_use]
    pub const fn screen_poll_interval(&self) -> Duration {
        self.screen_poll_interval
    }

    /// Register a callback that will be invoked with every chunk of bytes
    /// read from the transport.
    ///
    /// Taps observe the raw byte stream as it arrives — they receive bytes
    /// in the same form the underlying process produced them, including any
    /// ANSI escape sequences. Taps are invoked synchronously inside the read
    /// loop after the bytes are appended to the matcher buffer; they should
    /// be cheap and non-blocking. Use a channel if expensive work is required.
    ///
    /// Multiple taps may be registered; they are invoked in registration
    /// order. Taps are dropped when the session is dropped.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use std::sync::Arc;
    /// use std::sync::Mutex;
    /// let captured: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    /// let buf = captured.clone();
    /// session.add_output_tap(move |chunk| {
    ///     buf.lock().unwrap().extend_from_slice(chunk);
    /// });
    /// ```
    pub fn add_output_tap<F>(&mut self, f: F) -> TapId
    where
        F: Fn(&[u8]) + Send + Sync + 'static,
    {
        let id = TapId(self.next_tap_id);
        // Plain addition (not wrapping_add): on the astronomically unlikely
        // event of u64 exhaustion on a single session, we'd rather panic
        // loudly than silently issue a colliding id.
        self.next_tap_id += 1;
        self.output_taps.push((id, Arc::new(f)));
        id
    }

    /// Remove a previously registered output tap by its [`TapId`]. Returns
    /// `true` if a tap was removed, `false` if the id was not registered
    /// (already removed, or never existed).
    pub fn remove_output_tap(&mut self, id: TapId) -> bool {
        let len_before = self.output_taps.len();
        self.output_taps.retain(|(existing, _)| *existing != id);
        self.output_taps.len() != len_before
    }

    /// Iterate the callbacks for all currently registered output taps.
    ///
    /// Exposed for instrumentation and inspection only — the read loops in
    /// [`expect`](Self::expect) and [`interact`](Self::interact) invoke
    /// these themselves. Returns the callback `Arc`s in registration
    /// order; ids are intentionally omitted (use
    /// [`add_output_tap`](Self::add_output_tap)'s return value if you
    /// need the id).
    pub fn output_tap_callbacks(&self) -> impl Iterator<Item = &OutputTap> {
        self.output_taps.iter().map(|(_, cb)| cb)
    }

    /// Attach a virtual terminal screen to this session.
    ///
    /// Creates a [`Screen`](crate::screen::Screen) with the session's
    /// configured dimensions and registers an output tap that feeds every
    /// chunk of output into the screen's ANSI parser. The screen is then
    /// accessible via [`screen()`](Self::screen) and is automatically updated
    /// whenever output is read from the transport (i.e. inside `expect_*`,
    /// `wait`, or `wait_screen_stable`).
    ///
    /// Repeated calls replace the previous screen.
    ///
    /// Available with the `screen` feature.
    #[cfg(feature = "screen")]
    pub fn attach_screen(&mut self) {
        let (cols, rows) = self.config.dimensions;
        self.attach_screen_with_dims(rows, cols);
    }

    /// Attach a screen with custom dimensions.
    ///
    /// `rows` and `cols` are the screen size in cells. Note that this does
    /// not resize the PTY itself — use [`resize_pty`](Self::resize_pty) for
    /// that. The two should normally match, but it can be useful to set a
    /// larger virtual screen for transcript capture.
    ///
    /// Available with the `screen` feature.
    #[cfg(feature = "screen")]
    pub fn attach_screen_with_dims(&mut self, rows: u16, cols: u16) {
        // Replace any previous screen + its tap so we don't leak callbacks.
        self.detach_screen();
        let screen = Arc::new(StdMutex::new(Screen::new(rows as usize, cols as usize)));
        let screen_for_tap = screen.clone();
        let id = self.add_output_tap(move |chunk| {
            // Reuse the shared poison-recovery helper so the tap-side and
            // read-side recovery logic stays in lockstep.
            lock_screen(&screen_for_tap).process(chunk);
        });
        self.screen = Some(screen);
        self.screen_tap_id = Some(id);
    }

    /// Attach a screen with a bounded scrollback history, sized to the
    /// session's configured dimensions.
    ///
    /// Rows that scroll off the top are retained (up to `scrollback_lines`)
    /// and readable via the attached [`Screen`](crate::screen::Screen)'s
    /// `scrollback()` / `full_text()`. For lossless capture independent of the
    /// bound, register [`on_screen_line_scrolled_out`](Self::on_screen_line_scrolled_out).
    ///
    /// Available with the `screen` feature.
    #[cfg(feature = "screen")]
    pub fn attach_screen_with_scrollback(&mut self, scrollback_lines: usize) {
        let (cols, rows) = self.config.dimensions;
        // Replace any previous screen + its tap so we don't leak callbacks.
        self.detach_screen();
        let screen = Arc::new(StdMutex::new(Screen::with_scrollback(
            rows as usize,
            cols as usize,
            scrollback_lines,
        )));
        let screen_for_tap = screen.clone();
        let id = self.add_output_tap(move |chunk| {
            lock_screen(&screen_for_tap).process(chunk);
        });
        self.screen = Some(screen);
        self.screen_tap_id = Some(id);
    }

    /// Register a callback fired for each row that scrolls off the attached
    /// screen, delivered as the row finalizes. Returns `false` if no screen is
    /// attached.
    ///
    /// See [`Screen::on_line_scrolled_out`](crate::screen::Screen::on_line_scrolled_out)
    /// for the reentrancy contract: the callback runs while the screen lock is
    /// held and must not re-enter the `Session`/`Screen`.
    ///
    /// Available with the `screen` feature.
    #[cfg(feature = "screen")]
    pub fn on_screen_line_scrolled_out<F>(&mut self, callback: F) -> bool
    where
        F: FnMut(&crate::screen::Row) + Send + 'static,
    {
        if let Some(screen) = self.screen.as_ref() {
            lock_screen(screen).on_line_scrolled_out(callback);
            true
        } else {
            false
        }
    }

    /// Detach the currently attached screen, also removing its output tap.
    /// No-op if no screen is attached. Returns `true` if a screen was
    /// detached.
    ///
    /// Available with the `screen` feature.
    #[cfg(feature = "screen")]
    pub fn detach_screen(&mut self) -> bool {
        if let Some(id) = self.screen_tap_id.take() {
            self.remove_output_tap(id);
        }
        self.screen.take().is_some()
    }

    /// Get the attached virtual terminal screen, if any.
    ///
    /// Returns a shared handle protected by a [`std::sync::Mutex`]. Lock it
    /// briefly to read screen state — the lock is also taken by the output
    /// tap on every read, so holding it for long stretches blocks the read
    /// loop.
    ///
    /// Available with the `screen` feature.
    #[cfg(feature = "screen")]
    #[must_use]
    pub const fn screen(&self) -> Option<&Arc<StdMutex<Screen>>> {
        self.screen.as_ref()
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
    pub const fn pattern_manager_mut(&mut self) -> &mut PatternManager {
        &mut self.pattern_manager
    }

    /// Set the session state.
    pub const fn set_state(&mut self, state: SessionState) {
        self.state = state;
    }

    /// Send bytes to the process.
    ///
    /// # Errors
    ///
    /// Returns [`ExpectError::SessionClosed`] if the child has already exited
    /// (so the write would go to a dead PTY), or an I/O error if the write
    /// otherwise fails.
    #[allow(clippy::significant_drop_tightening)]
    pub async fn send(&mut self, data: &[u8]) -> Result<()> {
        if matches!(self.state, SessionState::Closed | SessionState::Exited(_)) {
            return Err(ExpectError::SessionClosed);
        }

        // Perform the write under the lock, then release it before touching
        // session state so the error-handling path can take `&mut self`.
        let result = {
            let mut transport = self.transport.lock().await;
            match transport.write_all(data).await {
                Ok(()) => transport.flush().await,
                Err(e) => Err(e),
            }
        };

        match result {
            Ok(()) => Ok(()),
            // A write to an already-exited child's PTY fails once the slave end
            // closes (EIO on Unix, BrokenPipe on Windows). Surface that as a
            // clean SessionClosed rather than a raw OS error, and mark the
            // session closed so subsequent sends short-circuit immediately.
            Err(e) if write_error_means_closed(&e) => {
                self.state = SessionState::Closed;
                Err(ExpectError::SessionClosed)
            }
            Err(e) => Err(ExpectError::io_context("writing to process", e)),
        }
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

    /// Send a Shift+Tab keystroke.
    ///
    /// Sends the xterm "back tab" sequence `\x1b[Z` (CSI Z). Most TUIs use
    /// this to cycle a focused-element ring backwards or, in Claude Code's
    /// case, to cycle permission modes. Compatible with both plain xterm
    /// and the kitty keyboard protocol's CSI-u fallback mode.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub async fn send_shift_tab(&mut self) -> Result<()> {
        self.send(b"\x1b[Z").await
    }

    /// Send text using bracketed paste mode (DECSET 2004).
    ///
    /// Wraps the content in `\x1b[200~` and `\x1b[201~` markers. Applications
    /// that have enabled bracketed paste treat the enclosed content as
    /// pasted input rather than typed input — this suppresses autocomplete,
    /// command-history scanning, and per-character interpretation such as a
    /// leading `/` triggering a slash-command popup. Safe to call even when
    /// the receiver hasn't enabled bracketed paste: most terminals ignore
    /// the markers and deliver the inner text as-is.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails or if `text` contains the
    /// closing paste marker `\x1b[201~`, which would let the receiver drop
    /// out of paste mode mid-payload. Callers that want to send such bytes
    /// should write them through the regular [`send`](Self::send) path.
    pub async fn send_paste(&mut self, text: &str) -> Result<()> {
        if memchr::memmem::find(text.as_bytes(), b"\x1b[201~").is_some() {
            return Err(ExpectError::InvalidInput {
                api: "send_paste".to_string(),
                reason:
                    "input contains the bracketed-paste end marker (\\x1b[201~); use send() for raw bytes that include this sequence"
                        .to_string(),
            });
        }
        self.send(b"\x1b[200~").await?;
        self.send(text.as_bytes()).await?;
        self.send(b"\x1b[201~").await
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
            if let Some((_, action)) = self
                .pattern_manager
                .check_before(&self.matcher.buffer_str())
            {
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
                    return Ok(Match::new(
                        0,
                        String::new(),
                        self.matcher.buffer_str(),
                        String::new(),
                    ));
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
    pub async fn expect_timeout(
        &mut self,
        pattern: impl Into<Pattern>,
        timeout: Duration,
    ) -> Result<Match> {
        let pattern = pattern.into();
        let mut patterns = PatternSet::new();
        patterns.add(pattern).add(Pattern::timeout(timeout));
        self.expect_any(&patterns).await
    }

    /// Wait until the attached screen contains the given substring.
    ///
    /// Drives reads from the transport in short increments, checking the
    /// rendered screen text after each. Returns successfully as soon as
    /// `needle` appears in the screen text, or with [`ExpectError::Timeout`]
    /// when `timeout` elapses without a match. Returns [`ExpectError::Eof`]
    /// if the process exits before the substring appears.
    ///
    /// This is the screen-aware counterpart to [`expect`](Self::expect): use
    /// it when the byte stream is full of ANSI escape sequences (e.g. when
    /// driving a TUI), where literal substring matching on the byte stream
    /// would fail because of interleaved cursor positioning and SGR codes.
    ///
    /// Requires an attached screen — call [`attach_screen`](Self::attach_screen)
    /// first.
    ///
    /// # Errors
    ///
    /// Returns an error if no screen is attached, the timeout expires, EOF
    /// is reached, or an I/O error occurs.
    ///
    /// Available with the `screen` feature.
    #[cfg(feature = "screen")]
    pub async fn expect_screen_contains(&mut self, needle: &str, timeout: Duration) -> Result<()> {
        let Some(screen) = self.screen.clone() else {
            return Err(ExpectError::ScreenNotAttached);
        };

        let start = tokio::time::Instant::now();
        let poll = self.screen_poll_interval;

        loop {
            if lock_screen(&screen).query().contains(needle) {
                return Ok(());
            }
            if self.eof {
                return Err(ExpectError::Eof {
                    buffer: lock_screen(&screen).text(),
                });
            }
            let elapsed = start.elapsed();
            if elapsed >= timeout {
                return Err(ExpectError::Timeout {
                    duration: timeout,
                    pattern: needle.to_string(),
                    buffer: lock_screen(&screen).text(),
                });
            }
            let remaining = timeout.saturating_sub(elapsed);
            self.read_with_timeout(poll.min(remaining)).await?;
        }
    }

    /// Wait until the attached screen no longer contains the given substring.
    ///
    /// The inverse of [`expect_screen_contains`](Self::expect_screen_contains).
    /// Returns successfully as soon as `needle` is absent from the rendered
    /// screen, or with [`ExpectError::Timeout`] when `timeout` elapses with
    /// the substring still present. EOF is treated as "absent" (the screen
    /// state is frozen at the final paint).
    ///
    /// Useful for anchoring on the *disappearance* of an indicator —
    /// e.g. waiting for a "request in flight" status to clear, a spinner
    /// glyph to stop, or a modal to close.
    ///
    /// Requires an attached screen.
    ///
    /// # Errors
    ///
    /// Returns an error if no screen is attached, the timeout expires while
    /// the substring is still visible, or an I/O error occurs.
    ///
    /// Available with the `screen` feature.
    #[cfg(feature = "screen")]
    pub async fn wait_screen_not_contains(
        &mut self,
        needle: &str,
        timeout: Duration,
    ) -> Result<()> {
        let Some(screen) = self.screen.clone() else {
            return Err(ExpectError::ScreenNotAttached);
        };

        let start = tokio::time::Instant::now();
        let poll = self.screen_poll_interval;

        loop {
            if !lock_screen(&screen).query().contains(needle) {
                return Ok(());
            }
            if self.eof {
                return Ok(());
            }
            let elapsed = start.elapsed();
            if elapsed >= timeout {
                return Err(ExpectError::Timeout {
                    duration: timeout,
                    pattern: format!("!{needle}"),
                    buffer: lock_screen(&screen).text(),
                });
            }
            let remaining = timeout.saturating_sub(elapsed);
            self.read_with_timeout(poll.min(remaining)).await?;
        }
    }

    /// Wait until the attached screen has been unchanged for `quiet_period`.
    ///
    /// Drives reads in short increments and tracks whether the rendered
    /// screen text changes between reads. Returns successfully when the
    /// screen has been quiescent for `quiet_period`, or with
    /// [`ExpectError::Timeout`] if `max_wait` elapses first.
    ///
    /// Useful as a generic "wait for the TUI to finish drawing" primitive
    /// when no specific anchor is available — for example, after submitting
    /// a prompt and before reading the response.
    ///
    /// A small `quiet_period` (e.g. 100-300 ms) catches paint completion;
    /// a larger one (1-2 s) waits out streaming responses with mid-stream
    /// pauses. Tune to the specific application.
    ///
    /// Requires an attached screen.
    ///
    /// # Errors
    ///
    /// Returns an error if no screen is attached, `max_wait` elapses, or an
    /// I/O error occurs. EOF is **not** an error — if the process exits, the
    /// final screen state is considered stable and the method returns Ok.
    ///
    /// Available with the `screen` feature.
    #[cfg(feature = "screen")]
    pub async fn wait_screen_stable(
        &mut self,
        quiet_period: Duration,
        max_wait: Duration,
    ) -> Result<()> {
        let Some(screen) = self.screen.clone() else {
            return Err(ExpectError::ScreenNotAttached);
        };

        let start = tokio::time::Instant::now();
        let poll = self.screen_poll_interval;
        let mut last_revision = lock_screen(&screen).revision();
        let mut last_change = tokio::time::Instant::now();

        loop {
            if last_change.elapsed() >= quiet_period {
                return Ok(());
            }
            if self.eof {
                return Ok(());
            }
            if start.elapsed() >= max_wait {
                return Err(ExpectError::Timeout {
                    duration: max_wait,
                    pattern: "<screen stability>".to_string(),
                    buffer: lock_screen(&screen).text(),
                });
            }
            self.read_with_timeout(poll).await?;
            let current_revision = lock_screen(&screen).revision();
            if current_revision != last_revision {
                last_revision = current_revision;
                last_change = tokio::time::Instant::now();
            }
        }
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
                // Run taps in catch_unwind so a panicking user callback can't
                // unwind across our await boundary or poison subsequent taps.
                // We log and continue rather than propagate — taps are
                // observers, not error sources.
                for (id, tap) in &self.output_taps {
                    let tap_clone = tap.clone();
                    let chunk = &buf[..n];
                    let result =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| tap_clone(chunk)));
                    if result.is_err() {
                        tracing::warn!(
                            %id,
                            "output tap panicked; the panic was caught and other taps continue"
                        );
                    }
                }
                Ok(n)
            }
            Ok(Err(e)) => {
                // On Linux, reading from PTY master returns EIO when the slave is closed
                // (i.e., the child process has terminated). Treat this as EOF.
                // See: https://bugs.python.org/issue5380
                if is_pty_eof_error(&e) {
                    self.eof = true;
                    Ok(0)
                } else {
                    Err(ExpectError::io_context("reading from process", e))
                }
            }
            Err(_) => {
                // Timeout, but not an error - caller will handle
                Ok(0)
            }
        }
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
    /// ```no_run
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
    #[must_use]
    pub fn interact(&self) -> InteractBuilder<'_, T>
    where
        T: 'static,
    {
        // Snapshot the currently registered output taps so the interact
        // read loop can fire them on every chunk. Without this, attached
        // screens and transcript recorders would silently freeze for the
        // duration of interact().
        let taps: Vec<OutputTap> = self
            .output_taps
            .iter()
            .map(|(_, tap)| tap.clone())
            .collect();
        InteractBuilder::new(&self.transport, taps)
    }

    /// Run a dialog on this session.
    ///
    /// A dialog is a predefined sequence of expect/send operations.
    /// This method executes the dialog and returns the result.
    ///
    /// # Example
    ///
    /// ```no_run
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
    /// ```no_run
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

    /// Run a batch of commands, waiting for the prompt after each.
    ///
    /// This is a convenience method for executing multiple shell commands
    /// in sequence. For each command, it sends the command line and waits
    /// for the prompt pattern to appear.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rust_expect::{Session, Pattern};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), rust_expect::ExpectError> {
    ///     let mut session = Session::spawn("/bin/bash", &[]).await?;
    ///     session.expect(Pattern::shell_prompt()).await?;
    ///
    ///     // Run a batch of commands
    ///     let results = session.run_script(
    ///         &["pwd", "whoami", "date"],
    ///         Pattern::shell_prompt(),
    ///     ).await?;
    ///
    ///     for result in &results {
    ///         println!("Output: {}", result.before.trim());
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if any command times out or I/O fails.
    /// On error, partial results are lost; consider using [`Self::run_script_with_results`]
    /// if you need to capture partial results on failure.
    pub async fn run_script<I, S>(&mut self, commands: I, prompt: Pattern) -> Result<Vec<Match>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut results = Vec::new();

        for cmd in commands {
            self.send_line(cmd.as_ref()).await?;
            let result = self.expect(prompt.clone()).await?;
            results.push(result);
        }

        Ok(results)
    }

    /// Run a batch of commands with a specific timeout per command.
    ///
    /// Like [`run_script`](Self::run_script), but applies the given timeout
    /// to each command individually.
    ///
    /// # Errors
    ///
    /// Returns an error if any command times out or I/O fails.
    pub async fn run_script_timeout<I, S>(
        &mut self,
        commands: I,
        prompt: Pattern,
        timeout: Duration,
    ) -> Result<Vec<Match>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut results = Vec::new();

        for cmd in commands {
            self.send_line(cmd.as_ref()).await?;
            let result = self.expect_timeout(prompt.clone(), timeout).await?;
            results.push(result);
        }

        Ok(results)
    }

    /// Run a batch of commands, collecting results even on failure.
    ///
    /// Unlike [`run_script`](Self::run_script), this method continues
    /// collecting results and returns them along with any error that occurred.
    ///
    /// # Returns
    ///
    /// A tuple of `(results, error)` where:
    /// - `results` contains the matches for successfully completed commands
    /// - `error` is `Some(err)` if an error occurred, `None` if all commands succeeded
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rust_expect::{Session, Pattern};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), rust_expect::ExpectError> {
    ///     let mut session = Session::spawn("/bin/bash", &[]).await?;
    ///     session.expect(Pattern::shell_prompt()).await?;
    ///
    ///     let (results, error) = session.run_script_with_results(
    ///         &["pwd", "bad_command", "date"],
    ///         Pattern::shell_prompt(),
    ///     ).await;
    ///
    ///     println!("Completed {} commands", results.len());
    ///     if let Some(e) = error {
    ///         eprintln!("Script failed at command {}: {}", results.len(), e);
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn run_script_with_results<I, S>(
        &mut self,
        commands: I,
        prompt: Pattern,
    ) -> (Vec<Match>, Option<ExpectError>)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut results = Vec::new();

        for cmd in commands {
            match self.send_line(cmd.as_ref()).await {
                Ok(()) => {}
                Err(e) => return (results, Some(e)),
            }

            match self.expect(prompt.clone()).await {
                Ok(result) => results.push(result),
                Err(e) => return (results, Some(e)),
            }
        }

        (results, None)
    }
}

/// Process-lifecycle methods available when the transport can report a child's
/// exit status (PTY-backed sessions). Transports without a child process use the
/// default [`ChildExit`] impl and report [`ProcessExitStatus::Unknown`].
impl<T: AsyncReadExt + AsyncWriteExt + Unpin + Send + ChildExit> Session<T> {
    /// Wait for the process to exit.
    ///
    /// Blocks until EOF is detected on the session — which happens when the
    /// child closes the slave end of the PTY, i.e. when it terminates — and
    /// then reaps the child to report its real exit status.
    ///
    /// # Warning
    ///
    /// This method has no timeout and may block indefinitely if the process
    /// does not exit. Consider using [`wait_timeout`](Self::wait_timeout) or
    /// [`expect_eof_timeout`](Self::expect_eof_timeout) for bounded waits.
    ///
    /// # Errors
    ///
    /// Returns an error if waiting fails due to I/O error.
    pub async fn wait(&mut self) -> Result<ProcessExitStatus> {
        // Read until EOF (child closed the PTY slave / terminated).
        while !self.eof {
            if self.read_with_timeout(Duration::from_millis(100)).await? == 0 && !self.eof {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }

        let status = self.reap_exit_status().await;
        self.state = SessionState::Exited(status);
        Ok(status)
    }

    /// Wait for the process to exit with a timeout.
    ///
    /// Like [`wait`](Self::wait), but with a maximum duration to wait.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The timeout expires before the process exits
    /// - An I/O error occurs while waiting
    pub async fn wait_timeout(&mut self, timeout: Duration) -> Result<ProcessExitStatus> {
        let deadline = tokio::time::Instant::now() + timeout;

        while !self.eof {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(ExpectError::timeout(
                    timeout,
                    "<EOF>",
                    self.matcher.buffer_str(),
                ));
            }

            // Use smaller of remaining time or 100ms for polling
            let poll_timeout = remaining.min(Duration::from_millis(100));
            if self.read_with_timeout(poll_timeout).await? == 0 && !self.eof {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }

        let status = self.reap_exit_status().await;
        self.state = SessionState::Exited(status);
        Ok(status)
    }

    /// Reap the child's real exit status after EOF has been observed.
    ///
    /// EOF means the child closed the PTY slave, so it has exited or is about
    /// to. Poll the transport's non-blocking reap briefly to collect the real
    /// `Exited`/`Signaled` status, falling back to [`ProcessExitStatus::Unknown`]
    /// (the historical return) rather than blocking — e.g. for a non-process
    /// transport, or a child that closed its output but lingers before exiting.
    async fn reap_exit_status(&self) -> ProcessExitStatus {
        // ~100ms ceiling (20 × 5ms); the common case resolves on the first poll.
        const ATTEMPTS: u32 = 20;
        for _ in 0..ATTEMPTS {
            // Lock released at the end of this statement, before the sleep.
            let status = self.transport.lock().await.try_exit_status();
            if let Some(status) = status {
                return status;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        ProcessExitStatus::Unknown
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
    /// ```no_run
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
    /// Also resizes the attached screen (if any) so it stays consistent
    /// with the PTY. Without this, screen-aware assertions would drift
    /// after a resize.
    ///
    /// # Errors
    ///
    /// Returns an error if the resize ioctl fails.
    pub async fn resize_pty(&mut self, cols: u16, rows: u16) -> Result<()> {
        {
            let mut transport = self.transport.lock().await;
            transport.resize(cols, rows)?;
        }
        self.config.dimensions = (cols, rows);
        #[cfg(feature = "screen")]
        if let Some(screen) = self.screen.as_ref()
            && let Ok(mut s) = screen.lock()
        {
            s.resize(rows as usize, cols as usize);
        }
        Ok(())
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

    /// Check whether the child process is still running.
    ///
    /// Performs a non-blocking `waitpid(WNOHANG)` peek, so it reports the truth
    /// immediately after the child exits. The portable counterpart of
    /// [`Session::<WindowsAsyncPty>::is_running`].
    #[must_use]
    pub fn is_running(&self) -> bool {
        if let Ok(mut transport) = self.transport.try_lock() {
            transport.is_running()
        } else {
            true // Assume running if we can't check
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
    /// ```no_run
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
    fn resize(
        &mut self,
        dimensions: Dimensions,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
}

/// Check if an I/O error indicates PTY EOF.
///
/// On Linux, reading from the PTY master returns EIO when the slave side
/// has been closed (i.e., the child process has terminated). This is different
/// from the standard EOF behavior where `read()` returns 0 bytes.
///
/// This function returns true for errors that should be treated as EOF:
/// - EIO (errno 5) on Unix systems
/// - `BrokenPipe` on any platform
fn is_pty_eof_error(e: &std::io::Error) -> bool {
    use std::io::ErrorKind;

    // BrokenPipe indicates the other end has closed
    if e.kind() == ErrorKind::BrokenPipe {
        return true;
    }

    // On Unix, check for EIO which indicates slave PTY closed
    #[cfg(unix)]
    {
        if let Some(errno) = e.raw_os_error() {
            // EIO is 5 on Linux/macOS/BSD
            if errno == libc::EIO {
                return true;
            }
        }
    }

    false
}
