//! Dialog-based interaction scripting.
//!
//! This module provides a high-level abstraction for scripting
//! interactive terminal sessions using dialog definitions.
//!
//! Dialogs define a sequence of expect/send steps that can be executed
//! against a session. They support variable substitution and branching.
//!
//! # Examples
//!
//! ## Basic Dialog
//!
//! ```
//! use rust_expect::{Dialog, DialogStep};
//!
//! // Create a simple login dialog
//! let dialog = Dialog::named("login")
//!     .step(DialogStep::new("username")
//!         .with_expect("login:")
//!         .with_send("admin\n"))
//!     .step(DialogStep::new("password")
//!         .with_expect("password:")
//!         .with_send("secret\n"));
//!
//! assert_eq!(dialog.len(), 2);
//! ```
//!
//! ## With Variables
//!
//! ```
//! use rust_expect::{Dialog, DialogStep};
//!
//! // Variables are substituted in send text
//! let dialog = Dialog::named("login")
//!     .variable("USER", "admin")
//!     .variable("PASS", "secret123")
//!     .step(DialogStep::new("username")
//!         .with_expect("login:")
//!         .with_send("${USER}\n"))
//!     .step(DialogStep::new("password")
//!         .with_expect("password:")
//!         .with_send("${PASS}\n"));
//!
//! // Variables are substituted when executing
//! assert_eq!(dialog.substitute("${USER}"), "admin");
//! ```
//!
//! ## Using the Builder
//!
//! ```
//! use rust_expect::DialogBuilder;
//!
//! let dialog = DialogBuilder::named("setup")
//!     .var("HOST", "server.example.com")
//!     .expect_send("prompt", "> ", "connect ${HOST}\n")
//!     .expect_send("auth", "password:", "mypassword\n")
//!     .build();
//!
//! assert_eq!(dialog.name, "setup");
//! ```
//!
//! ## Async Execution
//!
//! ```ignore
//! use rust_expect::{Session, Dialog, DialogStep};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), rust_expect::ExpectError> {
//!     let mut session = Session::spawn("/bin/bash", &[]).await?;
//!
//!     let dialog = Dialog::named("example")
//!         .step(DialogStep::new("prompt")
//!             .with_expect("$ ")
//!             .with_send("echo hello\n"));
//!
//!     let result = session.run_dialog(&dialog).await?;
//!     assert!(result.success);
//!     Ok(())
//! }
//! ```

pub mod common;
pub mod definition;
pub mod executor;

pub use common::*;
pub use definition::{Dialog, DialogBuilder, DialogStep};
pub use executor::{DialogExecutor, DialogResult, StepResult};
