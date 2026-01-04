//! Signal handling for PTY operations.
//!
//! This module provides utilities for handling Unix signals relevant to
//! PTY operations, particularly SIGWINCH (window size change) and SIGCHLD
//! (child process state change).

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use signal_hook::consts::signal::{SIGCHLD, SIGWINCH};
use signal_hook::iterator::Signals;
use tokio::sync::mpsc;

/// Signal types relevant to PTY operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PtySignalEvent {
    /// Window size changed (SIGWINCH).
    WindowChanged,
    /// Child process state changed (SIGCHLD).
    ChildStateChanged,
}

/// A handle to the signal handler that can be used to stop it.
#[derive(Debug)]
pub struct SignalHandle {
    /// Flag to signal shutdown.
    shutdown: Arc<AtomicBool>,
}

impl SignalHandle {
    /// Signal the handler to stop.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }
}

impl Drop for SignalHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Start a background task that monitors signals and sends events.
///
/// Returns a receiver for signal events and a handle to control the handler.
///
/// # Errors
///
/// Returns an error if signal registration fails.
pub fn start_signal_handler() -> io::Result<(mpsc::UnboundedReceiver<PtySignalEvent>, SignalHandle)>
{
    let mut signals = Signals::new([SIGWINCH, SIGCHLD])?;
    let (tx, rx) = mpsc::unbounded_channel();
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = Arc::clone(&shutdown);

    std::thread::Builder::new()
        .name("pty-signal-handler".into())
        .spawn(move || {
            for signal in signals.forever() {
                if shutdown_clone.load(Ordering::SeqCst) {
                    break;
                }

                let event = match signal {
                    SIGWINCH => PtySignalEvent::WindowChanged,
                    SIGCHLD => PtySignalEvent::ChildStateChanged,
                    _ => continue,
                };

                if tx.send(event).is_err() {
                    // Receiver dropped, exit
                    break;
                }
            }
        })?;

    Ok((rx, SignalHandle { shutdown }))
}

/// Register a callback for SIGWINCH (window resize) signals.
///
/// The callback will be invoked each time the terminal window is resized.
/// Returns a handle that must be kept alive for the handler to remain active.
///
/// # Errors
///
/// Returns an error if signal registration fails.
pub fn on_window_change<F>(callback: F) -> io::Result<SignalHandle>
where
    F: Fn() + Send + 'static,
{
    let mut signals = Signals::new([SIGWINCH])?;
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = Arc::clone(&shutdown);

    std::thread::Builder::new()
        .name("pty-sigwinch-handler".into())
        .spawn(move || {
            for _ in signals.forever() {
                if shutdown_clone.load(Ordering::SeqCst) {
                    break;
                }
                callback();
            }
        })?;

    Ok(SignalHandle { shutdown })
}

/// Check if a signal number is SIGCHLD.
#[must_use]
pub const fn is_sigchld(signal: i32) -> bool {
    signal == SIGCHLD
}

/// Check if a signal number is SIGWINCH.
#[must_use]
pub const fn is_sigwinch(signal: i32) -> bool {
    signal == SIGWINCH
}

/// Get the signal number for SIGWINCH.
#[must_use]
pub const fn sigwinch() -> i32 {
    SIGWINCH
}

/// Get the signal number for SIGCHLD.
#[must_use]
pub const fn sigchld() -> i32 {
    SIGCHLD
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_constants() {
        assert!(is_sigchld(sigchld()));
        assert!(is_sigwinch(sigwinch()));
        assert!(!is_sigchld(sigwinch()));
        assert!(!is_sigwinch(sigchld()));
    }

    #[test]
    fn signal_handle_shutdown() {
        let shutdown = Arc::new(AtomicBool::new(false));
        let handle = SignalHandle {
            shutdown: Arc::clone(&shutdown),
        };

        assert!(!shutdown.load(Ordering::SeqCst));
        handle.shutdown();
        assert!(shutdown.load(Ordering::SeqCst));
    }
}
