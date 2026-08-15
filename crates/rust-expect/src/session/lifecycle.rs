//! Signals that can be sent to a session's child process.
//!
//! This module used to hold a second, parallel lifecycle model —
//! `LifecycleManager`, `LifecycleEvent`, `LifecycleCallback`, `ShutdownConfig`
//! and `ShutdownStrategy` — implementing the same state machine as `Session`
//! with its own copy of the state and a callback surface. Nothing in the
//! workspace ever constructed any of it. It was deleted under AR-004: the
//! authoritative state machine now lives in `session::state`, and the callback
//! surface belongs to the session event stream rather than to a second manager.

use crate::types::ControlChar;

/// Signals that can be sent to a process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    /// Interrupt (Ctrl+C).
    Interrupt,
    /// Quit (Ctrl+\).
    Quit,
    /// Terminate.
    Terminate,
    /// Kill (non-catchable).
    Kill,
    /// Hangup.
    Hangup,
    /// User defined signal 1.
    User1,
    /// User defined signal 2.
    User2,
}

impl Signal {
    /// Get the control character for this signal, if applicable.
    #[must_use]
    pub const fn as_control_char(&self) -> Option<ControlChar> {
        match self {
            Self::Interrupt => Some(ControlChar::CtrlC),
            Self::Quit => Some(ControlChar::CtrlBackslash),
            _ => None,
        }
    }

    /// Get the Unix signal number for this signal.
    #[cfg(unix)]
    #[must_use]
    pub const fn as_signal_number(&self) -> i32 {
        match self {
            Self::Interrupt => 2,  // SIGINT
            Self::Quit => 3,       // SIGQUIT
            Self::Terminate => 15, // SIGTERM
            Self::Kill => 9,       // SIGKILL
            Self::Hangup => 1,     // SIGHUP
            Self::User1 => 10,     // SIGUSR1
            Self::User2 => 12,     // SIGUSR2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_control_char() {
        assert_eq!(
            Signal::Interrupt.as_control_char(),
            Some(ControlChar::CtrlC)
        );
        assert_eq!(Signal::Terminate.as_control_char(), None);
    }
}
