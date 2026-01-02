//! Dialog-based interaction scripting.
//!
//! This module provides a high-level abstraction for scripting
//! interactive terminal sessions using dialog definitions.

pub mod common;
pub mod definition;
pub mod executor;

pub use common::*;
pub use definition::{Dialog, DialogBuilder, DialogStep};
pub use executor::{DialogExecutor, DialogResult, StepResult};
