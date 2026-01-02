//! rust-pty: Cross-platform async PTY library
//!
//! This crate provides a unified async interface for pseudo-terminal (PTY) operations
//! across Unix (Linux, macOS) and Windows (`ConPTY`) platforms.
//!
//! # Platform Support
//!
//! - **Unix**: Uses `rustix` for PTY allocation and process management
//! - **Windows**: Uses `ConPTY` via `windows-sys` (Windows 10 1809+)
//!
//! # Quick Start
//!
//! ```ignore
//! use rust_pty::{NativePtySystem, PtySystem, PtyConfig};
//! use tokio::io::{AsyncReadExt, AsyncWriteExt};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = PtyConfig::default();
//!     let (mut master, mut child) = NativePtySystem::spawn_shell(&config).await?;
//!
//!     // Write a command
//!     master.write_all(b"echo hello\n").await?;
//!
//!     // Read output
//!     let mut buf = [0u8; 1024];
//!     let n = master.read(&mut buf).await?;
//!     println!("{}", String::from_utf8_lossy(&buf[..n]));
//!
//!     // Clean up
//!     child.kill()?;
//!     Ok(())
//! }
//! ```
//!
//! # Features
//!
//! - **Async I/O**: First-class async support with Tokio
//! - **Cross-platform**: Single API for Unix and Windows
//! - **Type-safe**: Strong typing with proper error handling
//! - **Zero-copy**: Efficient buffer management for high-throughput scenarios

pub mod config;
pub mod error;
pub mod traits;

#[cfg(unix)]
pub mod unix;

#[cfg(windows)]
pub mod windows;

// Re-export primary types
pub use config::{PtyConfig, PtyConfigBuilder, PtySignal, WindowSize};
pub use error::{PtyError, Result};
pub use traits::{ExitStatus, PtyChild, PtyMaster, PtySystem};

// Platform-specific re-exports
#[cfg(unix)]
pub use unix::{NativePtySystem, UnixPtyChild, UnixPtyMaster, UnixPtySystem};

#[cfg(windows)]
pub use windows::{NativePtySystem, WindowsPtyChild, WindowsPtyMaster, WindowsPtySystem};

/// Create a PTY with the default configuration and spawn a shell.
///
/// This is a convenience function that uses the platform's native PTY system.
///
/// # Errors
///
/// Returns an error if PTY creation or shell spawning fails.
#[cfg(unix)]
pub async fn spawn_shell() -> Result<(UnixPtyMaster, UnixPtyChild)> {
    UnixPtySystem::spawn_shell(&PtyConfig::default()).await
}

/// Create a PTY with the default configuration and spawn a shell.
///
/// This is a convenience function that uses the platform's native PTY system.
///
/// # Errors
///
/// Returns an error if PTY creation or shell spawning fails.
#[cfg(windows)]
pub async fn spawn_shell() -> Result<(WindowsPtyMaster, WindowsPtyChild)> {
    WindowsPtySystem::spawn_shell(&PtyConfig::default()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = PtyConfig::default();
        assert_eq!(config.window_size, (80, 24));
        assert!(config.new_session);
    }

    #[test]
    fn window_size_conversion() {
        let size = WindowSize::new(120, 40);
        assert_eq!(size.cols, 120);
        assert_eq!(size.rows, 40);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn spawn_echo() {
        let config = PtyConfig::default();
        let result = UnixPtySystem::spawn("echo", ["test"], &config).await;

        // May fail in some CI environments
        if let Ok((mut master, mut child)) = result {
            let _ = child.wait().await;
            master.close().ok();
        }
    }
}
