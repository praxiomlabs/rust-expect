//! Prometheus metrics export.
//!
//! This module provides Prometheus-compatible metrics export for
//! terminal automation sessions.
//!
//! # Usage
//!
//! ```rust,ignore
//! use rust_expect::metrics::prometheus_export::{ExpectMetrics, gather_metrics};
//!
//! // Get global metrics instance
//! let metrics = ExpectMetrics::global();
//!
//! // Record operations
//! metrics.session_started();
//! metrics.bytes_sent(1024);
//! metrics.expect_succeeded("prompt", 0.5);
//!
//! // Export metrics in Prometheus format
//! let output = gather_metrics();
//! println!("{}", output);
//! ```
//!
//! # HTTP Server Integration
//!
//! ```rust,ignore
//! use rust_expect::metrics::prometheus_export::gather_metrics;
//!
//! // In your HTTP handler for /metrics endpoint:
//! fn metrics_handler() -> String {
//!     gather_metrics()
//! }
//! ```

use prometheus::{
    Counter, CounterVec, Encoder, Gauge, Histogram, HistogramOpts, HistogramVec, Opts, Registry,
    TextEncoder,
};
use std::sync::OnceLock;

/// Global metrics registry.
static REGISTRY: OnceLock<Registry> = OnceLock::new();

/// Global expect metrics instance.
static METRICS: OnceLock<ExpectMetrics> = OnceLock::new();

/// Get or create the global registry.
fn registry() -> &'static Registry {
    REGISTRY.get_or_init(Registry::new)
}

/// Prometheus metrics for rust-expect.
#[derive(Debug, Clone)]
pub struct ExpectMetrics {
    // Session metrics
    sessions_active: Gauge,
    sessions_total: Counter,
    session_duration: Histogram,

    // I/O metrics
    bytes_sent_total: Counter,
    bytes_received_total: Counter,

    // Expect metrics
    expect_total: CounterVec,
    expect_duration: HistogramVec,
    expect_timeouts: Counter,

    // Command metrics
    commands_total: Counter,
    command_duration: Histogram,

    // Error metrics
    errors_total: CounterVec,

    // Dialog metrics
    dialogs_total: Counter,
    dialog_duration: Histogram,
    dialog_steps: Histogram,
}

