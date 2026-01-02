//! Session groups for parallel operations.

use std::collections::HashMap;
use std::time::Duration;

/// Session group identifier.
pub type GroupId = String;

/// Session identifier within a group.
pub type SessionId = usize;

/// Result of a group operation.
#[derive(Debug, Clone)]
pub struct GroupResult {
    /// Session ID.
    pub session_id: SessionId,
    /// Whether the operation succeeded.
    pub success: bool,
    /// Output or error.
    pub output: String,
}

/// A group of sessions for parallel operations.
#[derive(Debug, Default)]
pub struct SessionGroup {
    /// Group name.
    name: String,
    /// Sessions in the group.
    sessions: HashMap<SessionId, SessionInfo>,
    /// Next session ID.
    next_id: SessionId,
    /// Default timeout.
    timeout: Duration,
}

/// Information about a session in a group.
#[derive(Debug, Clone)]
struct SessionInfo {
    /// Session label.
    label: String,
    /// Whether session is active.
    active: bool,
    /// Accumulated output.
    output: String,
}

impl SessionGroup {
    /// Create a new session group.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            timeout: Duration::from_secs(30),
            ..Default::default()
        }
    }

    /// Set timeout for group operations.
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Get group name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Add a session to the group.
    pub fn add(&mut self, label: impl Into<String>) -> SessionId {
        let id = self.next_id;
        self.next_id += 1;
        self.sessions.insert(
            id,
            SessionInfo {
                label: label.into(),
                active: true,
                output: String::new(),
            },
        );
        id
    }

    /// Remove a session from the group.
    pub fn remove(&mut self, id: SessionId) {
        self.sessions.remove(&id);
    }

    /// Get session count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Check if empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Get active session count.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.sessions.values().filter(|s| s.active).count()
    }

    /// Get session label.
    #[must_use]
    pub fn label(&self, id: SessionId) -> Option<&str> {
        self.sessions.get(&id).map(|s| s.label.as_str())
    }

    /// Set session active state.
    pub fn set_active(&mut self, id: SessionId, active: bool) {
        if let Some(session) = self.sessions.get_mut(&id) {
            session.active = active;
        }
    }

    /// Append output for a session.
    pub fn append_output(&mut self, id: SessionId, output: &str) {
        if let Some(session) = self.sessions.get_mut(&id) {
            session.output.push_str(output);
        }
    }

    /// Get output for a session.
    #[must_use]
    pub fn output(&self, id: SessionId) -> Option<&str> {
        self.sessions.get(&id).map(|s| s.output.as_str())
    }

    /// Clear output for all sessions.
    pub fn clear_output(&mut self) {
        for session in self.sessions.values_mut() {
            session.output.clear();
        }
    }

    /// Get all session IDs.
    #[must_use]
    pub fn session_ids(&self) -> Vec<SessionId> {
        self.sessions.keys().copied().collect()
    }

    /// Get active session IDs.
    #[must_use]
    pub fn active_ids(&self) -> Vec<SessionId> {
        self.sessions
            .iter()
            .filter(|(_, s)| s.active)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Execute an operation on all active sessions.
    pub fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(SessionId, &str),
    {
        for (id, session) in &self.sessions {
            if session.active {
                f(*id, &session.label);
            }
        }
    }
}

/// Builder for session groups.
#[derive(Debug, Default)]
pub struct GroupBuilder {
    name: String,
    timeout: Duration,
    labels: Vec<String>,
}

impl GroupBuilder {
    /// Create a new builder.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            timeout: Duration::from_secs(30),
            labels: Vec::new(),
        }
    }

    /// Set timeout.
    #[must_use]
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Add a session by label.
    #[must_use]
    pub fn add(mut self, label: impl Into<String>) -> Self {
        self.labels.push(label.into());
        self
    }

    /// Build the group.
    #[must_use]
    pub fn build(self) -> SessionGroup {
        let mut group = SessionGroup::new(self.name).with_timeout(self.timeout);
        for label in self.labels {
            group.add(label);
        }
        group
    }
}

/// Manager for multiple session groups.
#[derive(Debug, Default)]
pub struct GroupManager {
    /// Groups by name.
    groups: HashMap<GroupId, SessionGroup>,
}

impl GroupManager {
    /// Create a new manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create and add a new group.
    pub fn create(&mut self, name: impl Into<String>) -> &mut SessionGroup {
        let name = name.into();
        self.groups
            .entry(name.clone())
            .or_insert_with(|| SessionGroup::new(name))
    }

    /// Get a group by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&SessionGroup> {
        self.groups.get(name)
    }

    /// Get a mutable group by name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut SessionGroup> {
        self.groups.get_mut(name)
    }

    /// Remove a group.
    pub fn remove(&mut self, name: &str) -> Option<SessionGroup> {
        self.groups.remove(name)
    }

    /// Get all group names.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.groups.keys().map(std::string::String::as_str).collect()
    }

    /// Get total session count across all groups.
    #[must_use]
    pub fn total_sessions(&self) -> usize {
        self.groups.values().map(SessionGroup::len).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_basic() {
        let mut group = SessionGroup::new("test");
        let id1 = group.add("server1");
        let id2 = group.add("server2");

        assert_eq!(group.len(), 2);
        assert_eq!(group.label(id1), Some("server1"));
        assert_eq!(group.label(id2), Some("server2"));
    }

    #[test]
    fn group_builder() {
        let group = GroupBuilder::new("servers")
            .add("server1")
            .add("server2")
            .add("server3")
            .build();

        assert_eq!(group.name(), "servers");
        assert_eq!(group.len(), 3);
    }

    #[test]
    fn group_manager() {
        let mut manager = GroupManager::new();
        manager.create("web").add("web1");
        manager.create("db").add("db1");

        assert_eq!(manager.names().len(), 2);
        assert_eq!(manager.total_sessions(), 2);
    }

    #[test]
    fn group_active() {
        let mut group = SessionGroup::new("test");
        let id = group.add("server");

        assert_eq!(group.active_count(), 1);

        group.set_active(id, false);
        assert_eq!(group.active_count(), 0);
    }
}
