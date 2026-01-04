//! OpenTelemetry tracing integration.
//!
//! This module provides integration with OpenTelemetry for distributed
//! tracing of terminal automation sessions.
//!
//! # Setup
//!
//! ```rust,ignore
//! use rust_expect::metrics::otel::{init_tracing, shutdown_tracing, TracingConfig};
//!
//! // Basic setup with defaults
//! init_tracing("my-service", "http://localhost:4317")?;
//!
//! // Or with custom configuration
//! let config = TracingConfig::new("my-service")
//!     .with_endpoint("http://localhost:4317")
//!     .with_sampling_ratio(0.5)
//!     .with_resource_attribute("environment", "production");
//!
//! init_tracing_with_config(config)?;
//!
//! // Run your application...
//!
//! // Clean shutdown
//! shutdown_tracing();
//! ```
//!
//! # Span Helpers
//!
//! ```rust,ignore
//! use rust_expect::metrics::otel::{session_span, expect_span, send_span};
//! use tracing::info_span;
//!
//! // Create a session span
//! let _guard = session_span("my-session", "bash", 12345);
//!
//! // Create an expect span
//! let _guard = expect_span("waiting for prompt", r"\$");
//!
//! // Create a send span
//! let _guard = send_span("ls -la");
//! ```

use opentelemetry::KeyValue;
use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    Resource,
    trace::{RandomIdGenerator, Sampler, SdkTracerProvider},
};
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Global tracer provider for clean shutdown.
static TRACER_PROVIDER: OnceLock<SdkTracerProvider> = OnceLock::new();

/// Configuration for OpenTelemetry tracing.
#[derive(Debug, Clone)]
pub struct TracingConfig {
    /// Service name for traces.
    pub service_name: String,
    /// OTLP endpoint URL.
    pub endpoint: String,
    /// Sampling ratio (0.0 to 1.0).
    pub sampling_ratio: f64,
    /// Additional resource attributes.
    pub resource_attributes: HashMap<String, String>,
    /// Export timeout.
    pub export_timeout: Duration,
    /// Whether to also log to console.
    pub console_output: bool,
}

impl TracingConfig {
    /// Create a new tracing configuration.
    #[must_use]
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
            endpoint: "http://localhost:4317".to_string(),
            sampling_ratio: 1.0,
            resource_attributes: HashMap::new(),
            export_timeout: Duration::from_secs(30),
            console_output: false,
        }
    }

    /// Set the OTLP endpoint.
    #[must_use]
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    /// Set the sampling ratio.
    #[must_use]
    pub const fn with_sampling_ratio(mut self, ratio: f64) -> Self {
        self.sampling_ratio = ratio.clamp(0.0, 1.0);
        self
    }

    /// Add a resource attribute.
    #[must_use]
    pub fn with_resource_attribute(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.resource_attributes.insert(key.into(), value.into());
        self
    }

    /// Set export timeout.
    #[must_use]
    pub const fn with_export_timeout(mut self, timeout: Duration) -> Self {
        self.export_timeout = timeout;
        self
    }

    /// Enable console output alongside OTLP.
    #[must_use]
    pub const fn with_console_output(mut self, enabled: bool) -> Self {
        self.console_output = enabled;
        self
    }
}

