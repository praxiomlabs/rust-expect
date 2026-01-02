//! Multi-session handling.
//!
//! This module provides functionality for managing multiple terminal
//! sessions simultaneously, including selection and grouping.

pub mod group;
pub mod select;

pub use group::{GroupBuilder, GroupManager, GroupResult, SessionGroup};
pub use select::{PatternSelector, ReadyType, SelectResult, Selector};
