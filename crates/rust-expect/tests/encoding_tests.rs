//! Integration tests for encoding utilities.

use rust_expect::{
    decode_utf8_lossy, detect_encoding_from_env, detect_line_ending, normalize_line_endings,
    strip_ansi, DetectedEncoding, LineEndingStyle,
};

#[test]
fn decode_utf8_valid() {
    let bytes = b"Hello, World!";
    let result = decode_utf8_lossy(bytes);
    assert_eq!(result.text, "Hello, World!");
    assert!(!result.had_errors);
}

#[test]
fn decode_utf8_with_replacement() {
    // Invalid UTF-8 sequence
    let bytes = b"Hello \xff World";
    let result = decode_utf8_lossy(bytes);
    assert!(result.text.contains("Hello"));
    assert!(result.text.contains("World"));
    assert!(result.had_errors);
}

#[test]
fn decode_utf8_unicode() {
    let text = "こんにちは世界";
    let bytes = text.as_bytes();
    let result = decode_utf8_lossy(bytes);
    assert_eq!(result.text, text);
    assert!(!result.had_errors);
}

#[test]
fn detect_line_ending_lf() {
    let text = "line1\nline2\nline3";
    let style = detect_line_ending(text);
    assert_eq!(style, Some(LineEndingStyle::Lf));
}

#[test]
fn detect_line_ending_crlf() {
    let text = "line1\r\nline2\r\nline3";
    let style = detect_line_ending(text);
    assert_eq!(style, Some(LineEndingStyle::CrLf));
}

#[test]
fn detect_line_ending_none() {
    let text = "no line endings here";
    let style = detect_line_ending(text);
    assert_eq!(style, None);
}

#[test]
fn normalize_to_lf() {
    let text = "line1\r\nline2\r\nline3";
    let normalized = normalize_line_endings(text, LineEndingStyle::Lf);
    assert_eq!(normalized.as_ref(), "line1\nline2\nline3");
}

#[test]
fn normalize_to_crlf() {
    let text = "line1\nline2\nline3";
    let normalized = normalize_line_endings(text, LineEndingStyle::CrLf);
    assert_eq!(normalized.as_ref(), "line1\r\nline2\r\nline3");
}

#[test]
fn strip_ansi_colors() {
    let text = "\x1b[31mRed\x1b[0m text";
    let stripped = strip_ansi(text);
    assert_eq!(stripped.as_ref(), "Red text");
}

#[test]
fn strip_ansi_cursor() {
    let text = "\x1b[2J\x1b[HClear screen";
    let stripped = strip_ansi(text);
    assert_eq!(stripped.as_ref(), "Clear screen");
}

#[test]
fn strip_ansi_complex() {
    let text = "\x1b[38;2;255;0;0mTruecolor\x1b[0m \x1b[1;4mBold Underline\x1b[0m";
    let stripped = strip_ansi(text);
    assert_eq!(stripped.as_ref(), "Truecolor Bold Underline");
}

#[test]
fn strip_ansi_preserves_plain() {
    let text = "No escape sequences here";
    let stripped = strip_ansi(text);
    assert_eq!(stripped.as_ref(), text);
}

#[test]
fn detected_encoding_is_utf8() {
    let encoding = DetectedEncoding::Utf8;
    assert!(encoding.is_utf8());
}

#[test]
fn detect_encoding_from_env_works() {
    // This should return something reasonable based on environment
    let encoding = detect_encoding_from_env();
    // Just verify it doesn't panic and returns a valid encoding
    assert!(!format!("{:?}", encoding).is_empty());
}

#[test]
fn line_ending_style_as_str() {
    assert_eq!(LineEndingStyle::Lf.as_str(), "\n");
    assert_eq!(LineEndingStyle::CrLf.as_str(), "\r\n");
    assert_eq!(LineEndingStyle::Cr.as_str(), "\r");
}

#[test]
fn line_ending_style_as_bytes() {
    assert_eq!(LineEndingStyle::Lf.as_bytes(), b"\n");
    assert_eq!(LineEndingStyle::CrLf.as_bytes(), b"\r\n");
    assert_eq!(LineEndingStyle::Cr.as_bytes(), b"\r");
}
