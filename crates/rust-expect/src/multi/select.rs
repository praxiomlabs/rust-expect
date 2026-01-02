//! Multi-session selection.

use std::collections::HashMap;
use std::time::Duration;

/// Session identifier.
pub type SessionId = usize;

/// Result of a select operation.
#[derive(Debug, Clone)]
pub struct SelectResult {
    /// Session that is ready.
    pub session_id: SessionId,
    /// Type of readiness.
    pub ready_type: ReadyType,
    /// Data if available.
    pub data: Option<Vec<u8>>,
}

/// Type of readiness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadyType {
    /// Session has data to read.
    Readable,
    /// Session is ready for writing.
    Writable,
    /// Session has closed.
    Closed,
    /// Session has an error.
    Error,
}

/// Multi-session selector.
#[derive(Debug, Default)]
pub struct Selector {
    /// Registered sessions.
    sessions: HashMap<SessionId, SessionState>,
    /// Next session ID.
    next_id: SessionId,
    /// Default timeout.
    timeout: Duration,
}

/// State of a session in the selector.
#[derive(Debug, Clone)]
struct SessionState {
    /// Whether we're watching for readable.
    readable: bool,
    /// Whether we're watching for writable.
    writable: bool,
    /// Pending data.
    pending_data: Option<Vec<u8>>,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            readable: true,
            writable: false,
            pending_data: None,
        }
    }
}

impl Selector {
    /// Create a new selector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            ..Default::default()
        }
    }

    /// Set default timeout.
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Register a session and return its ID.
    pub fn register(&mut self) -> SessionId {
        let id = self.next_id;
        self.next_id += 1;
        self.sessions.insert(id, SessionState::default());
        id
    }

    /// Unregister a session.
    pub fn unregister(&mut self, id: SessionId) {
        self.sessions.remove(&id);
    }

    /// Set interest in readable events.
    pub fn set_readable(&mut self, id: SessionId, interest: bool) {
        if let Some(state) = self.sessions.get_mut(&id) {
            state.readable = interest;
        }
    }

    /// Set interest in writable events.
    pub fn set_writable(&mut self, id: SessionId, interest: bool) {
        if let Some(state) = self.sessions.get_mut(&id) {
            state.writable = interest;
        }
    }

    /// Push data for a session (simulates data arriving).
    pub fn push_data(&mut self, id: SessionId, data: Vec<u8>) {
        if let Some(state) = self.sessions.get_mut(&id) {
            state.pending_data = Some(data);
        }
    }

    /// Check if any session is ready.
    #[must_use]
    pub fn poll(&mut self) -> Option<SelectResult> {
        for (&id, state) in &mut self.sessions {
            if state.readable && state.pending_data.is_some() {
                let data = state.pending_data.take();
                return Some(SelectResult {
                    session_id: id,
                    ready_type: ReadyType::Readable,
                    data,
                });
            }
        }
        None
    }

    /// Wait for a session to be ready.
    #[must_use]
    pub fn select(&mut self) -> Option<SelectResult> {
        // In a real implementation, this would block
        self.poll()
    }

    /// Wait for a session with timeout.
    #[must_use]
    pub fn select_timeout(&mut self, _timeout: Duration) -> Option<SelectResult> {
        // In a real implementation, this would block with timeout
        self.poll()
    }

    /// Get number of registered sessions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Check if empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Get all session IDs.
    #[must_use]
    pub fn session_ids(&self) -> Vec<SessionId> {
        self.sessions.keys().copied().collect()
    }
}

/// Select on multiple patterns across sessions.
#[derive(Debug)]
pub struct PatternSelector {
    /// Patterns to match.
    patterns: Vec<(SessionId, String)>,
}

impl PatternSelector {
    /// Create a new pattern selector.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            patterns: Vec::new(),
        }
    }

    /// Add a pattern for a session.
    #[must_use]
    pub fn add(mut self, session_id: SessionId, pattern: impl Into<String>) -> Self {
        self.patterns.push((session_id, pattern.into()));
        self
    }

    /// Get patterns for a session.
    #[must_use]
    pub fn patterns_for(&self, session_id: SessionId) -> Vec<&str> {
        self.patterns
            .iter()
            .filter(|(id, _)| *id == session_id)
            .map(|(_, p)| p.as_str())
            .collect()
    }
}

impl Default for PatternSelector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_register() {
        let mut sel = Selector::new();
        let id1 = sel.register();
        let id2 = sel.register();

        assert_ne!(id1, id2);
        assert_eq!(sel.len(), 2);
    }

    #[test]
    fn selector_poll() {
        let mut sel = Selector::new();
        let id = sel.register();

        assert!(sel.poll().is_none());

        sel.push_data(id, b"hello".to_vec());
        let result = sel.poll().unwrap();

        assert_eq!(result.session_id, id);
        assert_eq!(result.ready_type, ReadyType::Readable);
        assert_eq!(result.data, Some(b"hello".to_vec()));
    }

    #[test]
    fn pattern_selector() {
        let sel = PatternSelector::new()
            .add(0, "login:")
            .add(0, "password:")
            .add(1, "prompt>");

        assert_eq!(sel.patterns_for(0).len(), 2);
        assert_eq!(sel.patterns_for(1).len(), 1);
    }
}
