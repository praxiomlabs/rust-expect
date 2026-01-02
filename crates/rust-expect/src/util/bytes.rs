//! Byte manipulation utilities.
//!
//! This module provides utilities for working with byte sequences,
//! including pattern matching, escaping, and conversion.

use std::fmt;

/// Convert bytes to a human-readable hexdump format.
#[must_use] pub fn hexdump(data: &[u8]) -> String {
    let mut result = String::new();
    
    for (i, chunk) in data.chunks(16).enumerate() {
        // Offset
        result.push_str(&format!("{:08x}  ", i * 16));
        
        // Hex bytes
        for (j, byte) in chunk.iter().enumerate() {
            result.push_str(&format!("{byte:02x} "));
            if j == 7 {
                result.push(' ');
            }
        }
        
        // Padding for incomplete lines
        for j in chunk.len()..16 {
            result.push_str("   ");
            if j == 7 {
                result.push(' ');
            }
        }
        
        result.push_str(" |");
        
        // ASCII representation
        for byte in chunk {
            let c = if byte.is_ascii_graphic() || *byte == b' ' {
                *byte as char
            } else {
                '.'
            };
            result.push(c);
        }
        
        result.push_str("|\n");
    }
    
    result
}

/// Escape bytes for display.
#[must_use] pub fn escape_bytes(data: &[u8]) -> String {
    let mut result = String::new();
    
    for byte in data {
        match byte {
            b'\n' => result.push_str("\\n"),
            b'\r' => result.push_str("\\r"),
            b'\t' => result.push_str("\\t"),
            b'\0' => result.push_str("\\0"),
            b'\\' => result.push_str("\\\\"),
            0x1b => result.push_str("\\e"),
            0x07 => result.push_str("\\a"),
            0x08 => result.push_str("\\b"),
            b if b.is_ascii_graphic() || *b == b' ' => result.push(*b as char),
            b => result.push_str(&format!("\\x{b:02x}")),
        }
    }
    
    result
}

/// Parse an escaped string back to bytes.
#[must_use] pub fn unescape_bytes(s: &str) -> Vec<u8> {
    let mut result = Vec::new();
    let mut chars = s.chars().peekable();
    
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push(b'\n'),
                Some('r') => result.push(b'\r'),
                Some('t') => result.push(b'\t'),
                Some('0') => result.push(b'\0'),
                Some('\\') => result.push(b'\\'),
                Some('e') => result.push(0x1b),
                Some('a') => result.push(0x07),
                Some('b') => result.push(0x08),
                Some('x') => {
                    let hex: String = chars.by_ref().take(2).collect();
                    if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                        result.push(byte);
                    }
                }
                Some(other) => {
                    result.push(b'\\');
                    let mut buf = [0u8; 4];
                    result.extend(other.encode_utf8(&mut buf).as_bytes());
                }
                None => result.push(b'\\'),
            }
        } else {
            let mut buf = [0u8; 4];
            result.extend(c.encode_utf8(&mut buf).as_bytes());
        }
    }
    
    result
}

/// Find a pattern in a byte slice.
#[must_use] pub fn find_pattern(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    if needle.len() > haystack.len() {
        return None;
    }
    
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Find all occurrences of a pattern in a byte slice.
#[must_use] pub fn find_all_patterns(haystack: &[u8], needle: &[u8]) -> Vec<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return Vec::new();
    }
    
    haystack
        .windows(needle.len())
        .enumerate()
        .filter_map(|(i, window)| {
            if window == needle {
                Some(i)
            } else {
                None
            }
        })
        .collect()
}

/// Replace all occurrences of a pattern in a byte slice.
#[must_use] pub fn replace_pattern(haystack: &[u8], needle: &[u8], replacement: &[u8]) -> Vec<u8> {
    if needle.is_empty() {
        return haystack.to_vec();
    }
    
    let mut result = Vec::with_capacity(haystack.len());
    let mut i = 0;
    
    while i < haystack.len() {
        if i + needle.len() <= haystack.len() && &haystack[i..i + needle.len()] == needle {
            result.extend(replacement);
            i += needle.len();
        } else {
            result.push(haystack[i]);
            i += 1;
        }
    }
    
    result
}

/// Strip ANSI escape sequences from bytes.
#[must_use] pub fn strip_ansi(data: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(data.len());
    let mut i = 0;
    
    while i < data.len() {
        if data[i] == 0x1b {
            // Skip escape sequence
            if i + 1 < data.len() && data[i + 1] == b'[' {
                // CSI sequence
                i += 2;
                while i < data.len() && !data[i].is_ascii_alphabetic() && data[i] != b'@' {
                    i += 1;
                }
                if i < data.len() {
                    i += 1; // Skip final character
                }
            } else {
                // Simple escape
                i += 2;
            }
        } else {
            result.push(data[i]);
            i += 1;
        }
    }
    
    result
}

/// A wrapper for bytes that implements Display with escaping.
pub struct EscapedBytes<'a>(pub &'a [u8]);

impl fmt::Display for EscapedBytes<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", escape_bytes(self.0))
    }
}

impl fmt::Debug for EscapedBytes<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\"{}\"", escape_bytes(self.0))
    }
}

/// Convert bytes to a lossy UTF-8 string with control characters visible.
#[must_use] pub fn to_visible_string(data: &[u8]) -> String {
    let s = String::from_utf8_lossy(data);
    let mut result = String::new();
    
    for c in s.chars() {
        if c.is_control() && c != '\n' && c != '\t' {
            if c as u32 <= 26 {
                result.push('^');
                result.push((b'@' + c as u8) as char);
            } else {
                result.push_str(&format!("\\x{:02x}", c as u32));
            }
        } else {
            result.push(c);
        }
    }
    
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hexdump() {
        let data = b"Hello, World!";
        let dump = hexdump(data);
        assert!(dump.contains("48 65 6c 6c")); // "Hell"
        assert!(dump.contains("|Hello, World!|"));
    }

    #[test]
    fn test_escape_unescape() {
        let original = b"Hello\n\tWorld\x1b[31m";
        let escaped = escape_bytes(original);
        let unescaped = unescape_bytes(&escaped);
        assert_eq!(original, unescaped.as_slice());
    }

    #[test]
    fn test_find_pattern() {
        let data = b"Hello, World!";
        assert_eq!(find_pattern(data, b"World"), Some(7));
        assert_eq!(find_pattern(data, b"foo"), None);
    }

    #[test]
    fn test_replace_pattern() {
        let data = b"Hello, World!";
        let result = replace_pattern(data, b"World", b"Rust");
        assert_eq!(result, b"Hello, Rust!");
    }

    #[test]
    fn test_strip_ansi() {
        let data = b"\x1b[31mHello\x1b[0m";
        let stripped = strip_ansi(data);
        assert_eq!(stripped, b"Hello");
    }

    #[test]
    fn test_visible_string() {
        let data = b"Hello\x03World";
        let visible = to_visible_string(data);
        assert!(visible.contains("^C"));
    }
}
