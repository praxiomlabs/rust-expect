//! Integration tests for multi-session management.

use rust_expect::multi::{MultiSessionManager, PatternSelector, ReadyType};
use rust_expect::{GroupBuilder, GroupManager, GroupResult, SessionGroup};

#[test]
fn session_group_new() {
    let group = SessionGroup::new("test-group");
    assert_eq!(group.name(), "test-group");
    assert!(group.is_empty());
    assert_eq!(group.len(), 0);
}

#[test]
fn session_group_add_sessions() {
    let mut group = SessionGroup::new("servers");
    let id1 = group.add("server1");
    let id2 = group.add("server2");

    assert_ne!(id1, id2);
    assert_eq!(group.len(), 2);
    assert!(!group.is_empty());
}

#[test]
fn session_group_labels() {
    let mut group = SessionGroup::new("hosts");
    let id = group.add("web-server");

    assert_eq!(group.label(id), Some("web-server"));
}

#[test]
fn session_group_remove() {
    let mut group = SessionGroup::new("test");
    let id = group.add("session1");

    assert_eq!(group.len(), 1);
    group.remove(id);
    assert!(group.is_empty());
}

#[test]
fn session_group_active_state() {
    let mut group = SessionGroup::new("test");
    let id = group.add("session");

    assert_eq!(group.active_count(), 1);

    group.set_active(id, false);
    assert_eq!(group.active_count(), 0);

    group.set_active(id, true);
    assert_eq!(group.active_count(), 1);
}

#[test]
fn session_group_output() {
    let mut group = SessionGroup::new("test");
    let id = group.add("session");

    group.append_output(id, "line 1\n");
    group.append_output(id, "line 2\n");

    assert_eq!(group.output(id), Some("line 1\nline 2\n"));
}

#[test]
fn session_group_clear_output() {
    let mut group = SessionGroup::new("test");
    let id = group.add("session");
    group.append_output(id, "data");

    group.clear_output();
    assert_eq!(group.output(id), Some(""));
}

#[test]
fn session_group_ids() {
    let mut group = SessionGroup::new("test");
    let id1 = group.add("a");
    let id2 = group.add("b");

    let ids = group.session_ids();
    assert!(ids.contains(&id1));
    assert!(ids.contains(&id2));
}

#[test]
fn session_group_for_each() {
    let mut group = SessionGroup::new("test");
    group.add("server1");
    group.add("server2");

    let mut count = 0;
    group.for_each(|_id, _label| {
        count += 1;
    });

    assert_eq!(count, 2);
}

#[test]
fn group_builder_basic() {
    let group = GroupBuilder::new("cluster")
        .add("node1")
        .add("node2")
        .add("node3")
        .build();

    assert_eq!(group.name(), "cluster");
    assert_eq!(group.len(), 3);
}

#[test]
fn group_builder_with_timeout() {
    use std::time::Duration;

    let group = GroupBuilder::new("test")
        .timeout(Duration::from_secs(60))
        .add("session1")
        .build();

    assert_eq!(group.len(), 1);
}

#[test]
fn group_manager_new() {
    let manager = GroupManager::new();
    assert!(manager.names().is_empty());
}

#[test]
fn group_manager_create() {
    let mut manager = GroupManager::new();
    manager.create("web-servers");

    assert!(manager.get("web-servers").is_some());
}

#[test]
fn group_manager_multiple_groups() {
    let mut manager = GroupManager::new();
    manager.create("group1");
    manager.create("group2");
    manager.create("group3");

    assert_eq!(manager.names().len(), 3);
}

#[test]
fn group_manager_get_mut() {
    let mut manager = GroupManager::new();
    manager.create("test");

    if let Some(group) = manager.get_mut("test") {
        group.add("session1");
    }

    assert_eq!(manager.get("test").unwrap().len(), 1);
}

#[test]
fn group_manager_remove() {
    let mut manager = GroupManager::new();
    manager.create("ephemeral");

    let removed = manager.remove("ephemeral");
    assert!(removed.is_some());
    assert!(manager.get("ephemeral").is_none());
}

#[test]
fn group_manager_total_sessions() {
    let mut manager = GroupManager::new();
    manager.create("group1").add("s1");
    manager.create("group2").add("s2");
    manager.get_mut("group2").unwrap().add("s3");

    assert_eq!(manager.total_sessions(), 3);
}

