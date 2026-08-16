//! Mock session implementation for testing.
//!
//! This module provides a mock session that can be used for testing
//! expect scripts without spawning real processes.

use std::collections::VecDeque;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::time::{Instant, Sleep};

use super::event::{EventTimeline, MockEvent};
use super::scenario::Scenario;

/// Shared state for the mock transport.
#[derive(Debug)]
struct MockState {
    /// Output buffer (data to be read by the client).
    output: VecDeque<u8>,
    /// Input buffer (data written by the client).
    input: VecDeque<u8>,
    /// Everything the client has ever written. Kept separately from `input`,
    /// which [`MockTransport::take_input`] drains, so that scripted input
    /// gates still see what was written before the test read it back.
    input_log: Vec<u8>,
    /// How far into `input_log` scripted input gates have been satisfied, so
    /// one write cannot satisfy the same gate twice.
    input_cursor: usize,
    /// Event timeline.
    timeline: EventTimeline,
    /// Whether EOF has been signaled.
    eof: bool,
    /// Error to return on next read.
    error: Option<String>,
    /// Exit code if exited.
    exit_code: Option<i32>,
    /// When a pending [`MockEvent::Delay`] elapses. While set and in the
    /// future, reads park rather than delivering the output behind it.
    resume_at: Option<Instant>,
    /// Timer backing `resume_at`, created on the first poll after the delay
    /// starts. Held across polls so the timer registration — and with it the
    /// wake-up — survives; a `Sleep` dropped between polls never fires.
    sleep: Option<Pin<Box<Sleep>>>,
    /// Tasks parked in `poll_read` waiting for output, EOF, or an error.
    ///
    /// A hand-written [`AsyncRead`] that returns `Pending` without keeping the
    /// waker is never polled again. This mock did exactly that, so queued
    /// output never woke a waiting read — reads only completed because the
    /// caller's own timeout re-polled the task, which made every mock-backed
    /// read take the full timeout instead of returning when data arrived.
    wakers: Vec<Waker>,
}

impl MockState {
    const fn new(timeline: EventTimeline) -> Self {
        Self {
            output: VecDeque::new(),
            input: VecDeque::new(),
            input_log: Vec::new(),
            input_cursor: 0,
            timeline,
            eof: false,
            error: None,
            exit_code: None,
            resume_at: None,
            sleep: None,
            wakers: Vec::new(),
        }
    }

    /// Record a reader waiting for the state to change.
    fn park_reader(&mut self, cx: &Context<'_>) {
        let waker = cx.waker();
        if !self.wakers.iter().any(|w| w.will_wake(waker)) {
            self.wakers.push(waker.clone());
        }
    }

    /// Advance the timeline until it produces something a reader can observe.
    ///
    /// Stops *without consuming* an [`MockEvent::Input`] event until the client
    /// has actually written matching bytes: a scripted response must not appear
    /// before the input it responds to. A `Delay` arms `resume_at` and stops
    /// there until it elapses. `Resize` is consumed and skipped — the mock has
    /// no terminal to resize.
    fn process_event(&mut self) {
        while let Some(event) = self.timeline.peek().cloned() {
            match event {
                MockEvent::Input(expected) => {
                    if !self.consume_expected_input(&expected) {
                        return;
                    }
                    self.timeline.next();
                }
                MockEvent::Output(data) => {
                    self.timeline.next();
                    self.output.extend(data);
                    return;
                }
                MockEvent::Eof => {
                    self.timeline.next();
                    self.eof = true;
                    return;
                }
                MockEvent::Error(msg) => {
                    self.timeline.next();
                    self.error = Some(msg);
                    return;
                }
                MockEvent::Exit(code) => {
                    self.timeline.next();
                    self.exit_code = Some(code);
                    self.eof = true;
                    return;
                }
                MockEvent::Delay(duration) => {
                    self.timeline.next();
                    // Hold everything behind this event until the delay is up,
                    // so a scripted pause is a pause rather than a no-op.
                    self.resume_at = Some(Instant::now() + duration);
                    return;
                }
                MockEvent::Resize { .. } => {
                    self.timeline.next();
                }
            }
        }
    }

