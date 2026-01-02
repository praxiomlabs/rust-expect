//! Backend module for different transport implementations.
//!
//! This module provides various backends for session communication,
//! including PTY for local processes and SSH for remote connections.

mod pty;

pub use pty::{EnvMode, PtyConfig, PtyHandle, PtySpawner, PtyTransport};

// SSH backend is conditionally compiled
#[cfg(feature = "ssh")]
pub mod ssh;

/// Trait for session backends.
pub trait Backend {
    /// The transport type produced by this backend.
    type Transport;

    /// Check if the backend is available.
    fn is_available(&self) -> bool;

    /// Get the backend name.
    fn name(&self) -> &'static str;
}

/// Available backend types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendType {
    /// Local PTY backend.
    Pty,
    /// SSH backend for remote connections.
    Ssh,
    /// Mock backend for testing.
    Mock,
}

impl BackendType {
    /// Check if this backend is available.
    #[must_use]
    pub const fn is_available(self) -> bool {
        match self {
            Self::Pty => cfg!(unix) || cfg!(windows),
            Self::Ssh => cfg!(feature = "ssh"),
            Self::Mock => cfg!(feature = "mock"),
        }
    }

    /// Get the backend name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Pty => "pty",
            Self::Ssh => "ssh",
            Self::Mock => "mock",
        }
    }
}
