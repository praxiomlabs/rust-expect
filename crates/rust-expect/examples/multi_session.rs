//! Multi-session management example.
//!
//! This example demonstrates running and managing multiple concurrent
//! terminal sessions using the multi module.
//!
//! Run with: `cargo run --example multi_session`

use std::time::Duration;

use rust_expect::multi::{MultiSessionManager, PatternSelector};
use rust_expect::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("rust-expect Multi-Session Example");
    println!("==================================\n");

    // Example 1: PatternSelector for per-session patterns
    println!("1. Using PatternSelector...");

    let selector = PatternSelector::new()
        .session(0, "login:")
        .session(0, "password:")
        .session(1, "prompt>")
        .default_pattern("$");

    println!(
        "   Patterns for session 0: {} patterns",
        selector.patterns_for(0).len()
    );
    println!(
        "   Patterns for session 1: {} patterns",
        selector.patterns_for(1).len()
    );
    println!(
        "   Patterns for unknown session: {} patterns",
        selector.patterns_for(99).len()
    );

    // Example 2: Real concurrent sessions with MultiSessionManager
    println!("\n2. Running concurrent shell sessions...");

    // Spawn multiple shell sessions
    let mut session1 = Session::spawn("/bin/sh", &[]).await?;
    let mut session2 = Session::spawn("/bin/sh", &[]).await?;

    // Wait for both prompts
    session1
        .expect_timeout(Pattern::shell_prompt(), Duration::from_secs(2))
        .await?;
    session2
        .expect_timeout(Pattern::shell_prompt(), Duration::from_secs(2))
        .await?;

    println!("   Session 1 PID: {:?}", session1.pid());
    println!("   Session 2 PID: {:?}", session2.pid());

    // Send commands to both
    session1.send_line("echo 'Hello from session 1'").await?;
    session2.send_line("echo 'Hello from session 2'").await?;

    // Collect responses
    let m1 = session1.expect("session 1").await?;
    let m2 = session2.expect("session 2").await?;

    println!("   Session 1 output: {}", m1.matched.trim());
    println!("   Session 2 output: {}", m2.matched.trim());

    // Clean up
    session1.send_line("exit").await?;
    session2.send_line("exit").await?;
    session1.wait().await?;
    session2.wait().await?;

    // Example 3: MultiSessionManager for expect_any/expect_all
    println!("\n3. Using MultiSessionManager for concurrent expect...");

    // Create new sessions for the manager demo
    let s1 = Session::spawn("/bin/sh", &[]).await?;
    let s2 = Session::spawn("/bin/sh", &[]).await?;

    let mut multi_manager: MultiSessionManager<_> = MultiSessionManager::new();
    let id1 = multi_manager.add(s1, "shell-1");
    let id2 = multi_manager.add(s2, "shell-2");

    println!("   Added {} sessions to manager", multi_manager.len());
    println!("   Session IDs: {id1}, {id2}");

    // Get labels
    let label1 = multi_manager.label(id1).await;
    let label2 = multi_manager.label(id2).await;
    println!("   Labels: {label1:?}, {label2:?}");

    // Wait for prompts on all sessions
    let results = multi_manager.expect_all(Pattern::shell_prompt()).await?;
    println!("   Got {} prompt responses", results.len());

    // Send to all sessions at once
    let send_results = multi_manager.send_all(b"echo 'hello'\n").await;
    println!("   Sent to {} sessions", send_results.len());

    // Wait for any to match
    let first_match = multi_manager.expect_any("hello").await?;
    println!("   First session to respond: {}", first_match.session_id);

    // Clean up by sending exit to all
    let _ = multi_manager.send_all(b"exit\n").await;

    println!("\nMulti-session examples completed successfully!");
    Ok(())
}
