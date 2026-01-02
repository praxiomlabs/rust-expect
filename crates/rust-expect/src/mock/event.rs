//! Mock events for testing expect scripts.
//!
//! This module provides event types for recording and replaying
//! interactions in mock sessions.

use std::time::Duration;

/// An event that can occur during a session.
#[derive(Debug, Clone)]
pub enum MockEvent {
    /// Data received from the process.
    Output(Vec<u8>),
    /// Data sent to the process.
    Input(Vec<u8>),
    /// Delay before next event.
    Delay(Duration),
    /// Process terminated with exit code.
    Exit(i32),
    /// End of file reached.
    Eof,
    /// Error occurred.
    Error(String),
    /// Window size changed.
    Resize {
        /// Number of rows.
        rows: u16,
        /// Number of columns.
        cols: u16,
    },
}

impl MockEvent {
    /// Create an output event from bytes.
    pub fn output(data: impl Into<Vec<u8>>) -> Self {
        Self::Output(data.into())
    }

    /// Create an output event from a string.
    #[must_use] pub fn output_str(s: &str) -> Self {
        Self::Output(s.as_bytes().to_vec())
    }

    /// Create an input event from bytes.
    pub fn input(data: impl Into<Vec<u8>>) -> Self {
        Self::Input(data.into())
    }

    /// Create an input event from a string.
    #[must_use] pub fn input_str(s: &str) -> Self {
        Self::Input(s.as_bytes().to_vec())
    }

    /// Create a delay event.
    #[must_use] pub const fn delay(duration: Duration) -> Self {
        Self::Delay(duration)
    }

    /// Create a delay event from milliseconds.
    #[must_use] pub const fn delay_ms(ms: u64) -> Self {
        Self::Delay(Duration::from_millis(ms))
    }

    /// Create an exit event.
    #[must_use] pub const fn exit(code: i32) -> Self {
        Self::Exit(code)
    }

    /// Create an EOF event.
    #[must_use] pub const fn eof() -> Self {
        Self::Eof
    }

    /// Create an error event.
    pub fn error(msg: impl Into<String>) -> Self {
        Self::Error(msg.into())
    }

    /// Create a resize event.
    #[must_use] pub const fn resize(rows: u16, cols: u16) -> Self {
        Self::Resize { rows, cols }
    }

    /// Check if this is an output event.
    #[must_use]
    pub const fn is_output(&self) -> bool {
        matches!(self, Self::Output(_))
    }

    /// Check if this is an input event.
    #[must_use]
    pub const fn is_input(&self) -> bool {
        matches!(self, Self::Input(_))
    }

    /// Check if this is a delay event.
    #[must_use]
    pub const fn is_delay(&self) -> bool {
        matches!(self, Self::Delay(_))
    }

    /// Check if this is an exit event.
    #[must_use]
    pub const fn is_exit(&self) -> bool {
        matches!(self, Self::Exit(_))
    }

    /// Check if this is an EOF event.
    #[must_use]
    pub const fn is_eof(&self) -> bool {
        matches!(self, Self::Eof)
    }
}

/// A timeline of events for a mock session.
#[derive(Debug, Clone, Default)]
pub struct EventTimeline {
    events: Vec<MockEvent>,
    position: usize,
}

impl EventTimeline {
    /// Create a new empty timeline.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a timeline from a list of events.
    #[must_use]
    pub const fn from_events(events: Vec<MockEvent>) -> Self {
        Self { events, position: 0 }
    }

    /// Add an event to the timeline.
    pub fn push(&mut self, event: MockEvent) {
        self.events.push(event);
    }

    /// Get the next event.
    pub fn next(&mut self) -> Option<&MockEvent> {
        if self.position < self.events.len() {
            let event = &self.events[self.position];
            self.position += 1;
            Some(event)
        } else {
            None
        }
    }

    /// Peek at the next event without advancing.
    #[must_use]
    pub fn peek(&self) -> Option<&MockEvent> {
        self.events.get(self.position)
    }

    /// Reset the timeline to the beginning.
    pub fn reset(&mut self) {
        self.position = 0;
    }

    /// Check if there are more events.
    #[must_use]
    pub fn has_more(&self) -> bool {
        self.position < self.events.len()
    }

    /// Get the number of remaining events.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.events.len().saturating_sub(self.position)
    }

    /// Get all events.
    #[must_use]
    pub fn events(&self) -> &[MockEvent] {
        &self.events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_basic() {
        let mut timeline = EventTimeline::new();
        timeline.push(MockEvent::output_str("hello"));
        timeline.push(MockEvent::delay_ms(100));
        timeline.push(MockEvent::exit(0));

        assert!(timeline.has_more());
        assert!(timeline.next().unwrap().is_output());
        assert!(timeline.next().unwrap().is_delay());
        assert!(timeline.next().unwrap().is_exit());
        assert!(!timeline.has_more());
    }

    #[test]
    fn timeline_reset() {
        let mut timeline = EventTimeline::from_events(vec![
            MockEvent::output_str("test"),
            MockEvent::eof(),
        ]);

        assert_eq!(timeline.remaining(), 2);
        timeline.next();
        assert_eq!(timeline.remaining(), 1);
        timeline.reset();
        assert_eq!(timeline.remaining(), 2);
    }
}
