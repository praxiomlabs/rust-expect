//! Multi-session management example.
//!
//! This example demonstrates running and managing multiple concurrent
//! terminal sessions using the multi module.
//!
//! Run with: `cargo run --example multi_session`

use rust_expect::multi::{GroupBuilder, GroupManager, MultiSessionManager, PatternSelector, SessionGroup};
use rust_expect::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    println!("rust-expect Multi-Session Example");
    println!("==================================\n");

    // Example 1: Session groups
    println!("1. Managing session groups...");

    let mut group = SessionGroup::new("servers");
    let web1 = group.add("web-server-1");
    let web2 = group.add("web-server-2");
    let db1 = group.add("database-1");

    println!("   Created group '{}' with {} sessions", group.name(), group.len());
    println!("   Sessions: web1={web1}, web2={web2}, db1={db1}");

    // Mark a session as inactive
    group.set_active(db1, false);
    println!("   Active sessions: {}", group.active_count());

    // Example 2: Group builder pattern
    println!("\n2. Using GroupBuilder...");

    let group = GroupBuilder::new("production")
        .timeout(Duration::from_secs(60))
        .add("app-server-1")
        .add("app-server-2")
        .add("cache-server")
        .build();

    println!("   Built group: {}", group.name());
    println!("   Sessions: {}", group.len());

    // Example 3: Group manager for multiple groups
    println!("\n3. Managing multiple groups...");

    let mut manager = GroupManager::new();

    // Create web servers group
    let web_group = manager.create("web");
    web_group.add("nginx-1");
    web_group.add("nginx-2");

    // Create database group
    let db_group = manager.create("database");
    db_group.add("postgres-primary");
    db_group.add("postgres-replica");

    println!("   Groups: {:?}", manager.names());
    println!("   Total sessions: {}", manager.total_sessions());

    // Example 4: PatternSelector for per-session patterns
    println!("\n4. Using PatternSelector...");

    let selector = PatternSelector::new()
        .session(0, "login:")
        .session(0, "password:")
        .session(1, "prompt>")
        .default_pattern("$");

    println!("   Patterns for session 0: {} patterns", selector.patterns_for(0).len());
    println!("   Patterns for session 1: {} patterns", selector.patterns_for(1).len());
    println!("   Patterns for unknown session: {} patterns", selector.patterns_for(99).len());

    // Example 5: Iterating over group sessions
    println!("\n5. Iterating over sessions...");

    let mut group = SessionGroup::new("test-group");
    group.add("session-a");
    group.add("session-b");
    group.add("session-c");

    group.for_each(|id, label| {
        println!("   Session {id}: {label}");
    });

    // Example 6: Real concurrent sessions with MultiSessionManager
    println!("\n6. Running concurrent shell sessions...");

    // Spawn multiple shell sessions
    let mut session1 = Session::spawn("/bin/sh", &[]).await?;
    let mut session2 = Session::spawn("/bin/sh", &[]).await?;

    // Wait for both prompts
    session1.expect_timeout(Pattern::regex(r"[$#>]").unwrap(), Duration::from_secs(2)).await?;
    session2.expect_timeout(Pattern::regex(r"[$#>]").unwrap(), Duration::from_secs(2)).await?;

    println!("   Session 1 PID: {}", session1.pid());
    println!("   Session 2 PID: {}", session2.pid());

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

    // Example 7: MultiSessionManager for expect_any/expect_all
    println!("\n7. Using MultiSessionManager for concurrent expect...");

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
    let results = multi_manager.expect_all(Pattern::regex(r"[$#>]").unwrap()).await?;
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