impl ExpectMetrics {
    /// Get the global metrics instance, initializing if needed.
    ///
    /// # Panics
    ///
    /// Panics if metrics registration fails (should only happen on first call).
    pub fn global() -> &'static Self {
        METRICS.get_or_init(|| Self::new(registry()).expect("Failed to register metrics"))
    }

    /// Create new metrics registered with the given registry.
    ///
    /// # Errors
    ///
    /// Returns an error if metric registration fails.
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        // Session metrics
        let sessions_active = Gauge::with_opts(Opts::new(
            "expect_sessions_active",
            "Number of currently active expect sessions",
        ))?;
        registry.register(Box::new(sessions_active.clone()))?;

        let sessions_total = Counter::with_opts(Opts::new(
            "expect_sessions_total",
            "Total number of expect sessions started",
        ))?;
        registry.register(Box::new(sessions_total.clone()))?;

        let session_duration = Histogram::with_opts(
            HistogramOpts::new(
                "expect_session_duration_seconds",
                "Duration of expect sessions in seconds",
            )
            .buckets(vec![0.1, 0.5, 1.0, 5.0, 10.0, 30.0, 60.0, 300.0, 600.0]),
        )?;
        registry.register(Box::new(session_duration.clone()))?;

        // I/O metrics
        let bytes_sent_total = Counter::with_opts(Opts::new(
            "expect_bytes_sent_total",
            "Total bytes sent to sessions",
        ))?;
        registry.register(Box::new(bytes_sent_total.clone()))?;

        let bytes_received_total = Counter::with_opts(Opts::new(
            "expect_bytes_received_total",
            "Total bytes received from sessions",
        ))?;
        registry.register(Box::new(bytes_received_total.clone()))?;

        // Expect metrics
        let expect_total = CounterVec::new(
            Opts::new("expect_operations_total", "Total expect operations"),
            &["status"],
        )?;
        registry.register(Box::new(expect_total.clone()))?;

        let expect_duration = HistogramVec::new(
            HistogramOpts::new(
                "expect_operation_duration_seconds",
                "Duration of expect operations in seconds",
            )
            .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0]),
            &["pattern_type"],
        )?;
        registry.register(Box::new(expect_duration.clone()))?;

        let expect_timeouts = Counter::with_opts(Opts::new(
            "expect_timeouts_total",
            "Total number of expect timeout errors",
        ))?;
        registry.register(Box::new(expect_timeouts.clone()))?;

        // Command metrics
        let commands_total = Counter::with_opts(Opts::new(
            "expect_commands_total",
            "Total commands sent to sessions",
        ))?;
        registry.register(Box::new(commands_total.clone()))?;

        let command_duration = Histogram::with_opts(
            HistogramOpts::new(
                "expect_command_duration_seconds",
                "Duration of command execution in seconds",
            )
            .buckets(vec![0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0, 30.0]),
        )?;
        registry.register(Box::new(command_duration.clone()))?;

        // Error metrics
        let errors_total = CounterVec::new(
            Opts::new("expect_errors_total", "Total errors by type"),
            &["error_type"],
        )?;
        registry.register(Box::new(errors_total.clone()))?;

        // Dialog metrics
        let dialogs_total =
            Counter::with_opts(Opts::new("expect_dialogs_total", "Total dialog executions"))?;
        registry.register(Box::new(dialogs_total.clone()))?;

        let dialog_duration = Histogram::with_opts(
            HistogramOpts::new(
                "expect_dialog_duration_seconds",
                "Duration of dialog execution in seconds",
            )
            .buckets(vec![0.1, 0.5, 1.0, 5.0, 10.0, 30.0, 60.0]),
        )?;
        registry.register(Box::new(dialog_duration.clone()))?;

        let dialog_steps = Histogram::with_opts(
            HistogramOpts::new("expect_dialog_steps", "Number of steps in executed dialogs")
                .buckets(vec![1.0, 2.0, 5.0, 10.0, 20.0, 50.0]),
        )?;
        registry.register(Box::new(dialog_steps.clone()))?;

        Ok(Self {
            sessions_active,
            sessions_total,
            session_duration,
            bytes_sent_total,
            bytes_received_total,
            expect_total,
            expect_duration,
            expect_timeouts,
            commands_total,
            command_duration,
            errors_total,
            dialogs_total,
            dialog_duration,
            dialog_steps,
        })
    }

    // Session methods

    /// Record a new session starting.
    pub fn session_started(&self) {
        self.sessions_total.inc();
        self.sessions_active.inc();
    }

    /// Record a session ending.
    pub fn session_ended(&self, duration_seconds: f64) {
        self.sessions_active.dec();
        self.session_duration.observe(duration_seconds);
    }

    // I/O methods

    /// Record bytes sent.
    pub fn bytes_sent(&self, count: u64) {
        self.bytes_sent_total.inc_by(count as f64);
    }

    /// Record bytes received.
    pub fn bytes_received(&self, count: u64) {
        self.bytes_received_total.inc_by(count as f64);
    }

    // Expect methods

    /// Record a successful expect operation.
    pub fn expect_succeeded(&self, pattern_type: &str, duration_seconds: f64) {
        self.expect_total.with_label_values(&["success"]).inc();
        self.expect_duration
            .with_label_values(&[pattern_type])
            .observe(duration_seconds);
    }

    /// Record a failed expect operation.
    pub fn expect_failed(&self, pattern_type: &str, duration_seconds: f64) {
        self.expect_total.with_label_values(&["failure"]).inc();
        self.expect_duration
            .with_label_values(&[pattern_type])
            .observe(duration_seconds);
    }

    /// Record an expect timeout.
    pub fn expect_timeout(&self) {
        self.expect_timeouts.inc();
        self.expect_total.with_label_values(&["timeout"]).inc();
    }

    // Command methods

    /// Record a command sent.
    pub fn command_sent(&self) {
        self.commands_total.inc();
    }

    /// Record command completion with duration.
    pub fn command_completed(&self, duration_seconds: f64) {
        self.command_duration.observe(duration_seconds);
    }

    // Error methods

    /// Record an error by type.
    pub fn error(&self, error_type: &str) {
        self.errors_total.with_label_values(&[error_type]).inc();
    }

    /// Record an I/O error.
    pub fn io_error(&self) {
        self.error("io");
    }

    /// Record a timeout error.
    pub fn timeout_error(&self) {
        self.error("timeout");
    }

    /// Record a pattern error.
    pub fn pattern_error(&self) {
        self.error("pattern");
    }

    /// Record an EOF error.
    pub fn eof_error(&self) {
        self.error("eof");
    }

    // Dialog methods

    /// Record a dialog started.
    pub fn dialog_started(&self, step_count: usize) {
        self.dialogs_total.inc();
        self.dialog_steps.observe(step_count as f64);
    }

    /// Record a dialog completed.
    pub fn dialog_completed(&self, duration_seconds: f64) {
        self.dialog_duration.observe(duration_seconds);
    }

    // Getters for current values

    /// Get current active session count.
    #[must_use]
    pub fn active_sessions(&self) -> u64 {
        self.sessions_active.get() as u64
    }

    /// Get total sessions started.
    #[must_use]
    pub fn total_sessions(&self) -> u64 {
        self.sessions_total.get() as u64
    }

    /// Get total bytes sent.
    #[must_use]
    pub fn total_bytes_sent(&self) -> u64 {
        self.bytes_sent_total.get() as u64
    }

    /// Get total bytes received.
    #[must_use]
    pub fn total_bytes_received(&self) -> u64 {
        self.bytes_received_total.get() as u64
    }

    /// Get total timeout count.
    #[must_use]
    pub fn total_timeouts(&self) -> u64 {
        self.expect_timeouts.get() as u64
    }
}

