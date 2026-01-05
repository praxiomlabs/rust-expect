//! Test session for testing.
//!
//! Provides a simple session mock for unit testing expect operations
//! without spawning actual processes.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::time::Duration;

/// A recorded interaction.
#[derive(Debug, Clone)]
pub struct RecordedInteraction {
    /// Direction of the interaction.
    pub direction: InteractionDirection,
    /// The data.
    pub data: Vec<u8>,
    /// Timestamp (relative to session start).
    pub timestamp: Duration,
}

impl RecordedInteraction {
    /// Create an input interaction.
    #[must_use]
    pub fn input(data: impl Into<Vec<u8>>, timestamp: Duration) -> Self {
        Self {
            direction: InteractionDirection::Input,
            data: data.into(),
            timestamp,
        }
    }

    /// Create an output interaction.
    #[must_use]
    pub fn output(data: impl Into<Vec<u8>>, timestamp: Duration) -> Self {
        Self {
            direction: InteractionDirection::Output,
            data: data.into(),
            timestamp,
        }
    }
}

/// Direction of interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionDirection {
    /// Data sent to the session.
    Input,
    /// Data received from the session.
    Output,
}

/// A test session for unit testing.
#[derive(Debug)]
pub struct TestSession {
    /// Queued responses.
    responses: VecDeque<Vec<u8>>,
    /// Recorded interactions.
    interactions: Vec<RecordedInteraction>,
    /// Session start time.
    start_time: std::time::Instant,
    /// Whether the session is closed.
    closed: bool,
}

impl TestSession {
    /// Create a new test session.
    #[must_use]
    pub fn new() -> Self {
        Self {
            responses: VecDeque::new(),
            interactions: Vec::new(),
            start_time: std::time::Instant::now(),
            closed: false,
        }
    }

    /// Create with a builder.
    #[must_use]
    pub fn builder() -> TestSessionBuilder {
        TestSessionBuilder::new()
    }

    /// Queue a response.
    pub fn queue_response(&mut self, data: impl Into<Vec<u8>>) {
        self.responses.push_back(data.into());
    }

    /// Queue a string response.
    pub fn queue_response_str(&mut self, s: &str) {
        self.queue_response(s.as_bytes().to_vec());
    }

    /// Queue a line response (with CRLF).
    pub fn queue_line(&mut self, line: &str) {
        let mut data = line.as_bytes().to_vec();
        data.extend_from_slice(b"\r\n");
        self.queue_response(data);
    }

    /// Simulate sending data (records and discards).
    pub fn send(&mut self, data: &[u8]) {
        let elapsed = self.start_time.elapsed();
        self.interactions
            .push(RecordedInteraction::input(data.to_vec(), elapsed));
    }

    /// Simulate receiving data.
    pub fn receive(&mut self) -> Option<Vec<u8>> {
        let data = self.responses.pop_front()?;
        let elapsed = self.start_time.elapsed();
        self.interactions
            .push(RecordedInteraction::output(data.clone(), elapsed));
        Some(data)
    }

    /// Get all recorded interactions.
    #[must_use]
    pub fn interactions(&self) -> &[RecordedInteraction] {
        &self.interactions
    }

    /// Get only input interactions.
    #[must_use]
    pub fn inputs(&self) -> Vec<&RecordedInteraction> {
        self.interactions
            .iter()
            .filter(|i| i.direction == InteractionDirection::Input)
            .collect()
    }

    /// Get only output interactions.
    #[must_use]
    pub fn outputs(&self) -> Vec<&RecordedInteraction> {
        self.interactions
            .iter()
            .filter(|i| i.direction == InteractionDirection::Output)
            .collect()
    }

    /// Check if the session has pending responses.
    #[must_use]
    pub fn has_pending(&self) -> bool {
        !self.responses.is_empty()
    }

    /// Close the session.
    pub const fn close(&mut self) {
        self.closed = true;
    }

    /// Check if closed.
    #[must_use]
    pub const fn is_closed(&self) -> bool {
        self.closed
    }

    /// Get all input as a combined string.
    #[must_use]
    pub fn all_input_str(&self) -> String {
        let bytes: Vec<u8> = self.inputs().iter().flat_map(|i| i.data.clone()).collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// Get all output as a combined string.
    #[must_use]
    pub fn all_output_str(&self) -> String {
        let bytes: Vec<u8> = self.outputs().iter().flat_map(|i| i.data.clone()).collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// Assert that a specific string was sent.
    pub fn assert_sent(&self, needle: &str) {
        let input = self.all_input_str();
        assert!(
            input.contains(needle),
            "Expected to send {needle:?}, but sent:\n{input}"
        );
    }

    /// Assert that a specific string was NOT sent.
    pub fn assert_not_sent(&self, needle: &str) {
        let input = self.all_input_str();
        assert!(
            !input.contains(needle),
            "Expected NOT to send {needle:?}, but found it in:\n{input}"
        );
    }
}

impl Default for TestSession {
    fn default() -> Self {
        Self::new()
    }
}

impl Read for TestSession {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.closed {
            return Ok(0);
        }
        match self.receive() {
            Some(data) => {
                let len = buf.len().min(data.len());
                buf[..len].copy_from_slice(&data[..len]);
                Ok(len)
            }
            None => Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "No data available",
            )),
        }
    }
}

impl Write for TestSession {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.closed {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "Session closed",
            ));
        }
        self.send(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Builder for test sessions.
#[derive(Debug, Default)]
pub struct TestSessionBuilder {
    responses: Vec<Vec<u8>>,
}

impl TestSessionBuilder {
    /// Create a new builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a response.
    #[must_use]
    pub fn response(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.responses.push(data.into());
        self
    }

    /// Queue a string response.
    #[must_use]
    pub fn response_str(self, s: &str) -> Self {
        self.response(s.as_bytes().to_vec())
    }

    /// Queue a line response.
    #[must_use]
    pub fn line(self, line: &str) -> Self {
        let mut data = line.as_bytes().to_vec();
        data.extend_from_slice(b"\r\n");
        self.response(data)
    }

    /// Queue a prompt.
    #[must_use]
    pub fn prompt(self, prompt: &str) -> Self {
        self.response_str(prompt)
    }

    /// Simulate a login sequence.
    #[must_use]
    pub fn login_sequence(self, username: &str, password: &str) -> Self {
        self.prompt("Login: ")
            .line(username)
            .prompt("Password: ")
            .line(password)
            .line("Welcome!")
            .prompt("$ ")
    }

    /// Build the test session.
    #[must_use]
    pub fn build(self) -> TestSession {
        let mut session = TestSession::new();
        for response in self.responses {
            session.queue_response(response);
        }
        session
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_basic() {
        let mut session = TestSession::new();
        session.queue_response_str("Hello");
        session.queue_response_str("World");

        assert_eq!(session.receive(), Some(b"Hello".to_vec()));
        session.send(b"test");
        assert_eq!(session.receive(), Some(b"World".to_vec()));

        assert_eq!(session.inputs().len(), 1);
        assert_eq!(session.outputs().len(), 2);
    }

    #[test]
    fn test_session_builder() {
        let mut session = TestSession::builder()
            .prompt("$ ")
            .line("output line")
            .build();

        assert_eq!(session.receive(), Some(b"$ ".to_vec()));
        assert_eq!(session.receive(), Some(b"output line\r\n".to_vec()));
    }

    #[test]
    fn test_session_assertions() {
        let mut session = TestSession::new();
        session.send(b"hello world");
        session.send(b"test");

        session.assert_sent("hello");
        session.assert_sent("world");
        session.assert_not_sent("goodbye");
    }
}
