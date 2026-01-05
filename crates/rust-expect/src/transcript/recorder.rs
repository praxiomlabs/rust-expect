//! Session recording.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::format::{Transcript, TranscriptEvent, TranscriptMetadata};

/// A session recorder.
#[derive(Debug)]
pub struct Recorder {
    /// Start time.
    start: Instant,
    /// Recorded transcript.
    transcript: Arc<Mutex<Transcript>>,
    /// Whether recording is active.
    recording: bool,
    /// Maximum recording duration.
    max_duration: Option<Duration>,
    /// Maximum events to record.
    max_events: Option<usize>,
}

impl Recorder {
    /// Create a new recorder.
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            start: Instant::now(),
            transcript: Arc::new(Mutex::new(Transcript::new(TranscriptMetadata::new(
                width, height,
            )))),
            recording: true,
            max_duration: None,
            max_events: None,
        }
    }

    /// Set maximum duration.
    #[must_use]
    pub const fn with_max_duration(mut self, duration: Duration) -> Self {
        self.max_duration = Some(duration);
        self
    }

    /// Set maximum events.
    #[must_use]
    pub const fn with_max_events(mut self, count: usize) -> Self {
        self.max_events = Some(count);
        self
    }

    /// Get elapsed time since start.
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// Check if recording is active.
    #[must_use]
    pub const fn is_recording(&self) -> bool {
        self.recording
    }

    /// Stop recording.
    pub fn stop(&mut self) {
        self.recording = false;
        if let Ok(mut t) = self.transcript.lock() {
            t.metadata.duration = Some(self.elapsed());
        }
    }

    /// Record an output event.
    pub fn record_output(&self, data: &[u8]) {
        if !self.should_record() {
            return;
        }
        self.push_event(TranscriptEvent::output(self.elapsed(), data));
    }

    /// Record an input event.
    pub fn record_input(&self, data: &[u8]) {
        if !self.should_record() {
            return;
        }
        self.push_event(TranscriptEvent::input(self.elapsed(), data));
    }

    /// Record a resize event.
    pub fn record_resize(&self, cols: u16, rows: u16) {
        if !self.should_record() {
            return;
        }
        self.push_event(TranscriptEvent::resize(self.elapsed(), cols, rows));
    }

    /// Add a marker.
    pub fn add_marker(&self, label: &str) {
        if !self.should_record() {
            return;
        }
        self.push_event(TranscriptEvent::marker(self.elapsed(), label));
    }

    /// Check if we should still record.
    fn should_record(&self) -> bool {
        if !self.recording {
            return false;
        }

        if let Some(max_dur) = self.max_duration
            && self.elapsed() > max_dur
        {
            return false;
        }

        if let Some(max_events) = self.max_events
            && let Ok(t) = self.transcript.lock()
            && t.events.len() >= max_events
        {
            return false;
        }

        true
    }

    /// Push an event to the transcript.
    fn push_event(&self, event: TranscriptEvent) {
        if let Ok(mut t) = self.transcript.lock() {
            t.push(event);
        }
    }

    /// Get the transcript.
    #[must_use]
    pub fn transcript(&self) -> Arc<Mutex<Transcript>> {
        Arc::clone(&self.transcript)
    }

    /// Take the transcript, consuming the recorder.
    #[must_use]
    pub fn into_transcript(self) -> Transcript {
        Arc::try_unwrap(self.transcript)
            .ok()
            .and_then(|m| m.into_inner().ok())
            .unwrap_or_else(|| Transcript::new(TranscriptMetadata::new(80, 24)))
    }

    /// Get event count.
    #[must_use]
    pub fn event_count(&self) -> usize {
        self.transcript.lock().map(|t| t.events.len()).unwrap_or(0)
    }
}

/// Builder for creating recorders.
#[derive(Debug, Default)]
pub struct RecorderBuilder {
    width: u16,
    height: u16,
    command: Option<String>,
    title: Option<String>,
    max_duration: Option<Duration>,
    max_events: Option<usize>,
}

impl RecorderBuilder {
    /// Create a new builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            width: 80,
            height: 24,
            ..Default::default()
        }
    }

    /// Set dimensions.
    #[must_use]
    pub const fn size(mut self, width: u16, height: u16) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Set command.
    #[must_use]
    pub fn command(mut self, cmd: impl Into<String>) -> Self {
        self.command = Some(cmd.into());
        self
    }

    /// Set title.
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set maximum duration.
    #[must_use]
    pub const fn max_duration(mut self, duration: Duration) -> Self {
        self.max_duration = Some(duration);
        self
    }

    /// Set maximum events.
    #[must_use]
    pub const fn max_events(mut self, count: usize) -> Self {
        self.max_events = Some(count);
        self
    }

    /// Build the recorder.
    #[must_use]
    pub fn build(self) -> Recorder {
        let mut recorder = Recorder::new(self.width, self.height);

        if let Some(duration) = self.max_duration {
            recorder.max_duration = Some(duration);
        }
        if let Some(events) = self.max_events {
            recorder.max_events = Some(events);
        }

        if let Ok(mut t) = recorder.transcript.lock() {
            t.metadata.command = self.command;
            t.metadata.title = self.title;
        }

        recorder
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorder_basic() {
        let recorder = Recorder::new(80, 24);
        recorder.record_output(b"hello");
        recorder.record_input(b"world");

        assert_eq!(recorder.event_count(), 2);
    }

    #[test]
    fn recorder_stop() {
        let mut recorder = Recorder::new(80, 24);
        recorder.record_output(b"before");
        recorder.stop();
        recorder.record_output(b"after");

        assert_eq!(recorder.event_count(), 1);
    }

    #[test]
    fn recorder_builder() {
        let recorder = RecorderBuilder::new()
            .size(120, 40)
            .title("Test")
            .max_events(10)
            .build();

        assert!(recorder.is_recording());
    }
}