/// Gather all metrics in Prometheus text format.
///
/// # Returns
///
/// A string containing all metrics in Prometheus exposition format.
#[must_use]
pub fn gather_metrics() -> String {
    let encoder = TextEncoder::new();
    let metric_families = registry().gather();
    let mut buffer = Vec::new();
    encoder
        .encode(&metric_families, &mut buffer)
        .unwrap_or_default();
    String::from_utf8(buffer).unwrap_or_default()
}

/// Get the global Prometheus registry.
///
/// Useful for integrating with existing Prometheus setups.
#[must_use]
pub fn global_registry() -> &'static Registry {
    registry()
}

/// Create a new isolated registry for testing.
#[must_use]
pub fn new_registry() -> Registry {
    Registry::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_registration() {
        let registry = new_registry();
        let metrics = ExpectMetrics::new(&registry).expect("Failed to create metrics");

        // Basic operations shouldn't panic
        metrics.session_started();
        metrics.bytes_sent(100);
        metrics.expect_succeeded("string", 0.1);
        metrics.error("test");
    }

    #[test]
    fn session_tracking() {
        let registry = new_registry();
        let metrics = ExpectMetrics::new(&registry).unwrap();

        assert_eq!(metrics.active_sessions(), 0);
        assert_eq!(metrics.total_sessions(), 0);

        metrics.session_started();
        assert_eq!(metrics.active_sessions(), 1);
        assert_eq!(metrics.total_sessions(), 1);

        metrics.session_started();
        assert_eq!(metrics.active_sessions(), 2);
        assert_eq!(metrics.total_sessions(), 2);

        metrics.session_ended(1.0);
        assert_eq!(metrics.active_sessions(), 1);
        assert_eq!(metrics.total_sessions(), 2);
    }

    #[test]
    fn byte_counters() {
        let registry = new_registry();
        let metrics = ExpectMetrics::new(&registry).unwrap();

        metrics.bytes_sent(100);
        metrics.bytes_sent(50);
        assert_eq!(metrics.total_bytes_sent(), 150);

        metrics.bytes_received(200);
        assert_eq!(metrics.total_bytes_received(), 200);
    }

    #[test]
    fn gather_output() {
        // Use a fresh registry for isolated test
        let registry = new_registry();
        let metrics = ExpectMetrics::new(&registry).unwrap();

        metrics.session_started();
        metrics.bytes_sent(1000);

        let encoder = TextEncoder::new();
        let metric_families = registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        let output = String::from_utf8(buffer).unwrap();

        // Check that metrics are present
        assert!(output.contains("expect_sessions_total"));
        assert!(output.contains("expect_bytes_sent_total"));
    }

    #[test]
    fn error_types() {
        let registry = new_registry();
        let metrics = ExpectMetrics::new(&registry).unwrap();

        metrics.io_error();
        metrics.timeout_error();
        metrics.pattern_error();
        metrics.eof_error();
        metrics.error("custom");

        // Verify no panics and metrics are recorded
    }

    #[test]
    fn dialog_metrics() {
        let registry = new_registry();
        let metrics = ExpectMetrics::new(&registry).unwrap();

        metrics.dialog_started(5);
        metrics.dialog_completed(2.5);

        // Verify no panics
    }
}
