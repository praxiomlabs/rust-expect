//! Basic rust-expect usage example.
//!
//! This example demonstrates the fundamental spawn/expect workflow for
//! interacting with terminal applications.
//!
//! Run with: `cargo run --example basic`

use std::time::Duration;

use rust_expect::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("rust-expect Basic Example");
    println!("==========================\n");

    // Example 1: Simple command output capture
    println!("1. Running 'echo' command...");
    let mut session = Session::spawn("echo", &["Hello, rust-expect!"]).await?;

    // Wait for the output
    let m = session.expect("Hello").await?;
    println!("   Matched: '{}'", m.matched.trim());
    println!("   Buffer after match: '{}'", m.after.trim());

    // Example 2: Interactive shell session
    println!("\n2. Interactive shell session...");

    // Spawn a shell
    let mut session = Session::spawn("/bin/sh", &[]).await?;

    // Wait for shell prompt ($ or similar)
    // Use the convenience method for common shell prompts
    session
        .expect_timeout(Pattern::shell_prompt(), Duration::from_secs(5))
        .await?;
    println!("   Shell started, prompt detected");

    // Send a command
    session.send_line("echo 'Interactive test'").await?;
    println!("   Sent: echo 'Interactive test'");

    // Expect the output
    let m = session.expect("Interactive test").await?;
    println!("   Received: '{}'", m.matched.trim());

    // Clean exit
    session.send_line("exit").await?;
    println!("   Sent: exit");

    // Wait for EOF
    session.wait().await?;
    println!("   Session ended cleanly");

    // Example 3: Using pattern sets
    println!("\n3. Pattern matching with multiple patterns...");

    let mut session = Session::spawn("/bin/sh", &["-c", "echo 'success'"]).await?;

    // Create a pattern set that matches either success or failure
    let mut patterns = PatternSet::new();
    patterns.add(Pattern::literal("success"));
    patterns.add(Pattern::literal("failure"));
    patterns.add(Pattern::timeout(Duration::from_secs(3)));

    let m = session.expect_any(&patterns).await?;
    if m.matched.contains("success") {
        println!("   Command succeeded!");
    } else if m.matched.contains("failure") {
        println!("   Command failed!");
    }

    // Example 4: Terminal dimensions
    println!("\n4. Terminal resize...");

    let mut session = Session::spawn("/bin/sh", &[]).await?;

    // Get initial dimensions from config
    let (cols, rows) = session.config().dimensions;
    println!("   Initial dimensions: {cols}x{rows}");

    // Resize the terminal
    session.resize_pty(120, 40).await?;
    println!("   Resized to: 120x40");

    // Clean up
    session.send_line("exit").await?;
    session.wait().await?;

    println!("\nBasic examples completed successfully!");
    Ok(())
}
