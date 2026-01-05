//! Resilient SSH session wrapper.

use std::time::{Duration, Instant};

use super::keepalive::{KeepaliveConfig, KeepaliveManager};
use super::retry::{RetryPolicy, RetryState, RetryStrategy};
use super::session::{SshConfig, SshSession};

/// Resilient session configuration.
#[derive(Debug, Clone)]
pub struct ResilientConfig {
    /// SSH configuration.
    pub ssh: SshConfig,
    /// Retry policy.
    pub retry: RetryPolicy,
    /// Keepalive configuration.
    pub keepalive: KeepaliveConfig,
    /// Auto-reconnect on disconnect.
    pub auto_reconnect: bool,
    /// Maximum reconnect attempts.
    pub max_reconnect_attempts: u32,
    /// Delay before reconnect.
    pub reconnect_delay: Duration,
}

impl ResilientConfig {
    /// Create new config.
    #[must_use]
    pub fn new(ssh: SshConfig) -> Self {
        Self {
            ssh,
            retry: RetryPolicy::default(),
            keepalive: KeepaliveConfig::default(),
            auto_reconnect: true,
            max_reconnect_attempts: 5,
            reconnect_delay: Duration::from_secs(5),
        }
    }

    /// Set retry policy.
    #[must_use]
    pub fn with_retry(mut self, policy: RetryPolicy) -> Self {
        self.retry = policy;
        self
    }

    /// Set keepalive config.
    #[must_use]
    pub const fn with_keepalive(mut self, config: KeepaliveConfig) -> Self {
        self.keepalive = config;
        self
    }

    /// Disable auto-reconnect.
    #[must_use]
    pub const fn no_auto_reconnect(mut self) -> Self {
        self.auto_reconnect = false;
        self
    }
}

/// Resilient session state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResilientState {
    /// Disconnected.
    Disconnected,
    /// Connected.
    Connected,
    /// Reconnecting.
    Reconnecting,
    /// Failed (gave up).
    Failed,
}

/// A resilient SSH session with auto-reconnect and keepalive.
#[derive(Debug)]
pub struct ResilientSession {
    /// Configuration.
    config: ResilientConfig,
    /// Underlying session.
    session: Option<SshSession>,
    /// Current state.
    state: ResilientState,
    /// Reconnect state.
    reconnect_state: RetryState,
    /// Keepalive manager.
    keepalive: KeepaliveManager,
    /// Last activity time.
    last_activity: Instant,
    /// Total reconnect count.
    reconnect_count: u32,
}

impl ResilientSession {
    /// Create a new resilient session.
    #[must_use]
    pub fn new(config: ResilientConfig) -> Self {
        let keepalive = KeepaliveManager::new(config.keepalive.clone());
        let reconnect_strategy =
            RetryStrategy::exponential(config.reconnect_delay, config.max_reconnect_attempts);

        Self {
            config,
            session: None,
            state: ResilientState::Disconnected,
            reconnect_state: RetryState::new(reconnect_strategy),
            keepalive,
            last_activity: Instant::now(),
            reconnect_count: 0,
        }
    }

    /// Get current state.
    #[must_use]
    pub const fn state(&self) -> ResilientState {
        self.state
    }

    /// Check if connected.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.state == ResilientState::Connected
            && self
                .session
                .as_ref()
                .is_some_and(super::session::SshSession::is_connected)
    }

    /// Get reconnect count.
    #[must_use]
    pub const fn reconnect_count(&self) -> u32 {
        self.reconnect_count
    }

    /// Connect to the server.
    pub fn connect(&mut self) -> crate::error::Result<()> {
        let mut session = SshSession::new(self.config.ssh.clone());
        session.connect()?;

        self.session = Some(session);
        self.state = ResilientState::Connected;
        self.last_activity = Instant::now();
        self.reconnect_state.reset();

        Ok(())
    }

    /// Disconnect from the server.
    pub fn disconnect(&mut self) {
        if let Some(ref mut session) = self.session {
            session.disconnect();
        }
        self.session = None;
        self.state = ResilientState::Disconnected;
    }

    /// Try to reconnect.
    pub fn reconnect(&mut self) -> crate::error::Result<()> {
        if !self.config.auto_reconnect {
            return Err(crate::error::ExpectError::SessionClosed);
        }

        if !self.reconnect_state.should_retry() {
            self.state = ResilientState::Failed;
            return Err(crate::error::ExpectError::SessionClosed);
        }

        self.state = ResilientState::Reconnecting;

        // Wait before reconnecting
        if let Some(delay) = self.reconnect_state.next_delay() {
            std::thread::sleep(delay);
        }

        self.reconnect_state.record_attempt();

        match self.connect() {
            Ok(()) => {
                self.reconnect_count += 1;
                Ok(())
            }
            Err(e) => {
                if !self.reconnect_state.should_retry() {
                    self.state = ResilientState::Failed;
                }
                Err(e)
            }
        }
    }

    /// Handle a disconnection event.
    pub fn handle_disconnect(&mut self) -> crate::error::Result<()> {
        self.session = None;

        if self.config.auto_reconnect {
            self.reconnect()
        } else {
            self.state = ResilientState::Disconnected;
            Err(crate::error::ExpectError::SessionClosed)
        }
    }

    /// Tick the keepalive manager.
    /// Returns `true` if the session is still considered alive.
    pub fn keepalive_tick(&mut self) -> bool {
        use super::keepalive::KeepaliveAction;

        if !self.is_connected() {
            return false;
        }

        match self.keepalive.tick() {
            KeepaliveAction::None | KeepaliveAction::SendKeepalive => true,
            KeepaliveAction::Timeout | KeepaliveAction::Disconnect => false,
        }
    }

    /// Check session health.
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.is_connected() && self.keepalive.state().is_alive()
    }

    /// Get underlying session.
    #[must_use]
    pub const fn session(&self) -> Option<&SshSession> {
        self.session.as_ref()
    }

    /// Get mutable underlying session.
    pub fn session_mut(&mut self) -> Option<&mut SshSession> {
        self.session.as_mut()
    }

    /// Record activity (resets idle timer).
    pub fn record_activity(&mut self) {
        self.last_activity = Instant::now();
        self.keepalive.handle_response();
    }

    /// Get time since last activity.
    #[must_use]
    pub fn idle_time(&self) -> Duration {
        self.last_activity.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resilient_config() {
        let ssh_config = SshConfig::new("example.com");
        let config = ResilientConfig::new(ssh_config)
            .with_keepalive(KeepaliveConfig::new().interval(Duration::from_secs(10)));

        assert!(config.auto_reconnect);
        assert_eq!(config.keepalive.interval, Duration::from_secs(10));
    }

    #[test]
    fn resilient_session_state() {
        let ssh_config = SshConfig::new("example.com");
        let config = ResilientConfig::new(ssh_config);
        let session = ResilientSession::new(config);

        assert_eq!(session.state(), ResilientState::Disconnected);
        assert!(!session.is_connected());
    }
}
