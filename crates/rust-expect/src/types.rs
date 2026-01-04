//! Common types for rust-expect.
//!
//! This module defines core types used throughout the library including
//! patterns, matches, and session state.

use std::fmt;
use std::time::Duration;

/// A match result from an expect operation.
#[derive(Debug, Clone)]
pub struct Match {
    /// The index of the pattern that matched (for multi-pattern expects).
    pub pattern_index: usize,

    /// The full text that matched.
    pub matched: String,

    /// Capture groups from regex patterns.
    pub captures: Vec<String>,

    /// Text before the match.
    pub before: String,

    /// Text after the match (remaining in buffer).
    pub after: String,
}

impl Match {
    /// Create a new match result.
    #[must_use]
    pub fn new(
        pattern_index: usize,
        matched: impl Into<String>,
        before: impl Into<String>,
        after: impl Into<String>,
    ) -> Self {
        Self {
            pattern_index,
            matched: matched.into(),
            captures: Vec::new(),
            before: before.into(),
            after: after.into(),
        }
    }

    /// Create a match with captures.
    #[must_use]
    pub fn with_captures(mut self, captures: Vec<String>) -> Self {
        self.captures = captures;
        self
    }

    /// Get a capture group by index.
    #[must_use]
    pub fn capture(&self, index: usize) -> Option<&str> {
        self.captures.get(index).map(String::as_str)
    }

    /// Get the full matched text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.matched
    }
}

impl fmt::Display for Match {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.matched)
    }
}

/// Result of an expect operation with multiple patterns.
#[derive(Debug, Clone)]
pub enum ExpectResult {
    /// A pattern matched.
    Matched(Match),

    /// End of file was reached.
    Eof {
        /// Buffer contents when EOF was reached.
        buffer: String,
    },

    /// Timeout occurred.
    Timeout {
        /// The duration that elapsed.
        duration: Duration,
        /// Buffer contents at timeout.
        buffer: String,
    },
}

impl ExpectResult {
    /// Check if this is a successful match.
    #[must_use]
    pub const fn is_match(&self) -> bool {
        matches!(self, Self::Matched(_))
    }

    /// Check if this is an EOF.
    #[must_use]
    pub const fn is_eof(&self) -> bool {
        matches!(self, Self::Eof { .. })
    }

    /// Check if this is a timeout.
    #[must_use]
    pub const fn is_timeout(&self) -> bool {
        matches!(self, Self::Timeout { .. })
    }

    /// Get the match if this is a successful match.
    #[must_use]
    pub fn into_match(self) -> Option<Match> {
        match self {
            Self::Matched(m) => Some(m),
            _ => None,
        }
    }

    /// Get the buffer contents (for EOF or timeout).
    #[must_use]
    pub fn buffer(&self) -> Option<&str> {
        match self {
            Self::Eof { buffer } | Self::Timeout { buffer, .. } => Some(buffer),
            Self::Matched(_) => None,
        }
    }
}

/// The state of a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Session is starting up.
    Starting,

    /// Session is running and ready for operations.
    Running,

    /// Session is in interact mode.
    Interacting,

    /// Session is closing.
    Closing,

    /// Session is closed.
    Closed,

    /// Process has exited with status.
    Exited(ProcessExitStatus),
}

impl SessionState {
    /// Check if the session is usable for operations.
    #[must_use]
    pub const fn is_usable(&self) -> bool {
        matches!(self, Self::Running | Self::Interacting)
    }

    /// Check if the session is closed or exited.
    #[must_use]
    pub const fn is_closed(&self) -> bool {
        matches!(self, Self::Closed | Self::Exited(_))
    }

    /// Get the exit status if the session has exited.
    #[must_use]
    pub const fn exit_status(&self) -> Option<&ProcessExitStatus> {
        if let Self::Exited(status) = self {
            Some(status)
        } else {
            None
        }
    }
}

impl fmt::Display for SessionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Starting => "starting".to_string(),
            Self::Running => "running".to_string(),
            Self::Interacting => "interacting".to_string(),
            Self::Closing => "closing".to_string(),
            Self::Closed => "closed".to_string(),
            Self::Exited(status) => format!("exited ({status})"),
        };
        write!(f, "{s}")
    }
}

/// Exit status of a process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessExitStatus {
    /// Process exited with a code.
    Exited(i32),

    /// Process was terminated by a signal (Unix).
    Signaled(i32),

    /// Exit status is unknown.
    Unknown,
}

impl ProcessExitStatus {
    /// Check if the process exited successfully (code 0).
    #[must_use]
    pub const fn success(self) -> bool {
        matches!(self, Self::Exited(0))
    }

    /// Get the exit code if the process exited normally.
    #[must_use]
    pub const fn code(self) -> Option<i32> {
        match self {
            Self::Exited(code) => Some(code),
            _ => None,
        }
    }

    /// Get the signal number if the process was signaled.
    #[must_use]
    pub const fn signal(self) -> Option<i32> {
        match self {
            Self::Signaled(sig) => Some(sig),
            _ => None,
        }
    }
}

