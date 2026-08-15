//! Session handle for interacting with spawned processes.
//!
//! This module provides the main `Session` type that users interact with
//! to control spawned processes, send input, and expect output.

use std::sync::Arc;
#[cfg(any(feature = "screen", feature = "pii-redaction"))]
use std::sync::{Mutex as StdMutex, MutexGuard};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[cfg(unix)]
use crate::backend::{AsyncPty, PtyConfig, PtySpawner};
use crate::backend::{ChildExit, ProcessControl, ProcessHandle, Resizable};
#[cfg(windows)]
use crate::backend::{PtyConfig, PtySpawner, WindowsAsyncPty};
use crate::config::SessionConfig;
use crate::dialog::{Dialog, DialogExecutor, DialogResult};
use crate::error::{ExpectError, Result};
use crate::expect::{
    ExpectState, HandlerAction, MatchResult, Matcher, Pattern, PatternManager, PatternSet,
};
use crate::interact::InteractBuilder;
use crate::metrics::SessionMetrics;
#[cfg(feature = "pii-redaction")]
use crate::pii::StreamingRedactor;
#[cfg(feature = "screen")]
use crate::screen::Screen;
pub use crate::session::events::{OutputTap, TapId};
use crate::session::events::{SessionEvent, Subscribers};
use crate::session::state::SessionLifecycle;
use crate::session::transport::SharedTransport;
use crate::transcript::Recorder;
use crate::types::{ControlChar, Dimensions, Match, ProcessExitStatus, SessionId, SessionState};

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

