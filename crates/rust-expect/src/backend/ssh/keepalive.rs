//! SSH keepalive management.
//!
//! This module provides keepalive management for SSH sessions, supporting both
//! SSH-level keepalives (OpenSSH-compatible) and TCP-level keepalives.
//!
//! # SSH Keepalives
//!
//! SSH keepalives send periodic "keep-alive@openssh.com" global requests to
//! the server. If the server doesn't respond within a timeout, the connection
//! is considered dead. This is the preferred method as it operates at the
//! application layer.
//!
//! # Example
//!
//! ```ignore
//! use rust_expect::backend::ssh::{KeepaliveConfig, KeepaliveManager};
//! use std::time::Duration;
//!
//! let config = KeepaliveConfig::new()
//!     .interval(Duration::from_secs(30))
//!     .max_missed(3);
//!
//! let mut manager = KeepaliveManager::new(config);
//!
//! // In your connection loop:
//! if manager.is_due() {
//!     // Send keepalive via russh
//!     handle.send_keepalive(true).await?;
//!     manager.record_sent();
//! }
//! ```

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
    ///
    /// SSH-level keepalives use OpenSSH's "keep-alive@openssh.com" global request.
    /// TCP-level keepalives rely on the operating system's TCP keepalive mechanism.
    pub use_ssh_keepalive: bool,
    /// Timeout for each keepalive response.
    pub response_timeout: Duration,
}

impl Default for KeepaliveConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30),
            max_missed: 3,
            enabled: true,
            use_ssh_keepalive: true,
            response_timeout: Duration::from_secs(15),
        }
    }
}

impl KeepaliveConfig {
    /// Create new config.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set interval between keepalive messages.
    #[must_use]
    pub const fn interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    /// Set maximum missed keepalives before considering connection dead.
    #[must_use]
    pub const fn max_missed(mut self, max: u32) -> Self {
        self.max_missed = max;
        self
    }

    /// Enable or disable keepalive.
    #[must_use]
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set response timeout for each keepalive.
    #[must_use]
    pub const fn response_timeout(mut self, timeout: Duration) -> Self {
        self.response_timeout = timeout;
        self
    }

    /// Use SSH-level keepalives (OpenSSH compatible).
    #[must_use]
    pub const fn use_ssh_keepalive(mut self, use_ssh: bool) -> Self {
        self.use_ssh_keepalive = use_ssh;
        self
    }

    /// Create a disabled keepalive config.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Create a config optimized for high-latency connections.
    #[must_use]
    pub fn high_latency() -> Self {
        Self {
            interval: Duration::from_secs(60),
            max_missed: 5,
            response_timeout: Duration::from_secs(30),
            ..Default::default()
        }
    }

    /// Create a config optimized for unstable connections.
    #[must_use]
    pub fn aggressive() -> Self {
        Self {
            interval: Duration::from_secs(15),
            max_missed: 2,
            response_timeout: Duration::from_secs(10),
            ..Default::default()
        }
    }
}

/// Keepalive state tracking.
///
/// This struct tracks the state of keepalive messages, including when they were
/// sent, when responses were received, and whether the connection is considered
/// alive based on the number of missed keepalives.
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
    /// Whether we're waiting for a response.
    pending_response: bool,
    /// When the pending keepalive was sent.
    pending_since: Option<Instant>,
    /// Total keepalives sent.
    total_sent: u64,
    /// Total responses received.
    total_received: u64,
}

