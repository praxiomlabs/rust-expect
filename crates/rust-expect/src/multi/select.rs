//! Async multi-session selection.
//!
//! This module provides true async selection across multiple sessions,
//! allowing you to wait for patterns to match on any of several sessions
//! simultaneously using Tokio's async primitives.
//!
//! # Example
//!
//! ```ignore
//! use rust_expect::multi::{MultiSessionManager, SelectResult};
//! use rust_expect::Session;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), rust_expect::ExpectError> {
//!     let mut manager = MultiSessionManager::new();
//!
//!     // Add sessions
//!     let id1 = manager.spawn("bash", &["-c", "echo server1"]).await?;
//!     let id2 = manager.spawn("bash", &["-c", "echo server2"]).await?;
//!
//!     // Wait for any session to produce output
//!     let result = manager.expect_any("server").await?;
//!     println!("Session {} matched: {}", result.session_id, result.matched.matched);
//!
//!     Ok(())
//! }
//! ```

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::stream::{FuturesUnordered, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

use crate::config::SessionConfig;
use crate::error::{ExpectError, Result};
use crate::expect::{Pattern, PatternSet};
use crate::types::Match;

/// Unique identifier for a session within a multi-session manager.
pub type SessionId = usize;

/// Result of a multi-session select operation.
#[derive(Debug, Clone)]
pub struct SelectResult {
    /// The session that matched.
    pub session_id: SessionId,
    /// The match result.
    pub matched: Match,
    /// Index of the pattern that matched (if multiple patterns provided).
    pub pattern_index: usize,
}

/// Result of a multi-session send operation.
#[derive(Debug, Clone)]
pub struct SendResult {
    /// The session the data was sent to.
    pub session_id: SessionId,
    /// Whether the send succeeded.
    pub success: bool,
    /// Error message if failed.
    pub error: Option<String>,
}

/// Type of readiness event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadyType {
    /// Session has data matching a pattern.
    Matched,
    /// Session has data available to read.
    Readable,
    /// Session is ready for writing.
    Writable,
    /// Session has closed (EOF).
    Closed,
    /// Session encountered an error.
    Error,
}

/// A managed session with its metadata.
struct ManagedSession<T: AsyncReadExt + AsyncWriteExt + Unpin + Send> {
    /// The underlying session.
    session: crate::session::Session<T>,
    /// Session label for identification.
    label: String,
    /// Whether the session is active.
    active: bool,
}

impl<T: AsyncReadExt + AsyncWriteExt + Unpin + Send> fmt::Debug for ManagedSession<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ManagedSession")
            .field("label", &self.label)
            .field("active", &self.active)
            .finish_non_exhaustive()
    }
}

/// Manager for multiple async sessions with select capabilities.
///
/// This provides the core multi-session functionality, allowing you to:
/// - Manage multiple sessions simultaneously
/// - Wait for any session to match a pattern (`expect_any`)
/// - Wait for all sessions to match patterns (`expect_all`)
/// - Send to multiple sessions in parallel
/// - Select on multiple sessions with different patterns per session
pub struct MultiSessionManager<T: AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static> {
    /// Sessions indexed by ID.
    sessions: HashMap<SessionId, Arc<Mutex<ManagedSession<T>>>>,
    /// Next session ID to assign.
    next_id: SessionId,
    /// Default timeout for operations.
    default_timeout: Duration,
    /// Default configuration for spawned sessions.
    default_config: SessionConfig,
}

impl<T: AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static> fmt::Debug
    for MultiSessionManager<T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MultiSessionManager")
            .field("session_count", &self.sessions.len())
            .field("next_id", &self.next_id)
            .field("default_timeout", &self.default_timeout)
            .finish()
    }
}

