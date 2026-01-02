//! Send module for writing data to sessions.
//!
//! This module provides functionality for sending data to spawned processes,
//! including basic send operations, ANSI escape sequences, and human-like typing.

mod basic;
mod human;

pub use basic::{AnsiSend, AnsiSequences, BasicSend, Sender};
pub use human::{HumanSend, HumanTyper, TypeEvent, TypingSpeed};