impl KeepaliveState {
    /// Create new state.
    #[must_use]
    pub fn new(config: KeepaliveConfig) -> Self {
        Self {
            config,
            last_sent: None,
            last_received: None,
            missed_count: 0,
            alive: true,
            pending_response: false,
            pending_since: None,
            total_sent: 0,
            total_received: 0,
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
    ///
    /// Returns true if keepalive is enabled and either:
    /// - No keepalive has been sent yet
    /// - The interval has elapsed since the last keepalive
    #[must_use]
    pub fn is_keepalive_due(&self) -> bool {
        if !self.config.enabled {
            return false;
        }

        // Don't send if we're waiting for a response
        if self.pending_response {
            return false;
        }

        match self.last_sent {
            Some(last) => last.elapsed() >= self.config.interval,
            None => true,
        }
    }

    /// Check if a pending keepalive has timed out.
    #[must_use]
    pub fn is_response_timed_out(&self) -> bool {
        if !self.pending_response {
            return false;
        }

        self.pending_since
            .map(|t| t.elapsed() >= self.config.response_timeout)
            .unwrap_or(false)
    }

    /// Record sending a keepalive.
    pub fn record_sent(&mut self) {
        let now = Instant::now();
        self.last_sent = Some(now);
        self.pending_response = true;
        self.pending_since = Some(now);
        self.total_sent += 1;
    }

    /// Record receiving a response.
    pub fn record_received(&mut self) {
        self.last_received = Some(Instant::now());
        self.missed_count = 0;
        self.alive = true;
        self.pending_response = false;
        self.pending_since = None;
        self.total_received += 1;
    }

    /// Record a missed keepalive (timeout).
    pub fn record_missed(&mut self) {
        self.missed_count += 1;
        self.pending_response = false;
        self.pending_since = None;

        if self.missed_count >= self.config.max_missed {
            self.alive = false;
        }
    }

    /// Get missed count.
    #[must_use]
    pub const fn missed_count(&self) -> u32 {
        self.missed_count
    }

    /// Get total keepalives sent.
    #[must_use]
    pub const fn total_sent(&self) -> u64 {
        self.total_sent
    }

    /// Get total responses received.
    #[must_use]
    pub const fn total_received(&self) -> u64 {
        self.total_received
    }

    /// Check if we're waiting for a response.
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        self.pending_response
    }

    /// Get time since last activity (sent or received).
    #[must_use]
    pub fn time_since_activity(&self) -> Duration {
        self.last_received
            .or(self.last_sent)
            .map_or(Duration::ZERO, |t| t.elapsed())
    }

    /// Get time since last successful response.
    #[must_use]
    pub fn time_since_response(&self) -> Option<Duration> {
        self.last_received.map(|t| t.elapsed())
    }

    /// Get the configuration.
    #[must_use]
    pub const fn config(&self) -> &KeepaliveConfig {
        &self.config
    }

    /// Reset state for reconnection.
    pub fn reset(&mut self) {
        self.last_sent = None;
        self.last_received = None;
        self.missed_count = 0;
        self.alive = true;
        self.pending_response = false;
        self.pending_since = None;
        // Note: we preserve total_sent and total_received for statistics
    }
}

/// Result of a keepalive tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeepaliveAction {
    /// No action needed.
    None,
    /// A keepalive should be sent.
    SendKeepalive,
    /// A pending keepalive timed out.
    Timeout,
    /// Connection is considered dead.
    Disconnect,
}

/// Keepalive manager.
///
/// This manages the keepalive lifecycle and provides integration points for
/// the SSH session to send and receive keepalive messages.
///
/// # Usage with russh
///
/// ```ignore
/// use rust_expect::backend::ssh::{KeepaliveConfig, KeepaliveManager, KeepaliveAction};
///
/// let mut manager = KeepaliveManager::new(KeepaliveConfig::default());
///
/// loop {
///     match manager.tick() {
///         KeepaliveAction::SendKeepalive => {
///             // Send keepalive via russh: handle.send_keepalive(true).await
///             manager.record_sent();
///         }
///         KeepaliveAction::Timeout => {
///             // Keepalive timed out, record as missed
///             manager.record_timeout();
///         }
///         KeepaliveAction::Disconnect => {
///             // Too many missed keepalives, close connection
///             break;
///         }
///         KeepaliveAction::None => {
///             // Nothing to do
///         }
///     }
///
///     tokio::time::sleep(Duration::from_secs(1)).await;
/// }
/// ```
#[derive(Debug)]
pub struct KeepaliveManager {
    /// State.
    state: KeepaliveState,
}

impl KeepaliveManager {
    /// Create new manager.
    #[must_use]
    pub fn new(config: KeepaliveConfig) -> Self {
        Self {
            state: KeepaliveState::new(config),
        }
    }

    /// Create a disabled manager.
    #[must_use]
    pub fn disabled() -> Self {
        Self::new(KeepaliveConfig::disabled())
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

    /// Check if keepalive is enabled.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.state.is_enabled()
    }

