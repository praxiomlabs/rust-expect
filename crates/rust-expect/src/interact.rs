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
//!
//! # Hook Execution Order
//!
//! Understanding hook execution order is important when using multiple hooks:
//!
//! ## Pattern Hooks (on_output, on_input)
//!
//! Pattern hooks are evaluated in **registration order**. The first matching
//! hook's action is executed:
//!
//! ```ignore
//! session.interact()
//!     .on_output("error", |_| InteractAction::Stop)      // Checked first
//!     .on_output("warning", |_| InteractAction::Continue) // Checked second
//!     .on_output("error.*", |_| InteractAction::Continue) // Checked third
//!     .start()
//! ```
//!
//! If output contains "error", the first hook matches and stops. The regex
//! hook is never reached even though it also matches.
//!
//! ## Processing Hooks (HookManager)
//!
//! Processing hooks form a **pipeline**. Each hook receives the output of the
//! previous hook:
//!
//! ```ignore
//! let manager = HookBuilder::new()
//!     .with_crlf()    // LF -> CRLF conversion
//!     .with_echo()    // Echo to stdout (receives CRLF output)
//!     .build();
//! ```
//!
//! ## Resize Hooks (on_resize)
//!
//! Only one resize hook can be registered. If multiple are set, the last one
//! wins:
//!
//! ```ignore
//! session.interact()
//!     .on_resize(|_| InteractAction::Continue)  // Overwritten
//!     .on_resize(|ctx| {                        // This one is used
//!         eprintln!("Resized to {}x{}", ctx.size.cols, ctx.size.rows);
//!         InteractAction::Continue
//!     })
//! ```
//!
//! ## Event Processing Order
//!
//! During each iteration of the interact loop:
//!
//! 1. **Resize events** are processed first (Unix SIGWINCH)
//! 2. **Session output** is read and pattern-matched
//! 3. **User input** is read and pattern-matched
//! 4. Actions are executed in the order above
//!
//! If any action is `InteractAction::Stop` or `InteractAction::Error`, the
//! loop terminates immediately.
//!
//! ## Best Practices
//!
//! - Register specific patterns before general patterns (most specific first)
//! - Use `InteractAction::Continue` to let multiple hooks observe data
//! - For logging, use event hooks rather than pattern hooks
//! - Keep hook callbacks fast to avoid blocking the event loop

pub mod hooks;
pub mod mode;
pub mod session;
pub mod terminal;

pub use hooks::{HookBuilder, HookManager, InteractionEvent};
pub use mode::{InputFilter, InteractionMode, OutputFilter};
pub use session::{
    InteractAction, InteractBuilder, InteractContext, InteractEndReason, InteractResult,
    PatternHook, ResizeContext, ResizeHook,
};
pub use terminal::{Terminal, TerminalMode, TerminalSize, TerminalState};
