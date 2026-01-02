//! Interaction mode handling.

use std::time::Duration;

/// Interaction mode configuration.
#[derive(Debug, Clone)]
pub struct InteractionMode {
    /// Whether to echo input locally.
    pub local_echo: bool,
    /// Whether to translate CR to CRLF.
    pub crlf: bool,
    /// Input buffer size.
    pub buffer_size: usize,
    /// Read timeout.
    pub read_timeout: Duration,
    /// Exit character (e.g., Ctrl+]).
    pub exit_char: Option<u8>,
    /// Escape character for commands.
    pub escape_char: Option<u8>,
}

impl Default for InteractionMode {
    fn default() -> Self {
        Self {
            local_echo: false,
            crlf: true,
            buffer_size: 4096,
            read_timeout: Duration::from_millis(100),
            exit_char: Some(0x1d), // Ctrl+]
            escape_char: Some(0x1e), // Ctrl+^
        }
    }
}

impl InteractionMode {
    /// Create a new mode with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable local echo.
    #[must_use]
    pub const fn with_local_echo(mut self, echo: bool) -> Self {
        self.local_echo = echo;
        self
    }

    /// Enable CRLF translation.
    #[must_use]
    pub const fn with_crlf(mut self, crlf: bool) -> Self {
        self.crlf = crlf;
        self
    }

    /// Set buffer size.
    #[must_use]
    pub const fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    /// Set read timeout.
    #[must_use]
    pub const fn with_read_timeout(mut self, timeout: Duration) -> Self {
        self.read_timeout = timeout;
        self
    }

    /// Set exit character.
    #[must_use]
    pub const fn with_exit_char(mut self, ch: Option<u8>) -> Self {
        self.exit_char = ch;
        self
    }

    /// Set escape character.
    #[must_use]
    pub const fn with_escape_char(mut self, ch: Option<u8>) -> Self {
        self.escape_char = ch;
        self
    }

    /// Check if a character is the exit character.
    #[must_use]
    pub fn is_exit_char(&self, ch: u8) -> bool {
        self.exit_char == Some(ch)
    }

    /// Check if a character is the escape character.
    #[must_use]
    pub fn is_escape_char(&self, ch: u8) -> bool {
        self.escape_char == Some(ch)
    }
}

/// Input filter for processing user input.
#[derive(Debug, Clone, Default)]
pub struct InputFilter {
    /// Characters to filter out.
    pub filter_chars: Vec<u8>,
    /// Whether to allow control characters.
    pub allow_control: bool,
    /// Whether to strip high bit.
    pub strip_high_bit: bool,
}

impl InputFilter {
    /// Create a new filter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add characters to filter.
    #[must_use]
    pub fn filter(mut self, chars: &[u8]) -> Self {
        self.filter_chars.extend_from_slice(chars);
        self
    }

    /// Allow control characters.
    #[must_use]
    pub const fn with_control(mut self, allow: bool) -> Self {
        self.allow_control = allow;
        self
    }

    /// Apply filter to input.
    #[must_use]
    pub fn apply(&self, input: &[u8]) -> Vec<u8> {
        input
            .iter()
            .copied()
            .filter(|&b| !self.filter_chars.contains(&b))
            .filter(|&b| self.allow_control || b >= 0x20 || b == b'\r' || b == b'\n' || b == b'\t')
            .map(|b| if self.strip_high_bit { b & 0x7f } else { b })
            .collect()
    }
}

/// Output filter for processing session output.
#[derive(Debug, Clone, Default)]
pub struct OutputFilter {
    /// Whether to strip ANSI sequences.
    pub strip_ansi: bool,
    /// Whether to convert CRLF to LF.
    pub normalize_newlines: bool,
    /// Whether to strip null bytes.
    pub strip_nulls: bool,
}

impl OutputFilter {
    /// Create a new filter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Strip ANSI sequences.
    #[must_use]
    pub const fn with_strip_ansi(mut self, strip: bool) -> Self {
        self.strip_ansi = strip;
        self
    }

    /// Normalize newlines.
    #[must_use]
    pub const fn with_normalize_newlines(mut self, normalize: bool) -> Self {
        self.normalize_newlines = normalize;
        self
    }

    /// Apply filter to output.
    #[must_use]
    pub fn apply(&self, output: &[u8]) -> Vec<u8> {
        let mut result: Vec<u8> = output
            .iter()
            .copied()
            .filter(|&b| !self.strip_nulls || b != 0)
            .collect();

        if self.normalize_newlines {
            // Replace CRLF with LF
            let mut i = 0;
            let mut normalized = Vec::with_capacity(result.len());
            while i < result.len() {
                if i + 1 < result.len() && result[i] == b'\r' && result[i + 1] == b'\n' {
                    normalized.push(b'\n');
                    i += 2;
                } else {
                    normalized.push(result[i]);
                    i += 1;
                }
            }
            result = normalized;
        }

        if self.strip_ansi {
            result = crate::util::bytes::strip_ansi(&result);
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_defaults() {
        let mode = InteractionMode::new();
        assert!(!mode.local_echo);
        assert!(mode.crlf);
    }

    #[test]
    fn input_filter() {
        let filter = InputFilter::new().filter(b"x");
        let result = filter.apply(b"text");
        assert_eq!(result, b"tet");
    }

    #[test]
    fn output_normalize_newlines() {
        let filter = OutputFilter::new().with_normalize_newlines(true);
        let result = filter.apply(b"line1\r\nline2\r\n");
        assert_eq!(result, b"line1\nline2\n");
    }
}
