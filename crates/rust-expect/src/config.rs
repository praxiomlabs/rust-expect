//! Configuration types for rust-expect.
//!
//! This module defines configuration structures for sessions, timeouts,
//! logging, and other customizable behavior.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Default timeout duration (30 seconds).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Default buffer size (100 MB).
pub const DEFAULT_BUFFER_SIZE: usize = 100 * 1024 * 1024;

/// Default terminal width.
pub const DEFAULT_TERMINAL_WIDTH: u16 = 80;

/// Default terminal height.
pub const DEFAULT_TERMINAL_HEIGHT: u16 = 24;

/// Default TERM environment variable value.
pub const DEFAULT_TERM: &str = "xterm-256color";

/// Default delay before send operations.
pub const DEFAULT_DELAY_BEFORE_SEND: Duration = Duration::from_millis(50);

/// Configuration for a session.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// The command to execute.
    pub command: String,

    /// Command arguments.
    pub args: Vec<String>,

    /// Environment variables to set.
    pub env: HashMap<String, String>,

    /// Whether to inherit the parent environment.
    pub inherit_env: bool,

    /// Working directory for the process.
    pub working_dir: Option<PathBuf>,

    /// Terminal dimensions (width, height).
    pub dimensions: (u16, u16),

    /// Timeout configuration.
    pub timeout: TimeoutConfig,

    /// Buffer configuration.
    pub buffer: BufferConfig,

    /// Line ending configuration.
    pub line_ending: LineEnding,

    /// Delay before send operations.
    pub delay_before_send: Duration,
}

impl Default for SessionConfig {
    fn default() -> Self {
        let mut env = HashMap::new();
        env.insert("TERM".to_string(), DEFAULT_TERM.to_string());

        Self {
            command: String::new(),
            args: Vec::new(),
            env,
            inherit_env: true,
            working_dir: None,
            dimensions: (DEFAULT_TERMINAL_WIDTH, DEFAULT_TERMINAL_HEIGHT),
            timeout: TimeoutConfig::default(),
            buffer: BufferConfig::default(),
            line_ending: LineEnding::default(),
            delay_before_send: DEFAULT_DELAY_BEFORE_SEND,
        }
    }
}

impl SessionConfig {
    /// Create a new session configuration with the given command.
    #[must_use]
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            ..Default::default()
        }
    }

    /// Set the command arguments.
    #[must_use]
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    /// Add an environment variable.
    #[must_use]
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Set whether to inherit the parent environment.
    #[must_use]
    pub const fn inherit_env(mut self, inherit: bool) -> Self {
        self.inherit_env = inherit;
        self
    }

    /// Set the working directory.
    #[must_use]
    pub fn working_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(path.into());
        self
    }

    /// Set the terminal dimensions.
    #[must_use]
    pub const fn dimensions(mut self, width: u16, height: u16) -> Self {
        self.dimensions = (width, height);
        self
    }

    /// Set the default timeout.
    #[must_use]
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout.default = timeout;
        self
    }

    /// Set the line ending style.
    #[must_use]
    pub const fn line_ending(mut self, line_ending: LineEnding) -> Self {
        self.line_ending = line_ending;
        self
    }

    /// Set the delay before send operations.
    #[must_use]
    pub const fn delay_before_send(mut self, delay: Duration) -> Self {
        self.delay_before_send = delay;
        self
    }
}

/// Configuration for timeouts.
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    /// Default timeout for expect operations.
    pub default: Duration,

    /// Timeout for spawn operations.
    pub spawn: Duration,

    /// Timeout for close operations.
    pub close: Duration,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            default: DEFAULT_TIMEOUT,
            spawn: Duration::from_secs(60),
            close: Duration::from_secs(10),
        }
    }
}

impl TimeoutConfig {
    /// Create a new timeout configuration with the given default timeout.
    #[must_use]
    pub fn new(default: Duration) -> Self {
        Self {
            default,
            ..Default::default()
        }
    }

    /// Set the spawn timeout.
    #[must_use]
    pub const fn spawn(mut self, timeout: Duration) -> Self {
        self.spawn = timeout;
        self
    }

    /// Set the close timeout.
    #[must_use]
    pub const fn close(mut self, timeout: Duration) -> Self {
        self.close = timeout;
        self
    }
}

/// Configuration for the output buffer.
#[derive(Debug, Clone)]
pub struct BufferConfig {
    /// Maximum buffer size in bytes.
    pub max_size: usize,

