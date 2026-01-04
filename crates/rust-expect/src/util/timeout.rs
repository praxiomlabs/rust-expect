//! Timeout utilities.
//!
//! This module provides utilities for handling timeouts in expect operations.

use std::future::Future;
use std::time::Duration;
use tokio::time::{Timeout, timeout};

/// Extension trait for adding timeouts to futures.
pub trait TimeoutExt: Sized {
    /// Wrap this future with a timeout.
    fn with_timeout(self, duration: Duration) -> Timeout<Self>;

    /// Wrap this future with a timeout in seconds.
    fn with_timeout_secs(self, secs: u64) -> Timeout<Self> {
        self.with_timeout(Duration::from_secs(secs))
    }

    /// Wrap this future with a timeout in milliseconds.
    fn with_timeout_ms(self, ms: u64) -> Timeout<Self> {
        self.with_timeout(Duration::from_millis(ms))
    }
}

impl<F: Future> TimeoutExt for F {
    fn with_timeout(self, duration: Duration) -> Timeout<Self> {
        timeout(duration, self)
    }
}

/// A timeout configuration.
#[derive(Debug, Clone, Copy)]
pub struct TimeoutConfig {
    /// Default timeout for expect operations.
    pub expect: Duration,
    /// Timeout for connection operations.
    pub connect: Duration,
    /// Timeout for read operations.
    pub read: Duration,
    /// Timeout for write operations.
    pub write: Duration,
    /// Timeout for close operations.
    pub close: Duration,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            expect: Duration::from_secs(30),
            connect: Duration::from_secs(60),
            read: Duration::from_secs(10),
            write: Duration::from_secs(10),
            close: Duration::from_secs(5),
        }
    }
}

impl TimeoutConfig {
    /// Create a new timeout configuration with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the expect timeout.
    #[must_use]
    pub const fn expect(mut self, timeout: Duration) -> Self {
        self.expect = timeout;
        self
    }

    /// Set the connect timeout.
    #[must_use]
    pub const fn connect(mut self, timeout: Duration) -> Self {
        self.connect = timeout;
        self
    }

    /// Set the read timeout.
    #[must_use]
    pub const fn read(mut self, timeout: Duration) -> Self {
        self.read = timeout;
        self
    }

    /// Set the write timeout.
    #[must_use]
    pub const fn write(mut self, timeout: Duration) -> Self {
        self.write = timeout;
        self
    }

    /// Set the close timeout.
    #[must_use]
    pub const fn close(mut self, timeout: Duration) -> Self {
        self.close = timeout;
        self
    }

    /// Create a configuration with all timeouts set to the same value.
    #[must_use]
    pub const fn uniform(timeout: Duration) -> Self {
        Self {
            expect: timeout,
            connect: timeout,
            read: timeout,
            write: timeout,
            close: timeout,
        }
    }

    /// Create a configuration with no timeouts (effectively infinite).
    #[must_use]
    pub const fn none() -> Self {
        let max = Duration::from_secs(u64::MAX / 2);
        Self::uniform(max)
    }
}

/// A deadline tracker for operations with multiple steps.
#[derive(Debug, Clone)]
pub struct Deadline {
    /// The deadline instant.
    deadline: tokio::time::Instant,
}

impl Deadline {
    /// Create a new deadline from now.
    #[must_use]
    pub fn from_now(duration: Duration) -> Self {
        Self {
            deadline: tokio::time::Instant::now() + duration,
        }
    }

    /// Check if the deadline has passed.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        tokio::time::Instant::now() >= self.deadline
    }

    /// Get the remaining time until the deadline.
    #[must_use]
    pub fn remaining(&self) -> Duration {
        self.deadline
            .saturating_duration_since(tokio::time::Instant::now())
    }

    /// Check if there is time remaining.
    #[must_use]
    pub fn has_time(&self) -> bool {
        !self.is_expired()
    }

    /// Sleep until the deadline.
    pub async fn sleep(&self) {
        let remaining = self.remaining();
        if !remaining.is_zero() {
            tokio::time::sleep(remaining).await;
        }
    }

    /// Apply this deadline to a future.
    pub fn apply<F: Future>(&self, future: F) -> Timeout<F> {
        timeout(self.remaining(), future)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_config_default() {
        let config = TimeoutConfig::default();
        assert_eq!(config.expect, Duration::from_secs(30));
    }

    #[test]
    fn timeout_config_uniform() {
        let config = TimeoutConfig::uniform(Duration::from_secs(5));
        assert_eq!(config.expect, Duration::from_secs(5));
        assert_eq!(config.connect, Duration::from_secs(5));
    }

    #[tokio::test]
    async fn deadline_remaining() {
        let deadline = Deadline::from_now(Duration::from_secs(10));
        assert!(deadline.has_time());
        assert!(deadline.remaining() > Duration::from_secs(9));
    }

    #[tokio::test]
    async fn timeout_ext() {
        let result = async { 42 }.with_timeout(Duration::from_secs(1)).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }
}
