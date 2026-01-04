//! Metrics collection and reporting.
//!
//! This module provides metrics collection for monitoring session
//! performance and behavior. It includes:
//!
//! - Core metrics (counters, gauges, histograms)
//! - OpenTelemetry span integration (with `metrics` feature)
//! - Prometheus export (with `metrics` feature)
//!
//! # Basic Usage
//!
//! ```rust
//! use rust_expect::metrics::{Counter, Gauge, Timer, SessionMetrics};
//!
//! // Use basic metrics
//! let counter = Counter::new();
//! counter.inc();
//!
//! let gauge = Gauge::new();
//! gauge.set(42);
//!
//! // Time an operation
//! let timer = Timer::start();
//! // ... do work ...
//! let elapsed = timer.stop();
//! ```
//!
//! # OpenTelemetry Integration (with `metrics` feature)
//!
//! ```rust,ignore
//! use rust_expect::metrics::otel::{init_tracing, shutdown_tracing};
//!
//! // Initialize OpenTelemetry tracing
//! init_tracing("my-service", "http://localhost:4317")?;
//!
//! // Your application code with tracing spans...
//!
//! // Clean shutdown
//! shutdown_tracing();
//! ```

mod core;

#[cfg(feature = "metrics")]
pub mod otel;

#[cfg(feature = "metrics")]
pub mod prometheus_export;

pub use core::*;
