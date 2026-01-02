//! Session module for managing spawned process interactions.
//!
//! This module provides the core session types and functionality for
//! interacting with spawned processes, including the session handle,
//! builder, lifecycle management, and screen buffer integration.

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
