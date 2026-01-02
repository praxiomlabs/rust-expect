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

    /// Logging configuration.
    pub logging: LoggingConfig,

    /// Line ending configuration.
    pub line_ending: LineEnding,

    /// Encoding configuration.
    pub encoding: EncodingConfig,

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
            logging: LoggingConfig::default(),
            line_ending: LineEnding::default(),
            encoding: EncodingConfig::default(),
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

    /// Whether to use a ring buffer (discard oldest data when full).
    pub ring_buffer: bool,
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            max_size: DEFAULT_BUFFER_SIZE,
            search_window: None,
            ring_buffer: true,
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

    /// Set whether to use a ring buffer.
    #[must_use]
    pub const fn ring_buffer(mut self, enabled: bool) -> Self {
        self.ring_buffer = enabled;
        self
    }
}

/// Configuration for logging.
#[derive(Debug, Clone, Default)]
pub struct LoggingConfig {
    /// Path to log file.
    pub log_file: Option<PathBuf>,

    /// Whether to echo output to stdout.
    pub log_user: bool,

    /// Log format.
    pub format: LogFormat,

    /// Whether to log sent data separately from received data.
    pub separate_io: bool,

    /// Patterns to redact from logs.
    pub redact_patterns: Vec<String>,
}

impl LoggingConfig {
    /// Create a new logging configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the log file path.
    #[must_use]
    pub fn log_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.log_file = Some(path.into());
        self
    }

    /// Set whether to echo to stdout.
    #[must_use]
    pub const fn log_user(mut self, enabled: bool) -> Self {
        self.log_user = enabled;
        self
    }

    /// Set the log format.
    #[must_use]
    pub const fn format(mut self, format: LogFormat) -> Self {
        self.format = format;
        self
    }

    /// Add a pattern to redact from logs.
    #[must_use]
    pub fn redact(mut self, pattern: impl Into<String>) -> Self {
        self.redact_patterns.push(pattern.into());
        self
    }
}

/// Log format options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum LogFormat {
    /// Raw output (no formatting).
    #[default]
    Raw,

    /// Timestamped output.
    Timestamped,

    /// Newline-delimited JSON.
    Ndjson,

    /// Asciicast v2 format (asciinema compatible).
    Asciicast,
}

/// Line ending styles.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LineEnding {
    /// Unix-style line ending (LF).
    #[default]
    Lf,

    /// Windows-style line ending (CRLF).
    CrLf,

    /// Classic Mac line ending (CR).
    Cr,
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
        if cfg!(windows) {
            Self::CrLf
        } else {
            Self::Lf
        }
    }
}

/// Configuration for text encoding.
#[derive(Debug, Clone)]
pub struct EncodingConfig {
    /// The encoding to use (default: UTF-8).
    pub encoding: Encoding,

    /// How to handle invalid sequences.
    pub error_handling: EncodingErrorHandling,

    /// Whether to normalize line endings.
    pub normalize_line_endings: bool,
}

impl Default for EncodingConfig {
    fn default() -> Self {
        Self {
            encoding: Encoding::Utf8,
            error_handling: EncodingErrorHandling::Replace,
            normalize_line_endings: false,
        }
    }
}

impl EncodingConfig {
    /// Create a new encoding configuration.
    #[must_use]
    pub fn new(encoding: Encoding) -> Self {
        Self {
            encoding,
            ..Default::default()
        }
    }

    /// Set the error handling mode.
    #[must_use]
    pub const fn error_handling(mut self, mode: EncodingErrorHandling) -> Self {
        self.error_handling = mode;
        self
    }

    /// Set whether to normalize line endings.
    #[must_use]
    pub const fn normalize_line_endings(mut self, normalize: bool) -> Self {
        self.normalize_line_endings = normalize;
        self
    }
}

/// Supported text encodings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Encoding {
    /// UTF-8 encoding.
    #[default]
    Utf8,

    /// Raw bytes (no encoding).
    Raw,

    /// ISO-8859-1 (Latin-1).
    #[cfg(feature = "legacy-encoding")]
    Latin1,

    /// Windows-1252.
    #[cfg(feature = "legacy-encoding")]
    Windows1252,
}

/// How to handle encoding errors.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EncodingErrorHandling {
    /// Replace invalid sequences with the replacement character.
    #[default]
    Replace,

    /// Skip invalid sequences.
    Skip,

    /// Return an error on invalid sequences.
    Strict,

    /// Escape invalid bytes as hex.
    Escape,
}

/// Configuration for interact mode.
#[derive(Debug, Clone)]
pub struct InteractConfig {
    /// Escape character to exit interact mode.
    pub escape_char: Option<char>,

    /// Timeout for idle detection.
    pub idle_timeout: Option<Duration>,

    /// Whether to echo input.
    pub echo: bool,

    /// Output hooks.
    pub output_hooks: Vec<InteractHook>,

    /// Input hooks.
    pub input_hooks: Vec<InteractHook>,
}

impl Default for InteractConfig {
    fn default() -> Self {
        Self {
            escape_char: Some('\x1d'), // Ctrl+]
            idle_timeout: None,
            echo: true,
            output_hooks: Vec::new(),
            input_hooks: Vec::new(),
        }
    }
}

impl InteractConfig {
    /// Create a new interact configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the escape character.
    #[must_use]
    pub const fn escape_char(mut self, c: char) -> Self {
        self.escape_char = Some(c);
        self
    }

    /// Disable the escape character.
    #[must_use]
    pub const fn no_escape(mut self) -> Self {
        self.escape_char = None;
        self
    }

    /// Set the idle timeout.
    #[must_use]
    pub const fn idle_timeout(mut self, timeout: Duration) -> Self {
        self.idle_timeout = Some(timeout);
        self
    }

    /// Set whether to echo input.
    #[must_use]
    pub const fn echo(mut self, enabled: bool) -> Self {
        self.echo = enabled;
        self
    }
}

/// A hook for interact mode.
#[derive(Debug, Clone)]
pub struct InteractHook {
    /// The pattern to match.
    pub pattern: String,

    /// Whether this is a regex pattern.
    pub is_regex: bool,
}

impl InteractHook {
    /// Create a new interact hook with a literal pattern.
    #[must_use]
    pub fn literal(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            is_regex: false,
        }
    }

    /// Create a new interact hook with a regex pattern.
    #[must_use]
    pub fn regex(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            is_regex: true,
        }
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

    #[test]
    fn logging_config_builder() {
        let config = LoggingConfig::new()
            .log_file("/tmp/session.log")
            .log_user(true)
            .format(LogFormat::Ndjson)
            .redact("password");

        assert_eq!(config.log_file, Some(PathBuf::from("/tmp/session.log")));
        assert!(config.log_user);
        assert_eq!(config.format, LogFormat::Ndjson);
        assert_eq!(config.redact_patterns, vec!["password"]);
    }
}