/// Lock a redactor, recovering from poisoning as [`lock_screen`] does: a panic
/// in one subscriber must not silently stop redacting the transcript.
#[cfg(feature = "pii-redaction")]
fn lock_redactor(redactor: &Arc<StdMutex<StreamingRedactor>>) -> MutexGuard<'_, StreamingRedactor> {
    match redactor.lock() {
        Ok(g) => g,
        Err(poison) => {
            tracing::warn!("redactor mutex was poisoned; recovering inner state");
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
    ///
    /// Held behind a per-poll lock rather than one taken across the read's
    /// await, so a parked read leaves the session writable. See
    /// [`SharedTransport`].
    transport: SharedTransport<T>,
    /// Control over the child process, held separately from the transport so
    /// that killing or signalling never waits on a read.
    ///
    /// `None` for transports with no local child process — mock streams, SSH
    /// channels, duplex pipes — whose process-control calls report
    /// [`ExpectError::Unsupported`].
    control: Option<ProcessHandle>,
    /// Session configuration.
    config: SessionConfig,
    /// Pattern matcher.
    matcher: Matcher,
    /// Pattern manager for before/after patterns.
    pattern_manager: PatternManager,
    /// Session state and the transitions between states.
    ///
    /// The single writer: state and EOF used to be two independent fields
    /// updated at separate call sites, which let them disagree. See
    /// [`SessionLifecycle`].
    lifecycle: SessionLifecycle,
    /// Unique session identifier.
    id: SessionId,
    /// Registered output taps and event subscribers, in registration order.
    subscribers: Subscribers,
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
    ///
    /// The session has no process control: [`signal`](Self::signal),
    /// [`kill`](Self::kill) and friends report
    /// [`ExpectError::Unsupported`] until one is attached with
    /// [`with_process_control`](Self::with_process_control). `Session::spawn`
    /// attaches one for you.
    pub fn new(transport: T, config: SessionConfig) -> Self {
        let buffer_size = config.buffer.max_size;
        let mut matcher = Matcher::new(buffer_size);
        matcher.set_default_timeout(config.timeout.default);
        Self {
            transport: SharedTransport::new(transport),
            control: None,
            config,
            matcher,
            pattern_manager: PatternManager::new(),
            lifecycle: SessionLifecycle::new(),
            id: SessionId::new(),
            subscribers: Subscribers::new(),
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
        self.subscribers.add_tap(Arc::new(f))
    }

    /// Register a callback invoked for every [`SessionEvent`] this session
    /// emits.
    ///
    /// The general form of [`add_output_tap`](Self::add_output_tap): a tap sees
    /// only [`SessionEvent::Output`], while a subscriber also sees input,
    /// resizes, state transitions and read errors. Input in particular has no
    /// other observation point — it is what makes transcript recording of what
    /// was *sent* possible.
    ///
    /// Subscribers are invoked synchronously in registration order, sharing one
    /// order with output taps, at the moment the event occurs. They should be
    /// cheap and non-blocking; use a channel if expensive work is required. A
    /// panicking subscriber is caught and logged, and the rest still run.
    ///
    /// Remove with [`remove_output_tap`](Self::remove_output_tap), which
    /// accepts the returned id.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use rust_expect::SessionEvent;
    /// session.add_event_subscriber(|event| match event {
    ///     SessionEvent::Input(bytes) => eprintln!("sent {} bytes", bytes.len()),
    ///     SessionEvent::StateChanged { from, to } => eprintln!("{from} -> {to}"),
    ///     _ => {}
    /// });
    /// ```
    pub fn add_event_subscriber<F>(&mut self, f: F) -> TapId
    where
        F: Fn(&SessionEvent<'_>) + Send + Sync + 'static,
    {
        self.subscribers.add_subscriber(Arc::new(f))
    }

    /// Emit an event to every registered tap and subscriber.
    fn emit(&self, event: &SessionEvent<'_>) {
        self.subscribers.emit(event);
    }

    /// Apply a state transition and emit [`SessionEvent::StateChanged`] if it
    /// changed anything.
    ///
    /// Every transition goes through here, so no state change can reach a
    /// caller without also reaching subscribers.
    fn transition(&mut self, apply: impl FnOnce(&mut SessionLifecycle)) {
        let from = self.lifecycle.state();
        apply(&mut self.lifecycle);
        let to = self.lifecycle.state();
        if from != to {
            self.emit(&SessionEvent::StateChanged { from, to });
        }
    }

    /// Remove a previously registered output tap by its [`TapId`]. Returns
    /// `true` if a tap was removed, `false` if the id was not registered
    /// (already removed, or never existed).
    pub fn remove_output_tap(&mut self, id: TapId) -> bool {
        self.subscribers.remove(id)
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
        self.subscribers.taps()
    }

    /// How many observers are registered on this session, counting output taps
    /// and event subscribers alike.
    ///
    /// [`output_tap_callbacks`](Self::output_tap_callbacks) counts only taps,
    /// so it does not see internally-registered subscribers such as the one
    /// [`attach_screen`](Self::attach_screen) installs. This does. Exposed for
    /// instrumentation and for checking that a subsystem cleaned up after
    /// itself.
    #[must_use]
    pub const fn observer_count(&self) -> usize {
        self.subscribers.len()
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

    /// Record this session to a transcript.
    ///
    /// The recorder observes output, input and resizes — the three things it
    /// already knows how to record. Input in particular had no route to it
    /// before the event stream existed: `send()` had no hook, so
    /// `Recorder::record_input` was unreachable from a session even though the
    /// recorder implemented it.
    ///
    /// The returned [`TapId`] removes the recorder via
    /// [`remove_output_tap`](Self::remove_output_tap). The caller keeps the
    /// `Arc` and reads the result with [`Recorder::transcript`].
    ///
    /// Records what the child actually sent. To keep secrets out of the
    /// transcript, use [`attach_redacted_recorder`](Self::attach_redacted_recorder).
    pub fn attach_recorder(&mut self, recorder: &Arc<Recorder>) -> TapId {
        let recorder = Arc::clone(recorder);
        self.add_event_subscriber(move |event| match event {
            SessionEvent::Output(chunk) => recorder.record_output(chunk),
            SessionEvent::Input(chunk) => recorder.record_input(chunk),
            SessionEvent::Resize { cols, rows } => recorder.record_resize(*cols, *rows),
            _ => {}
        })
    }

    /// Record this session to a transcript, redacting PII on the way in.
    ///
    /// # Where redaction sits
    ///
    /// Between the event stream and the transcript, and **nowhere else**. The
    /// matcher has already seen the raw bytes by the time subscribers run, so
    /// redaction here cannot affect what [`expect`](Self::expect) matches: a
    /// caller expecting on a password prompt still matches it, and the password
    /// still does not reach the transcript. That ordering is the whole point,
    /// and it is structural rather than documented — this method has no way to
    /// install a redactor anywhere else.
    ///
    /// [`StreamingRedactor`] holds back a partial trailing line so a secret
    /// split across two reads is still caught. The remainder is flushed when
    /// the session reaches a terminal state, so nothing is left buffered.
    ///
    /// Available with the `pii-redaction` feature.
    #[cfg(feature = "pii-redaction")]
    pub fn attach_redacted_recorder(
        &mut self,
        recorder: &Arc<Recorder>,
        redactor: StreamingRedactor,
    ) -> TapId {
        let recorder = Arc::clone(recorder);
        let redactor = Arc::new(StdMutex::new(redactor));
        self.add_event_subscriber(move |event| match event {
            SessionEvent::Output(chunk) => {
                let safe = lock_redactor(&redactor).process(&String::from_utf8_lossy(chunk));
                if !safe.is_empty() {
                    recorder.record_output(safe.as_bytes());
                }
            }
            SessionEvent::Input(chunk) => {
                // Input is redacted with the same detector but not buffered
                // across events: a keystroke chunk is not a stream, and holding
                // input back would misorder it against the output it produced.
                let safe = lock_redactor(&redactor)
                    .redactor()
                    .redact(&String::from_utf8_lossy(chunk));
                recorder.record_input(safe.as_bytes());
            }
            SessionEvent::Resize { cols, rows } => recorder.record_resize(*cols, *rows),
            // No more output is coming, so nothing may stay buffered.
            SessionEvent::StateChanged {
                to:
                    SessionState::Eof
                    | SessionState::Exited(_)
                    | SessionState::Closed
                    | SessionState::Failed(_),
                ..
            } => {
                let tail = lock_redactor(&redactor).flush();
                if !tail.is_empty() {
                    recorder.record_output(tail.as_bytes());
                }
            }
            _ => {}
        })
    }

    /// Feed session metrics from the event stream.
    ///
    /// Wires the counters the stream can honestly supply: bytes in and out,
    /// pattern matches, and errors. `timeouts` and the duration histograms are
    /// not fed — a timeout is the *absence* of an event, and the durations
    /// belong to whoever is timing the operation.
    ///
    /// The returned [`TapId`] removes it via
    /// [`remove_output_tap`](Self::remove_output_tap).
    pub fn attach_metrics(&mut self, metrics: &Arc<SessionMetrics>) -> TapId {
        let metrics = Arc::clone(metrics);
        self.add_event_subscriber(move |event| match event {
            SessionEvent::Output(chunk) => metrics.bytes_received.add(chunk.len() as u64),
            SessionEvent::Input(chunk) => metrics.bytes_sent.add(chunk.len() as u64),
            SessionEvent::Matched { .. } => metrics.pattern_matches.inc(),
            SessionEvent::Error(_) => metrics.errors.inc(),
            _ => {}
        })
    }

    /// Attach a screen with custom dimensions.
    ///
    /// `rows` and `cols` are the screen size in cells. Note that this does
    /// not resize the PTY itself — use [`resize_pty`](Self::resize_pty) for
    /// that. The two should normally match, but it can be useful to set a
    /// larger virtual screen for transcript capture.
    ///
    /// # Argument order
    ///
    /// This takes **`(rows, cols)`** — height first — which is the opposite of
    /// [`SessionBuilder::dimensions`](crate::SessionBuilder::dimensions) and
    /// [`resize_pty`](Self::resize_pty), both of which take `(cols, rows)`.
    /// The orders are inconsistent for historical reasons; transposing them
    /// silently produces a screen of the wrong shape rather than an error, so
    /// double-check the call site.
    ///
    /// Available with the `screen` feature.
    #[cfg(feature = "screen")]
    pub fn attach_screen_with_dims(&mut self, rows: u16, cols: u16) {
        // Replace any previous screen + its tap so we don't leak callbacks.
        self.detach_screen();
        let screen = Arc::new(StdMutex::new(Screen::new(rows as usize, cols as usize)));
        let id = self.subscribe_screen(&screen);
        self.screen = Some(screen);
        self.screen_tap_id = Some(id);
    }

    /// Subscribe a screen to the session's event stream.
    ///
    /// The screen consumes `Output` (fed to its ANSI parser) and `Resize` (so
    /// it keeps the shape of the terminal it mirrors). It was an output tap
    /// before the event stream existed, which meant `resize_pty` had to poke it
    /// directly on the side; now both arrive by the same route.
    #[cfg(feature = "screen")]
    fn subscribe_screen(&mut self, screen: &Arc<StdMutex<Screen>>) -> TapId {
        let screen = screen.clone();
        self.add_event_subscriber(move |event| match event {
            // Reuse the shared poison-recovery helper so the subscriber-side
            // and read-side recovery logic stays in lockstep.
            SessionEvent::Output(chunk) => lock_screen(&screen).process(chunk),
            SessionEvent::Resize { cols, rows } => {
                lock_screen(&screen).resize(*rows as usize, *cols as usize);
            }
            _ => {}
        })
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
        let id = self.subscribe_screen(&screen);
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
        self.lifecycle.state()
    }

    /// Get the session configuration.
    #[must_use]
    pub const fn config(&self) -> &SessionConfig {
        &self.config
    }

    /// Check if EOF has been detected.
    #[must_use]
    pub const fn is_eof(&self) -> bool {
        self.lifecycle.is_eof()
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

    /// Send bytes to the process.
    ///
    /// # Errors
    ///
    /// Returns [`ExpectError::SessionClosed`] if the session is no longer
    /// writable — the child has exited, closed its output, or the session
    /// failed — or an I/O error if the write otherwise fails.
    #[allow(clippy::significant_drop_tightening)]
    pub async fn send(&mut self, data: &[u8]) -> Result<()> {
        // Checked against the state machine rather than against the transport,
        // so a write to a child that has closed its output is rejected the same
        // way on every backend. A PTY happens to fail such a write with EIO,
        // but a transport that keeps accepting writes would otherwise swallow
        // it silently.
        if !self.lifecycle.can_send() {
            return Err(ExpectError::SessionClosed);
        }

        // Perform the write under the lock, then release it before touching
        // session state so the error-handling path can take `&mut self`.
        let result = {
            let mut transport = self.transport.clone();
            match transport.write_all(data).await {
                Ok(()) => transport.flush().await,
                Err(e) => Err(e),
            }
        };

        match result {
            Ok(()) => {
                self.emit(&SessionEvent::Input(data));
                Ok(())
            }
            // A write to an already-exited child's PTY fails once the slave end
            // closes (EIO on Unix, BrokenPipe on Windows). Surface that as a
            // clean SessionClosed rather than a raw OS error, and mark the
            // session closed so subsequent sends short-circuit immediately.
            Err(e) if write_error_means_closed(&e) => {
                self.transition(SessionLifecycle::closed);
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
            // Before patterns run before the explicit patterns (highest priority).
            if let Some((_, action, pattern)) = self
                .pattern_manager
                .check_before(&self.matcher.buffer_str())
                && let Some(outcome) = self.apply_ambient_action(action, &pattern).await?
            {
                return Ok(outcome);
            }

            // Check for pattern match
            if let Some(result) = self.matcher.try_match_any(patterns) {
                return Ok(self.consume_and_announce(&result));
            }

            // After patterns run only as a fallback, once the explicit patterns
            // have failed to match on this poll.
            if let Some((_, action, pattern)) =
                self.pattern_manager.check_after(&self.matcher.buffer_str())
                && let Some(outcome) = self.apply_ambient_action(action, &pattern).await?
            {
                return Ok(outcome);
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
            if self.lifecycle.is_eof() {
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

    /// Apply an ambient (before/after) handler action.
    ///
    /// Returns `Some(match)` if the expect operation should return now
    /// (`Return`), or `None` to continue the loop (`Continue`/`Respond`). For
    /// `Respond` and `Return` the triggering match is consumed from the buffer
    /// first, so the ambient pattern cannot re-fire on the next poll or on the
    /// next `expect` call against the same buffer.
    async fn apply_ambient_action(
        &mut self,
        action: HandlerAction,
        pattern: &Pattern,
    ) -> Result<Option<Match>> {
        match action {
            HandlerAction::Continue => Ok(None),
            HandlerAction::Respond(s) => {
                self.consume_ambient(pattern);
                self.send_str(&s).await?;
                Ok(None)
            }
            HandlerAction::Return(s) => {
                // Consume the ambient match, but return the handler's value `s`
                // as the matched string (preserving `Return`'s semantics); take
                // before/after from the consumed match.
                let (before, after) = match self.consume_ambient(pattern) {
                    Some(m) => (m.before, m.after),
                    None => (String::new(), self.matcher.buffer_str()),
                };
                Ok(Some(Match::new(0, s, before, after)))
            }
            HandlerAction::Abort(msg) => Err(ExpectError::PatternNotFound {
                pattern: msg,
                buffer: self.matcher.buffer_str(),
            }),
        }
    }

    /// Consume an ambient pattern's match from the buffer so it can't re-fire.
    ///
    /// Uses the real [`Matcher`] path (search-window offset, `Pattern::Bytes`
    /// handling) rather than a raw offset. Returns the consumed [`Match`] if the
    /// pattern still matches, else `None`.
    fn consume_ambient(&mut self, pattern: &Pattern) -> Option<Match> {
        let result = self.matcher.try_match(pattern)?;
        Some(self.consume_and_announce(&result))
    }

    /// Consume a match from the buffer and tell subscribers it happened.
    ///
    /// Every match a session makes goes through here, so a match cannot reach a
    /// caller without also reaching metrics — the same rule `transition`
    /// enforces for state changes.
    pub(crate) fn consume_and_announce(&mut self, result: &MatchResult) -> Match {
        let matched = self.matcher.consume_match(result);
        self.emit(&SessionEvent::Matched {
            pattern_index: result.pattern_index,
        });
        matched
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
            if self.lifecycle.is_eof() {
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
            if self.lifecycle.is_eof() {
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
            if self.lifecycle.is_eof() {
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
        let mut transport = self.transport.clone();

        match tokio::time::timeout(timeout, transport.read(&mut buf)).await {
            Ok(outcome) => self.absorb_read(outcome, &buf),
            Err(_) => {
                // Timeout, but not an error - caller will handle
                Ok(0)
            }
        }
    }

    /// Fold the outcome of one transport read into the session: the matcher,
    /// the event stream, and the state machine.
    ///
    /// This is the only way output enters a session, and both read drivers go
    /// through it — [`expect`](Self::expect)'s loop above and `interact()`'s.
    /// They used to keep separate buffers and separate notions of EOF, so a
    /// caller who interacted and returned saw a session that had observed
    /// nothing.
    ///
    /// Takes the read's outcome rather than performing the read, because
    /// `interact()` reads inside a `select!` alongside the terminal and
    /// SIGWINCH and cannot hold a borrow of the session across it.
    pub(crate) fn absorb_read(
        &mut self,
        outcome: std::io::Result<usize>,
        buf: &[u8],
    ) -> Result<usize> {
        match outcome {
            Ok(0) => {
                self.transition(SessionLifecycle::reached_eof);
                Ok(0)
            }
            Ok(n) => {
                self.matcher.append(&buf[..n]);
                self.emit(&SessionEvent::Output(&buf[..n]));
                Ok(n)
            }
            // On Linux, reading from PTY master returns EIO when the slave is closed
            // (i.e., the child process has terminated). Treat this as EOF.
            // See: https://bugs.python.org/issue5380
            Err(e) if is_pty_eof_error(&e) => {
                self.transition(SessionLifecycle::reached_eof);
                Ok(0)
            }
            Err(e) => {
                // Not EOF dressed up as an error: the session cannot read
                // past this, so it ends here rather than leaving the state
                // reporting a healthy session the caller can retry.
                let kind = e.kind();
                self.transition(|lifecycle| lifecycle.failed(kind));
                let error = ExpectError::io_context("reading from process", e);
                self.emit(&SessionEvent::Error(&error));
                Err(error)
            }
        }
    }

    /// A handle on the transport for the interaction loop's own `select!`.
    ///
    /// Crate-internal on purpose: `Session::transport()` was public and was
    /// removed, because handing out the transport is how a second reader gets
    /// built. The interaction loop is not a second reader — everything it
    /// reads goes straight back through [`absorb_read`](Self::absorb_read).
    pub(crate) fn transport_handle(&self) -> SharedTransport<T> {
        self.transport.clone()
    }

    /// The matcher, for the interaction loop's pattern hooks.
    pub(crate) const fn matcher_mut(&mut self) -> &mut Matcher {
        &mut self.matcher
    }

    /// Mark the session as being driven by `interact()`.
    pub(crate) fn begin_interacting(&mut self) {
        self.transition(SessionLifecycle::began_interacting);
    }

    /// Mark the interaction as finished.
    pub(crate) fn end_interacting(&mut self) {
        self.transition(SessionLifecycle::stopped_interacting);
    }

    /// Check if a pattern matches immediately without blocking.
    #[must_use]
    pub fn check(&mut self, pattern: &Pattern) -> Option<MatchResult> {
        self.matcher.try_match(pattern)
    }

    /// Attach process control to this session.
    ///
    /// Sessions built by `Session::spawn` already have one. This is for
    /// callers who construct a session from a transport of their own with
    /// [`Session::new`] and can supply a [`ProcessControl`] for it.
    #[must_use]
    pub fn with_process_control(mut self, control: ProcessHandle) -> Self {
        self.control = Some(control);
        self
    }

    /// Run `f` against this session's process control.
    ///
    /// Central to the capability split: every process-control method routes
    /// through here and therefore touches `control`, never `transport`. That
    /// is what lets a session be killed while a read is parked on the
    /// transport lock.
    fn with_control<R>(
        &self,
        operation: &'static str,
        f: impl FnOnce(&mut (dyn ProcessControl + Send)) -> R,
    ) -> Result<R> {
        self.control
            .as_ref()
            .ok_or(ExpectError::Unsupported { operation })
            .map(|handle| handle.with(f))
    }

    /// Send a signal to the child process **group**.
    ///
    /// A session is a terminal, and a terminal signals its foreground process
    /// group — pressing Ctrl-C does not signal one process. Signalling only the
    /// leader left a shell's background jobs running behind it. Falls back to
    /// the child alone on backends where it does not lead a group of its own.
    ///
    /// Does not touch the transport, so it succeeds while a read is in flight.
    ///
    /// # Errors
    ///
    /// Returns [`ExpectError::Unsupported`] if the backend has no child
    /// process or no signals (Windows), [`ExpectError::SessionClosed`] if the
    /// child has already exited, or an I/O error if delivery fails.
    pub fn signal(&self, signal: i32) -> Result<()> {
        self.with_control("signal", |c| c.signal(signal))?
    }

    /// Kill the child process group.
    ///
    /// As [`signal`](Self::signal), with `SIGKILL`: descendants in the child's
    /// process group go too.
    ///
    /// Does not touch the transport, so it succeeds while a read is in flight.
    ///
    /// # Errors
    ///
    /// Returns [`ExpectError::Unsupported`] if the backend has no child
    /// process, or an error if the kill fails.
    pub fn kill(&self) -> Result<()> {
        self.with_control("kill", |c| c.kill())?
    }

    /// Give up ownership of the child, so dropping this session leaves it
    /// running.
    ///
    /// A session kills its child process group when the last handle to it goes,
    /// which is what makes "the child cannot outlive the session" true rather
    /// than a hope. This is the opt-out, for a caller that means to launch
    /// something and walk away. One-way, and it does not stop the child's
    /// controlling terminal from hanging up when the PTY master closes — a
    /// detached child that does not ignore `SIGHUP` will still take one.
    ///
    /// Does nothing on backends with no child process of their own.
    pub fn detach(&mut self) {
        if let Some(control) = self.control.as_ref() {
            control.with(|c| c.detach());
        }
    }

    /// Check whether the child process is still running.
    ///
    /// Performs a non-blocking reap, so it reports the truth immediately after
    /// the child exits. Returns `None` when the backend has no child process
    /// to ask about — previously this could not be distinguished from a live
    /// child, because a failed lock acquisition was reported as "running".
    #[must_use]
    pub fn is_running(&self) -> Option<bool> {
        self.with_control("is_running", |c| c.is_running()).ok()
    }

    /// Get the child process ID.
    ///
    /// Returns `None` when the backend has no child process.
    #[must_use]
    pub fn pid(&self) -> Option<u32> {
        self.with_control("pid", |c| c.pid()).ok().flatten()
    }

    /// Start an interactive session with pattern hooks.
    ///
    /// This returns a builder that allows you to configure pattern-based
    /// callbacks that fire when patterns match in the output or input.
    ///
    /// The interaction borrows the session for its duration. It is the
    /// session's read driver while it runs, not a second one: output it reads
    /// lands in the session's buffer, its writes are the session's writes, and
    /// EOF or a read failure moves the session's state. A caller that interacts
    /// and then expects sees everything that happened in between.
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
    pub fn interact(&mut self) -> InteractBuilder<'_, T>
    where
        T: 'static,
    {
        InteractBuilder::new(self)
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
        while !self.lifecycle.is_eof() {
            if self.read_with_timeout(Duration::from_millis(100)).await? == 0
                && !self.lifecycle.is_eof()
            {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }

        let status = self.reap_exit_status().await;
        self.transition(|lifecycle| lifecycle.exited(status));
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

        while !self.lifecycle.is_eof() {
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
            if self.read_with_timeout(poll_timeout).await? == 0 && !self.lifecycle.is_eof() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }

        let status = self.reap_exit_status().await;
        self.transition(|lifecycle| lifecycle.exited(status));
        Ok(status)
    }

    /// Shut the child down and reap it.
    ///
    /// The deterministic counterpart to dropping the session: ask the child's
    /// process group to exit, give it the configured grace period
    /// (`config.timeout.close`, 10s by default), then kill it and collect the
    /// status. Returns as soon as the child is gone, so a child that exits
    /// promptly costs nothing.
    ///
    /// This is where a graceful shutdown belongs, because it can wait. `Drop`
    /// cannot — it kills outright rather than stalling the dropping thread on a
    /// grace period.
    ///
    /// On Unix the first step is `SIGTERM` to the process group. On backends
    /// with no signals it goes straight to the kill.
    ///
    /// # Errors
    ///
    /// Returns an error only if the child could not be reaped after being
    /// killed. A child that has already exited is a success, not an error.
    pub async fn shutdown(&mut self) -> Result<ProcessExitStatus> {
        let grace = self.config.timeout.close;

        // Already gone: nothing to ask, nothing to kill.
        if self.is_running() == Some(false) {
            return self.wait_timeout(grace).await;
        }

        #[cfg(unix)]
        let asked = self.signal(libc::SIGTERM);
        #[cfg(not(unix))]
        let asked: Result<()> = Err(ExpectError::Unsupported {
            operation: "signal",
        });

        // `SessionClosed` here means the child exited between the check above
        // and the signal — a race this method exists to absorb, not an error.
        match asked {
            Ok(()) | Err(ExpectError::SessionClosed | ExpectError::Unsupported { .. }) => {}
            Err(e) => return Err(e),
        }

        if let Ok(status) = self.wait_timeout(grace).await {
            return Ok(status);
        }

        // It had its chance.
        match self.kill() {
            Ok(()) | Err(ExpectError::SessionClosed | ExpectError::Unsupported { .. }) => {}
            Err(e) => return Err(e),
        }
        self.wait_timeout(grace).await
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
            let status = self.transport.with(ChildExit::try_exit_status);
            if let Some(status) = status {
                return status;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        ProcessExitStatus::Unknown
    }
}

impl<T: AsyncReadExt + AsyncWriteExt + Unpin + Send + Resizable> Session<T> {
    /// Resize the terminal.
    ///
    /// Also resizes the attached screen (if any) so it stays consistent
    /// with the PTY. Without this, screen-aware assertions would drift
    /// after a resize.
    ///
    /// Available for any transport with the [`Resizable`] capability, rather
    /// than being written once per platform.
    ///
    /// # Errors
    ///
    /// Returns an error if the resize ioctl fails.
    // Stays `async` despite no longer awaiting: it is the implementation of
    // the async `SessionExt::resize`, and a resize is only synchronous because
    // every current backend resizes with an ioctl. An SSH channel resize is a
    // request/reply and would await.
    #[allow(clippy::unused_async)]
    pub async fn resize_pty(&mut self, cols: u16, rows: u16) -> Result<()> {
        self.transport.with(|t| t.resize(cols, rows))?;
        self.config.dimensions = (cols, rows);
        // An attached screen resizes by subscribing to this, rather than by
        // being poked here.
        self.emit(&SessionEvent::Resize { cols, rows });
        Ok(())
    }
}

impl<T: AsyncReadExt + AsyncWriteExt + Unpin + Send> std::fmt::Debug for Session<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("id", &self.id)
            .field("state", &self.state())
            .field("eof", &self.lifecycle.is_eof())
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

        // Create the session, taking process control off the transport so it
        // is reachable while a read holds the transport lock.
        let control = async_pty.process_handle();
        let mut session = Self::new(async_pty, config).with_process_control(control);
        session.transition(SessionLifecycle::started);

        Ok(session)
    }
}

// Windows-specific spawn implementation
#[cfg(windows)]
impl Session<WindowsAsyncPty> {
    /// Spawn a new process with the given command.
    ///
    /// This creates a new PTY using Windows `ConPTY`, spawns a child process,
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
    /// - `ConPTY` is not available (Windows version too old)
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
        let args_owned: Vec<String> = args.iter().map(std::string::ToString::to_string).collect();

        // Spawn the process
        let handle = spawner.spawn(command, &args_owned).await?;

        // Wrap in WindowsAsyncPty for async I/O
        let async_pty = WindowsAsyncPty::from_handle(handle);

        // Create the session, taking process control off the transport so it
        // is reachable while a read holds the transport lock.
        let control = async_pty.process_handle();
        let mut session = Self::new(async_pty, config).with_process_control(control);
        session.transition(SessionLifecycle::started);

        Ok(session)
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

/// Process control must not contend with transport I/O (AR-002).
///
/// Most of these hold the transport lock directly and then exercise a control
/// operation. Before the capability split every one went through
/// `transport.try_lock()` and so failed: `signal` and `kill` returned
/// `WouldBlock`, `is_running` answered `true` without checking, and `pid`
/// answered `0`.
///
/// Holding the lock is now a structural stand-in rather than a literal
/// simulation — since the transport moved behind a per-poll lock, a parked
/// read holds nothing. `kill_reaches_a_child_while_a_read_is_parked` is the
/// end-to-end version, which only became expressible once that was true.
#[cfg(all(test, unix))]
mod process_control_tests {
    use std::time::Duration;

    use super::Session;
    use crate::error::ExpectError;

    /// Spawn a child that will outlive the test unless killed.
    async fn sleeper() -> Session<crate::backend::AsyncPty> {
        Session::spawn("/bin/sleep", &["30"])
            .await
            .expect("spawn sleep")
    }

    #[tokio::test]
    async fn kill_succeeds_while_the_transport_lock_is_held() {
        let session = sleeper().await;
        let guard = session.transport.lock();

        session.kill().expect("kill must not wait on the transport");

        drop(guard);
    }

    #[tokio::test]
    async fn signal_succeeds_while_the_transport_lock_is_held() {
        let session = sleeper().await;
        let guard = session.transport.lock();

        session
            .signal(libc::SIGTERM)
            .expect("signal must not wait on the transport");

        drop(guard);
    }

    #[tokio::test]
    async fn liveness_and_pid_are_answered_while_the_transport_lock_is_held() {
        let session = sleeper().await;
        let expected_pid = session.pid().expect("a spawned session has a pid");
        let guard = session.transport.lock();

        assert_eq!(session.is_running(), Some(true));
        assert_eq!(session.pid(), Some(expected_pid));

        drop(guard);
        session.kill().expect("kill");
    }

    /// The point of returning `Option` rather than `bool`: liveness now
    /// reports what it observed, and observation no longer depends on a lock.
    #[tokio::test]
    async fn liveness_reports_exit_rather_than_assuming_running() {
        let session = sleeper().await;
        session.kill().expect("kill");

        // The reap is not instantaneous; poll briefly for the observed exit.
        for _ in 0..50u32 {
            if session.is_running() == Some(false) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("killed child still reported as running");
    }

    /// The end-to-end property: a real `expect` parked on a quiet child must
    /// not stop that child being killed, and the kill must be visible to the
    /// parked read as EOF.
    ///
    /// This is the end-to-end form of what the structural tests above assert.
    /// It passes as of the capability split, not the transport split — a
    /// control run against the pre-transport-split tree confirmed it already
    /// passed there, because the kill no longer travels through the transport
    /// lock at all. Kept because it exercises the EOF path the structural
    /// tests do not, but it is not evidence for the transport split; the
    /// evidence for that lives in `session::transport::tests`.
    #[tokio::test]
    async fn kill_reaches_a_child_while_a_read_is_parked() {
        let mut session = sleeper().await;
        let control = session
            .control
            .clone()
            .expect("a spawned session has process control");

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            control.with(|c| c.kill()).expect("kill from another task");
        });

        // `sleep 30` never writes, so this read parks until the kill closes
        // the PTY. If control were still behind the transport lock the kill
        // would never land and this would run the full 10 s timeout.
        let start = tokio::time::Instant::now();
        let result = session
            .expect_timeout("never appears", Duration::from_secs(10))
            .await;

        assert!(
            result.is_err(),
            "expect should end at EOF, not match: {result:?}"
        );
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "the parked read outlived the kill by {:?}; control did not reach the child",
            start.elapsed()
        );
    }

    /// A transport with no child process says so, instead of pretending.
    #[tokio::test]
    #[cfg(feature = "mock")]
    async fn control_without_a_child_reports_unsupported() {
        let session = Session::new(
            crate::mock::MockTransport::new(),
            crate::config::SessionConfig::default(),
        );

        assert!(matches!(
            session.kill(),
            Err(ExpectError::Unsupported { operation: "kill" })
        ));
        assert!(matches!(
            session.signal(libc::SIGTERM),
            Err(ExpectError::Unsupported {
                operation: "signal"
            })
        ));
        assert_eq!(session.is_running(), None);
        assert_eq!(session.pid(), None);
    }
}

/// Before/after ambient-pattern behavior driven through the real expect loop
/// (M1). Uses the mock transport so reads/writes are deterministic.
#[cfg(all(test, feature = "mock"))]
mod ambient_pattern_tests {
    use std::time::Duration;

    use super::Session;
    use crate::config::SessionConfig;
    use crate::expect::{HandlerAction, Pattern, PersistentPattern};
    use crate::mock::MockTransport;

    /// Build a session over a mock transport pre-loaded with `output`, keeping a
    /// cloned transport handle for queueing more output and reading what was sent.
    fn session_with(output: &str) -> (Session<MockTransport>, MockTransport) {
        let transport = MockTransport::new();
        let handle = transport.clone();
        handle.queue_output_str(output);
        let session = Session::new(transport, SessionConfig::default());
        (session, handle)
    }

    /// Bug A: after-patterns were never checked by the loop. An after-pattern
    /// must fire as a fallback when the explicit pattern does not match.
    #[tokio::test]
    async fn after_pattern_fires_as_fallback() {
        let (mut session, handle) = session_with("Show more? ");
        let after = PersistentPattern::with_response(Pattern::literal("more? "), "yes\n");
        session.pattern_manager_mut().add_after(after);

        // No explicit match: the after-pattern should respond, then we time out.
        let _ = session
            .expect_timeout(Pattern::literal("NEVER"), Duration::from_millis(150))
            .await;

        let sent = String::from_utf8_lossy(&handle.take_input()).into_owned();
        assert!(
            sent.contains("yes"),
            "after-pattern should have responded; sent = {sent:?}"
        );
    }

    /// Bug B: a before `Respond` must consume its trigger so it can't re-fire.
    /// We observe the consume via the following match's `before` (the prompt is
    /// gone) — before the fix, `before` still contains the un-consumed prompt.
    #[tokio::test]
    async fn before_respond_consumes_trigger() {
        let (mut session, handle) = session_with("password: welcome\n");
        let before = PersistentPattern::with_response(Pattern::literal("password:"), "secret\n");
        session.pattern_manager_mut().add_before(before);

        let m = session
            .expect_timeout(Pattern::literal("welcome"), Duration::from_secs(2))
            .await
            .expect("welcome should match");

        assert!(
            !m.before.contains("password"),
            "before-trigger was not consumed; before = {:?}",
            m.before
        );
        let sent = String::from_utf8_lossy(&handle.take_input()).into_owned();
        assert!(
            sent.contains("secret"),
            "responder should have sent; sent = {sent:?}"
        );
    }

    /// Reviewer note: a before `Return` must consume its trigger so the *next*
    /// expect call against the same (persistent) buffer doesn't immediately
    /// re-trigger instead of matching real output.
    #[tokio::test]
    async fn before_return_consumes_across_calls() {
        let (mut session, _handle) = session_with("prompt data\n");
        let before = PersistentPattern::new(
            Pattern::literal("prompt"),
            Box::new(|_| HandlerAction::Return("HANDLED".into())),
        );
        session.pattern_manager_mut().add_before(before);

        let first = session
            .expect_timeout(Pattern::literal("data"), Duration::from_secs(2))
            .await
            .expect("first expect");
        assert_eq!(first.matched, "HANDLED", "before Return should fire first");

        // Consumed, so the second call must match the real data, not re-Return.
        let second = session
            .expect_timeout(Pattern::literal("data"), Duration::from_secs(2))
            .await
            .expect("second expect should match data, not re-trigger");
        assert!(
            second.matched.contains("data"),
            "before Return re-triggered across calls; got {:?}",
            second.matched
        );
    }

    /// Priority: a before pattern takes precedence over the explicit pattern.
    #[tokio::test]
    async fn before_beats_explicit_pattern() {
        let (mut session, _handle) = session_with("xy\n");
        let before = PersistentPattern::new(
            Pattern::literal("x"),
            Box::new(|_| HandlerAction::Return("BEFORE".into())),
        );
        session.pattern_manager_mut().add_before(before);

        let m = session
            .expect_timeout(Pattern::literal("x"), Duration::from_secs(2))
            .await
            .expect("match");
        assert_eq!(
            m.matched, "BEFORE",
            "before should win over the explicit pattern"
        );
    }

    /// Priority: an explicit pattern suppresses an after-pattern that would also
    /// match (after runs only as a fallback once the explicit pattern fails).
    #[tokio::test]
    async fn explicit_beats_after_pattern() {
        let (mut session, _handle) = session_with("target\n");
        let after = PersistentPattern::new(
            Pattern::literal("target"),
            Box::new(|_| HandlerAction::Return("AFTER".into())),
        );
        session.pattern_manager_mut().add_after(after);

        let m = session
            .expect_timeout(Pattern::literal("target"), Duration::from_secs(2))
            .await
            .expect("match");
        assert_eq!(
            m.matched, "target",
            "explicit pattern should suppress the after-pattern"
        );
    }

    /// After-pattern consumption: like the before case, an after `Return` must
    /// consume its trigger so the next expect call matches real output instead
    /// of re-triggering the after-pattern.
    #[tokio::test]
    async fn after_return_consumes_across_calls() {
        let (mut session, _handle) = session_with("prompt data\n");
        let after = PersistentPattern::new(
            Pattern::literal("prompt"),
            Box::new(|_| HandlerAction::Return("A_HANDLED".into())),
        );
        session.pattern_manager_mut().add_after(after);

        // Explicit pattern doesn't match, so the after-pattern fires and returns.
        let first = session
            .expect_timeout(Pattern::literal("NOPE"), Duration::from_secs(2))
            .await
            .expect("after-pattern should fire as fallback");
        assert_eq!(first.matched, "A_HANDLED");

        // Consumed, so this must match the real data, not re-trigger the after.
        let second = session
            .expect_timeout(Pattern::literal("data"), Duration::from_secs(2))
            .await
            .expect("second expect should match data, not re-trigger");
        assert!(
            second.matched.contains("data"),
            "after Return re-triggered across calls; got {:?}",
            second.matched
        );
    }
}

/// Regression tests for AR-004: session state must be authoritative.
///
/// Before design stage 3, `SessionState` was advisory. EOF set a private
/// `eof: bool` and left the state at `Running`, so a session whose child had
/// closed its output still reported itself usable and still accepted writes.
/// On a PTY those writes then failed at the syscall with `EIO`, which `send`
/// translated to `SessionClosed` by accident — the right error for the wrong
/// reason, and only on transports whose writes happen to fail after EOF. Over a
/// transport that still accepts writes, the send simply succeeded.
///
/// `set_state` was also public, so any caller could put a session into any
/// state, and `Interacting` was never assigned by anything.
#[cfg(test)]
mod state_machine_tests {
    #[cfg(unix)]
    use std::time::Duration;

    use super::Session;
    use crate::types::SessionState;

    /// A session over a mock transport, plus a handle for driving it and
    /// reading back what the session wrote.
    #[cfg(feature = "mock")]
    fn mock_session() -> (
        Session<crate::mock::MockTransport>,
        crate::mock::MockTransport,
    ) {
        let transport = crate::mock::MockTransport::new();
        let handle = transport.clone();
        let session = Session::new(transport, crate::config::SessionConfig::default());
        (session, handle)
    }

    /// A child that writes one line and exits, for the EOF cases.
    #[cfg(unix)]
    async fn echoer() -> Session<crate::backend::AsyncPty> {
        Session::spawn("/bin/echo", &["hi"])
            .await
            .expect("spawn echo")
    }

    /// Guard, not a control: `spawn` already set `Running`. This pins that it
    /// stays set once transitions move behind the state machine.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_spawned_session_reports_running() {
        let mut session = Session::spawn("/bin/cat", &[]).await.expect("spawn cat");
        session.send_line("hello").await.expect("send");
        session.expect("hello").await.expect("expect");

        assert_eq!(session.state(), SessionState::Running);
        assert!(
            session.state().is_usable(),
            "a working session must not report itself unusable"
        );

        session.kill().ok();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn eof_is_a_state_transition_not_only_a_flag() {
        let mut session = echoer().await;
        session.expect_eof().await.expect("read to EOF");

        assert!(session.is_eof(), "the flag still reports EOF");
        assert_eq!(
            session.state(),
            SessionState::Eof,
            "EOF must move the state machine, not only set a private flag"
        );
    }

    /// The honest form of "writes are rejected after EOF".
    ///
    /// A PTY cannot show this: after EOF its writes fail with `EIO` anyway, so
    /// the test passes whether or not the session checks its own state. The
    /// mock keeps accepting writes after EOF, so the rejection can only come
    /// from the state machine — and `take_input` proves no bytes reached the
    /// transport.
    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn send_after_eof_is_rejected_by_the_session_not_the_transport() {
        let (mut session, handle) = mock_session();
        handle.queue_output_str("hi");
        session.expect("hi").await.expect("read the queued output");
        handle.signal_eof();
        session.expect_eof().await.expect("read to EOF");

        let err = session
            .send_line("too late")
            .await
            .expect_err("a write after EOF must be rejected");

        assert!(
            matches!(err, crate::error::ExpectError::SessionClosed),
            "expected SessionClosed, got {err:?}"
        );
        assert!(
            handle.take_input().is_empty(),
            "the rejected write must never reach the transport"
        );
    }

    #[cfg(feature = "mock")]
    #[tokio::test]
    async fn an_unrecoverable_read_error_moves_the_session_to_failed() {
        let (mut session, handle) = mock_session();
        handle.set_error("device fell off the bus");

        let err = session
            .expect_timeout("anything", std::time::Duration::from_millis(200))
            .await
            .expect_err("the read must surface the error");
        assert!(
            !matches!(err, crate::error::ExpectError::Timeout { .. }),
            "expected the I/O error, not a timeout"
        );

        assert!(
            matches!(session.state(), SessionState::Failed(_)),
            "an unrecoverable read error must end the session, got {:?}",
            session.state()
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn buffered_output_is_still_readable_after_eof() {
        let mut session = echoer().await;
        session.expect_eof().await.expect("read to EOF");

        assert!(
            session.buffer().contains("hi"),
            "EOF must not discard already-buffered output"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn reaping_moves_from_eof_to_exited() {
        let mut session = echoer().await;
        session.expect_eof().await.expect("read to EOF");
        assert_eq!(session.state(), SessionState::Eof);

        let status = session
            .wait_timeout(Duration::from_secs(5))
            .await
            .expect("wait");

        assert_eq!(
            session.state(),
            SessionState::Exited(status),
            "reaping must move Eof -> Exited"
        );
    }
}

/// Regression tests for AR-008: observability must be a session feature.
///
/// Before design stage 4 a session had exactly one observation point — an
/// output tap over bytes read from the transport. Input had none at all:
/// `send()` had no hook, so `Recorder::record_input` was unreachable from a
/// session even though the recorder implements it. Resize, state transitions
/// and read errors were equally unobservable, and an attached screen learned
/// about a resize only because `resize_pty` reached into it directly.
#[cfg(all(test, feature = "mock"))]
mod event_stream_tests {
    use std::sync::{Arc, Mutex};

    use super::Session;
    use crate::config::SessionConfig;
    use crate::mock::MockTransport;
    use crate::session::SessionEvent;
    use crate::types::SessionState;

    /// Record every event a session emits, as a printable label per event.
    fn recording_session() -> (
        Session<MockTransport>,
        MockTransport,
        Arc<Mutex<Vec<String>>>,
    ) {
        let transport = MockTransport::new();
        let handle = transport.clone();
        let mut session = Session::new(transport, SessionConfig::default());
        let log = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&log);
        session.add_event_subscriber(move |event| {
            let label = match event {
                SessionEvent::Output(bytes) => {
                    format!("output:{}", String::from_utf8_lossy(bytes))
                }
                SessionEvent::Input(bytes) => format!("input:{}", String::from_utf8_lossy(bytes)),
                SessionEvent::Resize { cols, rows } => format!("resize:{cols}x{rows}"),
                SessionEvent::StateChanged { from, to } => format!("state:{from}->{to}"),
                SessionEvent::Matched { pattern_index } => format!("matched:{pattern_index}"),
                SessionEvent::Error(e) => format!("error:{e}"),
            };
            sink.lock().unwrap().push(label);
        });
        (session, handle, log)
    }

    /// The half of AR-008 with no observation point at all before stage 4.
    #[tokio::test]
    async fn input_is_observable() {
        let (mut session, _handle, log) = recording_session();

        session.send_line("hello").await.expect("send");

        let events = log.lock().unwrap().clone();
        assert!(
            events.iter().any(|e| e.starts_with("input:hello")),
            "send() must emit Input; got {events:?}"
        );
    }

    #[tokio::test]
    async fn output_reaches_subscribers_as_well_as_taps() {
        let (mut session, handle, log) = recording_session();
        handle.queue_output_str("from the child");

        session.expect("child").await.expect("expect");

        let events = log.lock().unwrap().clone();
        assert!(
            events.iter().any(|e| e.contains("output:from the child")),
            "subscribers must see output; got {events:?}"
        );
    }

    #[tokio::test]
    async fn state_transitions_are_observable() {
        let (mut session, handle, log) = recording_session();
        handle.signal_eof();

        session.expect_eof().await.expect("read to EOF");

        let events = log.lock().unwrap().clone();
        assert!(
            events.iter().any(|e| e.contains("->at end of output")),
            "reaching EOF must emit StateChanged; got {events:?}"
        );
        assert_eq!(session.state(), SessionState::Eof);
    }

    /// A transition that changes nothing must not announce itself, or a
    /// subscriber counting transitions sees phantom ones.
    ///
    /// Waiting twice is the path that genuinely re-applies a transition: the
    /// second `wait_timeout` skips its read loop (already at EOF), reaps again,
    /// and re-applies `exited` with the same status. Calling `expect_eof` twice
    /// does *not* work here — it returns at the `is_eof` check without
    /// attempting a transition at all, so it would assert nothing.
    #[tokio::test]
    async fn an_unchanged_state_emits_nothing() {
        let (mut session, handle, log) = recording_session();
        handle.signal_eof();
        session.expect_eof().await.expect("read to EOF");
        let first = session
            .wait_timeout(std::time::Duration::from_secs(1))
            .await
            .expect("first wait");
        let after_first = log.lock().unwrap().len();

        let second = session
            .wait_timeout(std::time::Duration::from_secs(1))
            .await
            .expect("second wait");
        assert_eq!(first, second, "the status must be stable across reaps");

        let events = log.lock().unwrap().clone();
        let transitions = events[after_first..]
            .iter()
            .filter(|e| e.starts_with("state:"))
            .count();
        assert_eq!(
            transitions, 0,
            "re-entering the same state must not re-announce it; got {events:?}"
        );
    }

    #[tokio::test]
    async fn read_errors_are_observable() {
        let (mut session, handle, log) = recording_session();
        handle.set_error("device fell off the bus");

        session
            .expect_timeout("anything", std::time::Duration::from_millis(200))
            .await
            .expect_err("the read must fail");

        let events = log.lock().unwrap().clone();
        assert!(
            events.iter().any(|e| e.starts_with("error:")),
            "a read error must be emitted; got {events:?}"
        );
    }

    /// Output taps predate the event stream and must keep working unchanged,
    /// interleaved with subscribers in one registration order.
    #[tokio::test]
    async fn output_taps_still_work_and_share_one_order() {
        let transport = MockTransport::new();
        let handle = transport.clone();
        let mut session = Session::new(transport, SessionConfig::default());
        let order = Arc::new(Mutex::new(Vec::new()));

        let first = Arc::clone(&order);
        session.add_output_tap(move |_| first.lock().unwrap().push("tap"));
        let second = Arc::clone(&order);
        session.add_event_subscriber(move |event| {
            if matches!(event, SessionEvent::Output(_)) {
                second.lock().unwrap().push("subscriber");
            }
        });

        handle.queue_output_str("x");
        session.expect("x").await.expect("expect");

        assert_eq!(*order.lock().unwrap(), vec!["tap", "subscriber"]);
    }

    #[tokio::test]
    async fn a_removed_subscriber_stops_receiving() {
        let (mut session, handle, log) = recording_session();
        let extra = Arc::new(Mutex::new(0usize));
        let counter = Arc::clone(&extra);
        let id = session.add_event_subscriber(move |_| {
            *counter.lock().unwrap() += 1;
        });

        handle.queue_output_str("one");
        session.expect("one").await.expect("expect");
        let before = *extra.lock().unwrap();
        assert!(before > 0, "the subscriber must have been receiving");

        assert!(session.remove_output_tap(id), "removal reports success");
        handle.queue_output_str("two");
        session.expect("two").await.expect("expect");

        assert_eq!(*extra.lock().unwrap(), before, "removed means removed");
        assert!(
            !log.lock().unwrap().is_empty(),
            "the other subscriber is unaffected"
        );
    }
}

/// Stage 6 of the event-pump design: the built-in observers.
///
/// `Recorder`, `SessionMetrics` and `StreamingRedactor` were all
/// subscriber-shaped already — `&self` methods with interior mutability — and
/// none of them was referenced anywhere under `src/session/`. These tests pin
/// the wiring, and above all the rule that redaction sits between the stream
/// and the transcript and never in front of the matcher.
#[cfg(all(test, feature = "mock"))]
mod subscriber_tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::Session;
    use crate::config::SessionConfig;
    use crate::expect::Pattern;
    use crate::metrics::SessionMetrics;
    use crate::mock::MockTransport;
    use crate::transcript::Recorder;

    fn session_with(output: &str) -> (Session<MockTransport>, MockTransport) {
        let transport = MockTransport::new();
        let handle = transport.clone();
        handle.queue_output_str(output);
        (Session::new(transport, SessionConfig::default()), handle)
    }

    fn transcript_of(recorder: &Arc<Recorder>) -> crate::transcript::Transcript {
        recorder
            .transcript()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    #[tokio::test]
    async fn a_recorder_sees_output_and_input() {
        let (mut session, _handle) = session_with("hello\n");
        let recorder = Arc::new(Recorder::new(80, 24));
        session.attach_recorder(&recorder);

        session
            .expect_timeout(Pattern::literal("hello"), Duration::from_secs(2))
            .await
            .expect("match");
        session.send(b"who\n").await.expect("send");

        let transcript = transcript_of(&recorder);
        assert!(
            transcript.output_text().contains("hello"),
            "output missing: {:?}",
            transcript.output_text()
        );
        assert_eq!(
            transcript.input_text(),
            "who\n",
            "input had no route to the recorder before the event stream"
        );
    }

    #[tokio::test]
    async fn a_removed_recorder_stops_recording() {
        let (mut session, handle) = session_with("first\n");
        let recorder = Arc::new(Recorder::new(80, 24));
        let id = session.attach_recorder(&recorder);

        session
            .expect_timeout(Pattern::literal("first"), Duration::from_secs(2))
            .await
            .expect("match");
        assert!(session.remove_output_tap(id));

        handle.queue_output_str("second\n");
        session
            .expect_timeout(Pattern::literal("second"), Duration::from_secs(2))
            .await
            .expect("match");

        let text = transcript_of(&recorder).output_text();
        assert!(text.contains("first"), "lost what it did record: {text:?}");
        assert!(
            !text.contains("second"),
            "kept recording after removal: {text:?}"
        );
    }

    #[tokio::test]
    async fn metrics_count_bytes_matches_and_errors() {
        let (mut session, handle) = session_with("ready\n");
        let metrics = Arc::new(SessionMetrics::new());
        session.attach_metrics(&metrics);

        session
            .expect_timeout(Pattern::literal("ready"), Duration::from_secs(2))
            .await
            .expect("match");
        session.send(b"go\n").await.expect("send");

        assert_eq!(metrics.bytes_received.get(), 6, "\"ready\\n\" is six bytes");
        assert_eq!(metrics.bytes_sent.get(), 3);
        assert_eq!(
            metrics.pattern_matches.get(),
            1,
            "a counter that never counts is a false contract"
        );
        assert_eq!(metrics.errors.get(), 0);

        handle.set_error("broken");
        let _ = session
            .expect_timeout(Pattern::literal("never"), Duration::from_secs(1))
            .await;
        assert_eq!(metrics.errors.get(), 1);
    }

    /// The rule that must not be relaxed. Redaction sits between the event
    /// stream and the transcript; the matcher has already seen the raw bytes.
    /// A caller expecting on a secret still matches it, and the secret still
    /// does not reach the transcript.
    #[cfg(feature = "pii-redaction")]
    #[tokio::test]
    async fn redaction_reaches_the_transcript_but_never_the_matcher() {
        use crate::pii::{PiiRedactor, StreamingRedactor};

        let (mut session, _handle) = session_with("login for admin@example.com ok\n");
        let recorder = Arc::new(Recorder::new(80, 24));
        session.attach_redacted_recorder(&recorder, StreamingRedactor::new(PiiRedactor::new()));

        let matched = session
            .expect_timeout(
                Pattern::literal("admin@example.com"),
                Duration::from_secs(2),
            )
            .await
            .expect("the matcher must still see the raw address");
        assert_eq!(matched.matched, "admin@example.com");

        let text = transcript_of(&recorder).output_text();
        assert!(
            !text.contains("admin@example.com"),
            "the address reached the transcript unredacted: {text:?}"
        );
        assert!(
            text.contains("login for"),
            "redaction ate the surrounding output too: {text:?}"
        );
    }

    /// `StreamingRedactor` holds back a partial trailing line so a secret split
    /// across two reads is still caught. Whatever it is holding when the
    /// session ends must still reach the transcript.
    #[cfg(feature = "pii-redaction")]
    #[tokio::test]
    async fn the_redactors_buffered_tail_is_flushed_at_eof() {
        use crate::pii::{PiiRedactor, StreamingRedactor};

        // No trailing newline: the redactor has no safe split point and holds
        // the whole line back.
        let (mut session, handle) = session_with("tail without a newline");
        let recorder = Arc::new(Recorder::new(80, 24));
        session.attach_redacted_recorder(&recorder, StreamingRedactor::new(PiiRedactor::new()));

        session
            .expect_timeout(Pattern::literal("tail"), Duration::from_secs(2))
            .await
            .expect("match");
        assert_eq!(
            transcript_of(&recorder).output_text(),
            "",
            "the tail should still be buffered at this point"
        );

        handle.signal_eof();
        let _ = session.expect_eof_timeout(Duration::from_secs(2)).await;

        assert!(
            transcript_of(&recorder)
                .output_text()
                .contains("tail without a newline"),
            "the buffered tail was never flushed: {:?}",
            transcript_of(&recorder).output_text()
        );
    }
}