    /// Whether the client has written `expected` since the last gate, and if
    /// so advance past it.
    ///
    /// Matching is a plain substring search over the raw bytes written.
    fn consume_expected_input(&mut self, expected: &[u8]) -> bool {
        if expected.is_empty() {
            return true;
        }
        let Some(pos) = memchr::memmem::find(&self.input_log[self.input_cursor..], expected) else {
            return false;
        };
        self.input_cursor += pos + expected.len();
        true
    }
}

/// A mock transport for testing.
#[derive(Debug, Clone)]
pub struct MockTransport {
    state: Arc<Mutex<MockState>>,
}

// A mock transport has no child process, so `wait`/`wait_timeout` report
// `ProcessExitStatus::Unknown` via the default `ChildExit` implementation.
impl crate::backend::ChildExit for MockTransport {}

impl MockTransport {
    /// Create a new mock transport.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(MockState::new(EventTimeline::new()))),
        }
    }

    /// Create a mock transport from an event timeline.
    #[must_use]
    pub fn from_timeline(timeline: EventTimeline) -> Self {
        let mut state = MockState::new(timeline);
        // Process initial events
        state.process_event();
        Self {
            state: Arc::new(Mutex::new(state)),
        }
    }

    /// Create a mock transport from a scenario.
    #[must_use]
    pub fn from_scenario(scenario: &Scenario) -> Self {
        Self::from_timeline(scenario.to_timeline())
    }

    /// Mutate the shared state, then wake any reader parked in `poll_read`.
    ///
    /// Wakers fire after the lock is released: a woken task polls straight
    /// away and would otherwise contend on a lock still held here.
    fn mutate<R>(&self, f: impl FnOnce(&mut MockState) -> R) -> R {
        let (result, wakers) = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let result = f(&mut state);
            (result, std::mem::take(&mut state.wakers))
        };
        for waker in wakers {
            waker.wake();
        }
        result
    }

    /// Queue output to be read.
    pub fn queue_output(&self, data: &[u8]) {
        self.mutate(|state| state.output.extend(data));
    }

    /// Queue a string to be read.
    pub fn queue_output_str(&self, s: &str) {
        self.queue_output(s.as_bytes());
    }

    /// Get data that was written by the client.
    #[must_use]
    pub fn take_input(&self) -> Vec<u8> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.input.drain(..).collect()
    }

    /// Get input as a string.
    #[must_use]
    pub fn take_input_str(&self) -> String {
        String::from_utf8_lossy(&self.take_input()).into_owned()
    }

    /// Signal EOF.
    pub fn signal_eof(&self) {
        self.mutate(|state| state.eof = true);
    }

    /// Signal exit with code.
    pub fn signal_exit(&self, code: i32) {
        self.mutate(|state| {
            state.exit_code = Some(code);
            state.eof = true;
        });
    }

    /// Set an error to return on next read.
    pub fn set_error(&self, msg: impl Into<String>) {
        self.mutate(|state| state.error = Some(msg.into()));
    }

    /// Check if EOF has been signaled.
    #[must_use]
    pub fn is_eof(&self) -> bool {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.eof
    }

    /// Get the exit code if exited.
    #[must_use]
    pub fn exit_code(&self) -> Option<i32> {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.exit_code
    }

    /// Process the next event from the timeline.
    pub fn advance(&self) {
        self.mutate(MockState::process_event);
    }
}

impl Default for MockTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl AsyncRead for MockTransport {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        loop {
            // Check for error
            if let Some(error) = state.error.take() {
                return Poll::Ready(Err(io::Error::other(error)));
            }

            // A scripted delay holds back everything behind it. The timer is
            // created here rather than where the delay is armed, because the
            // arming can happen outside a runtime — a mock is often built
            // synchronously — and because holding it across polls is what
            // makes the wake-up actually arrive.
            if let Some(deadline) = state.resume_at {
                let sleep = state
                    .sleep
                    .get_or_insert_with(|| Box::pin(tokio::time::sleep_until(deadline)));
                if sleep.as_mut().poll(cx).is_pending() {
                    return Poll::Pending;
                }
                state.resume_at = None;
                state.sleep = None;
            }