impl fmt::Display for ProcessExitStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exited(code) => write!(f, "exited with code {code}"),
            Self::Signaled(sig) => write!(f, "terminated by signal {sig}"),
            Self::Unknown => write!(f, "unknown exit status"),
        }
    }
}

impl From<std::process::ExitStatus> for ProcessExitStatus {
    fn from(status: std::process::ExitStatus) -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            if let Some(code) = status.code() {
                Self::Exited(code)
            } else if let Some(sig) = status.signal() {
                Self::Signaled(sig)
            } else {
                Self::Unknown
            }
        }

        #[cfg(not(unix))]
        {
            if let Some(code) = status.code() {
                Self::Exited(code)
            } else {
                Self::Unknown
            }
        }
    }
}

/// Terminal dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dimensions {
    /// Width in columns.
    pub cols: u16,

    /// Height in rows.
    pub rows: u16,
}

impl Dimensions {
    /// Create new dimensions.
    #[must_use]
    pub const fn new(cols: u16, rows: u16) -> Self {
        Self { cols, rows }
    }

    /// Standard 80x24 terminal.
    pub const STANDARD: Self = Self::new(80, 24);

    /// Wide terminal (120x40).
    pub const WIDE: Self = Self::new(120, 40);
}

impl Default for Dimensions {
    fn default() -> Self {
        Self::STANDARD
    }
}

impl From<(u16, u16)> for Dimensions {
    fn from((cols, rows): (u16, u16)) -> Self {
        Self::new(cols, rows)
    }
}

impl From<Dimensions> for (u16, u16) {
    fn from(dim: Dimensions) -> Self {
        (dim.cols, dim.rows)
    }
}

/// Control characters that can be sent to a terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlChar {
    /// Ctrl+A (SOH)
    CtrlA,
    /// Ctrl+B (STX)
    CtrlB,
    /// Ctrl+C (ETX) - Interrupt
    CtrlC,
    /// Ctrl+D (EOT) - End of transmission / EOF
    CtrlD,
    /// Ctrl+E (ENQ)
    CtrlE,
    /// Ctrl+F (ACK)
    CtrlF,
    /// Ctrl+G (BEL) - Bell
    CtrlG,
    /// Ctrl+H (BS) - Backspace
    CtrlH,
    /// Ctrl+I (HT) - Tab
    CtrlI,
    /// Ctrl+J (LF) - Line feed
    CtrlJ,
    /// Ctrl+K (VT) - Vertical tab
    CtrlK,
    /// Ctrl+L (FF) - Form feed / Clear screen
    CtrlL,
    /// Ctrl+M (CR) - Carriage return
    CtrlM,
    /// Ctrl+N (SO)
    CtrlN,
    /// Ctrl+O (SI)
    CtrlO,
    /// Ctrl+P (DLE)
    CtrlP,
    /// Ctrl+Q (DC1) - XON / Resume
    CtrlQ,
    /// Ctrl+R (DC2)
    CtrlR,
    /// Ctrl+S (DC3) - XOFF / Pause
    CtrlS,
    /// Ctrl+T (DC4)
    CtrlT,
    /// Ctrl+U (NAK) - Kill line
    CtrlU,
    /// Ctrl+V (SYN)
    CtrlV,
    /// Ctrl+W (ETB) - Kill word
    CtrlW,
    /// Ctrl+X (CAN)
    CtrlX,
    /// Ctrl+Y (EM)
    CtrlY,
    /// Ctrl+Z (SUB) - Suspend
    CtrlZ,
    /// Escape
    Escape,
    /// Ctrl+\ (FS) - Quit
    CtrlBackslash,
    /// Ctrl+] (GS)
    CtrlBracket,
    /// Ctrl+^ (RS)
    CtrlCaret,
    /// Ctrl+_ (US)
    CtrlUnderscore,
}

impl ControlChar {
    /// Get the byte value of this control character.
    #[must_use]
    pub const fn as_byte(self) -> u8 {
        match self {
            Self::CtrlA => 0x01,
            Self::CtrlB => 0x02,
            Self::CtrlC => 0x03,
            Self::CtrlD => 0x04,
            Self::CtrlE => 0x05,
            Self::CtrlF => 0x06,
            Self::CtrlG => 0x07,
            Self::CtrlH => 0x08,
            Self::CtrlI => 0x09,
            Self::CtrlJ => 0x0A,
            Self::CtrlK => 0x0B,
            Self::CtrlL => 0x0C,
            Self::CtrlM => 0x0D,
            Self::CtrlN => 0x0E,
            Self::CtrlO => 0x0F,
            Self::CtrlP => 0x10,
            Self::CtrlQ => 0x11,
            Self::CtrlR => 0x12,
            Self::CtrlS => 0x13,
            Self::CtrlT => 0x14,
            Self::CtrlU => 0x15,
            Self::CtrlV => 0x16,
            Self::CtrlW => 0x17,
            Self::CtrlX => 0x18,
            Self::CtrlY => 0x19,
            Self::CtrlZ => 0x1A,
            Self::Escape => 0x1B,
            Self::CtrlBackslash => 0x1C,
            Self::CtrlBracket => 0x1D,
            Self::CtrlCaret => 0x1E,
            Self::CtrlUnderscore => 0x1F,
        }
    }

