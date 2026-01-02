//! Encoding detection and conversion utilities.
//!
//! This module provides utilities for handling text encoding in terminal I/O,
//! including UTF-8 validation, encoding detection, and line ending normalization.

use std::borrow::Cow;

/// Result of encoding a byte sequence to text.
#[derive(Debug, Clone)]
pub struct EncodedText {
    /// The decoded text.
    pub text: String,
    /// Number of bytes consumed from input.
    pub bytes_consumed: usize,
    /// Whether there were any encoding errors.
    pub had_errors: bool,
    /// Number of replacement characters inserted.
    pub replacements: usize,
}

impl EncodedText {
    /// Create a successful encoding result.
    #[must_use]
    pub fn ok(text: impl Into<String>, bytes_consumed: usize) -> Self {
        Self {
            text: text.into(),
            bytes_consumed,
            had_errors: false,
            replacements: 0,
        }
    }

    /// Create an encoding result with errors.
    #[must_use]
    pub fn with_errors(text: impl Into<String>, bytes_consumed: usize, replacements: usize) -> Self {
        Self {
            text: text.into(),
            bytes_consumed,
            had_errors: replacements > 0,
            replacements,
        }
    }
}

/// Decode bytes as UTF-8, replacing invalid sequences.
///
/// This is the default behavior for rust-expect. Invalid UTF-8 sequences
/// are replaced with the Unicode replacement character (U+FFFD).
#[must_use]
pub fn decode_utf8_lossy(bytes: &[u8]) -> EncodedText {
    let text = String::from_utf8_lossy(bytes);
    let replacements = text.matches('\u{FFFD}').count();

    EncodedText {
        text: text.into_owned(),
        bytes_consumed: bytes.len(),
        had_errors: replacements > 0,
        replacements,
    }
}

/// Decode bytes as UTF-8, returning an error on invalid sequences.
///
/// # Errors
///
/// Returns an error if the input is not valid UTF-8.
pub fn decode_utf8_strict(bytes: &[u8]) -> Result<EncodedText, std::str::Utf8Error> {
    let text = std::str::from_utf8(bytes)?;
    Ok(EncodedText::ok(text, bytes.len()))
}

/// Decode bytes as UTF-8, escaping invalid bytes as hex.
///
/// Invalid bytes are replaced with `\xHH` escape sequences.
#[must_use]
pub fn decode_utf8_escape(bytes: &[u8]) -> EncodedText {
    let mut result = String::with_capacity(bytes.len());
    let mut replacements = 0;
    let mut i = 0;

    while i < bytes.len() {
        match std::str::from_utf8(&bytes[i..]) {
            Ok(valid) => {
                result.push_str(valid);
                break;
            }
            Err(e) => {
                // Add the valid prefix
                let valid_up_to = e.valid_up_to();
                if valid_up_to > 0 {
                    // Safe: from_utf8 confirmed these bytes are valid
                    result.push_str(unsafe { std::str::from_utf8_unchecked(&bytes[i..i + valid_up_to]) });
                }
                i += valid_up_to;

                // Handle the invalid byte(s)
                let error_len = e.error_len().unwrap_or(1);
                for byte in &bytes[i..i + error_len] {
                    result.push_str(&format!("\\x{byte:02x}"));
                    replacements += 1;
                }
                i += error_len;
            }
        }
    }

    EncodedText::with_errors(result, bytes.len(), replacements)
}

/// Skip invalid UTF-8 sequences.
///
/// Invalid bytes are simply removed from the output.
#[must_use]
pub fn decode_utf8_skip(bytes: &[u8]) -> EncodedText {
    let mut result = String::with_capacity(bytes.len());
    let mut replacements = 0;
    let mut i = 0;

    while i < bytes.len() {
        match std::str::from_utf8(&bytes[i..]) {
            Ok(valid) => {
                result.push_str(valid);
                break;
            }
            Err(e) => {
                let valid_up_to = e.valid_up_to();
                if valid_up_to > 0 {
                    result.push_str(unsafe { std::str::from_utf8_unchecked(&bytes[i..i + valid_up_to]) });
                }
                i += valid_up_to;

                let error_len = e.error_len().unwrap_or(1);
                replacements += error_len;
                i += error_len;
            }
        }
    }

    EncodedText::with_errors(result, bytes.len(), replacements)
}