/// Error type for tracing initialization.
#[derive(Debug, thiserror::Error)]
pub enum TracingError {
    /// OpenTelemetry trace error.
    #[error("OpenTelemetry trace error: {0}")]
    Trace(#[from] opentelemetry::trace::TraceError),

    /// Already initialized.
    #[error("Tracing already initialized")]
    AlreadyInitialized,

    /// Subscriber initialization failed.
    #[error("Failed to initialize tracing subscriber")]
    SubscriberInit,
}

/// Initialize OpenTelemetry tracing with default configuration.
///
/// # Errors
///
/// Returns an error if initialization fails or tracing is already initialized.
pub fn init_tracing(service_name: &str, endpoint: &str) -> Result<(), TracingError> {
    let config = TracingConfig::new(service_name).with_endpoint(endpoint);
    init_tracing_with_config(config)
}

/// Initialize OpenTelemetry tracing with custom configuration.
///
/// # Errors
///
/// Returns an error if initialization fails or tracing is already initialized.
pub fn init_tracing_with_config(config: TracingConfig) -> Result<(), TracingError> {
    // Build resource attributes
    let mut attributes = vec![KeyValue::new("service.name", config.service_name.clone())];
    for (key, value) in &config.resource_attributes {
        attributes.push(KeyValue::new(key.clone(), value.clone()));
    }

    let resource = Resource::builder().with_attributes(attributes).build();

    // Configure OTLP exporter
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(&config.endpoint)
        .with_timeout(config.export_timeout)
        .build()?;

    // Build tracer provider
    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_sampler(Sampler::TraceIdRatioBased(config.sampling_ratio))
        .with_id_generator(RandomIdGenerator::default())
        .with_resource(resource)
        .build();

    // Store provider for shutdown
    if TRACER_PROVIDER.set(provider.clone()).is_err() {
        return Err(TracingError::AlreadyInitialized);
    }

    // Create tracer
    let tracer = provider.tracer("rust-expect");

    // Create OpenTelemetry layer
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    // Build subscriber
    if config.console_output {
        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_target(true)
            .with_level(true);

        tracing_subscriber::registry()
            .with(otel_layer)
            .with(fmt_layer)
            .try_init()
            .map_err(|_| TracingError::SubscriberInit)?;
    } else {
        tracing_subscriber::registry()
            .with(otel_layer)
            .try_init()
            .map_err(|_| TracingError::SubscriberInit)?;
    }

    Ok(())
}

/// Shutdown OpenTelemetry tracing.
///
/// This should be called before application exit to ensure all spans
/// are exported.
pub fn shutdown_tracing() {
    if let Some(provider) = TRACER_PROVIDER.get() {
        // Force flush all pending spans - ignore errors during shutdown
        let _ = provider.force_flush();
    }
}

/// Create a span for a session operation.
///
/// Returns a span guard that will end the span when dropped.
///
/// # Example
///
/// ```rust,ignore
/// let _span = session_span("login-session", "bash", 12345);
/// // Session operations...
/// // Span ends when _span is dropped
/// ```
#[must_use]
pub fn session_span(session_id: &str, command: &str, pid: u32) -> tracing::span::EnteredSpan {
    tracing::info_span!(
        "session",
        session.id = session_id,
        session.command = command,
        session.pid = pid,
        otel.kind = "client"
    )
    .entered()
}

/// Create a span for an expect operation.
///
/// Returns a span guard that will end the span when dropped.
///
/// # Example
///
/// ```rust,ignore
/// let _span = expect_span("waiting for login", "Password:");
/// // Expect operation...
/// // Span ends when _span is dropped
/// ```
#[must_use]
pub fn expect_span(description: &str, pattern: &str) -> tracing::span::EnteredSpan {
    tracing::info_span!(
        "expect",
        expect.description = description,
        expect.pattern = pattern,
        otel.kind = "internal"
    )
    .entered()
}

/// Create a span for a send operation.
///
/// Returns a span guard that will end the span when dropped.
///
/// # Example
///
/// ```rust,ignore
/// let _span = send_span("ls -la");
/// // Send operation...
/// // Span ends when _span is dropped
/// ```
#[must_use]
pub fn send_span(data: &str) -> tracing::span::EnteredSpan {
    // Truncate long data for span name
    let display_data = if data.len() > 50 {
        format!("{}...", &data[..47])
    } else {
        data.to_string()
    };

    tracing::info_span!(
        "send",
        send.data = display_data.as_str(),
        send.bytes = data.len(),
        otel.kind = "internal"
    )
    .entered()
}

/// Create a span for a dialog execution.
///
/// Returns a span guard that will end the span when dropped.
#[must_use]
pub fn dialog_span(dialog_name: &str, step_count: usize) -> tracing::span::EnteredSpan {
    tracing::info_span!(
        "dialog",
        dialog.name = dialog_name,
        dialog.steps = step_count,
        otel.kind = "internal"
    )
    .entered()
}

/// Create a span for a transcript recording.
///
/// Returns a span guard that will end the span when dropped.
#[must_use]
pub fn transcript_span(session_id: &str, format: &str) -> tracing::span::EnteredSpan {
    tracing::info_span!(
        "transcript",
        transcript.session = session_id,
        transcript.format = format,
        otel.kind = "internal"
    )
    .entered()
}

/// Record an error on the current span.
pub fn record_error(error: &dyn std::error::Error) {
    tracing::error!(
        exception.type_ = std::any::type_name_of_val(error),
        exception.message = %error,
    );
}

/// Record a successful match on the current span.
pub fn record_match(pattern: &str, matched_text: &str, duration_ms: u64) {
    tracing::info!(
        match.pattern = pattern,
        match.text = matched_text,
        match.duration_ms = duration_ms,
    );
}

/// Record bytes transferred on the current span.
pub fn record_bytes(sent: u64, received: u64) {
    tracing::info!(bytes.sent = sent, bytes.received = received,);
}

/// Span attribute constants for consistency.
pub mod attributes {
    /// Session ID attribute key.
    pub const SESSION_ID: &str = "session.id";
    /// Session command attribute key.
    pub const SESSION_COMMAND: &str = "session.command";
    /// Session PID attribute key.
    pub const SESSION_PID: &str = "session.pid";
    /// Expect pattern attribute key.
    pub const EXPECT_PATTERN: &str = "expect.pattern";
    /// Expect timeout attribute key.
    pub const EXPECT_TIMEOUT_MS: &str = "expect.timeout_ms";
    /// Send data attribute key.
    pub const SEND_DATA: &str = "send.data";
    /// Send bytes attribute key.
    pub const SEND_BYTES: &str = "send.bytes";
    /// Match text attribute key.
    pub const MATCH_TEXT: &str = "match.text";
    /// Match duration attribute key.
    pub const MATCH_DURATION_MS: &str = "match.duration_ms";
    /// Error type attribute key.
    pub const ERROR_TYPE: &str = "error.type";
    /// Error message attribute key.
    pub const ERROR_MESSAGE: &str = "error.message";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracing_config_default() {
        let config = TracingConfig::new("test-service");
        assert_eq!(config.service_name, "test-service");
        assert_eq!(config.endpoint, "http://localhost:4317");
        assert!((config.sampling_ratio - 1.0).abs() < 0.001);
    }

    #[test]
    fn tracing_config_builder() {
        let config = TracingConfig::new("test")
            .with_endpoint("http://custom:4317")
            .with_sampling_ratio(0.5)
            .with_resource_attribute("env", "test")
            .with_console_output(true);

        assert_eq!(config.endpoint, "http://custom:4317");
        assert!((config.sampling_ratio - 0.5).abs() < 0.001);
        assert_eq!(
            config.resource_attributes.get("env"),
            Some(&"test".to_string())
        );
        assert!(config.console_output);
    }

    #[test]
    fn sampling_ratio_clamped() {
        let config = TracingConfig::new("test").with_sampling_ratio(2.0);
        assert!((config.sampling_ratio - 1.0).abs() < 0.001);

        let config = TracingConfig::new("test").with_sampling_ratio(-0.5);
        assert!(config.sampling_ratio.abs() < 0.001);
    }

    // Note: Full integration tests require a running OTLP collector
    // and are typically run manually or in CI with proper setup.
}