impl<T: AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static> Default for MultiSessionManager<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static> MultiSessionManager<T> {
    /// Create a new multi-session manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            next_id: 0,
            default_timeout: Duration::from_secs(30),
            default_config: SessionConfig::default(),
        }
    }

    /// Set the default timeout for operations.
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// Set the default session configuration.
    #[must_use]
    pub fn with_config(mut self, config: SessionConfig) -> Self {
        self.default_config = config;
        self
    }

    /// Add an existing session to the manager.
    ///
    /// Returns the assigned session ID.
    pub fn add(
        &mut self,
        session: crate::session::Session<T>,
        label: impl Into<String>,
    ) -> SessionId {
        let id = self.next_id;
        self.next_id += 1;

        let managed = ManagedSession {
            session,
            label: label.into(),
            active: true,
        };

        self.sessions.insert(id, Arc::new(Mutex::new(managed)));
        id
    }

    /// Remove a session from the manager.
    ///
    /// Returns the session if it existed.
    #[allow(clippy::unused_async)]
    pub async fn remove(&mut self, id: SessionId) -> Option<crate::session::Session<T>> {
        if let Some(arc) = self.sessions.remove(&id) {
            // Try to unwrap the Arc - this will only succeed if we have the only reference
            match Arc::try_unwrap(arc) {
                Ok(mutex) => Some(mutex.into_inner().session),
                Err(arc) => {
                    // Put it back and return None - someone else has a reference
                    self.sessions.insert(id, arc);
                    None
                }
            }
        } else {
            None
        }
    }

    /// Get the number of sessions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Check if there are no sessions.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Get all session IDs.
    #[must_use]
    pub fn session_ids(&self) -> Vec<SessionId> {
        self.sessions.keys().copied().collect()
    }

    /// Get the label for a session.
    pub async fn label(&self, id: SessionId) -> Option<String> {
        if let Some(arc) = self.sessions.get(&id) {
            let guard = arc.lock().await;
            Some(guard.label.clone())
        } else {
            None
        }
    }

    /// Check if a session is active.
    pub async fn is_active(&self, id: SessionId) -> bool {
        if let Some(arc) = self.sessions.get(&id) {
            let guard = arc.lock().await;
            guard.active
        } else {
            false
        }
    }

    /// Set a session's active state.
    pub async fn set_active(&self, id: SessionId, active: bool) {
        if let Some(arc) = self.sessions.get(&id) {
            let mut guard = arc.lock().await;
            guard.active = active;
        }
    }

    /// Get active session IDs.
    pub async fn active_ids(&self) -> Vec<SessionId> {
        let mut active = Vec::new();
        for &id in self.sessions.keys() {
            if self.is_active(id).await {
                active.push(id);
            }
        }
        active
    }

    /// Send data to a specific session.
    ///
    /// # Errors
    ///
    /// Returns an error if the session doesn't exist or the send fails.
    pub async fn send(&self, id: SessionId, data: &[u8]) -> Result<()> {
        let arc = self
            .sessions
            .get(&id)
            .ok_or(ExpectError::SessionNotFound { id })?;

        let mut guard = arc.lock().await;
        guard.session.send(data).await
    }

    /// Send a line to a specific session.
    ///
    /// # Errors
    ///
    /// Returns an error if the session doesn't exist or the send fails.
    pub async fn send_line(&self, id: SessionId, line: &str) -> Result<()> {
        let arc = self
            .sessions
            .get(&id)
            .ok_or(ExpectError::SessionNotFound { id })?;

        let mut guard = arc.lock().await;
        guard.session.send_line(line).await
    }

    /// Send data to all active sessions in parallel.
    ///
    /// Returns results for each session.
    pub async fn send_all(&self, data: &[u8]) -> Vec<SendResult> {
        let mut futures = FuturesUnordered::new();

        for (&id, arc) in &self.sessions {
            let arc = Arc::clone(arc);
            let data = data.to_vec();

            futures.push(async move {
                let mut guard = arc.lock().await;
                if !guard.active {
                    return SendResult {
                        session_id: id,
                        success: false,
                        error: Some("Session not active".to_string()),
                    };
                }

                match guard.session.send(&data).await {
                    Ok(()) => SendResult {
                        session_id: id,
                        success: true,
                        error: None,
                    },
                    Err(e) => SendResult {
                        session_id: id,
                        success: false,
                        error: Some(e.to_string()),
                    },
                }
            });
        }

        let mut results = Vec::new();
        while let Some(result) = futures.next().await {
            results.push(result);
        }
        results
    }

    /// Expect a pattern on a specific session.
    ///
    /// # Errors
    ///
    /// Returns an error if the session doesn't exist or expect fails.
    pub async fn expect(&self, id: SessionId, pattern: impl Into<Pattern>) -> Result<Match> {
        let arc = self
            .sessions
            .get(&id)
            .ok_or(ExpectError::SessionNotFound { id })?;

        let mut guard = arc.lock().await;
        guard.session.expect(pattern).await
    }

    /// Wait for any session to match the given pattern.
    ///
    /// Returns as soon as any session matches. This is the primary multi-session
    /// select operation, equivalent to TCL Expect's multi-spawn expect.
    ///
    /// # Errors
    ///
    /// Returns an error if all sessions timeout or encounter errors.
    #[allow(clippy::type_complexity)]
    pub async fn expect_any(&self, pattern: impl Into<Pattern>) -> Result<SelectResult> {
        let pattern = pattern.into();
        self.expect_any_of(&[pattern]).await
    }

    /// Wait for any session to match any of the given patterns.
    ///
    /// # Errors
    ///
    /// Returns an error if all sessions timeout or encounter errors.
    #[allow(clippy::type_complexity)]
    pub async fn expect_any_of(&self, patterns: &[Pattern]) -> Result<SelectResult> {
        if self.sessions.is_empty() {
            return Err(ExpectError::NoSessions);
        }

        let pattern_set = PatternSet::from_patterns(patterns.to_vec());

        // Create futures for all active sessions
        let mut futures: FuturesUnordered<
            Pin<Box<dyn Future<Output = (SessionId, Result<(Match, usize)>)> + Send>>,
        > = FuturesUnordered::new();

        for (&id, arc) in &self.sessions {
            let arc = Arc::clone(arc);
            let patterns = pattern_set.clone();

            let future: Pin<Box<dyn Future<Output = (SessionId, Result<(Match, usize)>)> + Send>> =
                Box::pin(async move {
                    let mut guard = arc.lock().await;
                    if !guard.active {
                        return (id, Err(ExpectError::SessionClosed));
                    }

                    match guard.session.expect_any(&patterns).await {
                        Ok(m) => (id, Ok((m, 0))), // pattern_index 0 for now
                        Err(e) => (id, Err(e)),
                    }
                });

            futures.push(future);
        }

        // Wait for the first successful match
        let mut last_error: Option<ExpectError> = None;

        while let Some((session_id, result)) = futures.next().await {
            match result {
                Ok((matched, pattern_index)) => {
                    return Ok(SelectResult {
                        session_id,
                        matched,
                        pattern_index,
                    });
                }
                Err(e) => {
                    // Store the error but continue waiting for others
                    // Only timeouts should be ignored; other errors are real failures
                    if !matches!(e, ExpectError::Timeout { .. }) {
                        last_error = Some(e);
                    }
                }
            }
        }

        // All futures completed without a match
        Err(last_error.unwrap_or_else(|| ExpectError::Timeout {
            duration: self.default_timeout,
            pattern: "multi-session expect".to_string(),
            buffer: String::new(),
        }))
    }

    /// Wait for all sessions to match patterns.
    ///
    /// Each session must match at least one pattern. Returns results for all sessions.
    ///
    /// # Errors
    ///
    /// Returns an error if any session fails to match.
    pub async fn expect_all(&self, pattern: impl Into<Pattern>) -> Result<Vec<SelectResult>> {
        let pattern = pattern.into();
        self.expect_all_of(&[pattern]).await
    }

    /// Wait for all sessions to match any of the given patterns.
    ///
    /// # Errors
    ///
    /// Returns an error if any session fails.
    #[allow(clippy::type_complexity)]
    pub async fn expect_all_of(&self, patterns: &[Pattern]) -> Result<Vec<SelectResult>> {
        if self.sessions.is_empty() {
            return Err(ExpectError::NoSessions);
        }

        let pattern_set = PatternSet::from_patterns(patterns.to_vec());

        // Create futures for all active sessions
        let mut futures: FuturesUnordered<
            Pin<Box<dyn Future<Output = (SessionId, Result<(Match, usize)>)> + Send>>,
        > = FuturesUnordered::new();

        for (&id, arc) in &self.sessions {
            let arc = Arc::clone(arc);
            let patterns = pattern_set.clone();

            let future: Pin<Box<dyn Future<Output = (SessionId, Result<(Match, usize)>)> + Send>> =
                Box::pin(async move {
                    let mut guard = arc.lock().await;
                    if !guard.active {
                        return (id, Err(ExpectError::SessionClosed));
                    }

                    match guard.session.expect_any(&patterns).await {
                        Ok(m) => (id, Ok((m, 0))),
                        Err(e) => (id, Err(e)),
                    }
                });

            futures.push(future);
        }

        // Collect all results
        let mut results = Vec::new();
        let mut errors = Vec::new();

        while let Some((session_id, result)) = futures.next().await {
            match result {
                Ok((matched, pattern_index)) => {
                    results.push(SelectResult {
                        session_id,
                        matched,
                        pattern_index,
                    });
                }
                Err(e) => {
                    errors.push((session_id, e));
                }
            }
        }

        // If any session failed, return the first error
        if let Some((id, error)) = errors.into_iter().next() {
            return Err(ExpectError::MultiSessionError {
                session_id: id,
                error: Box::new(error),
            });
        }

        Ok(results)
    }

    /// Execute a closure on a specific session.
    ///
    /// This provides direct access to the session for operations not covered
    /// by the manager's API.
    ///
    /// # Errors
    ///
    /// Returns an error if the session doesn't exist.
    pub async fn with_session<F, R>(&self, id: SessionId, f: F) -> Result<R>
    where
        F: FnOnce(&mut crate::session::Session<T>) -> R,
    {
        let arc = self
            .sessions
            .get(&id)
            .ok_or(ExpectError::SessionNotFound { id })?;

        let mut guard = arc.lock().await;
        Ok(f(&mut guard.session))
    }

    /// Execute an async closure on a specific session.
    ///
    /// # Errors
    ///
    /// Returns an error if the session doesn't exist.
    pub async fn with_session_async<F, Fut, R>(&self, id: SessionId, f: F) -> Result<R>
    where
        F: FnOnce(&mut crate::session::Session<T>) -> Fut,
        Fut: Future<Output = R>,
    {
        let arc = self
            .sessions
            .get(&id)
            .ok_or(ExpectError::SessionNotFound { id })?;

        let mut guard = arc.lock().await;
        Ok(f(&mut guard.session).await)
    }
}

