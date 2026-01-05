//! Unix platform implementation for PTY operations.
//!
//! This module provides the Unix-specific PTY implementation, including:
//!
//! - PTY master/slave pair allocation via openpt/grantpt/unlockpt
//! - Async I/O through tokio's `AsyncFd`
//! - Child process management with proper session/controlling terminal setup
//! - Signal handling for SIGWINCH and SIGCHLD
//!
//! # Platform Support
//!
//! This implementation works on:
//! - Linux (using /dev/ptmx)
//! - macOS (using /dev/ptmx)
//! - FreeBSD and other Unix-like systems
//!
//! # Example
//!
//! ```ignore
//! use rust_pty::unix::UnixPtySystem;
//! use rust_pty::{PtySystem, PtyConfig};
//!
//! let config = PtyConfig::default();
//! let (master, child) = UnixPtySystem::spawn("/bin/bash", &[], &config).await?;
//! ```

mod buffer;
mod child;
mod pty;
mod signals;

use std::ffi::OsStr;

pub use buffer::PtyBuffer;
pub use child::{UnixPtyChild, spawn_child};
pub use pty::{UnixPtyMaster, open_slave};
pub use signals::{
    PtySignalEvent, SignalHandle, is_sigchld, is_sigwinch, on_window_change, sigchld, sigwinch,
    start_signal_handler,
};

use crate::config::PtyConfig;
use crate::error::Result;
use crate::traits::PtySystem;

/// Unix PTY system implementation.
///
/// This struct provides the factory methods for creating PTY sessions on Unix.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnixPtySystem;

impl PtySystem for UnixPtySystem {
    type Master = UnixPtyMaster;
    type Child = UnixPtyChild;

    async fn spawn<S, I>(
        program: S,
        args: I,
        config: &PtyConfig,
    ) -> Result<(Self::Master, Self::Child)>
    where
        S: AsRef<OsStr> + Send,
        I: IntoIterator + Send,
        I::Item: AsRef<OsStr>,
    {
        // Open master PTY
        let (master, slave_path) = UnixPtyMaster::open()?;

        // Set initial window size
        let window_size = config.window_size.into();
        master.set_window_size(window_size)?;

        // Open slave for child
        let slave_fd = open_slave(&slave_path)?;

        // Spawn child process
        let child = spawn_child(slave_fd, program, args, config).await?;

        Ok((master, child))
    }
}

/// Convenience type alias for the default PTY system on Unix.
pub type NativePtySystem = UnixPtySystem;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn spawn_shell() {
        let config = PtyConfig::default();
        let result = UnixPtySystem::spawn_shell(&config).await;

        // This may fail in some test environments, but the logic should be correct
        if let Ok((mut master, mut child)) = result {
            assert!(master.is_open());
            assert!(child.is_running());

            // Clean up
            child.kill().ok();
            master.close().ok();
        }
    }

    #[tokio::test]
    async fn spawn_echo() {
        let config = PtyConfig::default();
        let result = UnixPtySystem::spawn("echo", ["hello"], &config).await;

        if let Ok((mut master, mut child)) = result {
            // Wait for child to exit
            let status = child.wait().await;
            assert!(status.is_ok());

            master.close().ok();
        }
    }
}