/// Normalize line endings in text.
///
/// Converts all line endings (CRLF, CR, LF) to the specified style.
#[must_use]
pub fn normalize_line_endings(text: &str, ending: LineEndingStyle) -> Cow<'_, str> {
    let target = ending.as_str();

    // Check if normalization is needed
    let needs_crlf = text.contains("\r\n");
    let needs_cr = text.contains('\r') && !needs_crlf;
    let needs_lf = text.contains('\n') && !needs_crlf;

    // If already normalized, return as-is
    match ending {
        LineEndingStyle::Lf if !needs_crlf && !needs_cr => return Cow::Borrowed(text),
        LineEndingStyle::CrLf if needs_crlf && !needs_cr && !needs_lf => return Cow::Borrowed(text),
        LineEndingStyle::Cr if needs_cr && !needs_crlf && !needs_lf => return Cow::Borrowed(text),
        _ => {}
    }

    // First normalize all endings to LF
    let normalized = if needs_crlf {
        text.replace("\r\n", "\n")
    } else {
        text.to_string()
    };

    let normalized = if normalized.contains('\r') {
        normalized.replace('\r', "\n")
    } else {
        normalized
    };

    // Then convert to target if not LF
    let result = if target == "\n" {
        normalized
    } else {
        normalized.replace('\n', target)
    };

    Cow::Owned(result)
}

/// Line ending styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineEndingStyle {
    /// Unix-style (LF)
    #[default]
    Lf,
    /// Windows-style (CRLF)
    CrLf,
    /// Classic Mac (CR)
    Cr,
}

impl LineEndingStyle {
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

    /// Detect the line ending style from environment.
    #[must_use]
    pub const fn from_env() -> Self {
        if cfg!(windows) {
            Self::CrLf
        } else {
            Self::Lf
        }
    }
}

/// Detect the predominant line ending in text.
#[must_use]
pub fn detect_line_ending(text: &str) -> Option<LineEndingStyle> {
    let crlf_count = text.matches("\r\n").count();
    let lf_only_count = text.matches('\n').count().saturating_sub(crlf_count);
    let cr_only_count = text
        .chars()
        .zip(text.chars().skip(1).chain(std::iter::once('\0')))
        .filter(|&(c, next)| c == '\r' && next != '\n')
        .count();

    if crlf_count == 0 && lf_only_count == 0 && cr_only_count == 0 {
        return None;
    }

    if crlf_count >= lf_only_count && crlf_count >= cr_only_count {
        Some(LineEndingStyle::CrLf)
    } else if lf_only_count >= cr_only_count {
        Some(LineEndingStyle::Lf)
    } else {
        Some(LineEndingStyle::Cr)
    }
}

/// Detect encoding from environment variables.
///
/// Checks `LC_ALL`, `LC_CTYPE`, and `LANG` in order.
#[must_use]
pub fn detect_encoding_from_env() -> DetectedEncoding {
    let locale = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_CTYPE"))
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default();

    let locale_lower = locale.to_lowercase();

    if locale_lower.contains("utf-8") || locale_lower.contains("utf8") {
        DetectedEncoding::Utf8
    } else if locale_lower.contains("iso-8859-1") || locale_lower.contains("iso8859-1") {
        DetectedEncoding::Latin1
    } else if locale_lower.contains("1252") {
        DetectedEncoding::Windows1252
    } else if locale.is_empty() {
        // Default to UTF-8 for modern systems
        DetectedEncoding::Utf8
    } else {
        DetectedEncoding::Unknown(locale)
    }
}

/// Detected encoding from environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectedEncoding {
    /// UTF-8 encoding.
    Utf8,
    /// ISO-8859-1 (Latin-1).
    Latin1,
    /// Windows-1252.
    Windows1252,
    /// Unknown encoding (contains the locale string).
    Unknown(String),
}

