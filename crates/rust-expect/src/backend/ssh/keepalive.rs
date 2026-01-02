//! SSH keepalive management.

use std::time::{Duration, Instant};

/// Keepalive configuration.
#[derive(Debug, Clone)]
pub struct KeepaliveConfig {
    /// Interval between keepalive messages.
    pub interval: Duration,
    /// Maximum missed keepalives before disconnect.
    pub max_missed: u32,
    /// Enable keepalive.
    pub enabled: bool,
    /// Use SSH-level keepalive (vs TCP).
    pub use_ssh_keepalive: bool,
}

impl Default for KeepaliveConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30),
            max_missed: 3,
            enabled: true,
            use_ssh_keepalive: true,
        }
    }
}

impl KeepaliveConfig {
    /// Create new config.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set interval.
    #[must_use]
    pub const fn interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    /// Set max missed.
    #[must_use]
    pub const fn max_missed(mut self, max: u32) -> Self {
        self.max_missed = max;
        self
    }

    /// Enable/disable.
    #[must_use]
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Disable keepalive.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }
}

/// Keepalive state.
#[derive(Debug)]
pub struct KeepaliveState {
    /// Configuration.
    config: KeepaliveConfig,
    /// Last keepalive sent.
    last_sent: Option<Instant>,
    /// Last response received.
    last_received: Option<Instant>,
    /// Consecutive missed keepalives.
    missed_count: u32,
    /// Whether connection is considered alive.
    alive: bool,
}

impl KeepaliveState {
    /// Create new state.
    #[must_use]
    pub const fn new(config: KeepaliveConfig) -> Self {
        Self {
            config,
            last_sent: None,
            last_received: None,
            missed_count: 0,
            alive: true,
        }
    }

    /// Check if keepalive is enabled.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Check if connection is alive.
    #[must_use]
    pub const fn is_alive(&self) -> bool {
        self.alive
    }

    /// Check if a keepalive is due.
    #[must_use]
    pub fn is_keepalive_due(&self) -> bool {
        if !self.config.enabled {
            return false;
        }

        match self.last_sent {
            Some(last) => last.elapsed() >= self.config.interval,
            None => true,
        }
    }

    /// Record sending a keepalive.
    pub fn record_sent(&mut self) {
        self.last_sent = Some(Instant::now());
    }

    /// Record receiving a response.
    pub fn record_received(&mut self) {
        self.last_received = Some(Instant::now());
        self.missed_count = 0;
        self.alive = true;
    }

    /// Record a missed keepalive (timeout).
    pub fn record_missed(&mut self) {
        self.missed_count += 1;
        if self.missed_count >= self.config.max_missed {
            self.alive = false;
        }
    }

    /// Get missed count.
    #[must_use]
    pub const fn missed_count(&self) -> u32 {
        self.missed_count
    }

    /// Get time since last activity.
    #[must_use]
    pub fn time_since_activity(&self) -> Duration {
        self.last_received
            .or(self.last_sent)
            .map_or(Duration::ZERO, |t| t.elapsed())
    }

    /// Reset state.
    pub fn reset(&mut self) {
        self.last_sent = None;
        self.last_received = None;
        self.missed_count = 0;
        self.alive = true;
    }
}

/// Keepalive manager.
#[derive(Debug)]
pub struct KeepaliveManager {
    /// State.
    state: KeepaliveState,
}

impl KeepaliveManager {
    /// Create new manager.
    #[must_use]
    pub const fn new(config: KeepaliveConfig) -> Self {
        Self {
            state: KeepaliveState::new(config),
        }
    }

    /// Get state.
    #[must_use]
    pub const fn state(&self) -> &KeepaliveState {
        &self.state
    }

    /// Get mutable state.
    pub fn state_mut(&mut self) -> &mut KeepaliveState {
        &mut self.state
    }

    /// Check and perform keepalive if needed.
    /// Returns true if a keepalive was sent.
    pub fn tick(&mut self) -> bool {
        if !self.state.is_enabled() {
            return false;
        }

        if self.state.is_keepalive_due() {
            self.state.record_sent();
            true
        } else {
            false
        }
    }

    /// Handle keepalive response.
    pub fn handle_response(&mut self) {
        self.state.record_received();
    }

    /// Handle keepalive timeout.
    pub fn handle_timeout(&mut self) {
        self.state.record_missed();
    }

    /// Check if connection should be closed.
    #[must_use]
    pub const fn should_disconnect(&self) -> bool {
        !self.state.is_alive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keepalive_config() {
        let config = KeepaliveConfig::new()
            .interval(Duration::from_secs(60))
            .max_missed(5);

        assert_eq!(config.interval, Duration::from_secs(60));
        assert_eq!(config.max_missed, 5);
    }

    #[test]
    fn keepalive_state() {
        let config = KeepaliveConfig::new().max_missed(2);
        let mut state = KeepaliveState::new(config);

        assert!(state.is_alive());

        state.record_missed();
        assert!(state.is_alive());

        state.record_missed();
        assert!(!state.is_alive());
    }

    #[test]
    fn keepalive_recovery() {
        let config = KeepaliveConfig::new().max_missed(2);
        let mut state = KeepaliveState::new(config);

        state.record_missed();
        state.record_received(); // Recovery
        assert_eq!(state.missed_count(), 0);
        assert!(state.is_alive());
    }
}
