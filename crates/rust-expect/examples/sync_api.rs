//! Synchronous API usage example.
//!
//! This example demonstrates using rust-expect with the synchronous (blocking)
//! API, which is useful when you don't need async or when integrating with
//! synchronous codebases.
//!
//! Run with: `cargo run --example sync_api`

use std::time::Duration;

use rust_expect::prelude::*;
use rust_expect::sync::SyncSession;

fn main() -> Result<()> {
    println!("rust-expect Synchronous API Example");
    println!("====================================\n");

    // Example 1: Basic synchronous spawn and expect
    println!("1. Basic synchronous session...");

    let mut session = SyncSession::spawn("echo", &["Hello from sync API!"])?;

    // Blocking expect
    let m = session.expect("Hello")?;
    println!("   Matched: '{}'", m.matched.trim());
    println!("   PID: {}", session.pid());

    // Example 2: Interactive synchronous session
    println!("\n2. Interactive synchronous session...");

    let mut session = SyncSession::spawn("/bin/sh", &[])?;
    println!("   Shell spawned with PID: {}", session.pid());

    // Wait for prompt
    session.expect_timeout(Pattern::regex(r"[$#>]").unwrap(), Duration::from_secs(5))?;
    println!("   Prompt detected");

    // Send commands synchronously
    session.send_line("echo 'Sync test 1'")?;
    session.expect("Sync test 1")?;
    println!("   First command completed");

    session.send_line("echo 'Sync test 2'")?;
    session.expect("Sync test 2")?;
    println!("   Second command completed");

    // Get buffer contents
    let buffer = session.buffer();
    println!("   Current buffer length: {} bytes", buffer.len());

    // Clean up
    session.send_line("exit")?;
    println!("   Session exited");

    // Example 3: Error handling
    println!("\n3. Timeout handling...");

    let mut session = SyncSession::spawn("/bin/sh", &[])?;
    session.expect_timeout(Pattern::regex(r"[$#>]").unwrap(), Duration::from_secs(2))?;

    // Send a command that won't produce the expected output
    session.send_line("echo 'different output'")?;

    // This will timeout since we're looking for something that doesn't exist
    match session.expect_timeout("nonexistent pattern", Duration::from_millis(500)) {
        Ok(_) => println!("   Pattern found (unexpected)"),
        Err(ExpectError::Timeout { .. }) => println!("   Timeout occurred as expected"),
        Err(e) => println!("   Other error: {e}"),
    }

    // Clean up
    session.kill()?;

    // Example 4: Custom configuration
    println!("\n4. Custom session configuration...");

    let config = SessionConfig {
        dimensions: (120, 40),
        timeout: TimeoutConfig {
            default: Duration::from_secs(30),
            spawn: Duration::from_secs(60),
            close: Duration::from_secs(10),
        },
        ..Default::default()
    };

    let session = SyncSession::spawn_with_config("/bin/sh", &[], config)?;
    println!("   Session created with custom config");
    println!("   Dimensions: {:?}", session.config().dimensions);
    println!("   Default timeout: {:?}", session.config().timeout.default);
    println!("   Close timeout: {:?}", session.config().timeout.close);

    // Clean up
    drop(session);

    println!("\nSynchronous API examples completed successfully!");
    Ok(())
}
