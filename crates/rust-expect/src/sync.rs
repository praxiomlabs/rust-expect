//! Synchronous wrapper for async expect operations.
//!
//! This module provides a blocking API for users who prefer or require
//! synchronous operations instead of async/await.

use std::time::Duration;

use tokio::runtime::{Builder, Runtime};

#[cfg(unix)]
use crate::backend::AsyncPty;
#[cfg(windows)]
use crate::backend::WindowsAsyncPty;
use crate::config::SessionConfig;
use crate::error::Result;
use crate::expect::Pattern;
use crate::session::Session;
use crate::types::{ControlChar, Match};

/// A synchronous session wrapper.
///
/// This wraps an async session and provides blocking methods for
/// use in synchronous contexts.
#[cfg(unix)]
pub struct SyncSession {
    /// The tokio runtime.
    runtime: Runtime,
    /// The inner async session.
    inner: Session<AsyncPty>,
}

/// A synchronous session wrapper (Windows).
///
/// This wraps an async session and provides blocking methods for
/// use in synchronous contexts using Windows ConPTY.
#[cfg(windows)]
pub struct SyncSession {
    /// The tokio runtime.
    runtime: Runtime,
    /// The inner async session.
    inner: Session<WindowsAsyncPty>,
}

#[cfg(unix)]
impl SyncSession {
    /// Spawn a command and create a session.
    ///
    /// # Errors
    ///
    /// Returns an error if spawning fails.
    pub fn spawn(command: &str, args: &[&str]) -> Result<Self> {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| crate::error::ExpectError::io_context("creating tokio runtime", e))?;

        let inner = runtime.block_on(Session::spawn(command, args))?;

        Ok(Self { runtime, inner })
    }

    /// Spawn with custom configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if spawning fails.
    pub fn spawn_with_config(command: &str, args: &[&str], config: SessionConfig) -> Result<Self> {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| crate::error::ExpectError::io_context("creating tokio runtime", e))?;

        let inner = runtime.block_on(Session::spawn_with_config(command, args, config))?;

        Ok(Self { runtime, inner })
    }

    /// Get the session configuration.
    #[must_use]
    pub const fn config(&self) -> &SessionConfig {
        self.inner.config()
    }

    /// Check if the session is active.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        !self.inner.is_eof()
    }

    /// Get the child process ID.
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.inner.pid()
    }

    /// Send bytes to the session.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub fn send(&mut self, data: &[u8]) -> Result<()> {
        self.runtime.block_on(self.inner.send(data))
    }

    /// Send a string to the session.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub fn send_str(&mut self, s: &str) -> Result<()> {
        self.runtime.block_on(self.inner.send_str(s))
    }

    /// Send a line to the session.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub fn send_line(&mut self, line: &str) -> Result<()> {
        self.runtime.block_on(self.inner.send_line(line))
    }

    /// Send a control character.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub fn send_control(&mut self, ctrl: ControlChar) -> Result<()> {
        self.runtime.block_on(self.inner.send_control(ctrl))
    }

    /// Expect a pattern in the output.
    ///
    /// # Errors
    ///
    /// Returns an error on timeout or EOF.
    pub fn expect(&mut self, pattern: impl Into<Pattern>) -> Result<Match> {
        self.runtime.block_on(self.inner.expect(pattern))
    }

    /// Expect a pattern with a specific timeout.
    ///
    /// # Errors
    ///
    /// Returns an error on timeout or EOF.
    pub fn expect_timeout(
        &mut self,
        pattern: impl Into<Pattern>,
        timeout: Duration,
    ) -> Result<Match> {
        self.runtime
            .block_on(self.inner.expect_timeout(pattern, timeout))
    }

    /// Get the current buffer contents.
    #[must_use]
    pub fn buffer(&mut self) -> String {
        self.inner.buffer()
    }

    /// Clear the buffer.
    pub fn clear_buffer(&mut self) {
        self.inner.clear_buffer();
    }

    /// Resize the terminal.
    ///
    /// # Errors
    ///
    /// Returns an error if the resize fails.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        self.runtime.block_on(self.inner.resize_pty(cols, rows))
    }

    /// Send a signal to the child process.
    ///
    /// # Errors
    ///
    /// Returns an error if sending the signal fails.
    pub fn signal(&self, signal: i32) -> Result<()> {
        self.inner.signal(signal)
    }

    /// Kill the child process.
    ///
    /// # Errors
    ///
    /// Returns an error if killing fails.
    pub fn kill(&self) -> Result<()> {
        self.inner.kill()
    }

    /// Run an async operation synchronously.
    pub fn block_on<F, T>(&self, future: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        self.runtime.block_on(future)
    }
}

