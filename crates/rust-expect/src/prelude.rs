//! Convenient re-exports for common rust-expect usage.
//!
//! This module provides a single import to access the most commonly used
//! types and traits from rust-expect.
//!
//! # Example
//!
//! ```ignore
//! use rust_expect::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let mut session = Session::spawn("bash", &[]).await?;
//!     session.expect("$").await?;
//!     session.send_line("echo hello").await?;
//!     session.expect("hello").await?;
//!     Ok(())
//! }
//! ```

// Core types
pub use crate::config::{
    BufferConfig, EncodingConfig, HumanTypingConfig, InteractConfig, LineEnding, LogFormat,
    LoggingConfig, SessionConfig, TimeoutConfig,
};

// Error handling
pub use crate::error::{ExpectError, Result, SpawnError};

// Common types
pub use crate::types::{
    ControlChar, Dimensions, ExpectResult, Match, ProcessExitStatus, SessionId, SessionState,
};

// Encoding utilities
pub use crate::encoding::{
    decode_utf8_lossy, detect_encoding_from_env, detect_line_ending, normalize_line_endings,
    strip_ansi, DetectedEncoding, EncodedText, LineEndingStyle,
};

// Macros (re-exported from rust-expect-macros)
pub use crate::{dialog, patterns, regex, timeout};

// Session types
pub use crate::session::{QuickSession, Session, SessionBuilder};

// Pattern types
pub use crate::expect::{Matcher, Pattern, PatternManager, PatternSet, RingBuffer};

// Send traits
pub use crate::send::{AnsiSend, BasicSend, HumanTyper, Sender};

// Backend types
pub use crate::backend::{BackendType, PtyConfig, PtySpawner};

// Sync wrapper
pub use crate::sync::{block_on, SyncSession};
