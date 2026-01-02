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
        .map_err(ExpectError::Io)?;

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
        writeln!(writer, "[{:.6}, \"{}\", \"{}\"]", time, event_type, escape_json(&data))
            .map_err(ExpectError::Io)?;
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
        .map_err(ExpectError::Io)?;

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
        let line = line.map_err(ExpectError::Io)?;
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
    // Simple JSON parsing for header
    let mut header = AsciicastHeader::default();

    // Extract width
    if let Some(start) = line.find("\"width\":") {
        let rest = &line[start + 8..];
        if let Some(end) = rest.find(|c: char| !c.is_numeric() && c != ' ') {
            if let Ok(w) = rest[..end].trim().parse() {
                header.width = w;
            }
        }
    }

    // Extract height
    if let Some(start) = line.find("\"height\":") {
        let rest = &line[start + 9..];
        if let Some(end) = rest.find(|c: char| !c.is_numeric() && c != ' ') {
            if let Ok(h) = rest[..end].trim().parse() {
                header.height = h;
            }
        }
    }

    Ok(header)
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

    let time: f64 = parts[0].trim().parse().map_err(|_| {
        ExpectError::config("Invalid timestamp")
    })?;

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
                Some('"') => result.push('"'),
                Some('\\') => result.push('\\'),
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
        transcript.push(TranscriptEvent::output(Duration::from_millis(100), b"hello"));

        let mut buf = Vec::new();
        write_asciicast(&mut buf, &transcript).unwrap();

        let parsed = read_asciicast(buf.as_slice()).unwrap();
        assert_eq!(parsed.events.len(), 1);
    }
}
