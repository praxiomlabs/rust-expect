//! SSH retry policies and strategies.

use std::time::Duration;

/// Retry strategy.
#[derive(Debug, Clone)]
pub enum RetryStrategy {
    /// No retries.
    None,
    /// Fixed delay between retries.
    Fixed {
        /// Delay between attempts.
        delay: Duration,
        /// Maximum attempts.
        max_attempts: u32,
    },
    /// Exponential backoff.
    Exponential {
        /// Initial delay.
        initial_delay: Duration,
        /// Maximum delay.
        max_delay: Duration,
        /// Multiplier for each attempt.
        multiplier: f64,
        /// Maximum attempts.
        max_attempts: u32,
    },
    /// Linear backoff.
    Linear {
        /// Initial delay.
        initial_delay: Duration,
        /// Increment per attempt.
        increment: Duration,
        /// Maximum delay.
        max_delay: Duration,
        /// Maximum attempts.
        max_attempts: u32,
    },
}

impl Default for RetryStrategy {
    fn default() -> Self {
        Self::Exponential {
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            multiplier: 2.0,
            max_attempts: 5,
        }
    }
}

impl RetryStrategy {
    /// Create no retry strategy.
    #[must_use]
    pub const fn none() -> Self {
        Self::None
    }

    /// Create fixed delay strategy.
    #[must_use]
    pub const fn fixed(delay: Duration, max_attempts: u32) -> Self {
        Self::Fixed {
            delay,
            max_attempts,
        }
    }

    /// Create exponential backoff strategy.
    #[must_use]
    pub const fn exponential(initial_delay: Duration, max_attempts: u32) -> Self {
        Self::Exponential {
            initial_delay,
            max_delay: Duration::from_secs(60),
            multiplier: 2.0,
            max_attempts,
        }
    }

    /// Get delay for attempt.
    #[must_use]
    pub fn delay_for_attempt(&self, attempt: u32) -> Option<Duration> {
        match self {
            Self::None => None,
            Self::Fixed {
                delay,
                max_attempts,
            } => {
                if attempt < *max_attempts {
                    Some(*delay)
                } else {
                    None
                }
            }
            Self::Exponential {
                initial_delay,
                max_delay,
                multiplier,
                max_attempts,
            } => {
                if attempt < *max_attempts {
                    let delay = initial_delay.as_secs_f64() * multiplier.powi(attempt as i32);
                    let delay = Duration::from_secs_f64(delay).min(*max_delay);
                    Some(delay)
                } else {
                    None
                }
            }
            Self::Linear {
                initial_delay,
                increment,
                max_delay,
                max_attempts,
            } => {
                if attempt < *max_attempts {
                    let delay = *initial_delay + (*increment * attempt);
                    Some(delay.min(*max_delay))
                } else {
                    None
                }
            }
        }
    }

    /// Check if should retry.
    #[must_use]
    pub fn should_retry(&self, attempt: u32) -> bool {
        self.delay_for_attempt(attempt).is_some()
    }

    /// Get max attempts.
    #[must_use]
    pub const fn max_attempts(&self) -> u32 {
        match self {
            Self::None => 1,
            Self::Fixed { max_attempts, .. }
            | Self::Exponential { max_attempts, .. }
            | Self::Linear { max_attempts, .. } => *max_attempts,
        }
    }
}

/// Retry policy for SSH operations.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Strategy for connection retries.
    pub connection: RetryStrategy,
    /// Strategy for command retries.
    pub command: RetryStrategy,
    /// Whether to retry on timeout.
    pub retry_on_timeout: bool,
    /// Whether to retry on disconnect.
    pub retry_on_disconnect: bool,
    /// Errors that should not be retried.
    pub non_retryable_errors: Vec<String>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            connection: RetryStrategy::default(),
            command: RetryStrategy::none(),
            retry_on_timeout: true,
            retry_on_disconnect: true,
            non_retryable_errors: vec![
                "authentication failed".to_string(),
                "permission denied".to_string(),
            ],
        }
    }
}

impl RetryPolicy {
    /// Create new policy.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set connection retry strategy.
    #[must_use]
    pub const fn with_connection_retries(mut self, strategy: RetryStrategy) -> Self {
        self.connection = strategy;
        self
    }

    /// Set command retry strategy.
    #[must_use]
    pub const fn with_command_retries(mut self, strategy: RetryStrategy) -> Self {
        self.command = strategy;
        self
    }

    /// Check if error is retryable.
    #[must_use]
    pub fn is_retryable(&self, error: &str) -> bool {
        let error_lower = error.to_lowercase();
        !self
            .non_retryable_errors
            .iter()
            .any(|e| error_lower.contains(e))
    }
}

/// Retry state tracker.
#[derive(Debug)]
pub struct RetryState {
    /// Current attempt (0-indexed).
    attempt: u32,
    /// Strategy in use.
    strategy: RetryStrategy,
    /// Total delay accumulated.
    total_delay: Duration,
}

impl RetryState {
    /// Create new state.
    #[must_use]
    pub const fn new(strategy: RetryStrategy) -> Self {
        Self {
            attempt: 0,
            strategy,
            total_delay: Duration::ZERO,
        }
    }

    /// Get current attempt.
    #[must_use]
    pub const fn attempt(&self) -> u32 {
        self.attempt
    }

    /// Check if should retry.
    #[must_use]
    pub fn should_retry(&self) -> bool {
        self.strategy.should_retry(self.attempt)
    }

    /// Get next delay.
    #[must_use]
    pub fn next_delay(&self) -> Option<Duration> {
        self.strategy.delay_for_attempt(self.attempt)
    }

    /// Record an attempt.
    pub fn record_attempt(&mut self) {
        if let Some(delay) = self.next_delay() {
            self.total_delay += delay;
        }
        self.attempt += 1;
    }

    /// Get total delay so far.
    #[must_use]
    pub const fn total_delay(&self) -> Duration {
        self.total_delay
    }

    /// Reset state.
    pub const fn reset(&mut self) {
        self.attempt = 0;
        self.total_delay = Duration::ZERO;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_strategy() {
        let strategy = RetryStrategy::fixed(Duration::from_millis(100), 3);

        assert!(strategy.should_retry(0));
        assert!(strategy.should_retry(2));
        assert!(!strategy.should_retry(3));
    }

    #[test]
    fn exponential_strategy() {
        let strategy = RetryStrategy::exponential(Duration::from_millis(100), 3);

        let d0 = strategy.delay_for_attempt(0).unwrap();
        let d1 = strategy.delay_for_attempt(1).unwrap();
        let d2 = strategy.delay_for_attempt(2).unwrap();

        assert!(d1 > d0);
        assert!(d2 > d1);
    }

    #[test]
    fn retry_state() {
        let strategy = RetryStrategy::fixed(Duration::from_millis(100), 2);
        let mut state = RetryState::new(strategy);

        assert!(state.should_retry());
        state.record_attempt();
        assert!(state.should_retry());
        state.record_attempt();
        assert!(!state.should_retry());
    }
}
