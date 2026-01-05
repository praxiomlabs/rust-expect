//! Backpressure and flow control utilities.
//!
//! This module provides utilities for managing data flow between
//! producers and consumers with backpressure.
//!
//! # Overview
//!
//! The [`Backpressure`] controller provides two complementary mechanisms:
//!
//! 1. **Buffer size tracking**: Track how much data is buffered and block when full.
//!    Use [`try_acquire`](Backpressure::try_acquire), [`acquire`](Backpressure::acquire),
//!    and [`release`](Backpressure::release) for this.
//!
//! 2. **Concurrent operation limiting**: Limit the number of concurrent operations
//!    using semaphore-based permits. Use [`try_acquire_permit`](Backpressure::try_acquire_permit)
//!    and [`acquire_permit`](Backpressure::acquire_permit) for this.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore, TryAcquireError};

/// A backpressure controller for limiting data flow and concurrent operations.
///
/// This controller provides two mechanisms:
///
/// - **Buffer size tracking**: For tracking how much data is in-flight or buffered.
/// - **Permit-based concurrency**: For limiting the number of concurrent operations.
///
/// Both mechanisms use the same `max_size` limit but operate independently,
/// allowing flexible backpressure strategies.
#[derive(Debug)]
pub struct Backpressure {
    /// Maximum buffer size / concurrent operations.
    max_size: usize,
    /// Current buffer size (for size-based tracking).
    current: AtomicUsize,
    /// Notify when space becomes available.
    space_available: Notify,
    /// Semaphore for permit-based concurrency limiting.
    /// Wrapped in Arc to allow owned permits.
    semaphore: Arc<Semaphore>,
}

impl Backpressure {
    /// Create a new backpressure controller.
    ///
    /// The `max_size` parameter controls both:
    /// - The maximum buffer size for size-based tracking
    /// - The number of available permits for concurrency limiting
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        Self {
            max_size,
            current: AtomicUsize::new(0),
            space_available: Notify::new(),
            semaphore: Arc::new(Semaphore::new(max_size)),
        }
    }

    // ========================================================================
    // Buffer Size Tracking Methods
    // ========================================================================

    /// Try to acquire space for the given amount.
    ///
    /// Returns true if space was acquired, false if the buffer is full.
    ///
    /// This is part of the buffer size tracking mechanism. Use [`release`](Self::release)
    /// to return the space when done.
    pub fn try_acquire(&self, amount: usize) -> bool {
        let current = self.current.load(Ordering::Acquire);
        if current + amount <= self.max_size {
            self.current.fetch_add(amount, Ordering::Release);
            true
        } else {
            false
        }
    }

    /// Acquire space for the given amount, waiting if necessary.
    ///
    /// This is part of the buffer size tracking mechanism. Use [`release`](Self::release)
    /// to return the space when done.
    pub async fn acquire(&self, amount: usize) {
        loop {
            if self.try_acquire(amount) {
                return;
            }
            self.space_available.notified().await;
        }
    }

    /// Release the given amount of space.
    ///
    /// This is part of the buffer size tracking mechanism. Call this after
    /// data has been processed/consumed to free space for new data.
    pub fn release(&self, amount: usize) {
        self.current.fetch_sub(amount, Ordering::Release);
        self.space_available.notify_one();
    }

    /// Get the current buffer usage.
    #[must_use]
    pub fn current_size(&self) -> usize {
        self.current.load(Ordering::Acquire)
    }

    /// Get the maximum buffer size.
    #[must_use]
    pub const fn max_size(&self) -> usize {
        self.max_size
    }

    /// Check if the buffer is full.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.current_size() >= self.max_size
    }

    /// Get the available space.
    #[must_use]
    pub fn available(&self) -> usize {
        self.max_size.saturating_sub(self.current_size())
    }

    // ========================================================================
    // Permit-Based Concurrency Limiting Methods
    // ========================================================================

    /// Try to acquire a permit for a concurrent operation.
    ///
    /// Returns `Ok(permit)` if a permit was acquired, or an error if no permits
    /// are available. The permit is automatically released when dropped.
    ///
    /// This is part of the permit-based concurrency mechanism. Use this when
    /// you want to limit the number of concurrent operations rather than
    /// tracking buffer sizes.
    ///
    /// # Example
    ///
    /// ```
    /// use rust_expect::util::backpressure::Backpressure;
    ///
    /// let bp = Backpressure::new(2); // Allow 2 concurrent operations
    ///
    /// let permit1 = bp.try_acquire_permit().unwrap();
    /// let permit2 = bp.try_acquire_permit().unwrap();
    ///
    /// // Third attempt fails - at capacity
    /// assert!(bp.try_acquire_permit().is_err());
    ///
    /// // Dropping a permit frees it
    /// drop(permit1);
    /// let permit3 = bp.try_acquire_permit().unwrap();
    /// ```
    pub fn try_acquire_permit(&self) -> Result<OwnedSemaphorePermit, TryAcquireError> {
        self.semaphore.clone().try_acquire_owned()
    }

    /// Acquire a permit for a concurrent operation, waiting if necessary.
    ///
    /// Returns a permit that is automatically released when dropped.
    ///
    /// This is part of the permit-based concurrency mechanism. Use this when
    /// you want to limit the number of concurrent operations.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rust_expect::util::backpressure::Backpressure;
    ///
    /// # async fn example() {
    /// let bp = Backpressure::new(10);
    ///
    /// // Acquire a permit - will wait if none available
    /// let permit = bp.acquire_permit().await;
    ///
    /// // Do work while holding the permit
    /// // ...
    ///
    /// // Permit is released when dropped
    /// drop(permit);
    /// # }
    /// ```
    pub async fn acquire_permit(&self) -> OwnedSemaphorePermit {
        self.semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("semaphore should not be closed")
    }

    /// Get the number of available permits.
    #[must_use]
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }
}