#[cfg(windows)]
impl SyncSession {
    /// Spawn a command and create a session.
    ///
    /// # Errors
    ///
    /// Returns an error if spawning fails.
    pub fn spawn(command: &str, args: &[&str]) -> Result<Self> {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| crate::error::ExpectError::io_context("creating tokio runtime", e))?;

        let inner = runtime.block_on(Session::spawn(command, args))?;

        Ok(Self { runtime, inner })
    }

    /// Spawn with custom configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if spawning fails.
    pub fn spawn_with_config(command: &str, args: &[&str], config: SessionConfig) -> Result<Self> {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| crate::error::ExpectError::io_context("creating tokio runtime", e))?;

        let inner = runtime.block_on(Session::spawn_with_config(command, args, config))?;

        Ok(Self { runtime, inner })
    }

    /// Get the session configuration.
    #[must_use]
    pub const fn config(&self) -> &SessionConfig {
        self.inner.config()
    }

    /// Check if the session is active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        !self.inner.is_eof()
    }

    /// Get the child process ID.
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.inner.pid()
    }

    /// Send bytes to the session.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub fn send(&mut self, data: &[u8]) -> Result<()> {
        self.runtime.block_on(self.inner.send(data))
    }

    /// Send a string to the session.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub fn send_str(&mut self, s: &str) -> Result<()> {
        self.runtime.block_on(self.inner.send_str(s))
    }

    /// Send a line to the session.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub fn send_line(&mut self, line: &str) -> Result<()> {
        self.runtime.block_on(self.inner.send_line(line))
    }

    /// Send a control character.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub fn send_control(&mut self, ctrl: ControlChar) -> Result<()> {
        self.runtime.block_on(self.inner.send_control(ctrl))
    }

    /// Expect a pattern in the output.
    ///
    /// # Errors
    ///
    /// Returns an error on timeout or EOF.
    pub fn expect(&mut self, pattern: impl Into<Pattern>) -> Result<Match> {
        self.runtime.block_on(self.inner.expect(pattern))
    }

    /// Expect a pattern with a specific timeout.
    ///
    /// # Errors
    ///
    /// Returns an error on timeout or EOF.
    pub fn expect_timeout(
        &mut self,
        pattern: impl Into<Pattern>,
        timeout: Duration,
    ) -> Result<Match> {
        self.runtime
            .block_on(self.inner.expect_timeout(pattern, timeout))
    }

    /// Get the current buffer contents.
    #[must_use]
    pub fn buffer(&mut self) -> String {
        self.inner.buffer()
    }

    /// Clear the buffer.
    pub fn clear_buffer(&mut self) {
        self.inner.clear_buffer();
    }

    /// Resize the terminal.
    ///
    /// # Errors
    ///
    /// Returns an error if the resize fails.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        self.runtime.block_on(self.inner.resize_pty(cols, rows))
    }

    /// Check if the child process is still running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.inner.is_running()
    }

    /// Kill the child process.
    ///
    /// # Errors
    ///
    /// Returns an error if killing fails.
    pub fn kill(&self) -> Result<()> {
        self.inner.kill()
    }

    /// Run an async operation synchronously.
    pub fn block_on<F, T>(&self, future: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        self.runtime.block_on(future)
    }
}

impl std::fmt::Debug for SyncSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncSession").finish_non_exhaustive()
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
        let timeout = session.config().timeout.default;
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
        .map_err(|e| {
            crate::error::ExpectError::io_context("creating tokio runtime for block_on", e)
        })?;

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
    fn block_on_simple() {
        let result = block_on(async { 42 });
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[cfg(unix)]
    #[test]
    fn sync_session_spawn_echo() {
        let mut session =
            SyncSession::spawn("/bin/echo", &["hello"]).expect("Failed to spawn echo");

        // Verify PID is valid
        assert!(session.pid() > 0);

        // Expect the output
        let m = session.expect("hello").expect("Failed to expect hello");
        assert!(m.matched.contains("hello"));
    }
}
