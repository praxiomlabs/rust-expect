//! Session module for managing spawned process interactions.
//!
//! This module provides the core session types and functionality for
//! interacting with spawned processes, including the session handle,
//! builder, lifecycle management, and screen buffer integration.
//!
//! # Overview
//!
//! The [`Session`] type is the main entry point for interacting with
//! terminal applications. It provides methods for:
//!
//! - Spawning processes with [`Session::spawn`]
//! - Sending input with [`Session::send`], [`Session::send_line`]
//! - Expecting output with [`Session::expect`], [`Session::expect_any`]
//! - Running dialogs with [`Session::run_dialog`]
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```no_run
//! use rust_expect::Session;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), rust_expect::ExpectError> {
//!     // Spawn a bash shell
//!     let mut session = Session::spawn("/bin/bash", &[]).await?;
//!
//!     // Wait for the prompt
//!     session.expect("$ ").await?;
//!
//!     // Send a command
//!     session.send_line("echo 'Hello, World!'").await?;
//!
//!     // Expect the output
//!     session.expect("Hello, World!").await?;
//!
//!     // Clean exit
//!     session.send_line("exit").await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Using the Builder
//!
//! ```no_run
//! use rust_expect::{Session, SessionBuilder};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), rust_expect::ExpectError> {
//!     // Builder produces a SessionConfig; spawn via Session::spawn_with_config.
//!     let config = SessionBuilder::new()
//!         .command("/bin/bash")
//!         .args(["-l"])
//!         .timeout(Duration::from_secs(30))
//!         .dimensions(120, 40)
//!         .env("TERM", "xterm-256color")
//!         .build();
//!
//!     let mut session = Session::spawn_with_config(
//!         &config.command,
//!         &config.args.iter().map(String::as_str).collect::<Vec<_>>(),
//!         config.clone(),
//!     ).await?;
//!
//!     session.expect("$ ").await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Multi-Pattern Matching
//!
//! ```no_run
//! use rust_expect::{Session, Pattern, PatternSet};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), rust_expect::ExpectError> {
//!     let mut session = Session::spawn("/bin/bash", &[]).await?;
//!
//!     // Create a pattern set with multiple options
//!     let mut patterns = PatternSet::new();
//!     patterns
//!         .add(Pattern::literal("$ "))
//!         .add(Pattern::literal("# "));
//!
//!     // Expect any of the patterns; pass a bound via expect_timeout if needed.
//!     let _ = Duration::from_secs(5);
//!     let result = session.expect_any(&patterns).await?;
//!     println!("Matched: {}", result.matched);
//!     Ok(())
//! }
//! ```

mod builder;
pub(crate) mod events;
mod handle;
mod screen;
pub(crate) mod state;
pub(crate) mod transport;

pub use builder::{QuickSession, SessionBuilder};
pub use events::{EventSubscriber, OutputTap, SessionEvent, TapId};
pub use handle::{Session, SessionExt};
pub use screen::{Cell, CellAttributes, Color, Position, Region, ScreenBuffer};
