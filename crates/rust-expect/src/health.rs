//! Health checking and diagnostics.
//!
//! This module provides health checking capabilities for sessions
//! and connections.

use std::time::{Duration, Instant};

/// Health status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Healthy and operational.
    Healthy,
    /// Degraded but functional.
    Degraded,
    /// Unhealthy and non-functional.
    Unhealthy,
    /// Status unknown.
    Unknown,
}

impl HealthStatus {
    /// Check if healthy.
    #[must_use]
    pub const fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy)
    }

    /// Check if operational (healthy or degraded).
    #[must_use]
    pub const fn is_operational(&self) -> bool {
        matches!(self, Self::Healthy | Self::Degraded)
    }
}

/// Health check result.
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    /// Status.
    pub status: HealthStatus,
    /// Message.
    pub message: Option<String>,
    /// Check duration.
    pub duration: Duration,
    /// Timestamp.
    pub timestamp: Instant,
}

impl HealthCheckResult {
    /// Create a healthy result.
    #[must_use]
    pub fn healthy() -> Self {
        Self {
            status: HealthStatus::Healthy,
            message: None,
            duration: Duration::ZERO,
            timestamp: Instant::now(),
        }
    }

    /// Create an unhealthy result.
    #[must_use]
    pub fn unhealthy(message: impl Into<String>) -> Self {
        Self {
            status: HealthStatus::Unhealthy,
            message: Some(message.into()),
            duration: Duration::ZERO,
            timestamp: Instant::now(),
        }
    }

    /// Create a degraded result.
    #[must_use]
    pub fn degraded(message: impl Into<String>) -> Self {
        Self {
            status: HealthStatus::Degraded,
            message: Some(message.into()),
            duration: Duration::ZERO,
            timestamp: Instant::now(),
        }
    }

    /// Set duration.
    #[must_use]
    pub const fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }
}

/// Health check configuration.
#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    /// Check interval.
    pub interval: Duration,
    /// Timeout for health checks.
    pub timeout: Duration,
    /// Number of failures before unhealthy.
    pub failure_threshold: u32,
    /// Number of successes before healthy.
    pub success_threshold: u32,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            failure_threshold: 3,
            success_threshold: 1,
        }
    }
}

impl HealthCheckConfig {
    /// Create new config.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set interval.
    #[must_use]
    pub const fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    /// Set timeout.
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set failure threshold.
    #[must_use]
    pub const fn with_failure_threshold(mut self, threshold: u32) -> Self {
        self.failure_threshold = threshold;
        self
    }

    /// Set success threshold.
    #[must_use]
    pub const fn with_success_threshold(mut self, threshold: u32) -> Self {
        self.success_threshold = threshold;
        self
    }
}

/// Health checker state.
#[derive(Debug)]
pub struct HealthChecker {
    /// Configuration.
    config: HealthCheckConfig,
    /// Current status.
    status: HealthStatus,
    /// Consecutive failures.
    failures: u32,
    /// Consecutive successes.
    successes: u32,
    /// Last check time.
    last_check: Option<Instant>,
    /// Last result.
    last_result: Option<HealthCheckResult>,
}

impl HealthChecker {
    /// Create a new health checker.
    #[must_use]
    pub const fn new(config: HealthCheckConfig) -> Self {
        Self {
            config,
            status: HealthStatus::Unknown,
            failures: 0,
            successes: 0,
            last_check: None,
            last_result: None,
        }
    }

    /// Get current status.
    #[must_use]
    pub const fn status(&self) -> HealthStatus {
        self.status
    }

    /// Get last result.
    #[must_use]
    pub const fn last_result(&self) -> Option<&HealthCheckResult> {
        self.last_result.as_ref()
    }

    /// Check if a health check is due.
    #[must_use]
    pub fn is_check_due(&self) -> bool {
        match self.last_check {
            Some(last) => last.elapsed() >= self.config.interval,
            None => true,
        }
    }

    /// Record a successful check.
    pub fn record_success(&mut self) {
        self.failures = 0;
        self.successes += 1;
        self.last_check = Some(Instant::now());

        if self.successes >= self.config.success_threshold {
            self.status = HealthStatus::Healthy;
        }

        self.last_result = Some(HealthCheckResult::healthy());
    }

    /// Record a failed check.
    pub fn record_failure(&mut self, message: impl Into<String>) {
        self.successes = 0;
        self.failures += 1;
        self.last_check = Some(Instant::now());

        if self.failures >= self.config.failure_threshold {
            self.status = HealthStatus::Unhealthy;
        } else if self.failures > 0 {
            self.status = HealthStatus::Degraded;
        }

        self.last_result = Some(HealthCheckResult::unhealthy(message));
    }

    /// Reset the checker.
    pub fn reset(&mut self) {
        self.status = HealthStatus::Unknown;
        self.failures = 0;
        self.successes = 0;
        self.last_check = None;
        self.last_result = None;
    }
}

/// Simple liveness check.
#[must_use]
pub fn liveness_check() -> HealthCheckResult {
    HealthCheckResult::healthy()
}

/// Check if a process is alive by PID.
#[must_use]
#[cfg(unix)]
pub fn process_alive(pid: i32) -> bool {
    // Send signal 0 to check if process exists
    unsafe { libc::kill(pid, 0) == 0 }
}

#[cfg(not(unix))]
pub fn process_alive(_pid: i32) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_status() {
        assert!(HealthStatus::Healthy.is_healthy());
        assert!(HealthStatus::Healthy.is_operational());
        assert!(!HealthStatus::Degraded.is_healthy());
        assert!(HealthStatus::Degraded.is_operational());
        assert!(!HealthStatus::Unhealthy.is_operational());
    }

    #[test]
    fn health_checker_transitions() {
        let config = HealthCheckConfig {
            failure_threshold: 2,
            success_threshold: 1,
            ..Default::default()
        };
        let mut checker = HealthChecker::new(config);

        assert_eq!(checker.status(), HealthStatus::Unknown);

        checker.record_success();
        assert_eq!(checker.status(), HealthStatus::Healthy);

        checker.record_failure("test");
        assert_eq!(checker.status(), HealthStatus::Degraded);

        checker.record_failure("test");
        assert_eq!(checker.status(), HealthStatus::Unhealthy);

        checker.record_success();
        assert_eq!(checker.status(), HealthStatus::Healthy);
    }
}
