//! Synchronous wrapper for async expect operations.
//!
//! This module provides a blocking API for users who prefer or require
//! synchronous operations instead of async/await.

use crate::config::SessionConfig;
use crate::error::Result;
use crate::expect::Pattern;
use crate::types::{ControlChar, Match};
use std::time::Duration;
use tokio::runtime::{Builder, Runtime};

/// A synchronous session wrapper.
///
/// This wraps an async session and provides blocking methods for
/// use in synchronous contexts.
pub struct SyncSession {
    /// The tokio runtime.
    runtime: Runtime,
    /// Session configuration.
    config: SessionConfig,
    /// Whether the session is active.
    active: bool,
}

impl SyncSession {
    /// Create a new synchronous session.
    ///
    /// # Errors
    ///
    /// Returns an error if the runtime cannot be created.
    pub fn new(config: SessionConfig) -> Result<Self> {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(crate::error::ExpectError::Io)?;

        Ok(Self {
            runtime,
            config,
            active: false,
        })
    }

    /// Spawn a command and create a session.
    ///
    /// # Errors
    ///
    /// Returns an error if spawning fails.
    pub fn spawn(command: &str, args: &[&str]) -> Result<Self> {
        let config = SessionConfig {
            command: command.to_string(),
            args: args.iter().map(|s| (*s).to_string()).collect(),
            ..Default::default()
        };

        let mut session = Self::new(config)?;
        session.active = true;
        Ok(session)
    }

    /// Get the session configuration.
    #[must_use]
    pub const fn config(&self) -> &SessionConfig {
        &self.config
    }

    /// Check if the session is active.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active
    }

    /// Send bytes to the session.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub fn send(&mut self, data: &[u8]) -> Result<()> {
        // Placeholder - would delegate to async session
        let _ = data;
        Ok(())
    }

    /// Send a string to the session.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub fn send_str(&mut self, s: &str) -> Result<()> {
        self.send(s.as_bytes())
    }

    /// Send a line to the session.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub fn send_line(&mut self, line: &str) -> Result<()> {
        let data = format!("{line}\n");
        self.send(data.as_bytes())
    }

    /// Send a control character.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub fn send_control(&mut self, ctrl: ControlChar) -> Result<()> {
        self.send(&[ctrl.as_byte()])
    }

    /// Expect a pattern in the output.
    ///
    /// # Errors
    ///
    /// Returns an error on timeout or EOF.
    pub fn expect(&mut self, pattern: impl Into<Pattern>) -> Result<Match> {
        self.expect_timeout(pattern, self.config.timeout.default)
    }

    /// Expect a pattern with a specific timeout.
    ///
    /// # Errors
    ///
    /// Returns an error on timeout or EOF.
    pub fn expect_timeout(&mut self, pattern: impl Into<Pattern>, timeout: Duration) -> Result<Match> {
        let _pattern = pattern.into();
        let _timeout = timeout;

        // Placeholder - would delegate to async session via runtime.block_on()
        Ok(Match::new(0, String::new(), String::new(), String::new()))
    }

    /// Read available data without blocking.
    ///
    /// Returns the data read, which may be empty if nothing is available.
    pub fn read_nonblocking(&mut self) -> Result<String> {
        // Placeholder
        Ok(String::new())
    }

    /// Close the session.
    pub fn close(&mut self) {
        self.active = false;
    }

    /// Run an async operation synchronously.
    pub fn block_on<F, T>(&self, future: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        self.runtime.block_on(future)
    }
}

impl Drop for SyncSession {
    fn drop(&mut self) {
        self.close();
    }
}

impl std::fmt::Debug for SyncSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncSession")
            .field("config", &self.config)
            .field("active", &self.active)
            .finish()
    }
}

/// A blocking expect operation.
pub struct BlockingExpect<'a> {
    session: &'a mut SyncSession,
    timeout: Duration,
}

impl<'a> BlockingExpect<'a> {
    /// Create a new blocking expect operation.
    pub fn new(session: &'a mut SyncSession) -> Self {
        let timeout = session.config.timeout.default;
        Self { session, timeout }
    }

    /// Set the timeout.
    #[must_use]
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Execute the expect operation.
    ///
    /// # Errors
    ///
    /// Returns an error on timeout or EOF.
    pub fn pattern(self, pattern: impl Into<Pattern>) -> Result<Match> {
        self.session.expect_timeout(pattern, self.timeout)
    }
}

/// Run async code synchronously.
///
/// This is a convenience function for running a single async operation
/// without managing a runtime.
///
/// # Errors
///
/// Returns an error if the runtime cannot be created.
pub fn block_on<F, T>(future: F) -> Result<T>
where
    F: std::future::Future<Output = T>,
{
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(crate::error::ExpectError::Io)?;

    Ok(runtime.block_on(future))
}

/// Spawn a session synchronously.
///
/// # Errors
///
/// Returns an error if spawning fails.
pub fn spawn(command: &str, args: &[&str]) -> Result<SyncSession> {
    SyncSession::spawn(command, args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_session_new() {
        let config = SessionConfig::default();
        let session = SyncSession::new(config);
        assert!(session.is_ok());
    }

    #[test]
    fn block_on_simple() {
        let result = block_on(async { 42 });
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }
}
