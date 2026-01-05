//! Multi-session management and selection.
//!
//! This module provides functionality for managing multiple terminal sessions
//! simultaneously and performing operations across them, such as:
//!
//! - Waiting for any session to match a pattern (`expect_any`)
//! - Waiting for all sessions to match patterns (`expect_all`)
//! - Sending to multiple sessions in parallel
//! - Per-session pattern selection
//!
//! # Example
//!
//! ```ignore
//! use rust_expect::multi::MultiSessionManager;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), rust_expect::ExpectError> {
//!     let mut manager = MultiSessionManager::new();
//!
//!     // Add sessions (assuming you have Session instances)
//!     // let id1 = manager.add(session1, "server1");
//!     // let id2 = manager.add(session2, "server2");
//!
//!     // Wait for any to match
//!     // let result = manager.expect_any("prompt>").await?;
//!
//!     Ok(())
//! }
//! ```

mod group;
mod select;

pub use group::{GroupBuilder, GroupManager, GroupResult, SessionGroup};
/// Session identifier type for multi-session operations.
/// This is distinct from `types::SessionId` which is a UUID-based identifier.
pub use select::SessionId as MultiSessionId;
pub use select::{MultiSessionManager, PatternSelector, ReadyType, SelectResult, SendResult};
