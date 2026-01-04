//! Asciinema asciicast v2 format support.

use super::format::{EventType, Transcript, TranscriptEvent, TranscriptMetadata};
use crate::error::{ExpectError, Result};
use std::io::{BufRead, Write};
use std::time::Duration;

/// Asciicast v2 header.
#[derive(Debug, Clone)]
pub struct AsciicastHeader {
    /// Format version.
    pub version: u8,
    /// Terminal width.
    pub width: u16,
    /// Terminal height.
    pub height: u16,
    /// Recording timestamp.
    pub timestamp: Option<u64>,
    /// Total duration.
    pub duration: Option<f64>,
    /// Idle time limit.
    pub idle_time_limit: Option<f64>,
    /// Command.
    pub command: Option<String>,
    /// Title.
    pub title: Option<String>,
    /// Environment.
    pub env: std::collections::HashMap<String, String>,
}

impl Default for AsciicastHeader {
    fn default() -> Self {
        Self {
            version: 2,
            width: 80,
            height: 24,
            timestamp: None,
            duration: None,
            idle_time_limit: None,
            command: None,
            title: None,
            env: std::collections::HashMap::new(),
        }
    }
}

impl AsciicastHeader {
    /// Create a new header.
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            ..Default::default()
        }
    }

    /// Convert to JSON string.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut parts = vec![
            format!("\"version\": {}", self.version),
            format!("\"width\": {}", self.width),
            format!("\"height\": {}", self.height),
        ];

        if let Some(ts) = self.timestamp {
            parts.push(format!("\"timestamp\": {ts}"));
        }
        if let Some(dur) = self.duration {
            parts.push(format!("\"duration\": {dur:.6}"));
        }
        if let Some(limit) = self.idle_time_limit {
            parts.push(format!("\"idle_time_limit\": {limit:.1}"));
        }
        if let Some(ref cmd) = self.command {
            parts.push(format!("\"command\": \"{}\"", escape_json(cmd)));
        }
        if let Some(ref title) = self.title {
            parts.push(format!("\"title\": \"{}\"", escape_json(title)));
        }
        if !self.env.is_empty() {
            let env_parts: Vec<String> = self
                .env
                .iter()
                .map(|(k, v)| format!("\"{}\": \"{}\"", escape_json(k), escape_json(v)))
                .collect();
            parts.push(format!("\"env\": {{{}}}", env_parts.join(", ")));
        }

        format!("{{{}}}", parts.join(", "))
    }
}

/// Write a transcript in asciicast v2 format.
pub fn write_asciicast<W: Write>(writer: &mut W, transcript: &Transcript) -> Result<()> {
    let header = AsciicastHeader {
        width: transcript.metadata.width,
        height: transcript.metadata.height,
        timestamp: transcript.metadata.timestamp,
        duration: transcript.metadata.duration.map(|d| d.as_secs_f64()),
        command: transcript.metadata.command.clone(),
        title: transcript.metadata.title.clone(),
        env: transcript.metadata.env.clone(),
        ..Default::default()
    };

    // Write header
    writeln!(writer, "{}", header.to_json())
        .map_err(|e| ExpectError::io_context("writing asciicast header", e))?;

    // Write events
    for event in &transcript.events {
        let time = event.timestamp.as_secs_f64();
        let event_type = match event.event_type {
            EventType::Output => "o",
            EventType::Input => "i",
            EventType::Resize => "r",
            EventType::Marker => "m",
        };
        let data = String::from_utf8_lossy(&event.data);
        writeln!(
            writer,
            "[{:.6}, \"{}\", \"{}\"]",
            time,
            event_type,
            escape_json(&data)
        )
        .map_err(|e| ExpectError::io_context("writing asciicast event", e))?;
    }

    Ok(())
}