impl DetectedEncoding {
    /// Check if this is UTF-8.
    #[must_use]
    pub const fn is_utf8(&self) -> bool {
        matches!(self, Self::Utf8)
    }
}

/// Strip ANSI escape sequences from text.
///
/// Removes all ANSI control sequences (CSI, OSC, etc.) from the input.
#[must_use]
pub fn strip_ansi(text: &str) -> Cow<'_, str> {
    // Quick check: if no escape character, return as-is
    if !text.contains('\x1b') {
        return Cow::Borrowed(text);
    }

    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Start of escape sequence
            if let Some(&next) = chars.peek() {
                match next {
                    '[' => {
                        // CSI sequence: ESC [ ... final byte
                        chars.next(); // consume '['
                        while let Some(&param) = chars.peek() {
                            if param.is_ascii_alphabetic() || param == '@' || param == '`' {
                                chars.next(); // consume final byte
                                break;
                            }
                            chars.next();
                        }
                    }
                    ']' => {
                        // OSC sequence: ESC ] ... ST or BEL
                        chars.next(); // consume ']'
                        while let Some(osc_char) = chars.next() {
                            if osc_char == '\x07' || osc_char == '\x1b' {
                                // BEL or possible ST
                                if osc_char == '\x1b' && chars.peek() == Some(&'\\') {
                                    chars.next(); // consume '\\'
                                }
                                break;
                            }
                        }
                    }
                    '(' | ')' | '*' | '+' => {
                        // Designate character set: ESC ( X
                        chars.next();
                        chars.next();
                    }
                    _ if next.is_ascii_uppercase() || next == '=' || next == '>' => {
                        // Simple escape sequence: ESC X
                        chars.next();
                    }
                    _ => {
                        // Unknown, just skip the ESC
                    }
                }
            }
        } else {
            result.push(c);
        }
    }

    Cow::Owned(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_valid_utf8() {
        let result = decode_utf8_lossy(b"hello world");
        assert_eq!(result.text, "hello world");
        assert!(!result.had_errors);
        assert_eq!(result.replacements, 0);
    }

    #[test]
    fn decode_invalid_utf8_lossy() {
        let result = decode_utf8_lossy(b"hello\xff\xfeworld");
        assert!(result.text.contains('\u{FFFD}'));
        assert!(result.had_errors);
        assert!(result.replacements > 0);
    }

    #[test]
    fn decode_invalid_utf8_escape() {
        let result = decode_utf8_escape(b"hello\xffworld");
        assert!(result.text.contains("\\xff"));
        assert!(result.had_errors);
    }

    #[test]
    fn decode_invalid_utf8_skip() {
        let result = decode_utf8_skip(b"hello\xff\xfeworld");
        assert_eq!(result.text, "helloworld");
        assert!(result.had_errors);
    }

    #[test]
    fn normalize_crlf_to_lf() {
        let text = "line1\r\nline2\r\nline3";
        let result = normalize_line_endings(text, LineEndingStyle::Lf);
        assert_eq!(result, "line1\nline2\nline3");
    }

    #[test]
    fn normalize_lf_to_crlf() {
        let text = "line1\nline2\nline3";
        let result = normalize_line_endings(text, LineEndingStyle::CrLf);
        assert_eq!(result, "line1\r\nline2\r\nline3");
    }

    #[test]
    fn detect_line_ending_lf() {
        assert_eq!(detect_line_ending("line1\nline2\n"), Some(LineEndingStyle::Lf));
    }

    #[test]
    fn detect_line_ending_crlf() {
        assert_eq!(detect_line_ending("line1\r\nline2\r\n"), Some(LineEndingStyle::CrLf));
    }

    #[test]
    fn strip_ansi_csi() {
        let text = "\x1b[32mgreen\x1b[0m text";
        let result = strip_ansi(text);
        assert_eq!(result, "green text");
    }

    #[test]
    fn strip_ansi_no_escape() {
        let text = "plain text";
        let result = strip_ansi(text);
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "plain text");
    }

    #[test]
    fn strip_ansi_osc() {
        let text = "\x1b]0;Window Title\x07normal text";
        let result = strip_ansi(text);
        assert_eq!(result, "normal text");
    }
}
