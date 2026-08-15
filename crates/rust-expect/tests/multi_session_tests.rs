//! Integration tests for multi-session management.
#![allow(clippy::similar_names)]

use rust_expect::multi::{MultiSessionManager, PatternSelector, ReadyType};

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
