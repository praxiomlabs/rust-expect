//! Integration tests for multi-session management.

use rust_expect::multi::group::{GroupBuilder, GroupResult};
use rust_expect::{GroupManager, Selector, SessionGroup};

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

#[test]
fn selector_new() {
    let selector = Selector::new();
    assert!(selector.is_empty());
    assert_eq!(selector.len(), 0);
}

#[test]
fn selector_register() {
    let mut selector = Selector::new();
    let id1 = selector.register();
    let id2 = selector.register();

    assert_ne!(id1, id2);
    assert_eq!(selector.len(), 2);
}

#[test]
fn selector_unregister() {
    let mut selector = Selector::new();
    let id = selector.register();

    assert_eq!(selector.len(), 1);
    selector.unregister(id);
    assert!(selector.is_empty());
}

#[test]
fn selector_poll_empty() {
    let mut selector = Selector::new();
    selector.register();

    // No data pushed, should return None
    assert!(selector.poll().is_none());
}

#[test]
fn selector_poll_with_data() {
    let mut selector = Selector::new();
    let id = selector.register();

    selector.push_data(id, b"hello".to_vec());
    let result = selector.poll();

    assert!(result.is_some());
    let result = result.unwrap();
    assert_eq!(result.session_id, id);
    assert_eq!(result.data, Some(b"hello".to_vec()));
}

#[test]
fn selector_session_ids() {
    let mut selector = Selector::new();
    let id1 = selector.register();
    let id2 = selector.register();

    let ids = selector.session_ids();
    assert!(ids.contains(&id1));
    assert!(ids.contains(&id2));
}

#[test]
fn selector_display() {
    let selector = Selector::new();
    let display = format!("{:?}", selector);
    assert!(!display.is_empty());
}
