//! Configuration types for PTY creation and management.
//!
//! This module provides [`PtyConfig`] for configuring PTY creation and
//! [`PtySignal`] for cross-platform signal representation.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

// Unix-specific imports for signal constants
#[cfg(unix)]
use libc;

/// Configuration for creating a new PTY session.
///
/// # Example
///
/// ```
/// use rust_pty::PtyConfig;
///
/// let config = PtyConfig::builder()
///     .working_directory("/home/user")
///     .env("TERM", "xterm-256color")
///     .window_size(80, 24)
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct PtyConfig {
    /// Working directory for the child process.
    pub working_directory: Option<PathBuf>,

    /// Environment variables to set for the child process.
    /// If None, inherits from the parent process.
    pub env: Option<HashMap<OsString, OsString>>,

    /// Additional environment variables to add (merged with inherited).
    pub env_add: HashMap<OsString, OsString>,

    /// Environment variables to remove from inherited environment.
    pub env_remove: Vec<OsString>,

    /// Initial window size (columns, rows).
    pub window_size: (u16, u16),

    /// Whether to create a new session (Unix setsid).
    pub new_session: bool,

    /// Timeout for spawn operation.
    pub spawn_timeout: Option<Duration>,

    /// Whether to use a controlling terminal (Unix).
    #[cfg(unix)]
    pub controlling_terminal: bool,

    /// Whether to allocate a console (Windows).
    #[cfg(windows)]
    pub allocate_console: bool,
}

impl Default for PtyConfig {
    fn default() -> Self {
        Self {
            working_directory: None,
            env: None,
            env_add: HashMap::new(),
            env_remove: Vec::new(),
            window_size: (80, 24),
            new_session: true,
            spawn_timeout: None,
            #[cfg(unix)]
            controlling_terminal: true,
            #[cfg(windows)]
            allocate_console: true,
        }
    }
}

impl PtyConfig {
    /// Create a new builder for `PtyConfig`.
    #[must_use]
    pub fn builder() -> PtyConfigBuilder {
        PtyConfigBuilder::new()
    }

    /// Create a new `PtyConfig` with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the effective environment for the child process.
    ///
    /// This merges the base environment (inherited or explicit), adds
    /// variables from `env_add`, and removes variables from `env_remove`.
    #[must_use]
    pub fn effective_env(&self) -> HashMap<OsString, OsString> {
        let mut env = self
            .env
            .clone()
            .unwrap_or_else(|| std::env::vars_os().collect());

        // Add additional variables
        env.extend(self.env_add.clone());

        // Remove specified variables
        for key in &self.env_remove {
            env.remove(key);
        }

        env
    }
}

/// Builder for [`PtyConfig`].
#[derive(Debug, Clone, Default)]
pub struct PtyConfigBuilder {
    config: PtyConfig,
}

impl PtyConfigBuilder {
    /// Create a new builder with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the working directory for the child process.
    #[must_use]
    pub fn working_directory(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.working_directory = Some(path.into());
        self
    }

    /// Set the complete environment for the child process.
    ///
    /// This replaces the inherited environment entirely.
    #[must_use]
    pub fn env_clear(mut self) -> Self {
        self.config.env = Some(HashMap::new());
        self
    }

    /// Add an environment variable.
    #[must_use]
    pub fn env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.config.env_add.insert(key.into(), value.into());
        self
    }

    /// Remove an environment variable.
    #[must_use]
    pub fn env_remove(mut self, key: impl Into<OsString>) -> Self {
        self.config.env_remove.push(key.into());
        self
    }

    /// Set the initial window size.
    #[must_use]
    pub const fn window_size(mut self, cols: u16, rows: u16) -> Self {
        self.config.window_size = (cols, rows);
        self
    }

    /// Set whether to create a new session.
    #[must_use]
    pub const fn new_session(mut self, value: bool) -> Self {
        self.config.new_session = value;
        self
    }

    /// Set the spawn timeout.
    #[must_use]
    pub const fn spawn_timeout(mut self, timeout: Duration) -> Self {
        self.config.spawn_timeout = Some(timeout);
        self
    }

    /// Set whether to use a controlling terminal (Unix only).
    #[cfg(unix)]
    #[must_use]
    pub const fn controlling_terminal(mut self, value: bool) -> Self {
        self.config.controlling_terminal = value;
        self
    }

    /// Set whether to allocate a console (Windows only).
    #[cfg(windows)]
    #[must_use]
    pub fn allocate_console(mut self, value: bool) -> Self {
        self.config.allocate_console = value;
        self
    }

    /// Build the configuration.
    #[must_use]
    pub fn build(self) -> PtyConfig {
        self.config
    }
}

