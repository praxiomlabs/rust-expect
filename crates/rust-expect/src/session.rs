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
//! ```ignore
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
//! ```ignore
//! use rust_expect::SessionBuilder;
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), rust_expect::ExpectError> {
//!     let mut session = SessionBuilder::new()
//!         .command("/bin/bash")
//!         .args(&["-l"])
//!         .timeout(Duration::from_secs(30))
//!         .dimensions(120, 40)
//!         .env("TERM", "xterm-256color")
//!         .spawn()
//!         .await?;
//!
//!     session.expect("$ ").await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Multi-Pattern Matching
//!
//! ```ignore
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
//!         .add(Pattern::literal("# "))
//!         .add(Pattern::timeout(Duration::from_secs(5)));
//!
//!     // Expect any of the patterns
//!     let result = session.expect_any(&patterns).await?;
//!     println!("Matched: {}", result.matched);
//!     Ok(())
//! }
//! ```

mod builder;
mod handle;
mod lifecycle;
mod screen;

pub use builder::{QuickSession, SessionBuilder};
pub use handle::{Session, SessionExt};
pub use lifecycle::{
    LifecycleCallback, LifecycleEvent, LifecycleManager, ShutdownConfig, ShutdownStrategy, Signal,
};
pub use screen::{Cell, CellAttributes, Color, Position, Region, ScreenBuffer};
