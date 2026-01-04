//! Transcript playback.

use super::format::{EventType, Transcript, TranscriptEvent};
use std::io::Write;
use std::time::{Duration, Instant};

/// Playback speed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaybackSpeed {
    /// Real-time playback.
    Realtime,
    /// Fixed speed multiplier.
    Speed(f64),
    /// Maximum speed (instant).
    Instant,
}

impl Default for PlaybackSpeed {
    fn default() -> Self {
        Self::Realtime
    }
}

/// Playback options.
#[derive(Debug, Clone)]
pub struct PlaybackOptions {
    /// Playback speed.
    pub speed: PlaybackSpeed,
    /// Maximum idle time between events.
    pub max_idle: Duration,
    /// Whether to show input events.
    pub show_input: bool,
    /// Whether to pause at markers.
    pub pause_at_markers: bool,
}

impl Default for PlaybackOptions {
    fn default() -> Self {
        Self {
            speed: PlaybackSpeed::Realtime,
            max_idle: Duration::from_secs(5),
            show_input: false,
            pause_at_markers: false,
        }
    }
}

impl PlaybackOptions {
    /// Create new options.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set playback speed.
    #[must_use]
    pub const fn with_speed(mut self, speed: PlaybackSpeed) -> Self {
        self.speed = speed;
        self
    }

    /// Set maximum idle time.
    #[must_use]
    pub const fn with_max_idle(mut self, max: Duration) -> Self {
        self.max_idle = max;
        self
    }

    /// Show input events.
    #[must_use]
    pub const fn with_show_input(mut self, show: bool) -> Self {
        self.show_input = show;
        self
    }
}

/// Transcript player state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    /// Not started.
    Stopped,
    /// Playing.
    Playing,
    /// Paused.
    Paused,
    /// Finished.
    Finished,
}

/// A transcript player.
pub struct Player<'a> {
    /// The transcript to play.
    transcript: &'a Transcript,
    /// Current event index.
    index: usize,
    /// Playback options.
    options: PlaybackOptions,
    /// Player state.
    state: PlayerState,
    /// Playback start time.
    start_time: Option<Instant>,
    /// Last event time.
    last_event_time: Duration,
}

impl<'a> Player<'a> {
    /// Create a new player.
    #[must_use]
    pub fn new(transcript: &'a Transcript) -> Self {
        Self {
            transcript,
            index: 0,
            options: PlaybackOptions::default(),
            state: PlayerState::Stopped,
            start_time: None,
            last_event_time: Duration::ZERO,
        }
    }

    /// Set playback options.
    #[must_use]
    pub const fn with_options(mut self, options: PlaybackOptions) -> Self {
        self.options = options;
        self
    }

    /// Get current state.
    #[must_use]
    pub const fn state(&self) -> PlayerState {
        self.state
    }

    /// Get current position (event index).
    #[must_use]
    pub const fn position(&self) -> usize {
        self.index
    }

    /// Get total events.
    #[must_use]
    pub fn total_events(&self) -> usize {
        self.transcript.events.len()
    }

    /// Get current timestamp.
    #[must_use]
    pub fn current_time(&self) -> Duration {
        if self.index < self.transcript.events.len() {
            self.transcript.events[self.index].timestamp
        } else {
            self.transcript.duration()
        }
    }

    /// Start playback.
    pub fn play(&mut self) {
        self.state = PlayerState::Playing;
        self.start_time = Some(Instant::now());
    }

    /// Pause playback.
    pub fn pause(&mut self) {
        self.state = PlayerState::Paused;
    }

    /// Stop playback.
    pub fn stop(&mut self) {
        self.state = PlayerState::Stopped;
        self.index = 0;
        self.start_time = None;
        self.last_event_time = Duration::ZERO;
    }

    /// Seek to a specific event.
    pub fn seek(&mut self, index: usize) {
        self.index = index.min(self.transcript.events.len());
        if self.index < self.transcript.events.len() {
            self.last_event_time = self.transcript.events[self.index].timestamp;
        }
    }

    /// Get next event to play.
    pub fn next_event(&mut self) -> Option<&TranscriptEvent> {
        if self.state != PlayerState::Playing || self.index >= self.transcript.events.len() {
            if self.index >= self.transcript.events.len() {
                self.state = PlayerState::Finished;
            }
            return None;
        }

        let event = &self.transcript.events[self.index];
        self.index += 1;
        self.last_event_time = event.timestamp;
        Some(event)
    }

    /// Calculate delay before next event.
    #[must_use]
    pub fn delay_to_next(&self) -> Duration {
        if self.index >= self.transcript.events.len() {
            return Duration::ZERO;
        }

        let next_time = self.transcript.events[self.index].timestamp;
        let delay = next_time.saturating_sub(self.last_event_time);

        // Apply speed
        let delay = match self.options.speed {
            PlaybackSpeed::Instant => Duration::ZERO,
            PlaybackSpeed::Realtime => delay,
            PlaybackSpeed::Speed(mult) => Duration::from_secs_f64(delay.as_secs_f64() / mult),
        };

        // Apply max idle
        delay.min(self.options.max_idle)
    }

    /// Play to a writer (blocking).
    pub fn play_to<W: Write>(&mut self, writer: &mut W) -> std::io::Result<()> {
        self.play();

        while let Some(event) = self.next_event() {
            // Clone what we need from event to release the borrow
            let event_type = event.event_type;
            let event_data = event.data.clone();

            // Now we can use self again
            let delay = self.delay_to_next();
            if delay > Duration::ZERO {
                std::thread::sleep(delay);
            }

            // Handle event using cloned data
            match event_type {
                EventType::Output => {
                    writer.write_all(&event_data)?;
                    writer.flush()?;
                }
                EventType::Input if self.options.show_input => {
                    // Could highlight input differently
                    writer.write_all(&event_data)?;
                    writer.flush()?;
                }
                EventType::Marker if self.options.pause_at_markers => {
                    self.pause();
                    // User would need to call play() to resume
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }
}

/// Play a transcript to stdout.
pub fn play_to_stdout(transcript: &Transcript, options: PlaybackOptions) -> std::io::Result<()> {
    let mut player = Player::new(transcript).with_options(options);
    let mut stdout = std::io::stdout();
    player.play_to(&mut stdout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::format::TranscriptMetadata;

    #[test]
    fn player_basic() {
        let mut transcript = Transcript::new(TranscriptMetadata::new(80, 24));
        transcript.push(TranscriptEvent::output(Duration::ZERO, b"hello"));
        transcript.push(TranscriptEvent::output(
            Duration::from_millis(100),
            b" world",
        ));

        let mut player = Player::new(&transcript);
        assert_eq!(player.total_events(), 2);
        assert_eq!(player.state(), PlayerState::Stopped);

        player.play();
        assert_eq!(player.state(), PlayerState::Playing);

        let event = player.next_event().unwrap();
        assert_eq!(event.data, b"hello");

        let event = player.next_event().unwrap();
        assert_eq!(event.data, b" world");

        assert!(player.next_event().is_none());
        assert_eq!(player.state(), PlayerState::Finished);
    }

    #[test]
    fn player_instant_speed() {
        let mut transcript = Transcript::new(TranscriptMetadata::new(80, 24));
        transcript.push(TranscriptEvent::output(Duration::ZERO, b"a"));
        transcript.push(TranscriptEvent::output(Duration::from_secs(10), b"b"));

        let player = Player::new(&transcript)
            .with_options(PlaybackOptions::new().with_speed(PlaybackSpeed::Instant));

        assert_eq!(player.delay_to_next(), Duration::ZERO);
    }
}