/// Builder for creating pattern selectors with per-session patterns.
///
/// This allows different patterns for different sessions, enabling
/// complex multi-session automation scenarios.
#[derive(Debug, Default)]
pub struct PatternSelector {
    /// Patterns per session.
    patterns: HashMap<SessionId, Vec<Pattern>>,
    /// Default patterns for sessions not explicitly configured.
    default_patterns: Vec<Pattern>,
    /// Timeout for the select operation.
    timeout: Option<Duration>,
}

impl PatternSelector {
    /// Create a new pattern selector.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a pattern for a specific session.
    #[must_use]
    pub fn session(mut self, id: SessionId, pattern: impl Into<Pattern>) -> Self {
        self.patterns.entry(id).or_default().push(pattern.into());
        self
    }

    /// Add multiple patterns for a specific session.
    #[must_use]
    pub fn session_patterns(mut self, id: SessionId, patterns: Vec<Pattern>) -> Self {
        self.patterns.entry(id).or_default().extend(patterns);
        self
    }

    /// Set default patterns for sessions not explicitly configured.
    #[must_use]
    pub fn default_pattern(mut self, pattern: impl Into<Pattern>) -> Self {
        self.default_patterns.push(pattern.into());
        self
    }

    /// Set timeout for the select operation.
    #[must_use]
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Get patterns for a session, falling back to defaults.
    #[must_use]
    pub fn patterns_for(&self, id: SessionId) -> &[Pattern] {
        self.patterns
            .get(&id)
            .map_or(&self.default_patterns, Vec::as_slice)
    }