    /// Create a control character from a regular character.
    ///
    /// For example, `ControlChar::from_char('c')` returns `Some(ControlChar::CtrlC)`.
    #[must_use]
    pub const fn from_char(c: char) -> Option<Self> {
        match c.to_ascii_lowercase() {
            'a' => Some(Self::CtrlA),
            'b' => Some(Self::CtrlB),
            'c' => Some(Self::CtrlC),
            'd' => Some(Self::CtrlD),
            'e' => Some(Self::CtrlE),
            'f' => Some(Self::CtrlF),
            'g' => Some(Self::CtrlG),
            'h' => Some(Self::CtrlH),
            'i' => Some(Self::CtrlI),
            'j' => Some(Self::CtrlJ),
            'k' => Some(Self::CtrlK),
            'l' => Some(Self::CtrlL),
            'm' => Some(Self::CtrlM),
            'n' => Some(Self::CtrlN),
            'o' => Some(Self::CtrlO),
            'p' => Some(Self::CtrlP),
            'q' => Some(Self::CtrlQ),
            'r' => Some(Self::CtrlR),
            's' => Some(Self::CtrlS),
            't' => Some(Self::CtrlT),
            'u' => Some(Self::CtrlU),
            'v' => Some(Self::CtrlV),
            'w' => Some(Self::CtrlW),
            'x' => Some(Self::CtrlX),
            'y' => Some(Self::CtrlY),
            'z' => Some(Self::CtrlZ),
            '[' => Some(Self::Escape),
            '\\' => Some(Self::CtrlBackslash),
            ']' => Some(Self::CtrlBracket),
            '^' => Some(Self::CtrlCaret),
            '_' => Some(Self::CtrlUnderscore),
            _ => None,
        }
    }
}

impl From<ControlChar> for u8 {
    fn from(c: ControlChar) -> Self {
        c.as_byte()
    }
}

impl From<ControlChar> for char {
    fn from(c: ControlChar) -> Self {
        c.as_byte() as Self
    }
}

/// A unique identifier for a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(u64);

impl SessionId {
    /// Create a new session ID.
    #[must_use]
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }

    /// Get the inner value.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "session-{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_creation() {
        let m =
            Match::new(0, "hello", "before ", " after").with_captures(vec!["capture1".to_string()]);

        assert_eq!(m.pattern_index, 0);
        assert_eq!(m.as_str(), "hello");
        assert_eq!(m.before, "before ");
        assert_eq!(m.after, " after");
        assert_eq!(m.capture(0), Some("capture1"));
        assert_eq!(m.capture(1), None);
    }

    #[test]
    fn session_state_checks() {
        assert!(SessionState::Running.is_usable());
        assert!(SessionState::Interacting.is_usable());
        assert!(!SessionState::Closed.is_usable());

        assert!(SessionState::Closed.is_closed());
        assert!(SessionState::Exited(ProcessExitStatus::Unknown).is_closed());
        assert!(!SessionState::Running.is_closed());
    }

    #[test]
    fn process_exit_status() {
        let success = ProcessExitStatus::Exited(0);
        assert!(success.success());
        assert_eq!(success.code(), Some(0));

        let failure = ProcessExitStatus::Exited(1);
        assert!(!failure.success());
        assert_eq!(failure.code(), Some(1));

        let signaled = ProcessExitStatus::Signaled(9);
        assert!(!signaled.success());
        assert_eq!(signaled.signal(), Some(9));
    }

    #[test]
    fn control_char_from_char() {
        assert_eq!(ControlChar::from_char('c'), Some(ControlChar::CtrlC));
        assert_eq!(ControlChar::from_char('C'), Some(ControlChar::CtrlC));
        assert_eq!(ControlChar::from_char('d'), Some(ControlChar::CtrlD));
        assert_eq!(ControlChar::from_char('?'), None);
    }

    #[test]
    fn control_char_as_byte() {
        assert_eq!(ControlChar::CtrlC.as_byte(), 0x03);
        assert_eq!(ControlChar::CtrlD.as_byte(), 0x04);
        assert_eq!(ControlChar::Escape.as_byte(), 0x1B);
    }

    #[test]
    fn session_id_unique() {
        let id1 = SessionId::new();
        let id2 = SessionId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn dimensions_conversion() {
        let dim = Dimensions::new(120, 40);
        let tuple: (u16, u16) = dim.into();
        assert_eq!(tuple, (120, 40));

        let dim2: Dimensions = (80, 24).into();
        assert_eq!(dim2, Dimensions::STANDARD);
    }
}