impl Default for Backpressure {
    fn default() -> Self {
        Self::new(64 * 1024) // 64KB default
    }
}

/// A rate limiter for controlling operation frequency.
#[derive(Debug)]
pub struct RateLimiter {
    /// Maximum operations per interval.
    max_ops: usize,
    /// Interval duration in milliseconds.
    interval_ms: u64,
    /// Current operation count.
    current: AtomicUsize,
    /// Last reset time.
    last_reset: std::sync::Mutex<std::time::Instant>,
}

impl RateLimiter {
    /// Create a new rate limiter.
    #[must_use]
    pub fn new(max_ops: usize, interval: std::time::Duration) -> Self {
        Self {
            max_ops,
            interval_ms: interval.as_millis() as u64,
            current: AtomicUsize::new(0),
            last_reset: std::sync::Mutex::new(std::time::Instant::now()),
        }
    }

    /// Try to perform an operation.
    ///
    /// Returns true if the operation is allowed, false if rate limited.
    pub fn try_acquire(&self) -> bool {
        self.maybe_reset();

        let current = self.current.fetch_add(1, Ordering::AcqRel);
        if current < self.max_ops {
            true
        } else {
            self.current.fetch_sub(1, Ordering::Release);
            false
        }
    }

    /// Perform an operation, waiting if necessary.
    pub async fn acquire(&self) {
        while !self.try_acquire() {
            let sleep_time = self.time_until_reset();
            tokio::time::sleep(sleep_time).await;
        }
    }

    /// Reset the counter if the interval has elapsed.
    fn maybe_reset(&self) {
        let mut last_reset = self
            .last_reset
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let elapsed = last_reset.elapsed();

        if elapsed.as_millis() as u64 >= self.interval_ms {
            self.current.store(0, Ordering::Release);
            *last_reset = std::time::Instant::now();
        }
    }

    /// Get the time until the next reset.
    #[allow(clippy::significant_drop_tightening)]
    fn time_until_reset(&self) -> std::time::Duration {
        let last_reset = self
            .last_reset
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let elapsed = last_reset.elapsed();
        let interval = std::time::Duration::from_millis(self.interval_ms);

        if elapsed >= interval {
            std::time::Duration::ZERO
        } else {
            interval - elapsed
        }
    }
}

