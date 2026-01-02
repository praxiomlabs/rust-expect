//! Session transcripts and recording.
//!
//! This module provides functionality for recording and playing back
//! terminal sessions, with support for the asciicast v2 format.

pub mod asciicast;
pub mod format;
pub mod player;
pub mod recorder;

pub use asciicast::{read_asciicast, write_asciicast, AsciicastHeader};
pub use format::{EventType, Transcript, TranscriptEvent, TranscriptMetadata};
pub use player::{play_to_stdout, PlaybackOptions, PlaybackSpeed, Player, PlayerState};
pub use recorder::{Recorder, RecorderBuilder};