/// Read a transcript from asciicast v2 format.
pub fn read_asciicast<R: BufRead>(reader: R) -> Result<Transcript> {
    let mut lines = reader.lines();

    // Parse header
    let header_line = lines
        .next()
        .ok_or_else(|| ExpectError::config("Empty asciicast file"))?
        .map_err(|e| ExpectError::io_context("reading asciicast header line", e))?;

    let header = parse_header(&header_line)?;

    let metadata = TranscriptMetadata {
        width: header.width,
        height: header.height,
        command: header.command,
        title: header.title,
        timestamp: header.timestamp,
        duration: header.duration.map(Duration::from_secs_f64),
        env: header.env,
    };

    let mut transcript = Transcript::new(metadata);

    // Parse events
    for line in lines {
        let line = line.map_err(|e| ExpectError::io_context("reading asciicast event line", e))?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(event) = parse_event(&line)? {
            transcript.push(event);
        }
    }

    Ok(transcript)
}

fn parse_header(line: &str) -> Result<AsciicastHeader> {
    let mut header = AsciicastHeader::default();

    // Parse numeric fields
    header.width = parse_json_number(line, "width").unwrap_or(80) as u16;
    header.height = parse_json_number(line, "height").unwrap_or(24) as u16;
    header.version = parse_json_number(line, "version").unwrap_or(2) as u8;

    if let Some(ts) = parse_json_number(line, "timestamp") {
        header.timestamp = Some(ts as u64);
    }

    if let Some(dur) = parse_json_float(line, "duration") {
        header.duration = Some(dur);
    }

    if let Some(limit) = parse_json_float(line, "idle_time_limit") {
        header.idle_time_limit = Some(limit);
    }

    // Parse string fields
    header.command = parse_json_string(line, "command");
    header.title = parse_json_string(line, "title");

    // Parse env object (simplified - handles flat env objects)
    if let Some(env) = parse_json_object(line, "env") {
        header.env = env;
    }

    Ok(header)
}

/// Parse a numeric JSON field.
fn parse_json_number(json: &str, field: &str) -> Option<i64> {
    let pattern = format!("\"{field}\":");
    let start = json.find(&pattern)?;
    let rest = &json[start + pattern.len()..];
    let rest = rest.trim_start();

    // Find the end of the number
    let end = rest
        .find(|c: char| !c.is_ascii_digit() && c != '-')
        .unwrap_or(rest.len());

    rest[..end].trim().parse().ok()
}

/// Parse a floating-point JSON field.
fn parse_json_float(json: &str, field: &str) -> Option<f64> {
    let pattern = format!("\"{field}\":");
    let start = json.find(&pattern)?;
    let rest = &json[start + pattern.len()..];
    let rest = rest.trim_start();

    // Find the end of the number (including decimal point and exponent)
    let end = rest
        .find(|c: char| {
            !c.is_ascii_digit() && c != '.' && c != '-' && c != 'e' && c != 'E' && c != '+'
        })
        .unwrap_or(rest.len());

    rest[..end].trim().parse().ok()
}

/// Parse a string JSON field.
fn parse_json_string(json: &str, field: &str) -> Option<String> {
    let pattern = format!("\"{field}\":");
    let start = json.find(&pattern)?;
    let rest = &json[start + pattern.len()..];
    let rest = rest.trim_start();

    // Must start with a quote
    if !rest.starts_with('"') {
        return None;
    }

    // Find the closing quote (handling escapes)
    let content = &rest[1..];
    let mut end = 0;
    let mut escaped = false;

    for (i, c) in content.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
            continue;
        }
        if c == '"' {
            end = i;
            break;
        }
    }

    if end == 0 && !content.is_empty() && !content.starts_with('"') {
        // No closing quote found, check if string is at end
        end = content.len();
    }

    Some(unescape_json(&content[..end]))
}