            // Read available data
            if !state.output.is_empty() {
                let to_read = buf.remaining().min(state.output.len());
                for _ in 0..to_read {
                    if let Some(byte) = state.output.pop_front() {
                        buf.put_slice(&[byte]);
                    }
                }
                return Poll::Ready(Ok(()));
            }

            // Check for EOF
            if state.eof {
                return Poll::Ready(Ok(()));
            }

            // Nothing buffered: let the timeline produce the next thing to
            // observe, then go round again to deliver it — or, if it armed a
            // delay, to start that delay's timer.
            state.process_event();
            if state.resume_at.is_some()
                || !state.output.is_empty()
                || state.eof
                || state.error.is_some()
            {
                continue;
            }

            // The timeline has nothing more to give right now. Keep the waker
            // so queued output, a signalled EOF, or a write that satisfies a
            // scripted input gate wakes this read.
            state.park_reader(cx);
            drop(state);
            return Poll::Pending;
        }
    }
}

impl AsyncWrite for MockTransport {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let len = buf.len();
        // Wakes parked readers: this write may satisfy the scripted input gate
        // they are waiting behind.
        self.mutate(|state| {
            state.input.extend(buf);
            state.input_log.extend_from_slice(buf);
        });
        Poll::Ready(Ok(len))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

/// A mock session wrapping a mock transport.
pub struct MockSession {
    transport: MockTransport,
}

impl MockSession {
    /// Create a new mock session.
    #[must_use]
    pub fn new() -> Self {
        Self {
            transport: MockTransport::new(),
        }
    }

    /// Create a mock session from a scenario.
    #[must_use]
    pub fn from_scenario(scenario: &Scenario) -> Self {
        Self {
            transport: MockTransport::from_scenario(scenario),
        }
    }

    /// Get the transport.
    #[must_use]
    pub const fn transport(&self) -> &MockTransport {
        &self.transport
    }

    /// Get mutable access to the transport.
    pub const fn transport_mut(&mut self) -> &mut MockTransport {
        &mut self.transport
    }

    /// Queue output to be read.
    pub fn queue_output(&self, data: &[u8]) {
        self.transport.queue_output(data);
    }

    /// Queue a string to be read.
    pub fn queue_output_str(&self, s: &str) {
        self.transport.queue_output_str(s);
    }

    /// Get data that was written.
    #[must_use]
    pub fn take_input(&self) -> Vec<u8> {
        self.transport.take_input()
    }

