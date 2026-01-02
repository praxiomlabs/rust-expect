//! rust-expect: Next-generation terminal automation library
//!
//! This crate provides an Expect-style API for automating interactive terminal applications.
//! It supports local PTY sessions, SSH connections, and mock sessions for testing.
//!
//! # Features
//!
//! - **Async-first design** with Tokio runtime
//! - **Cross-platform PTY support** via `rust-pty`
//! - **Flexible pattern matching** with regex, literal, and custom matchers
//! - **SSH backend** for remote automation (feature: `ssh`)
//! - **Mock backend** for testing (feature: `mock`)
//! - **Screen buffer** with ANSI parsing (feature: `screen`)
//! - **PII redaction** for sensitive data handling (feature: `pii-redaction`)
//!
//! # Example
//!
//! ```ignore
//! use rust_expect::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), ExpectError> {
//!     let mut session = Session::spawn("/bin/bash", &[]).await?;
//!     session.expect("$").await?;
//!     session.send_line("echo hello").await?;
//!     session.expect("hello").await?;
//!     Ok(())
//! }
//! ```

// Re-export macros
pub use rust_expect_macros::{dialog, patterns, regex, timeout};

// Core types (Phase 4)
pub mod config;
pub mod encoding;
pub mod error;
pub mod prelude;
pub mod types;
pub mod validation;

// Core modules (Phase 5)
pub mod backend;
pub mod expect;
pub mod send;
pub mod session;
pub mod sync;
pub mod util;

// Feature modules (Phase 6)
pub mod auto_config;
pub mod dialog;
pub mod health;
pub mod interact;
pub mod metrics;
pub mod multi;
pub mod transcript;

/// Mock backend for testing.
#[cfg(feature = "mock")]
pub mod mock;

/// Screen buffer with ANSI parsing.
#[cfg(feature = "screen")]
pub mod screen;

/// PII detection and redaction.
#[cfg(feature = "pii-redaction")]
pub mod pii;

// Re-export commonly used items from Phase 4
pub use config::{
    BufferConfig, EncodingConfig, HumanTypingConfig, InteractConfig, LineEnding, LogFormat,
    LoggingConfig, SessionConfig, TimeoutConfig,
};
pub use encoding::{
    decode_utf8_lossy, detect_encoding_from_env, detect_line_ending, normalize_line_endings,
    strip_ansi, DetectedEncoding, EncodedText, LineEndingStyle,
};
pub use error::{ExpectError, Result, SpawnError};
pub use types::{
    ControlChar, Dimensions, ExpectResult, Match, ProcessExitStatus, SessionId, SessionState,
};

// Re-export commonly used items from Phase 5
pub use backend::{BackendType, PtyConfig, PtySpawner};
pub use expect::{Matcher, Pattern, PatternManager, PatternSet, RingBuffer};
pub use send::{AnsiSend, BasicSend, HumanTyper, Sender};
pub use session::{QuickSession, Session, SessionBuilder};
pub use sync::{block_on, SyncSession};
pub use util::{Backpressure, Deadline, TimeoutExt};

// Re-export commonly used items from Phase 6
pub use auto_config::{detect_shell, LocaleInfo, ShellType};
pub use dialog::{Dialog, DialogBuilder, DialogStep};
pub use health::{HealthChecker, HealthStatus};
pub use interact::{
    InteractAction, InteractBuilder, InteractContext, InteractEndReason, InteractResult,
    InteractionMode, TerminalMode, TerminalState,
};
pub use metrics::{Counter, Gauge, Histogram, MetricsRegistry, SessionMetrics};
pub use multi::{
    GroupBuilder, GroupManager, GroupResult, MultiSessionManager, PatternSelector, ReadyType,
    SelectResult, SendResult, SessionGroup,
};
pub use transcript::{Player, Recorder, Transcript, TranscriptEvent};

// Conditional re-exports
#[cfg(feature = "mock")]
pub use mock::{MockBuilder, MockSession, MockTransport, Scenario};

#[cfg(feature = "screen")]
pub use screen::{Attributes, Cell, ScreenBuffer};

#[cfg(feature = "pii-redaction")]
pub use pii::{PiiDetector, PiiRedactor, PiiType};

// Test utilities (Phase 7)
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

#[cfg(any(test, feature = "test-utils"))]
pub use test_utils::{
    assert_output_contains, assert_output_matches, ExpectTestBuilder, FakePty, FakePtyPair,
    Fixtures, OutputAssertions, RecordedInteraction, SessionTestBuilder, TestFixture,
    TestSession, TestSessionBuilder,
};
