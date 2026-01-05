//! Session lifecycle management.
//!
//! This module provides utilities for managing session lifecycle,
//! including graceful shutdown, signal handling, and cleanup.

use std::time::Duration;

use crate::types::{ControlChar, ProcessExitStatus, SessionState};

/// Shutdown strategy for closing a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShutdownStrategy {
    /// Send exit command and wait for graceful shutdown.
    Graceful,
    /// Send SIGTERM (or equivalent) and wait.
    Terminate,
    /// Send SIGKILL (or equivalent) immediately.
    Kill,
    /// Try graceful, then terminate, then kill.
    #[default]
    Escalating,
}

/// Configuration for session shutdown.
#[derive(Debug, Clone)]
pub struct ShutdownConfig {
    /// The shutdown strategy to use.
    pub strategy: ShutdownStrategy,
    /// Timeout for graceful shutdown.
    pub graceful_timeout: Duration,
    /// Timeout for terminate signal.
    pub terminate_timeout: Duration,
    /// Exit command to send for graceful shutdown.
    pub exit_command: Option<String>,
    /// Whether to wait for process to exit.
    pub wait_for_exit: bool,
}

impl Default for ShutdownConfig {
    fn default() -> Self {
        Self {
            strategy: ShutdownStrategy::Escalating,
            graceful_timeout: Duration::from_secs(5),
            terminate_timeout: Duration::from_secs(3),
            exit_command: Some("exit".to_string()),
            wait_for_exit: true,
        }
    }
}

impl ShutdownConfig {
    /// Create a new shutdown config with graceful strategy.
    #[must_use]
    pub fn graceful() -> Self {
        Self {
            strategy: ShutdownStrategy::Graceful,
            ..Default::default()
        }
    }

    /// Create a new shutdown config with kill strategy.
    #[must_use]
    pub fn kill() -> Self {
        Self {
            strategy: ShutdownStrategy::Kill,
            wait_for_exit: false,
            ..Default::default()
        }
    }

    /// Create a new shutdown config with custom exit command.
    #[must_use]
    pub fn with_exit_command(mut self, command: impl Into<String>) -> Self {
        self.exit_command = Some(command.into());
        self
    }

    /// Set the graceful timeout.
    #[must_use]
    pub const fn with_graceful_timeout(mut self, timeout: Duration) -> Self {
        self.graceful_timeout = timeout;
        self
    }
}

/// Lifecycle events that can occur during a session.
#[derive(Debug, Clone)]
pub enum LifecycleEvent {
    /// Session started.
    Started,
    /// Session became ready (e.g., shell prompt appeared).
    Ready,
    /// Session state changed.
    StateChanged(SessionState),
    /// Session is shutting down.
    ShuttingDown,
    /// Session closed normally.
    Closed,
    /// Session exited with status.
    Exited(ProcessExitStatus),
    /// Session encountered an error.
    Error(String),
}

/// Callback type for lifecycle events.
pub type LifecycleCallback = Box<dyn Fn(LifecycleEvent) + Send + Sync>;

/// Manager for session lifecycle events.
pub struct LifecycleManager {
    /// Registered callbacks.
    callbacks: Vec<LifecycleCallback>,
    /// Current state.
    state: SessionState,
    /// Shutdown configuration.
    shutdown_config: ShutdownConfig,
}

impl LifecycleManager {
    /// Create a new lifecycle manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            callbacks: Vec::new(),
            state: SessionState::Starting,
            shutdown_config: ShutdownConfig::default(),
        }
    }

    /// Set the shutdown configuration.
    pub fn set_shutdown_config(&mut self, config: ShutdownConfig) {
        self.shutdown_config = config;
    }

    /// Get the shutdown configuration.
    #[must_use]
    pub const fn shutdown_config(&self) -> &ShutdownConfig {
        &self.shutdown_config
    }

    /// Register a lifecycle callback.
    pub fn on_event(&mut self, callback: LifecycleCallback) {
        self.callbacks.push(callback);
    }

    /// Emit a lifecycle event.
    pub fn emit(&self, event: &LifecycleEvent) {
        for callback in &self.callbacks {
            callback(event.clone());
        }
    }

    /// Update the session state and emit event.
    pub fn set_state(&mut self, state: SessionState) {
        self.state = state;
        self.emit(&LifecycleEvent::StateChanged(state));
    }

    /// Get the current state.
    #[must_use]
    pub const fn state(&self) -> &SessionState {
        &self.state
    }

    /// Signal that the session has started.
    pub fn started(&mut self) {
        self.set_state(SessionState::Running);
        self.emit(&LifecycleEvent::Started);
    }

    /// Signal that the session is ready.
    pub fn ready(&mut self) {
        self.emit(&LifecycleEvent::Ready);
    }

    /// Signal that the session is shutting down.
    pub fn shutting_down(&mut self) {
        self.set_state(SessionState::Closing);
        self.emit(&LifecycleEvent::ShuttingDown);
    }

    /// Signal that the session has closed.
    pub fn closed(&mut self) {
        self.set_state(SessionState::Closed);
        self.emit(&LifecycleEvent::Closed);
    }

    /// Signal that the session has exited.
    pub fn exited(&mut self, status: ProcessExitStatus) {
        self.set_state(SessionState::Exited(status));
        self.emit(&LifecycleEvent::Exited(status));
    }

    /// Signal an error.
    pub fn error(&mut self, message: impl Into<String>) {
        self.emit(&LifecycleEvent::Error(message.into()));
    }
}

impl Default for LifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for LifecycleManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LifecycleManager")
            .field("state", &self.state)
            .field("callbacks", &self.callbacks.len())
            .finish()
    }
}

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
    fn shutdown_config_default() {
        let config = ShutdownConfig::default();
        assert_eq!(config.strategy, ShutdownStrategy::Escalating);
        assert!(config.exit_command.is_some());
    }

    #[test]
    fn lifecycle_manager_state_transitions() {
        let mut manager = LifecycleManager::new();

        assert!(matches!(manager.state(), SessionState::Starting));

        manager.started();
        assert!(matches!(manager.state(), SessionState::Running));

        manager.shutting_down();
        assert!(matches!(manager.state(), SessionState::Closing));

        manager.closed();
        assert!(matches!(manager.state(), SessionState::Closed));
    }

    #[test]
    fn signal_control_char() {
        assert_eq!(
            Signal::Interrupt.as_control_char(),
            Some(ControlChar::CtrlC)
        );
        assert_eq!(Signal::Terminate.as_control_char(), None);
    }
}
