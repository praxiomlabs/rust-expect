//! Session handle for interacting with spawned processes.
//!
//! This module provides the main `Session` type that users interact with
//! to control spawned processes, send input, and expect output.

use crate::config::SessionConfig;
use crate::error::{ExpectError, Result};
use crate::expect::{ExpectState, MatchResult, Matcher, Pattern, PatternManager, PatternSet};
use crate::types::{ControlChar, Dimensions, Match, ProcessExitStatus, SessionId, SessionState};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

/// A session handle for interacting with a spawned process.
///
/// The session provides methods to send input, expect patterns in output,
/// and manage the lifecycle of the process.
pub struct Session<T: AsyncReadExt + AsyncWriteExt + Unpin + Send> {
    /// The underlying transport (PTY, SSH channel, etc.).
    transport: Arc<Mutex<T>>,
    /// Session configuration.
    config: SessionConfig,
    /// Pattern matcher.
    matcher: Matcher,
    /// Pattern manager for before/after patterns.
    pattern_manager: PatternManager,
    /// Current session state.
    state: SessionState,
    /// Unique session identifier.
    id: SessionId,
    /// EOF flag.
    eof: bool,
}

impl<T: AsyncReadExt + AsyncWriteExt + Unpin + Send> Session<T> {
    /// Create a new session with the given transport.
    pub fn new(transport: T, config: SessionConfig) -> Self {
        let buffer_size = config.buffer.max_size;
        Self {
            transport: Arc::new(Mutex::new(transport)),
            config,
            matcher: Matcher::new(buffer_size),
            pattern_manager: PatternManager::new(),
            state: SessionState::Starting,
            id: SessionId::new(),
            eof: false,
        }
    }

    /// Get the session ID.
    #[must_use]
    pub const fn id(&self) -> &SessionId {
        &self.id
    }

    /// Get the current session state.
    #[must_use]
    pub const fn state(&self) -> SessionState {
        self.state
    }

    /// Get the session configuration.
    #[must_use]
    pub const fn config(&self) -> &SessionConfig {
        &self.config
    }

    /// Check if EOF has been detected.
    #[must_use]
    pub const fn is_eof(&self) -> bool {
        self.eof
    }

    /// Get the current buffer contents.
    #[must_use]
    pub fn buffer(&mut self) -> String {
        self.matcher.buffer_str()
    }

    /// Clear the buffer.
    pub fn clear_buffer(&mut self) {
        self.matcher.clear();
    }

    /// Get the pattern manager for before/after patterns.
    #[must_use]
    pub const fn pattern_manager(&self) -> &PatternManager {
        &self.pattern_manager
    }

    /// Get mutable access to the pattern manager.
    pub fn pattern_manager_mut(&mut self) -> &mut PatternManager {
        &mut self.pattern_manager
    }

    /// Set the session state.
    pub fn set_state(&mut self, state: SessionState) {
        self.state = state;
    }

    /// Send bytes to the process.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub async fn send(&mut self, data: &[u8]) -> Result<()> {
        if matches!(self.state, SessionState::Closed | SessionState::Exited(_)) {
            return Err(ExpectError::SessionClosed);
        }

