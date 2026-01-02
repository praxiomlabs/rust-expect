//! Multi-session management example.
//!
//! This example demonstrates running and managing multiple concurrent
//! terminal sessions using the multi module.
//!
//! Run with: `cargo run --example multi_session`

use rust_expect::multi::{GroupBuilder, GroupManager, Selector, SessionGroup};
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
    println!("   Sessions: web1={}, web2={}, db1={}", web1, web2, db1);

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

    // Example 4: Session selector
    println!("\n4. Using session selector...");

    let mut selector = Selector::new().with_timeout(Duration::from_secs(10));

    let _s1 = selector.register();
    let s2 = selector.register();
    let _s3 = selector.register();

    println!("   Registered {} sessions", selector.len());

    // Simulate data arriving on session 2
    selector.push_data(s2, b"Login: ".to_vec());

    // Poll for ready sessions
    if let Some(result) = selector.poll() {
        println!("   Session {} is ready: {:?}", result.session_id, result.ready_type);
        if let Some(data) = result.data {
            println!("   Data: '{}'", String::from_utf8_lossy(&data));
        }
    }

    // Example 5: Iterating over group sessions
    println!("\n5. Iterating over sessions...");

    let mut group = SessionGroup::new("test-group");
    group.add("session-a");
    group.add("session-b");
    group.add("session-c");

    group.for_each(|id, label| {
        println!("   Session {}: {}", id, label);
    });

    // Example 6: Real concurrent sessions
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

    println!("\nMulti-session examples completed successfully!");
    Ok(())
}
