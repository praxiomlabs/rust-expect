//! Metrics collection and reporting.
//!
//! This module provides metrics collection for monitoring session
//! performance and behavior.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A counter metric.
#[derive(Debug, Default)]
pub struct Counter {
    value: AtomicU64,
}

impl Counter {
    /// Create a new counter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Increment by 1.
    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment by n.
    pub fn add(&self, n: u64) {
        self.value.fetch_add(n, Ordering::Relaxed);
    }

    /// Get current value.
    #[must_use]
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    /// Reset to zero.
    pub fn reset(&self) {
        self.value.store(0, Ordering::Relaxed);
    }
}

/// A gauge metric.
#[derive(Debug, Default)]
pub struct Gauge {
    value: AtomicU64,
}

impl Gauge {
    /// Create a new gauge.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the value.
    pub fn set(&self, value: u64) {
        self.value.store(value, Ordering::Relaxed);
    }

    /// Increment by 1.
    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement by 1.
    pub fn dec(&self) {
        self.value.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get current value.
    #[must_use]
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }
}

/// A histogram for measuring distributions.
#[derive(Debug)]
pub struct Histogram {
    /// Bucket boundaries.
    buckets: Vec<f64>,
    /// Counts per bucket.
    counts: Vec<AtomicU64>,
    /// Sum of all values.
    sum: AtomicU64,
    /// Total count.
    count: AtomicU64,
}

impl Histogram {
    /// Create with default buckets.
    #[must_use]
    pub fn new() -> Self {
        Self::with_buckets(vec![
            0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
        ])
    }

    /// Create with custom buckets.
    #[must_use]
    pub fn with_buckets(buckets: Vec<f64>) -> Self {
        let counts = (0..=buckets.len())
            .map(|_| AtomicU64::new(0))
            .collect();
        Self {
            buckets,
            counts,
            sum: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }
    }

    /// Observe a value.
    pub fn observe(&self, value: f64) {
        // Find bucket and increment
        let idx = self.buckets.iter().position(|&b| value <= b).unwrap_or(self.buckets.len());
        self.counts[idx].fetch_add(1, Ordering::Relaxed);

        // Update sum (as bits for f64 storage)
        let bits = value.to_bits();
        self.sum.fetch_add(bits, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get the count.
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    /// Get bucket counts.
    #[must_use]
    pub fn bucket_counts(&self) -> Vec<u64> {
        self.counts.iter().map(|c| c.load(Ordering::Relaxed)).collect()
    }
}

impl Default for Histogram {
    fn default() -> Self {
        Self::new()
    }
}

/// Timer for measuring durations.
#[derive(Debug)]
pub struct Timer {
    start: Instant,
}

impl Timer {
    /// Start a new timer.
    #[must_use]
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    /// Get elapsed time.
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// Stop and return duration in seconds.
    #[must_use]
    pub fn stop(self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    /// Stop and record to histogram.
    pub fn record_to(self, histogram: &Histogram) {
        histogram.observe(self.stop());
    }
}

/// Session metrics.
#[derive(Debug, Default)]
pub struct SessionMetrics {
    /// Bytes sent.
    pub bytes_sent: Counter,
    /// Bytes received.
    pub bytes_received: Counter,
    /// Commands executed.
    pub commands_executed: Counter,
    /// Pattern matches.
    pub pattern_matches: Counter,
    /// Timeouts.
    pub timeouts: Counter,
    /// Errors.
    pub errors: Counter,
    /// Active sessions.
    pub active_sessions: Gauge,
    /// Command duration histogram.
    pub command_duration: Histogram,
    /// Expect duration histogram.
    pub expect_duration: Histogram,
}

impl SessionMetrics {
    /// Create new session metrics.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Report a snapshot.
    #[must_use]
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            bytes_sent: self.bytes_sent.get(),
            bytes_received: self.bytes_received.get(),
            commands_executed: self.commands_executed.get(),
            pattern_matches: self.pattern_matches.get(),
            timeouts: self.timeouts.get(),
            errors: self.errors.get(),
            active_sessions: self.active_sessions.get(),
        }
    }
}

/// Snapshot of metrics.
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    /// Bytes sent.
    pub bytes_sent: u64,
    /// Bytes received.
    pub bytes_received: u64,
    /// Commands executed.
    pub commands_executed: u64,
    /// Pattern matches.
    pub pattern_matches: u64,
    /// Timeouts.
    pub timeouts: u64,
    /// Errors.
    pub errors: u64,
    /// Active sessions.
    pub active_sessions: u64,
}

/// Global metrics registry.
#[derive(Debug, Default)]
pub struct MetricsRegistry {
    counters: Arc<Mutex<HashMap<String, Arc<Counter>>>>,
    gauges: Arc<Mutex<HashMap<String, Arc<Gauge>>>>,
    histograms: Arc<Mutex<HashMap<String, Arc<Histogram>>>>,
}

impl MetricsRegistry {
    /// Create a new registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get or create a counter.
    #[must_use] pub fn counter(&self, name: &str) -> Arc<Counter> {
        let mut counters = self.counters.lock().unwrap_or_else(|e| e.into_inner());
        counters
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Counter::new()))
            .clone()
    }

    /// Get or create a gauge.
    #[must_use] pub fn gauge(&self, name: &str) -> Arc<Gauge> {
        let mut gauges = self.gauges.lock().unwrap_or_else(|e| e.into_inner());
        gauges
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Gauge::new()))
            .clone()
    }

    /// Get or create a histogram.
    #[must_use] pub fn histogram(&self, name: &str) -> Arc<Histogram> {
        let mut histograms = self.histograms.lock().unwrap_or_else(|e| e.into_inner());
        histograms
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Histogram::new()))
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_basic() {
        let counter = Counter::new();
        assert_eq!(counter.get(), 0);

        counter.inc();
        assert_eq!(counter.get(), 1);

        counter.add(5);
        assert_eq!(counter.get(), 6);
    }

    #[test]
    fn gauge_basic() {
        let gauge = Gauge::new();
        assert_eq!(gauge.get(), 0);

        gauge.set(10);
        assert_eq!(gauge.get(), 10);

        gauge.inc();
        assert_eq!(gauge.get(), 11);

        gauge.dec();
        assert_eq!(gauge.get(), 10);
    }

    #[test]
    fn histogram_basic() {
        let histogram = Histogram::new();
        histogram.observe(0.1);
        histogram.observe(0.5);
        histogram.observe(1.0);

        assert_eq!(histogram.count(), 3);
    }

    #[test]
    fn timer_basic() {
        let timer = Timer::start();
        std::thread::sleep(Duration::from_millis(10));
        let elapsed = timer.stop();

        assert!(elapsed >= 0.01);
    }

    #[test]
    fn session_metrics() {
        let metrics = SessionMetrics::new();
        metrics.bytes_sent.add(100);
        metrics.commands_executed.inc();

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.bytes_sent, 100);
        assert_eq!(snapshot.commands_executed, 1);
    }
}