#[test]
fn group_result_struct() {
    let result = GroupResult {
        session_id: 42,
        success: true,
        output: "Operation completed".to_string(),
    };

    assert_eq!(result.session_id, 42);
    assert!(result.success);
}

// Test MultiSessionManager basic operations
#[test]
fn multi_session_manager_new() {
    use tokio::io::DuplexStream;
    let manager: MultiSessionManager<DuplexStream> = MultiSessionManager::new();
    assert!(manager.is_empty());
    assert_eq!(manager.len(), 0);
}

#[test]
fn pattern_selector_new() {
    let selector = PatternSelector::new();
    // Should have empty default patterns
    assert!(selector.patterns_for(0).is_empty());
}

#[test]
fn pattern_selector_with_patterns() {
    let selector = PatternSelector::new()
        .session(0, "login:")
        .session(0, "password:")
        .session(1, "prompt>")
        .default_pattern("$");

    assert_eq!(selector.patterns_for(0).len(), 2);
    assert_eq!(selector.patterns_for(1).len(), 1);
    // Fallback to default for unknown session
    assert_eq!(selector.patterns_for(99).len(), 1);
}

#[test]
const fn ready_type_values() {
    // Test that ReadyType enum variants exist
    let _ = ReadyType::Matched;
    let _ = ReadyType::Readable;
    let _ = ReadyType::Writable;
    let _ = ReadyType::Closed;
    let _ = ReadyType::Error;
}

#[tokio::test]
async fn multi_session_manager_add_sessions() {
    use rust_expect::config::SessionConfig;
    use tokio::io::DuplexStream;

    let mut manager: MultiSessionManager<DuplexStream> = MultiSessionManager::new();
    let (client1, _server1) = tokio::io::duplex(1024);
    let (client2, _server2) = tokio::io::duplex(1024);

    let session1 = rust_expect::session::Session::new(client1, SessionConfig::default());
    let session2 = rust_expect::session::Session::new(client2, SessionConfig::default());

    let id1 = manager.add(session1, "session1");
    let id2 = manager.add(session2, "session2");

    assert_ne!(id1, id2);
    assert_eq!(manager.len(), 2);
    assert!(!manager.is_empty());
}

#[tokio::test]
async fn multi_session_manager_labels() {
    use rust_expect::config::SessionConfig;
    use tokio::io::DuplexStream;

    let mut manager: MultiSessionManager<DuplexStream> = MultiSessionManager::new();
    let (client, _server) = tokio::io::duplex(1024);
    let session = rust_expect::session::Session::new(client, SessionConfig::default());

    let id = manager.add(session, "my-session");
    assert_eq!(manager.label(id).await, Some("my-session".to_string()));
}

#[tokio::test]
async fn multi_session_manager_active_state() {
    use rust_expect::config::SessionConfig;
    use tokio::io::DuplexStream;

    let mut manager: MultiSessionManager<DuplexStream> = MultiSessionManager::new();
    let (client, _server) = tokio::io::duplex(1024);
    let session = rust_expect::session::Session::new(client, SessionConfig::default());

    let id = manager.add(session, "session");

    // Should be active by default
    assert!(manager.is_active(id).await);

    // Deactivate
    manager.set_active(id, false).await;
    assert!(!manager.is_active(id).await);

    // Check active_ids
    let active = manager.active_ids().await;
    assert!(active.is_empty());
}

#[tokio::test]
async fn multi_session_manager_session_ids() {
    use rust_expect::config::SessionConfig;
    use tokio::io::DuplexStream;

    let mut manager: MultiSessionManager<DuplexStream> = MultiSessionManager::new();
    let (c1, _) = tokio::io::duplex(1024);
    let (c2, _) = tokio::io::duplex(1024);

    let s1 = rust_expect::session::Session::new(c1, SessionConfig::default());
    let s2 = rust_expect::session::Session::new(c2, SessionConfig::default());

    let id1 = manager.add(s1, "a");
    let id2 = manager.add(s2, "b");

    let ids = manager.session_ids();
    assert!(ids.contains(&id1));
    assert!(ids.contains(&id2));
}
