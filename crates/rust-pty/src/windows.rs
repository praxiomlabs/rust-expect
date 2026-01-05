//! Windows platform implementation for PTY operations.
//!
//! This module provides the Windows-specific PTY implementation using ConPTY
//! (Console Pseudo Terminal), introduced in Windows 10 version 1809.
//!
//! # Platform Support
//!
//! ConPTY is only available on:
//! - Windows 10 version 1809 (build 17763) and later
//! - Windows Server 2019 and later
//!
//! On older Windows versions, PTY creation will fail with `PtyError::ConPtyNotAvailable`.
//!
//! # Example
//!
//! ```ignore
//! use rust_pty::windows::WindowsPtySystem;
//! use rust_pty::{PtySystem, PtyConfig};
//!
//! let config = PtyConfig::default();
//! let (master, child) = WindowsPtySystem::spawn("cmd.exe", &[], &config).await?;
//! ```

mod async_adapter;
mod child;
mod conpty;
mod pipes;

use std::ffi::OsStr;
use std::future::Future;
use std::sync::Arc;

pub use async_adapter::WindowsPtyMaster;
pub use child::{WindowsPtyChild, spawn_child};
pub use conpty::{ConPty, is_conpty_available};
pub use pipes::{PipePair, create_input_pipe, create_output_pipe, set_inheritable};

use crate::config::{PtyConfig, WindowSize};
use crate::error::{PtyError, Result};
use crate::traits::PtySystem;

/// Windows PTY system implementation using ConPTY.
///
/// This struct provides the factory methods for creating PTY sessions on Windows.
#[derive(Debug, Clone, Copy, Default)]
pub struct WindowsPtySystem;

impl PtySystem for WindowsPtySystem {
    type Master = WindowsPtyMaster;
    type Child = WindowsPtyChild;

    fn spawn<S, I>(
        program: S,
        args: I,
        config: &PtyConfig,
    ) -> impl Future<Output = Result<(Self::Master, Self::Child)>> + Send
    where
        S: AsRef<OsStr> + Send,
        I: IntoIterator + Send,
        I::Item: AsRef<OsStr>,
    {
        async move {
            // Check ConPTY availability
            if !is_conpty_available() {
                return Err(PtyError::ConPtyNotAvailable);
            }

            // Create pipes
            let input_pipe = create_input_pipe()?;
            let output_pipe = create_output_pipe()?;

            // Create ConPTY
            let window_size = WindowSize::from(config.window_size);
            let mut conpty = ConPty::new(
                window_size,
                input_pipe.read,
                output_pipe.write,
                input_pipe.write,
                output_pipe.read,
            )?;

            // Spawn child process
            let child = spawn_child(conpty.handle(), program, args, config)?;

            // Close the PTY pipe handles after CreateProcess per Microsoft docs.
            // This signals to ConPTY that no other handles exist on the "other side"
            // of the pipes, enabling proper channel detection.
            conpty.close_pty_pipes();

            // Duplicate handles for the master (Windows requires explicit handle duplication)
            let input_handle = conpty.input().try_clone().map_err(|e| PtyError::Spawn(e))?;
            let output_handle = conpty
                .output()
                .try_clone()
                .map_err(|e| PtyError::Spawn(e))?;

            // Now wrap in Arc for shared ownership
            let conpty = Arc::new(conpty);
            let conpty_for_resize = Arc::clone(&conpty);

            // Create master wrapper
            let master = WindowsPtyMaster::new(
                input_handle,
                output_handle,
                move |size| conpty_for_resize.resize(size),
                window_size,
            );

            Ok((master, child))
        }
    }
}

/// Convenience type alias for the default PTY system on Windows.
pub type NativePtySystem = WindowsPtySystem;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_availability() {
        // Just check that this doesn't panic
        let _ = is_conpty_available();
    }
}