    /// Check if connection is alive.
    #[must_use]
    pub const fn is_alive(&self) -> bool {
        self.state.is_alive()
    }

    /// Check if a keepalive should be sent now.
    #[must_use]
    pub fn is_due(&self) -> bool {
        self.state.is_keepalive_due()
    }

    /// Tick the keepalive manager and determine what action to take.
    ///
    /// Call this periodically (e.g., every second) to manage keepalives.
    /// The returned action tells you what to do:
    ///
    /// - `SendKeepalive`: Call `record_sent()` after sending a keepalive
    /// - `Timeout`: A pending keepalive timed out, call `record_timeout()`
    /// - `Disconnect`: Connection is dead, close it
    /// - `None`: Nothing to do
    #[must_use]
    pub fn tick(&mut self) -> KeepaliveAction {
        if !self.state.is_enabled() {
            return KeepaliveAction::None;
        }

        // Check if connection is dead
        if !self.state.is_alive() {
            return KeepaliveAction::Disconnect;
        }

        // Check for timeout on pending keepalive
        if self.state.is_response_timed_out() {
            return KeepaliveAction::Timeout;
        }

        // Check if we should send a keepalive
        if self.state.is_keepalive_due() {
            return KeepaliveAction::SendKeepalive;
        }

        KeepaliveAction::None
    }

    /// Record that a keepalive was sent.
    ///
    /// Call this after successfully sending a keepalive via russh.
    pub fn record_sent(&mut self) {
        self.state.record_sent();
    }

    /// Record that a keepalive response was received.
    ///
    /// Call this when russh indicates the keepalive succeeded.
    pub fn record_response(&mut self) {
        self.state.record_received();
    }

    /// Record that a keepalive timed out.
    ///
    /// Call this when the response timeout expires without a response.
    pub fn record_timeout(&mut self) {
        self.state.record_missed();
    }

    /// Handle keepalive response (alias for `record_response`).
    pub fn handle_response(&mut self) {
        self.record_response();
    }

    /// Handle keepalive timeout (alias for `record_timeout`).
    pub fn handle_timeout(&mut self) {
        self.record_timeout();
    }

    /// Check if connection should be closed due to missed keepalives.
    #[must_use]
    pub const fn should_disconnect(&self) -> bool {
        !self.state.is_alive()
    }

    /// Get statistics about keepalives.
    #[must_use]
    pub fn stats(&self) -> KeepaliveStats {
        KeepaliveStats {
            total_sent: self.state.total_sent(),
            total_received: self.state.total_received(),
            missed_count: self.state.missed_count(),
            is_alive: self.state.is_alive(),
            time_since_activity: self.state.time_since_activity(),
        }
    }

    /// Reset the manager for reconnection.
    pub fn reset(&mut self) {
        self.state.reset();
    }
}

/// Keepalive statistics.
#[derive(Debug, Clone)]
pub struct KeepaliveStats {
    /// Total keepalives sent.
    pub total_sent: u64,
    /// Total responses received.
    pub total_received: u64,
    /// Current consecutive missed count.
    pub missed_count: u32,
    /// Whether connection is alive.
    pub is_alive: bool,
    /// Time since last activity.
    pub time_since_activity: Duration,
}