/// Parse a JSON object field (simplified, handles flat string-value objects).
fn parse_json_object(json: &str, field: &str) -> Option<std::collections::HashMap<String, String>> {
    let pattern = format!("\"{field}\":");
    let start = json.find(&pattern)?;
    let rest = &json[start + pattern.len()..];
    let rest = rest.trim_start();

    // Must start with {
    if !rest.starts_with('{') {
        return None;
    }

    // Find matching closing brace
    let mut depth = 0;
    let mut end = 0;

    for (i, c) in rest.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = i + 1;
                    break;
                }
            }
            _ => {}
        }
    }

    if end == 0 {
        return None;
    }

    let obj_str = &rest[1..end - 1]; // Content inside braces
    let mut result = std::collections::HashMap::new();

    // Parse key-value pairs
    for pair in obj_str.split(',') {
        let pair = pair.trim();
        if let Some(colon) = pair.find(':') {
            let key = pair[..colon].trim().trim_matches('"');
            let value = pair[colon + 1..].trim().trim_matches('"');
            if !key.is_empty() {
                result.insert(key.to_string(), unescape_json(value));
            }
        }
    }

    Some(result)
}

fn parse_event(line: &str) -> Result<Option<TranscriptEvent>> {
    let line = line.trim();
    if !line.starts_with('[') || !line.ends_with(']') {
        return Ok(None);
    }

    let inner = &line[1..line.len() - 1];
    let parts: Vec<&str> = inner.splitn(3, ',').collect();
    if parts.len() < 3 {
        return Ok(None);
    }

    let time: f64 = parts[0]
        .trim()
        .parse()
        .map_err(|_| ExpectError::config("Invalid timestamp"))?;

    let event_type = parts[1].trim().trim_matches('"');
    let data = parts[2].trim().trim_matches('"');

    let event_type = match event_type {
        "o" => EventType::Output,
        "i" => EventType::Input,
        "r" => EventType::Resize,
        "m" => EventType::Marker,
        _ => return Ok(None),
    };

    Ok(Some(TranscriptEvent {
        timestamp: Duration::from_secs_f64(time),
        event_type,
        data: unescape_json(data).into_bytes(),
    }))
}

fn escape_json(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_control() => {
                result.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => result.push(c),
        }
    }
    result
}

