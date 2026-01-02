//! Interactive terminal sessions.
//!
//! This module provides functionality for interactive terminal sessions,
//! allowing direct user interaction with spawned processes.
//!
//! # Pattern Hooks
//!
//! The interact mode supports pattern-based callbacks that are triggered
//! when specific patterns appear in the output or input:
//!
//! ```ignore
//! use rust_expect::Session;
//!
//! let mut session = Session::spawn("/bin/bash", &[]).await?;
//!
//! session.interact()
//!     .on_output("password:", |ctx| {
//!         ctx.send("my_password\n")
//!     })
//!     .on_output("logout", |_| {
//!         InteractAction::Stop
//!     })
//!     .start()
//!     .await?;
//! ```

pub mod hooks;
pub mod mode;
pub mod session;
pub mod terminal;

pub use hooks::{HookBuilder, HookManager, InteractionEvent};
pub use mode::{InputFilter, InteractionMode, OutputFilter};
pub use session::{
    InteractAction, InteractBuilder, InteractContext, InteractEndReason, InteractResult,
    PatternHook,
};
pub use terminal::{Terminal, TerminalMode, TerminalSize, TerminalState};
