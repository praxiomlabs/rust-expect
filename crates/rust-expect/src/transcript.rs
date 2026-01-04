//! Session transcripts and recording.
//!
//! This module provides functionality for recording and playing back
//! terminal sessions, with support for the asciicast v2 format.

pub mod asciicast;
pub mod format;
pub mod player;
pub mod recorder;

pub use asciicast::{AsciicastHeader, read_asciicast, write_asciicast};
pub use format::{EventType, Transcript, TranscriptEvent, TranscriptMetadata};
pub use player::{PlaybackOptions, PlaybackSpeed, Player, PlayerState, play_to_stdout};
pub use recorder::{Recorder, RecorderBuilder};
