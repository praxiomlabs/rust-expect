//! Transcript format definitions.

use std::time::Duration;

/// A transcript event.
#[derive(Debug, Clone)]
pub struct TranscriptEvent {
    /// Timestamp from start.
    pub timestamp: Duration,
    /// Event type.
    pub event_type: EventType,
    /// Event data.
    pub data: Vec<u8>,
}

/// Event types in a transcript.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    /// Output from the session.
    Output,
    /// Input to the session.
    Input,
    /// Window resize.
    Resize,
    /// Marker/annotation.
    Marker,
}

impl TranscriptEvent {
    /// Create an output event.
    #[must_use]
    pub fn output(timestamp: Duration, data: impl Into<Vec<u8>>) -> Self {
        Self {
            timestamp,
            event_type: EventType::Output,
            data: data.into(),
        }
    }

    /// Create an input event.
    #[must_use]
    pub fn input(timestamp: Duration, data: impl Into<Vec<u8>>) -> Self {
        Self {
            timestamp,
            event_type: EventType::Input,
            data: data.into(),
        }
    }

    /// Create a resize event.
    #[must_use]
    pub fn resize(timestamp: Duration, cols: u16, rows: u16) -> Self {
        Self {
            timestamp,
            event_type: EventType::Resize,
            data: format!("{cols}x{rows}").into_bytes(),
        }
    }

    /// Create a marker event.
    #[must_use]
    pub fn marker(timestamp: Duration, label: &str) -> Self {
        Self {
            timestamp,
            event_type: EventType::Marker,
            data: label.as_bytes().to_vec(),
        }
    }
}

/// Transcript metadata.
#[derive(Debug, Clone, Default)]
pub struct TranscriptMetadata {
    /// Terminal width.
    pub width: u16,
    /// Terminal height.
    pub height: u16,
    /// Command that was run.
    pub command: Option<String>,
    /// Title for the transcript.
    pub title: Option<String>,
    /// Environment info.
    pub env: std::collections::HashMap<String, String>,
    /// Start time.
    pub timestamp: Option<u64>,
    /// Total duration.
    pub duration: Option<Duration>,
}

impl TranscriptMetadata {
    /// Create new metadata with dimensions.
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            ..Default::default()
        }
    }

    /// Set the command.
    #[must_use]
    pub fn with_command(mut self, cmd: impl Into<String>) -> Self {
        self.command = Some(cmd.into());
        self
    }

    /// Set the title.
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }
}

/// A complete transcript.
#[derive(Debug, Clone)]
pub struct Transcript {
    /// Metadata.
    pub metadata: TranscriptMetadata,
    /// Events.
    pub events: Vec<TranscriptEvent>,
}

impl Transcript {
    /// Create a new transcript.
    #[must_use]
    pub const fn new(metadata: TranscriptMetadata) -> Self {
        Self {
            metadata,
            events: Vec::new(),
        }
    }

    /// Add an event.
    pub fn push(&mut self, event: TranscriptEvent) {
        self.events.push(event);
    }

    /// Get total duration.
    #[must_use]
    pub fn duration(&self) -> Duration {
        self.events.last().map_or(Duration::ZERO, |e| e.timestamp)
    }

    /// Get all output as a string.
    #[must_use]
    pub fn output_text(&self) -> String {
        let output: Vec<u8> = self
            .events
            .iter()
            .filter(|e| e.event_type == EventType::Output)
            .flat_map(|e| e.data.clone())
            .collect();
        String::from_utf8_lossy(&output).into_owned()
    }

    /// Get all input as a string.
    #[must_use]
    pub fn input_text(&self) -> String {
        let input: Vec<u8> = self
            .events
            .iter()
            .filter(|e| e.event_type == EventType::Input)
            .flat_map(|e| e.data.clone())
            .collect();
        String::from_utf8_lossy(&input).into_owned()
    }

    /// Filter events by type.
    #[must_use]
    pub fn filter(&self, event_type: EventType) -> Vec<&TranscriptEvent> {
        self.events
            .iter()
            .filter(|e| e.event_type == event_type)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_events() {
        let mut transcript = Transcript::new(TranscriptMetadata::new(80, 24));
        transcript.push(TranscriptEvent::output(
            Duration::from_millis(100),
            b"hello",
        ));
        transcript.push(TranscriptEvent::input(Duration::from_millis(200), b"world"));

        assert_eq!(transcript.events.len(), 2);
        assert_eq!(transcript.duration(), Duration::from_millis(200));
    }

    #[test]
    fn transcript_output_text() {
        let mut transcript = Transcript::new(TranscriptMetadata::new(80, 24));
        transcript.push(TranscriptEvent::output(Duration::ZERO, b"hello "));
        transcript.push(TranscriptEvent::output(
            Duration::from_millis(100),
            b"world",
        ));

        assert_eq!(transcript.output_text(), "hello world");
    }
}
