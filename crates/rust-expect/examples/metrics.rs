//! Metrics collection example.
//!
//! This example demonstrates using the metrics module for
//! monitoring session performance and behavior.
//!
//! Run with: `cargo run --example metrics`

use rust_expect::metrics::{Counter, Gauge, Histogram, MetricsRegistry, SessionMetrics, Timer};
use rust_expect::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    println!("rust-expect Metrics Collection Example");
    println!("======================================\n");

    // Example 1: Counter metrics
    println!("1. Counter metrics...");

    let counter = Counter::new();
    println!("   Initial value: {}", counter.get());

    counter.inc();
    counter.inc();
    counter.add(10);
    println!("   After increments: {}", counter.get());

    counter.reset();
    println!("   After reset: {}", counter.get());

    // Example 2: Gauge metrics
    println!("\n2. Gauge metrics...");

    let gauge = Gauge::new();
    gauge.set(5);
    println!("   Set to: {}", gauge.get());

    gauge.inc();
    gauge.inc();
    println!("   After 2 increments: {}", gauge.get());

    gauge.dec();
    println!("   After decrement: {}", gauge.get());

    // Example 3: Histogram metrics
    println!("\n3. Histogram metrics...");

    let histogram = Histogram::new();

    // Observe some values
    histogram.observe(0.05);
    histogram.observe(0.15);
    histogram.observe(0.25);
    histogram.observe(0.5);
    histogram.observe(1.5);

    println!("   Observations: {}", histogram.count());
    println!("   Bucket counts: {:?}", histogram.bucket_counts());

    // Example 4: Timer for measuring durations
    println!("\n4. Timer for durations...");

    let timer = Timer::start();

    // Simulate some work
    tokio::time::sleep(Duration::from_millis(50)).await;

    let elapsed = timer.elapsed();
    println!("   Elapsed: {elapsed:?}");

    // Timer that records to histogram
    let histogram = Histogram::new();
    let timer = Timer::start();
    tokio::time::sleep(Duration::from_millis(10)).await;
    timer.record_to(&histogram);
    println!("   Recorded {} observations to histogram", histogram.count());

    // Example 5: Session metrics
    println!("\n5. Session metrics...");

    let metrics = SessionMetrics::new();

    // Simulate session activity
    metrics.bytes_sent.add(1024);
    metrics.bytes_received.add(4096);
    metrics.commands_executed.inc();
    metrics.commands_executed.inc();
    metrics.commands_executed.inc();
    metrics.pattern_matches.add(5);
    metrics.active_sessions.set(1);

    let snapshot = metrics.snapshot();
    println!("   Bytes sent: {}", snapshot.bytes_sent);
    println!("   Bytes received: {}", snapshot.bytes_received);
    println!("   Commands executed: {}", snapshot.commands_executed);
    println!("   Pattern matches: {}", snapshot.pattern_matches);
    println!("   Active sessions: {}", snapshot.active_sessions);

    // Example 6: Metrics registry
    println!("\n6. Metrics registry...");

    let registry = MetricsRegistry::new();

    // Get or create named metrics
    let sessions = registry.counter("sessions_total");
    let current_sessions = registry.gauge("sessions_active");
    let latency = registry.histogram("request_latency_seconds");

    sessions.inc();
    sessions.inc();
    current_sessions.set(2);
    latency.observe(0.1);

    println!("   sessions_total: {}", sessions.get());
    println!("   sessions_active: {}", current_sessions.get());
    println!("   request_latency observations: {}", latency.count());

    // Example 7: Real session with metrics
    println!("\n7. Session with metrics tracking...");

    let metrics = SessionMetrics::new();
    metrics.active_sessions.inc();

    let mut session = Session::spawn("/bin/sh", &[]).await?;
    session.expect_timeout(Pattern::regex(r"[$#>]").unwrap(), Duration::from_secs(2)).await?;

    // Track command execution
    let timer = Timer::start();
    session.send_line("echo 'Tracked command'").await?;
    metrics.bytes_sent.add(22); // approximate
    metrics.commands_executed.inc();

    session.expect("Tracked command").await?;
    metrics.pattern_matches.inc();
    timer.record_to(&metrics.command_duration);

    // Clean up
    session.send_line("exit").await?;
    session.wait().await?;
    metrics.active_sessions.dec();

    let final_snapshot = metrics.snapshot();
    println!("   Final metrics:");
    println!("   - Commands: {}", final_snapshot.commands_executed);
    println!("   - Pattern matches: {}", final_snapshot.pattern_matches);
    println!("   - Active sessions: {}", final_snapshot.active_sessions);

    println!("\nMetrics examples completed successfully!");
    Ok(())
}