impl KeepaliveStats {
    /// Get the success rate (responses / sent).
    #[must_use]
    pub fn success_rate(&self) -> f64 {
        if self.total_sent == 0 {
            1.0
        } else {
            self.total_received as f64 / self.total_sent as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keepalive_config_builder() {
        let config = KeepaliveConfig::new()
            .interval(Duration::from_secs(60))
            .max_missed(5)
            .response_timeout(Duration::from_secs(20))
            .use_ssh_keepalive(true)
            .enabled(true);

        assert_eq!(config.interval, Duration::from_secs(60));
        assert_eq!(config.max_missed, 5);
        assert_eq!(config.response_timeout, Duration::from_secs(20));
        assert!(config.use_ssh_keepalive);
        assert!(config.enabled);
    }

    #[test]
    fn keepalive_config_presets() {
        let disabled = KeepaliveConfig::disabled();
        assert!(!disabled.enabled);

        let high_latency = KeepaliveConfig::high_latency();
        assert_eq!(high_latency.interval, Duration::from_secs(60));
        assert_eq!(high_latency.max_missed, 5);

        let aggressive = KeepaliveConfig::aggressive();
        assert_eq!(aggressive.interval, Duration::from_secs(15));
        assert_eq!(aggressive.max_missed, 2);
    }

    #[test]
    fn keepalive_state_lifecycle() {
        let config = KeepaliveConfig::new().max_missed(2);
        let mut state = KeepaliveState::new(config);

        assert!(state.is_alive());
        assert!(!state.is_pending());
        assert_eq!(state.total_sent(), 0);

        // Send a keepalive
        state.record_sent();
        assert!(state.is_pending());
        assert_eq!(state.total_sent(), 1);

        // Receive response
        state.record_received();
        assert!(!state.is_pending());
        assert_eq!(state.total_received(), 1);
        assert!(state.is_alive());
    }

    #[test]
    fn keepalive_state_missed() {
        let config = KeepaliveConfig::new().max_missed(2);
        let mut state = KeepaliveState::new(config);

        assert!(state.is_alive());

        state.record_missed();
        assert!(state.is_alive());
        assert_eq!(state.missed_count(), 1);

        state.record_missed();
        assert!(!state.is_alive());
        assert_eq!(state.missed_count(), 2);
    }

    #[test]
    fn keepalive_state_recovery() {
        let config = KeepaliveConfig::new().max_missed(2);
        let mut state = KeepaliveState::new(config);

        state.record_missed();
        state.record_received(); // Recovery
        assert_eq!(state.missed_count(), 0);
        assert!(state.is_alive());
    }

    #[test]
    fn keepalive_manager_disabled() {
        let mut manager = KeepaliveManager::disabled();
        assert!(!manager.is_enabled());
        assert_eq!(manager.tick(), KeepaliveAction::None);
    }

    #[test]
    fn keepalive_manager_tick() {
        let config = KeepaliveConfig::new()
            .interval(Duration::from_millis(10));
        let mut manager = KeepaliveManager::new(config);

        // First tick should want to send
        assert_eq!(manager.tick(), KeepaliveAction::SendKeepalive);

        // Record sent
        manager.record_sent();

        // Should not want to send again immediately
        assert_eq!(manager.tick(), KeepaliveAction::None);
    }

    #[test]
    fn keepalive_manager_stats() {
        let config = KeepaliveConfig::new();
        let mut manager = KeepaliveManager::new(config);

        manager.record_sent();
        manager.record_response();

        let stats = manager.stats();
        assert_eq!(stats.total_sent, 1);
        assert_eq!(stats.total_received, 1);
        assert!(stats.is_alive);
        assert_eq!(stats.success_rate(), 1.0);
    }

    #[test]
    fn keepalive_stats_success_rate() {
        let stats = KeepaliveStats {
            total_sent: 10,
            total_received: 8,
            missed_count: 0,
            is_alive: true,
            time_since_activity: Duration::ZERO,
        };
        assert_eq!(stats.success_rate(), 0.8);

        let empty_stats = KeepaliveStats {
            total_sent: 0,
            total_received: 0,
            missed_count: 0,
            is_alive: true,
            time_since_activity: Duration::ZERO,
        };
        assert_eq!(empty_stats.success_rate(), 1.0);
    }

    #[test]
    fn keepalive_action_variants() {
        assert_eq!(KeepaliveAction::None, KeepaliveAction::None);
        assert_eq!(KeepaliveAction::SendKeepalive, KeepaliveAction::SendKeepalive);
        assert_eq!(KeepaliveAction::Timeout, KeepaliveAction::Timeout);
        assert_eq!(KeepaliveAction::Disconnect, KeepaliveAction::Disconnect);
    }

    #[test]
    fn keepalive_manager_reset() {
        let config = KeepaliveConfig::new().max_missed(1);
        let mut manager = KeepaliveManager::new(config);

        manager.record_sent();
        manager.record_timeout();
        assert!(!manager.is_alive());

        manager.reset();
        assert!(manager.is_alive());
        assert!(!manager.state().is_pending());
    }
}
