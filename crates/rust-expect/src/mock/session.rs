//! Mock session implementation for testing.
//!
//! This module provides a mock session that can be used for testing
//! expect scripts without spawning real processes.

use super::event::{EventTimeline, MockEvent};
use super::scenario::Scenario;
use std::collections::VecDeque;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// Shared state for the mock transport.
#[derive(Debug)]
struct MockState {
    /// Output buffer (data to be read by the client).
    output: VecDeque<u8>,
    /// Input buffer (data written by the client).
    input: VecDeque<u8>,
    /// Event timeline.
    timeline: EventTimeline,
    /// Whether EOF has been signaled.
    eof: bool,
    /// Error to return on next read.
    error: Option<String>,
    /// Exit code if exited.
    exit_code: Option<i32>,
}

impl MockState {
    const fn new(timeline: EventTimeline) -> Self {
        Self {
            output: VecDeque::new(),
            input: VecDeque::new(),
            timeline,
            eof: false,
            error: None,
            exit_code: None,
        }
    }

    fn process_event(&mut self) {
        if let Some(event) = self.timeline.next() {
            match event.clone() {
                MockEvent::Output(data) => {
                    self.output.extend(data);
                }
                MockEvent::Eof => {
                    self.eof = true;
                }
                MockEvent::Error(msg) => {
                    self.error = Some(msg);
                }
                MockEvent::Exit(code) => {
                    self.exit_code = Some(code);
                    self.eof = true;
                }
                MockEvent::Input(_) | MockEvent::Delay(_) | MockEvent::Resize { .. } => {
                    // These are handled differently
                }
            }
        }
    }
}

/// A mock transport for testing.
#[derive(Debug, Clone)]
pub struct MockTransport {
    state: Arc<Mutex<MockState>>,
}

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

    /// Queue output to be read.
    pub fn queue_output(&self, data: &[u8]) {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.output.extend(data);
    }

    /// Queue a string to be read.
    pub fn queue_output_str(&self, s: &str) {
        self.queue_output(s.as_bytes());
    }

    /// Get data that was written by the client.
    #[must_use] pub fn take_input(&self) -> Vec<u8> {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.input.drain(..).collect()
    }

    /// Get input as a string.
    #[must_use] pub fn take_input_str(&self) -> String {
        String::from_utf8_lossy(&self.take_input()).into_owned()
    }

    /// Signal EOF.
    pub fn signal_eof(&self) {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.eof = true;
    }

    /// Signal exit with code.
    pub fn signal_exit(&self, code: i32) {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.exit_code = Some(code);
        state.eof = true;
    }

    /// Set an error to return on next read.
    pub fn set_error(&self, msg: impl Into<String>) {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.error = Some(msg.into());
    }

    /// Check if EOF has been signaled.
    #[must_use]
    pub fn is_eof(&self) -> bool {
        let state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.eof
    }

    /// Get the exit code if exited.
    #[must_use]
    pub fn exit_code(&self) -> Option<i32> {
        let state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.exit_code
    }

    /// Process the next event from the timeline.
    pub fn advance(&self) {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.process_event();
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
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);

        // Check for error
        if let Some(error) = state.error.take() {
            return Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, error)));
        }

        // Check for EOF
        if state.output.is_empty() && state.eof {
            return Poll::Ready(Ok(()));
        }

        // Read available data
        if state.output.is_empty() {
            // Process next event and try again
            state.process_event();
            if !state.output.is_empty() {
                let to_read = buf.remaining().min(state.output.len());
                for _ in 0..to_read {
                    if let Some(byte) = state.output.pop_front() {
                        buf.put_slice(&[byte]);
                    }
                }
                Poll::Ready(Ok(()))
            } else if state.eof {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        } else {
            let to_read = buf.remaining().min(state.output.len());
            for _ in 0..to_read {
                if let Some(byte) = state.output.pop_front() {
                    buf.put_slice(&[byte]);
                }
            }
            Poll::Ready(Ok(()))
        }
    }
}

impl AsyncWrite for MockTransport {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.input.extend(buf);
        Poll::Ready(Ok(buf.len()))
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
    pub fn transport_mut(&mut self) -> &mut MockTransport {
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
    #[must_use] pub fn take_input(&self) -> Vec<u8> {
        self.transport.take_input()
    }

    /// Get input as a string.
    #[must_use] pub fn take_input_str(&self) -> String {
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
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