fn unescape_json(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('b') => result.push('\u{0008}'), // backspace
                Some('f') => result.push('\u{000C}'), // form feed
                Some('"') => result.push('"'),
                Some('\\') => result.push('\\'),
                Some('/') => result.push('/'),
                Some('u') => {
                    // Parse \uXXXX unicode escape
                    let mut hex = String::with_capacity(4);
                    for _ in 0..4 {
                        if let Some(&c) = chars.peek() {
                            if c.is_ascii_hexdigit() {
                                hex.push(chars.next().unwrap());
                            } else {
                                break;
                            }
                        }
                    }
                    if hex.len() == 4 {
                        if let Ok(code) = u32::from_str_radix(&hex, 16) {
                            if let Some(ch) = char::from_u32(code) {
                                result.push(ch);
                                continue;
                            }
                        }
                    }
                    // Invalid escape, keep as-is
                    result.push_str("\\u");
                    result.push_str(&hex);
                }
                Some(c) => {
                    result.push('\\');
                    result.push(c);
                }
                None => result.push('\\'),
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
    fn asciicast_header() {
        let header = AsciicastHeader::new(80, 24);
        let json = header.to_json();
        assert!(json.contains("\"version\": 2"));
        assert!(json.contains("\"width\": 80"));
    }

    #[test]
    fn escape_special_chars() {
        assert_eq!(escape_json("hello\nworld"), "hello\\nworld");
        assert_eq!(escape_json("say \"hi\""), "say \\\"hi\\\"");
    }

    #[test]
    fn roundtrip() {
        let mut transcript = Transcript::new(TranscriptMetadata::new(80, 24));
        transcript.push(TranscriptEvent::output(
            Duration::from_millis(100),
            b"hello",
        ));

        let mut buf = Vec::new();
        write_asciicast(&mut buf, &transcript).unwrap();

        let parsed = read_asciicast(buf.as_slice()).unwrap();
        assert_eq!(parsed.events.len(), 1);
    }

    #[test]
    fn parse_json_number_basic() {
        let json = r#"{"version": 2, "width": 120, "height": 40}"#;
        assert_eq!(parse_json_number(json, "version"), Some(2));
        assert_eq!(parse_json_number(json, "width"), Some(120));
        assert_eq!(parse_json_number(json, "height"), Some(40));
        assert_eq!(parse_json_number(json, "nonexistent"), None);
    }

    #[test]
    fn parse_json_number_negative() {
        let json = r#"{"offset": -100}"#;
        assert_eq!(parse_json_number(json, "offset"), Some(-100));
    }

    #[test]
    fn parse_json_float_basic() {
        let json = r#"{"duration": 123.456789, "idle_time_limit": 2.5}"#;
        assert!((parse_json_float(json, "duration").unwrap() - 123.456789).abs() < 0.000001);
        assert!((parse_json_float(json, "idle_time_limit").unwrap() - 2.5).abs() < 0.000001);
        assert_eq!(parse_json_float(json, "nonexistent"), None);
    }

    #[test]
    fn parse_json_float_scientific() {
        let json = r#"{"value": 1.5e10}"#;
        assert!((parse_json_float(json, "value").unwrap() - 1.5e10).abs() < 1.0);
    }

    #[test]
    fn parse_json_string_basic() {
        let json = r#"{"command": "/bin/bash", "title": "My Recording"}"#;
        assert_eq!(
            parse_json_string(json, "command"),
            Some("/bin/bash".to_string())
        );
        assert_eq!(
            parse_json_string(json, "title"),
            Some("My Recording".to_string())
        );
        assert_eq!(parse_json_string(json, "nonexistent"), None);
    }

    #[test]
    fn parse_json_string_escaped() {
        let json = r#"{"path": "C:\\Users\\test", "msg": "say \"hello\""}"#;
        assert_eq!(
            parse_json_string(json, "path"),
            Some("C:\\Users\\test".to_string())
        );
        assert_eq!(
            parse_json_string(json, "msg"),
            Some("say \"hello\"".to_string())
        );
    }

    #[test]
    fn parse_json_object_basic() {
        let json = r#"{"env": {"SHELL": "/bin/bash", "TERM": "xterm-256color"}}"#;
        let env = parse_json_object(json, "env").unwrap();
        assert_eq!(env.get("SHELL"), Some(&"/bin/bash".to_string()));
        assert_eq!(env.get("TERM"), Some(&"xterm-256color".to_string()));
    }

    #[test]
    fn parse_json_object_empty() {
        let json = r#"{"env": {}}"#;
        let env = parse_json_object(json, "env").unwrap();
        assert!(env.is_empty());
    }

    #[test]
    fn parse_header_full() {
        let header_json = r#"{"version": 2, "width": 120, "height": 40, "timestamp": 1704067200, "duration": 60.5, "idle_time_limit": 2.0, "command": "/bin/zsh", "title": "Demo", "env": {"SHELL": "/bin/zsh"}}"#;
        let header = parse_header(header_json).unwrap();

        assert_eq!(header.version, 2);
        assert_eq!(header.width, 120);
        assert_eq!(header.height, 40);
        assert_eq!(header.timestamp, Some(1704067200));
        assert!((header.duration.unwrap() - 60.5).abs() < 0.001);
        assert!((header.idle_time_limit.unwrap() - 2.0).abs() < 0.001);
        assert_eq!(header.command, Some("/bin/zsh".to_string()));
        assert_eq!(header.title, Some("Demo".to_string()));
        assert_eq!(header.env.get("SHELL"), Some(&"/bin/zsh".to_string()));
    }

    #[test]
    fn parse_header_minimal() {
        let header_json = r#"{"version": 2, "width": 80, "height": 24}"#;
        let header = parse_header(header_json).unwrap();

        assert_eq!(header.version, 2);
        assert_eq!(header.width, 80);
        assert_eq!(header.height, 24);
        assert_eq!(header.timestamp, None);
        assert_eq!(header.duration, None);
        assert_eq!(header.command, None);
        assert!(header.env.is_empty());
    }

    #[test]
    fn unescape_json_sequences() {
        assert_eq!(unescape_json("hello\\nworld"), "hello\nworld");
        assert_eq!(unescape_json("tab\\there"), "tab\there");
        assert_eq!(unescape_json("quote\\\"here"), "quote\"here");
        assert_eq!(unescape_json("back\\\\slash"), "back\\slash");
        assert_eq!(unescape_json("return\\rhere"), "return\rhere");
    }

    #[test]
    fn unescape_json_backspace_formfeed() {
        assert_eq!(unescape_json("back\\bspace"), "back\u{0008}space");
        assert_eq!(unescape_json("form\\ffeed"), "form\u{000C}feed");
    }

    #[test]
    fn unescape_json_forward_slash() {
        // Forward slash can be escaped but doesn't need to be
        assert_eq!(unescape_json("path\\/to\\/file"), "path/to/file");
        assert_eq!(unescape_json("path/to/file"), "path/to/file");
    }

    #[test]
    fn unescape_json_unicode() {
        // Basic ASCII via unicode escape
        assert_eq!(unescape_json("\\u0041"), "A");
        assert_eq!(unescape_json("\\u0048\\u0069"), "Hi");

        // Control characters
        assert_eq!(unescape_json("\\u001b"), "\u{001b}"); // ESC
        assert_eq!(unescape_json("\\u0000"), "\u{0000}"); // NULL

        // Non-ASCII unicode
        assert_eq!(unescape_json("\\u00e9"), "é");
        assert_eq!(unescape_json("\\u4e2d\\u6587"), "中文");

        // Mixed content
        assert_eq!(unescape_json("hello\\u0020world"), "hello world");
        assert_eq!(unescape_json("\\u0041\\u0042\\u0043"), "ABC");
    }

    #[test]
    fn unescape_json_unicode_invalid() {
        // Invalid: not enough hex digits
        assert_eq!(unescape_json("\\u00"), "\\u00");
        assert_eq!(unescape_json("\\u0"), "\\u0");
        assert_eq!(unescape_json("\\u"), "\\u");

        // Invalid: non-hex characters
        assert_eq!(unescape_json("\\u00GH"), "\\u00GH");
    }

    #[test]
    fn unescape_json_mixed_escapes() {
        // Combine various escape types
        assert_eq!(
            unescape_json("line1\\nline2\\ttab\\u0021"),
            "line1\nline2\ttab!"
        );
        assert_eq!(
            unescape_json("\\\"quoted\\\" and \\u003Ctag\\u003E"),
            "\"quoted\" and <tag>"
        );
    }

    #[test]
    fn escape_json_control_chars() {
        // Control characters should be escaped as \uXXXX
        assert_eq!(escape_json("\u{001b}"), "\\u001b"); // ESC
        assert_eq!(escape_json("\u{0007}"), "\\u0007"); // BEL
    }

    #[test]
    fn roundtrip_with_metadata() {
        let mut metadata = TranscriptMetadata::new(120, 40);
        metadata.command = Some("/bin/bash".to_string());
        metadata.title = Some("Test Recording".to_string());
        metadata.timestamp = Some(1704067200);
        metadata.duration = Some(Duration::from_secs_f64(30.5));
        metadata
            .env
            .insert("SHELL".to_string(), "/bin/bash".to_string());
        metadata.env.insert("TERM".to_string(), "xterm".to_string());

        let mut transcript = Transcript::new(metadata);
        transcript.push(TranscriptEvent::output(Duration::from_millis(100), b"$ "));
        transcript.push(TranscriptEvent::input(Duration::from_millis(200), b"ls\n"));
        transcript.push(TranscriptEvent::output(
            Duration::from_millis(300),
            b"file1.txt\nfile2.txt\n",
        ));

        let mut buf = Vec::new();
        write_asciicast(&mut buf, &transcript).unwrap();

        let parsed = read_asciicast(buf.as_slice()).unwrap();
        assert_eq!(parsed.metadata.width, 120);
        assert_eq!(parsed.metadata.height, 40);
        assert_eq!(parsed.metadata.command, Some("/bin/bash".to_string()));
        assert_eq!(parsed.metadata.title, Some("Test Recording".to_string()));
        assert_eq!(parsed.metadata.timestamp, Some(1704067200));
        assert_eq!(parsed.events.len(), 3);
    }
}