    /// Size of the search window for pattern matching.
    pub search_window: Option<usize>,
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            max_size: DEFAULT_BUFFER_SIZE,
            search_window: None,
        }
    }
}

impl BufferConfig {
    /// Create a new buffer configuration with the given max size.
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        Self {
            max_size,
            ..Default::default()
        }
    }

    /// Set the search window size.
    #[must_use]
    pub const fn search_window(mut self, size: usize) -> Self {
        self.search_window = Some(size);
        self
    }
}

/// Line ending styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    /// Unix-style line ending (LF).
    Lf,

    /// Windows-style line ending (CRLF).
    CrLf,

    /// Classic Mac line ending (CR).
    Cr,
}

/// The default is whatever the platform's terminal actually sends for ENTER, which
/// is not the same as the platform's text-file convention.
///
/// On Windows this is [`LineEnding::Cr`], not `CrLf`. `ConPTY` **discards a bare LF
/// entirely** — measured on Windows 11 26200.8893, a lone `\n` completes no line
/// read, queues nothing, and is not even echoed — so an LF default makes
/// [`send_line`](crate::Session::send_line) unable to submit a line at all. `\r` is
/// the byte a terminal sends for the Enter key.
///
/// `CrLf` also works today, but only because conhost happens to swallow the
/// trailing LF; that is undocumented, and against a child with `ENABLE_LINE_INPUT`
/// disabled an LF that *did* arrive would submit a second Enter. `Cr` cannot
/// double-submit on any build.
impl Default for LineEnding {
    fn default() -> Self {
        #[cfg(windows)]
        {
            Self::Cr
        }
        #[cfg(not(windows))]
        {
            Self::Lf
        }
    }
}

impl LineEnding {
    /// Get the line ending as a string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
            Self::Cr => "\r",
        }
    }

    /// Get the line ending as bytes.
    #[must_use]
    pub const fn as_bytes(self) -> &'static [u8] {
        match self {
            Self::Lf => b"\n",
            Self::CrLf => b"\r\n",
            Self::Cr => b"\r",
        }
    }

    /// Detect the appropriate line ending for the current platform.
    #[must_use]
    pub const fn platform_default() -> Self {
        if cfg!(windows) { Self::CrLf } else { Self::Lf }
    }
}

/// Configuration for human-like typing.
#[derive(Debug, Clone)]
pub struct HumanTypingConfig {
    /// Base delay between characters.
    pub base_delay: Duration,

    /// Variance in delay (random offset from base).
    pub variance: Duration,

    /// Chance of making a typo (0.0 to 1.0).
    pub typo_chance: f32,

    /// Chance of correcting a typo (0.0 to 1.0).
    pub correction_chance: f32,
}

impl Default for HumanTypingConfig {
    fn default() -> Self {
        Self {
            base_delay: Duration::from_millis(100),
            variance: Duration::from_millis(50),
            typo_chance: 0.01,
            correction_chance: 0.85,
        }
    }
}

impl HumanTypingConfig {
    /// Create a new human typing configuration.
    #[must_use]
    pub fn new(base_delay: Duration, variance: Duration) -> Self {
        Self {
            base_delay,
            variance,
            ..Default::default()
        }
    }

    /// Set the typo chance.
    #[must_use]
    pub const fn typo_chance(mut self, chance: f32) -> Self {
        self.typo_chance = chance;
        self
    }

    /// Set the correction chance.
    #[must_use]
    pub const fn correction_chance(mut self, chance: f32) -> Self {
        self.correction_chance = chance;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_config_builder() {
        let config = SessionConfig::new("bash")
            .args(["-l", "-i"])
            .env("MY_VAR", "value")
            .dimensions(120, 40)
            .timeout(Duration::from_secs(10));

        assert_eq!(config.command, "bash");
        assert_eq!(config.args, vec!["-l", "-i"]);
        assert_eq!(config.env.get("MY_VAR"), Some(&"value".to_string()));
        assert_eq!(config.dimensions, (120, 40));
        assert_eq!(config.timeout.default, Duration::from_secs(10));
    }

    #[test]
    fn line_ending_as_str() {
        assert_eq!(LineEnding::Lf.as_str(), "\n");
        assert_eq!(LineEnding::CrLf.as_str(), "\r\n");
        assert_eq!(LineEnding::Cr.as_str(), "\r");
    }

    #[test]
    fn default_config_has_term() {
        let config = SessionConfig::default();
        assert_eq!(config.env.get("TERM"), Some(&"xterm-256color".to_string()));
    }
}