    /// Get input as a string.
    #[must_use]
    pub fn take_input_str(&self) -> String {
        self.transport.take_input_str()
    }
}

impl Default for MockSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    #[tokio::test]
    async fn mock_transport_read_write() {
        let mut transport = MockTransport::new();
        transport.queue_output_str("hello");

        let mut buf = [0u8; 10];
        let n = transport.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], b"hello");

        transport.write_all(b"world").await.unwrap();
        assert_eq!(transport.take_input_str(), "world");
    }

    #[tokio::test]
    async fn mock_transport_eof() {
        let mut transport = MockTransport::new();
        transport.signal_eof();

        let mut buf = [0u8; 10];
        let n = transport.read(&mut buf).await.unwrap();
        assert_eq!(n, 0);
    }

    /// Returning `Pending` without keeping the waker means output queued later
    /// never wakes the read; it only completes when something else re-polls the
    /// task, e.g. the caller's own timeout. The margin here is wide on purpose:
    /// unwoken, this takes the full 5s timeout rather than the ~50ms it should.
    #[tokio::test]
    async fn queued_output_wakes_a_parked_read() {
        let transport = MockTransport::new();
        let handle = transport.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            handle.queue_output_str("late output");
        });

        let mut transport = transport;
        let mut buf = [0u8; 64];
        let start = std::time::Instant::now();
        let n = tokio::time::timeout(Duration::from_secs(5), transport.read(&mut buf))
            .await
            .expect("read should be woken by the queued output")
            .expect("read should succeed");
        let elapsed = start.elapsed();

        assert_eq!(&buf[..n], b"late output");
        assert!(
            elapsed < Duration::from_secs(1),
            "read was not woken by the queued output; it completed only after {elapsed:?}"
        );
    }

    /// EOF signalled while a read is parked has to wake it too.
    #[tokio::test]
    async fn signalled_eof_wakes_a_parked_read() {
        let transport = MockTransport::new();
        let handle = transport.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            handle.signal_eof();
        });

        let mut transport = transport;
        let mut buf = [0u8; 64];
        let n = tokio::time::timeout(Duration::from_secs(5), transport.read(&mut buf))
            .await
            .expect("read should be woken by the EOF signal")
            .expect("read should succeed");
        assert_eq!(n, 0, "EOF reads as zero bytes");
    }

    /// A scripted delay has to hold the output behind it for about that long —
    /// and then release it. Both bounds matter: the delay was previously a
    /// no-op that only looked like a pause because the read stalled until the
    /// caller's timeout, which satisfies a lower bound just as well.
    #[tokio::test]
    async fn scripted_delay_holds_then_releases() {
        let timeline = EventTimeline::from_events(vec![
            MockEvent::output_str("first"),
            MockEvent::delay_ms(150),
            MockEvent::output_str("second"),
        ]);
        let mut transport = MockTransport::from_timeline(timeline);

        let mut buf = [0u8; 64];
        let n = transport.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], b"first");

        let start = std::time::Instant::now();
        let n = tokio::time::timeout(Duration::from_secs(5), transport.read(&mut buf))
            .await
            .expect("the delayed output should arrive once the delay elapses")
            .expect("read should succeed");
        let elapsed = start.elapsed();

        assert_eq!(&buf[..n], b"second");
        assert!(
            elapsed >= Duration::from_millis(140),
            "delay was not honoured, output arrived after {elapsed:?}"
        );
        assert!(
            elapsed < Duration::from_secs(1),
            "read stalled well past the 150ms delay ({elapsed:?}); the delay is \
             holding until something else re-polls rather than arming a timer"
        );
    }

    /// A scenario step's `expect` used to be dropped when the scenario was
    /// converted to a timeline, so the response was emitted whether or not the
    /// client ever sent anything.
    #[tokio::test]
    async fn scenario_expect_gates_the_response() {
        let scenario = Scenario::new("login").expect_respond("secret", "welcome\n");
        let mut transport = MockTransport::from_scenario(&scenario);

        let mut buf = [0u8; 64];
        let premature =
            tokio::time::timeout(Duration::from_millis(150), transport.read(&mut buf)).await;
        assert!(
            premature.is_err(),
            "response must not be delivered before the expected input was sent"
        );

        transport.write_all(b"secret\n").await.unwrap();
        let n = tokio::time::timeout(Duration::from_secs(5), transport.read(&mut buf))
            .await
            .expect("the matching write should release the response")
            .expect("read should succeed");
        assert_eq!(&buf[..n], b"welcome\n");
    }

    /// The gate matches on what was written even if the test has already read
    /// the input back, and a single write does not satisfy two gates.
    #[tokio::test]
    async fn scenario_expect_survives_take_input() {
        let scenario = Scenario::new("two-step")
            .expect_respond("one", "first\n")
            .expect_respond("two", "second\n");
        let mut transport = MockTransport::from_scenario(&scenario);

        transport.write_all(b"one\n").await.unwrap();
        assert_eq!(transport.take_input_str(), "one\n");

        let mut buf = [0u8; 64];
        let n = transport.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], b"first\n");

        let premature =
            tokio::time::timeout(Duration::from_millis(150), transport.read(&mut buf)).await;
        assert!(
            premature.is_err(),
            "the second gate must not be satisfied by the first write"
        );

        transport.write_all(b"two\n").await.unwrap();
        let n = tokio::time::timeout(Duration::from_secs(5), transport.read(&mut buf))
            .await
            .expect("second response should be released")
            .expect("read should succeed");
        assert_eq!(&buf[..n], b"second\n");
    }

    #[tokio::test]
    async fn mock_transport_from_timeline() {
        let timeline = EventTimeline::from_events(vec![
            MockEvent::output_str("Welcome\n"),
            MockEvent::output_str("Login: "),
            MockEvent::eof(),
        ]);

        let mut transport = MockTransport::from_timeline(timeline);

        let mut buf = vec![0u8; 100];
        let n = transport.read(&mut buf).await.unwrap();
        assert!(n > 0);
    }
}