    /// Execute the select operation on a multi-session manager.
    ///
    /// # Errors
    ///
    /// Returns an error if no sessions match or all timeout.
    #[allow(clippy::type_complexity)]
    pub async fn select<T>(&self, manager: &MultiSessionManager<T>) -> Result<SelectResult>
    where
        T: AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static,
    {
        if manager.is_empty() {
            return Err(ExpectError::NoSessions);
        }

        let timeout = self.timeout.unwrap_or(manager.default_timeout);

        // Create futures for all configured sessions
        let mut futures: FuturesUnordered<
            Pin<Box<dyn Future<Output = (SessionId, Result<(Match, usize)>)> + Send>>,
        > = FuturesUnordered::new();

        for &id in &manager.session_ids() {
            let patterns = self.patterns_for(id);
            if patterns.is_empty() {
                continue;
            }

            let arc = match manager.sessions.get(&id) {
                Some(arc) => Arc::clone(arc),
                None => continue,
            };

            let pattern_set = PatternSet::from_patterns(patterns.to_vec());

            let future: Pin<Box<dyn Future<Output = (SessionId, Result<(Match, usize)>)> + Send>> =
                Box::pin(async move {
                    let mut guard = arc.lock().await;
                    if !guard.active {
                        return (id, Err(ExpectError::SessionClosed));
                    }

                    match guard.session.expect_any(&pattern_set).await {
                        Ok(m) => (id, Ok((m, 0))),
                        Err(e) => (id, Err(e)),
                    }
                });

            futures.push(future);
        }

        // Apply overall timeout
        let select_future = async {
            while let Some((session_id, result)) = futures.next().await {
                if let Ok((matched, pattern_index)) = result {
                    return Ok(SelectResult {
                        session_id,
                        matched,
                        pattern_index,
                    });
                }
            }
            Err(ExpectError::Timeout {
                duration: timeout,
                pattern: "pattern selector".to_string(),
                buffer: String::new(),
            })
        };

        tokio::time::timeout(timeout, select_future)
            .await
            .map_err(|_| ExpectError::Timeout {
                duration: timeout,
                pattern: "pattern selector".to_string(),
                buffer: String::new(),
            })?
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::DuplexStream;

    use super::*;

    // Helper to create a mock session transport
    fn create_mock_transport() -> (DuplexStream, DuplexStream) {
        tokio::io::duplex(1024)
    }

    #[tokio::test]
    async fn manager_add_remove() {
        let mut manager: MultiSessionManager<DuplexStream> = MultiSessionManager::new();

        let (client, _server) = create_mock_transport();
        let session = crate::session::Session::new(client, SessionConfig::default());

        let id = manager.add(session, "test");
        assert_eq!(manager.len(), 1);
        assert_eq!(manager.label(id).await, Some("test".to_string()));

        let removed = manager.remove(id).await;
        assert!(removed.is_some());
        assert!(manager.is_empty());
    }

    #[tokio::test]
    async fn manager_active_state() {
        let mut manager: MultiSessionManager<DuplexStream> = MultiSessionManager::new();

        let (client, _server) = create_mock_transport();
        let session = crate::session::Session::new(client, SessionConfig::default());

        let id = manager.add(session, "test");
        assert!(manager.is_active(id).await);

        manager.set_active(id, false).await;
        assert!(!manager.is_active(id).await);

        let active = manager.active_ids().await;
        assert!(active.is_empty());
    }

    #[tokio::test]
    async fn pattern_selector_build() {
        let selector = PatternSelector::new()
            .session(0, "login:")
            .session(0, "password:")
            .session(1, "prompt>")
            .default_pattern("$");

        assert_eq!(selector.patterns_for(0).len(), 2);
        assert_eq!(selector.patterns_for(1).len(), 1);
        assert_eq!(selector.patterns_for(99).len(), 1); // Falls back to default
    }

    #[tokio::test]
    async fn expect_any_no_sessions() {
        let manager: MultiSessionManager<DuplexStream> = MultiSessionManager::new();
        let result = manager.expect_any("test").await;
        assert!(matches!(result, Err(ExpectError::NoSessions)));
    }
}
