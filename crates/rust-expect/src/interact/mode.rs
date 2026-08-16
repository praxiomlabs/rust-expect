//! Interaction mode handling.

use std::time::Duration;

/// How the interaction loop treats the terminal.
///
/// Set with [`InteractBuilder::with_mode`](crate::interact::InteractBuilder::with_mode).
/// The escape sequence that ends an interaction is not here — it is
/// [`InteractBuilder::with_escape`](crate::interact::InteractBuilder::with_escape).
#[derive(Debug, Clone)]
pub struct InteractionMode {
    /// Write each keystroke back to the terminal as it is typed (default off).
    ///
    /// A PTY child in cooked mode echoes for itself, so this doubles every
    /// character there. It is for a child that has turned echo off, or a
    /// transport with no terminal behind it — the only way the user sees what
    /// they type.
    pub local_echo: bool,
    /// Rewrite a bare `\n` from the child as `\r\n` on the way to the terminal
    /// (default on).
    ///
    /// A raw-mode terminal does no output translation of its own, so a child
    /// whose newlines arrive as bare LF stair-steps down the screen. A PTY has
    /// already done this (ONLCR), and an existing `\r\n` is left alone, so the
    /// setting is a no-op there and only bites on transports that pass bare
    /// LF through. It changes what the terminal shows, not what the session
    /// buffers or what output hooks and patterns see.
    pub crlf: bool,
    /// How long one terminal read may wait before the loop goes round again.
    pub read_timeout: Duration,
}

impl Default for InteractionMode {
    fn default() -> Self {
        Self {
            local_echo: false,
            crlf: true,
            read_timeout: Duration::from_millis(100),
        }
    }
}

impl InteractionMode {
    /// Create a new mode with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Echo keystrokes to the terminal. See [`local_echo`](Self::local_echo).
    #[must_use]
    pub const fn with_local_echo(mut self, echo: bool) -> Self {
        self.local_echo = echo;
        self
    }

    /// Rewrite bare LF as CRLF for the terminal. See [`crlf`](Self::crlf).
    #[must_use]
    pub const fn with_crlf(mut self, crlf: bool) -> Self {
        self.crlf = crlf;
        self
    }

    /// Set the terminal read timeout. See [`read_timeout`](Self::read_timeout).
    #[must_use]
    pub const fn with_read_timeout(mut self, timeout: Duration) -> Self {
        self.read_timeout = timeout;
        self
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