/// Cross-platform signal representation.
///
/// This enum provides a unified interface for signals across Unix and Windows.
/// On Windows, signals are emulated using console events or process control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PtySignal {
    /// Interrupt signal (Ctrl+C).
    /// - Unix: SIGINT (2)
    /// - Windows: `CTRL_C_EVENT`
    Interrupt,

    /// Quit signal (Ctrl+\).
    /// - Unix: SIGQUIT (3)
    /// - Windows: Not directly supported
    Quit,

    /// Terminate signal.
    /// - Unix: SIGTERM (15)
    /// - Windows: `TerminateProcess`
    Terminate,

    /// Kill signal (cannot be caught).
    /// - Unix: SIGKILL (9)
    /// - Windows: `TerminateProcess`
    Kill,

    /// Hangup signal (terminal closed).
    /// - Unix: SIGHUP (1)
    /// - Windows: `CTRL_CLOSE_EVENT`
    Hangup,

    /// Window size change.
    /// - Unix: SIGWINCH (28)
    /// - Windows: Handled via `ConPTY` resize
    WindowChange,

    /// Stop signal (Ctrl+Z).
    /// - Unix: SIGTSTP (20)
    /// - Windows: Not supported
    #[cfg(unix)]
    Stop,

    /// Continue signal.
    /// - Unix: SIGCONT (18)
    /// - Windows: Not supported
    #[cfg(unix)]
    Continue,

    /// User-defined signal 1.
    /// - Unix: SIGUSR1 (10)
    /// - Windows: Not supported
    #[cfg(unix)]
    User1,

    /// User-defined signal 2.
    /// - Unix: SIGUSR2 (12)
    /// - Windows: Not supported
    #[cfg(unix)]
    User2,
}

impl PtySignal {
    /// Get the Unix signal number, if applicable.
    #[cfg(unix)]
    #[must_use]
    pub const fn as_unix_signal(self) -> Option<i32> {
        match self {
            Self::Interrupt => Some(libc::SIGINT),
            Self::Quit => Some(libc::SIGQUIT),
            Self::Terminate => Some(libc::SIGTERM),
            Self::Kill => Some(libc::SIGKILL),
            Self::Hangup => Some(libc::SIGHUP),
            Self::WindowChange => Some(libc::SIGWINCH),
            Self::Stop => Some(libc::SIGTSTP),
            Self::Continue => Some(libc::SIGCONT),
            Self::User1 => Some(libc::SIGUSR1),
            Self::User2 => Some(libc::SIGUSR2),
        }
    }
}

/// Window size for the PTY.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSize {
    /// Number of columns (characters per line).
    pub cols: u16,
    /// Number of rows (lines).
    pub rows: u16,
    /// Pixel width (optional, often 0).
    pub xpixel: u16,
    /// Pixel height (optional, often 0).
    pub ypixel: u16,
}

impl WindowSize {
    /// Create a new window size with the given dimensions.
    #[must_use]
    pub const fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols,
            rows,
            xpixel: 0,
            ypixel: 0,
        }
    }

    /// Create a window size with pixel dimensions.
    #[must_use]
    pub const fn with_pixels(cols: u16, rows: u16, xpixel: u16, ypixel: u16) -> Self {
        Self {
            cols,
            rows,
            xpixel,
            ypixel,
        }
    }
}

impl Default for WindowSize {
    fn default() -> Self {
        Self::new(80, 24)
    }
}

impl From<(u16, u16)> for WindowSize {
    fn from((cols, rows): (u16, u16)) -> Self {
        Self::new(cols, rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_builder() {
        let config = PtyConfig::builder()
            .working_directory("/tmp")
            .env("FOO", "bar")
            .window_size(120, 40)
            .build();

        assert_eq!(config.working_directory, Some(PathBuf::from("/tmp")));
        assert_eq!(config.window_size, (120, 40));
        assert!(config.env_add.contains_key(&OsString::from("FOO")));
    }

    #[test]
    fn window_size_default() {
        let size = WindowSize::default();
        assert_eq!(size.cols, 80);
        assert_eq!(size.rows, 24);
    }
}