        let mut transport = self.transport.lock().await;
        transport.write_all(data).await.map_err(ExpectError::Io)?;
        transport.flush().await.map_err(ExpectError::Io)?;
        Ok(())
    }

    /// Send a string to the process.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub async fn send_str(&mut self, s: &str) -> Result<()> {
        self.send(s.as_bytes()).await
    }

    /// Send a line to the process (appends newline based on config).
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub async fn send_line(&mut self, line: &str) -> Result<()> {
        let line_ending = self.config.line_ending.as_str();
        let data = format!("{line}{line_ending}");
        self.send(data.as_bytes()).await
    }

    /// Send a control character to the process.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub async fn send_control(&mut self, ctrl: ControlChar) -> Result<()> {
        self.send(&[ctrl.as_byte()]).await
    }

    /// Expect a pattern in the output.
    ///
    /// Blocks until the pattern is matched, EOF is detected, or timeout occurs.
    ///
    /// # Errors
    ///
    /// Returns an error on timeout, EOF (if not expected), or I/O error.
    pub async fn expect(&mut self, pattern: impl Into<Pattern>) -> Result<Match> {
        let patterns = PatternSet::from_patterns(vec![pattern.into()]);
        self.expect_any(&patterns).await
    }

    /// Expect any of the given patterns.
    ///
    /// # Errors
    ///
    /// Returns an error on timeout, EOF (if not expected), or I/O error.
    pub async fn expect_any(&mut self, patterns: &PatternSet) -> Result<Match> {
        let timeout = self.matcher.get_timeout(patterns);
        let state = ExpectState::new(patterns.clone(), timeout);

        loop {
            // Check before patterns first
            if let Some((_, action)) = self.pattern_manager.check_before(&self.matcher.buffer_str()) {
                match action {
                    crate::expect::HandlerAction::Continue => {}
                    crate::expect::HandlerAction::Return(s) => {
                        return Ok(Match::new(0, s, String::new(), self.matcher.buffer_str()));
                    }
                    crate::expect::HandlerAction::Abort(msg) => {
                        return Err(ExpectError::PatternNotFound {
                            pattern: msg,
                            buffer: self.matcher.buffer_str(),
                        });
                    }
                    crate::expect::HandlerAction::Respond(s) => {
                        self.send_str(&s).await?;
                    }
                }
            }

            // Check for pattern match
            if let Some(result) = self.matcher.try_match_any(patterns) {
                return Ok(self.matcher.consume_match(&result));
            }

            // Check for timeout
            if state.is_timed_out() {
                return Err(ExpectError::Timeout {
                    duration: timeout,
                    pattern: patterns
                        .iter()
                        .next()
                        .map(|p| p.pattern.as_str().to_string())
                        .unwrap_or_default(),
                    buffer: self.matcher.buffer_str(),
                });
            }

            // Check for EOF
            if self.eof {
                if state.expects_eof() {
                    return Ok(Match::new(0, String::new(), self.matcher.buffer_str(), String::new()));
                }
                return Err(ExpectError::Eof {
                    buffer: self.matcher.buffer_str(),
                });
            }

            // Read more data
            self.read_with_timeout(state.remaining_time()).await?;
        }
    }

    /// Expect with a specific timeout.
    ///
    /// # Errors
    ///
    /// Returns an error on timeout, EOF, or I/O error.
    pub async fn expect_timeout(&mut self, pattern: impl Into<Pattern>, timeout: Duration) -> Result<Match> {
        let pattern = pattern.into();
        let mut patterns = PatternSet::new();
        patterns.add(pattern).add(Pattern::timeout(timeout));
        self.expect_any(&patterns).await
    }

    /// Read data from the transport with timeout.
    async fn read_with_timeout(&mut self, timeout: Duration) -> Result<usize> {
        let mut buf = [0u8; 4096];
        let mut transport = self.transport.lock().await;

        match tokio::time::timeout(timeout, transport.read(&mut buf)).await {
            Ok(Ok(0)) => {
                self.eof = true;
                Ok(0)
            }
            Ok(Ok(n)) => {
                self.matcher.append(&buf[..n]);
                Ok(n)
            }
            Ok(Err(e)) => Err(ExpectError::Io(e)),
            Err(_) => {
                // Timeout, but not an error - caller will handle
                Ok(0)
            }
        }
    }

    /// Wait for the process to exit.
    ///
    /// # Errors
    ///
    /// Returns an error if waiting fails.
    pub async fn wait(&mut self) -> Result<ProcessExitStatus> {
        // Read until EOF
        while !self.eof {
            if self.read_with_timeout(Duration::from_millis(100)).await? == 0 && !self.eof {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }

        // Return unknown status - actual status depends on backend
        self.state = SessionState::Exited(ProcessExitStatus::Unknown);
        Ok(ProcessExitStatus::Unknown)
    }

    /// Check if a pattern matches immediately without blocking.
    #[must_use]
    pub fn check(&mut self, pattern: &Pattern) -> Option<MatchResult> {
        self.matcher.try_match(pattern)
    }

    /// Get the underlying transport.
    ///
    /// Use with caution as direct access bypasses session management.
    #[must_use] pub const fn transport(&self) -> &Arc<Mutex<T>> {
        &self.transport
    }
}

impl<T: AsyncReadExt + AsyncWriteExt + Unpin + Send> std::fmt::Debug for Session<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("id", &self.id)
            .field("state", &self.state)
            .field("eof", &self.eof)
            .finish_non_exhaustive()
    }
}

/// Extension trait for session operations.
pub trait SessionExt {
    /// Send and expect in one call.
    fn send_expect(
        &mut self,
        send: &str,
        expect: impl Into<Pattern>,
    ) -> impl std::future::Future<Output = Result<Match>> + Send;

    /// Resize the terminal.
    fn resize(&mut self, dimensions: Dimensions) -> impl std::future::Future<Output = Result<()>> + Send;
}