/// A token bucket for rate limiting with bursts.
#[derive(Debug)]
pub struct TokenBucket {
    /// Maximum tokens in the bucket.
    capacity: usize,
    /// Current tokens.
    tokens: AtomicUsize,
    /// Token refill rate (per second).
    refill_rate: f64,
    /// Last refill time.
    last_refill: std::sync::Mutex<std::time::Instant>,
}

impl TokenBucket {
    /// Create a new token bucket.
    #[must_use]
    pub fn new(capacity: usize, refill_rate: f64) -> Self {
        Self {
            capacity,
            tokens: AtomicUsize::new(capacity),
            refill_rate,
            last_refill: std::sync::Mutex::new(std::time::Instant::now()),
        }
    }

    /// Try to consume tokens.
    pub fn try_consume(&self, count: usize) -> bool {
        self.refill();

        loop {
            let current = self.tokens.load(Ordering::Acquire);
            if current < count {
                return false;
            }

            if self
                .tokens
                .compare_exchange(
                    current,
                    current - count,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                return true;
            }
        }
    }

    /// Consume tokens, waiting if necessary.
    pub async fn consume(&self, count: usize) {
        while !self.try_consume(count) {
            let wait_time = self.time_for_tokens(count);
            tokio::time::sleep(wait_time).await;
        }
    }

    /// Refill tokens based on elapsed time.
    fn refill(&self) {
        let mut last_refill = self
            .last_refill
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let elapsed = last_refill.elapsed().as_secs_f64();
        let new_tokens = (elapsed * self.refill_rate) as usize;

        if new_tokens > 0 {
            let current = self.tokens.load(Ordering::Acquire);
            let new_value = (current + new_tokens).min(self.capacity);
            self.tokens.store(new_value, Ordering::Release);
            *last_refill = std::time::Instant::now();
        }
    }

    /// Get the time needed to have the specified number of tokens.
    fn time_for_tokens(&self, count: usize) -> std::time::Duration {
        let current = self.tokens.load(Ordering::Acquire);
        if current >= count {
            return std::time::Duration::ZERO;
        }

        let needed = count - current;
        let seconds = needed as f64 / self.refill_rate;
        std::time::Duration::from_secs_f64(seconds)
    }

    /// Get the current token count.
    #[must_use]
    pub fn tokens(&self) -> usize {
        self.refill();
        self.tokens.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backpressure_acquire() {
        let bp = Backpressure::new(100);
        assert!(bp.try_acquire(50));
        assert!(bp.try_acquire(50));
        assert!(!bp.try_acquire(1)); // Full

        bp.release(50);
        assert!(bp.try_acquire(50));
    }

    #[test]
    fn backpressure_permits() {
        let bp = Backpressure::new(3);

        // Acquire all permits
        let p1 = bp.try_acquire_permit().unwrap();
        let p2 = bp.try_acquire_permit().unwrap();
        let p3 = bp.try_acquire_permit().unwrap();

        // Fourth should fail
        assert!(bp.try_acquire_permit().is_err());
        assert_eq!(bp.available_permits(), 0);

        // Dropping releases the permit
        drop(p1);
        assert_eq!(bp.available_permits(), 1);

        // Now we can acquire again
        let _p4 = bp.try_acquire_permit().unwrap();

        // Clean up
        drop(p2);
        drop(p3);
    }

    #[tokio::test]
    async fn backpressure_async_permit() {
        let bp = Backpressure::new(2);

        let permit1 = bp.acquire_permit().await;
        let permit2 = bp.acquire_permit().await;
        assert_eq!(bp.available_permits(), 0);

        drop(permit1);
        assert_eq!(bp.available_permits(), 1);

        drop(permit2);
        assert_eq!(bp.available_permits(), 2);
    }

    #[test]
    fn rate_limiter_basic() {
        let limiter = RateLimiter::new(5, std::time::Duration::from_secs(1));

        for _ in 0..5 {
            assert!(limiter.try_acquire());
        }
        assert!(!limiter.try_acquire()); // Rate limited
    }

    #[test]
    fn token_bucket_basic() {
        let bucket = TokenBucket::new(10, 5.0);

        assert!(bucket.try_consume(5));
        assert!(bucket.try_consume(5));
        assert!(!bucket.try_consume(1)); // Empty
    }
}
