//! PII redaction utilities.
//!
//! This module provides functionality for redacting detected PII
//! from text, replacing sensitive information with placeholders.

use super::detector::{PiiDetector, PiiMatch, PiiType};

/// Redaction style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedactionStyle {
    /// Replace with a placeholder (e.g., `\[REDACTED\]`).
    Placeholder,
    /// Replace with asterisks.
    Asterisks,
    /// Replace with X characters.
    Xs,
    /// Partially mask (show first/last characters).
    PartialMask,
    /// Custom replacement per type.
    Custom,
}

/// A PII redactor.
#[derive(Debug, Clone)]
pub struct PiiRedactor {
    /// The underlying detector.
    detector: PiiDetector,
    /// Redaction style.
    style: RedactionStyle,
    /// Custom placeholders per PII type.
    custom_placeholders: std::collections::HashMap<PiiType, String>,
}

impl Default for PiiRedactor {
    fn default() -> Self {
        Self::new()
    }
}

impl PiiRedactor {
    /// Create a new redactor with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            detector: PiiDetector::new(),
            style: RedactionStyle::Placeholder,
            custom_placeholders: std::collections::HashMap::new(),
        }
    }

    /// Create a redactor with a custom detector.
    #[must_use]
    pub fn with_detector(detector: PiiDetector) -> Self {
        Self {
            detector,
            ..Self::new()
        }
    }

    /// Set the redaction style.
    #[must_use]
    pub const fn style(mut self, style: RedactionStyle) -> Self {
        self.style = style;
        self
    }

    /// Set a custom placeholder for a PII type.
    #[must_use]
    pub fn custom_placeholder(mut self, pii_type: PiiType, placeholder: impl Into<String>) -> Self {
        self.custom_placeholders
            .insert(pii_type, placeholder.into());
        self
    }

    /// Redact PII from the given text.
    #[must_use]
    pub fn redact(&self, text: &str) -> String {
        let matches = self.detector.detect(text);

        if matches.is_empty() {
            return text.to_string();
        }

        let mut result = String::with_capacity(text.len());
        let mut last_end = 0;

        for m in &matches {
            // Add text before this match
            result.push_str(&text[last_end..m.start]);
            // Add redaction
            result.push_str(&self.get_replacement(m));
            last_end = m.end;
        }

        // Add remaining text
        result.push_str(&text[last_end..]);
        result
    }

    /// Redact PII from bytes (lossy UTF-8 conversion).
    #[must_use]
    pub fn redact_bytes(&self, data: &[u8]) -> Vec<u8> {
        let text = String::from_utf8_lossy(data);
        self.redact(&text).into_bytes()
    }

    /// Get the replacement string for a match.
    fn get_replacement(&self, m: &PiiMatch) -> String {
        // Check for custom placeholder first (for built-in types only)
        if !m.is_custom() {
            if let Some(custom) = self.custom_placeholders.get(&m.pii_type) {
                return custom.clone();
            }
        }

        match self.style {
            // Use PiiMatch::placeholder() which handles both built-in and custom patterns
            RedactionStyle::Placeholder => m.placeholder().to_string(),
            RedactionStyle::Asterisks => "*".repeat(m.len()),
            RedactionStyle::Xs => "X".repeat(m.len()),
            RedactionStyle::PartialMask => self.partial_mask(&m.text),
            RedactionStyle::Custom => m.placeholder().to_string(),
        }
    }

    /// Create a partial mask (show first 2 and last 2 characters).
    #[allow(clippy::unused_self)]
    fn partial_mask(&self, text: &str) -> String {
        let chars: Vec<char> = text.chars().collect();
        if chars.len() <= 4 {
            return "*".repeat(chars.len());
        }

        let visible = 2;
        let hidden = chars.len() - (visible * 2);

        format!(
            "{}{}{}",
            chars[..visible].iter().collect::<String>(),
            "*".repeat(hidden),
            chars[chars.len() - visible..].iter().collect::<String>()
        )
    }

    /// Get the detector.
    #[must_use]
    pub const fn detector(&self) -> &PiiDetector {
        &self.detector
    }
}

/// A streaming redactor for processing data in chunks.
pub struct StreamingRedactor {
    redactor: PiiRedactor,
    buffer: String,
    max_buffer: usize,
}

impl StreamingRedactor {
    /// Create a new streaming redactor.
    #[must_use]
    pub const fn new(redactor: PiiRedactor) -> Self {
        Self {
            redactor,
            buffer: String::new(),
            max_buffer: 4096,
        }
    }

    /// Set the maximum buffer size.
    #[must_use]
    pub const fn max_buffer(mut self, size: usize) -> Self {
        self.max_buffer = size;
        self
    }

    /// Process a chunk of data.
    ///
    /// Returns redacted output that is safe to emit.
    pub fn process(&mut self, data: &str) -> String {
        self.buffer.push_str(data);

        // Find a safe point to redact (end of line or max buffer)
        let safe_point = self.find_safe_point();

        if safe_point > 0 {
            let to_process = self.buffer[..safe_point].to_string();
            self.buffer = self.buffer[safe_point..].to_string();
            self.redactor.redact(&to_process)
        } else {
            String::new()
        }
    }

    /// Flush any remaining data.
    pub fn flush(&mut self) -> String {
        let remaining = std::mem::take(&mut self.buffer);
        self.redactor.redact(&remaining)
    }

    /// Find a safe point to split the buffer.
    fn find_safe_point(&self) -> usize {
        if self.buffer.len() >= self.max_buffer {
            // Force a split at max buffer
            return self.max_buffer;
        }

        // Try to find a newline
        if let Some(pos) = self.buffer.rfind('\n') {
            return pos + 1;
        }

        // Try to find a space
        if self.buffer.len() > 100 {
            if let Some(pos) = self.buffer[..100].rfind(' ') {
                return pos + 1;
            }
        }

        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_ssn() {
        let redactor = PiiRedactor::new();
        let result = redactor.redact("My SSN is 123-45-6789");
        assert!(result.contains("[SSN REDACTED]"));
        assert!(!result.contains("123-45-6789"));
    }

    #[test]
    fn redact_email() {
        let redactor = PiiRedactor::new();
        let result = redactor.redact("Email: user@example.com");
        assert!(result.contains("[EMAIL REDACTED]"));
    }

    #[test]
    fn redact_asterisks() {
        let redactor = PiiRedactor::new().style(RedactionStyle::Asterisks);
        let result = redactor.redact("SSN: 123-45-6789");
        assert!(result.contains("***********"));
    }

    #[test]
    fn partial_mask() {
        let redactor = PiiRedactor::new().style(RedactionStyle::PartialMask);
        let result = redactor.redact("Email: user@example.com");
        // Should show first and last chars with stars in between
        assert!(!result.contains("user@example.com"));
    }

    #[test]
    fn custom_placeholder() {
        let redactor = PiiRedactor::new().custom_placeholder(PiiType::Email, "***EMAIL***");
        let result = redactor.redact("Contact: test@test.com");
        assert!(result.contains("***EMAIL***"));
    }

    #[test]
    fn streaming_redactor() {
        let redactor = PiiRedactor::new();
        let mut streaming = StreamingRedactor::new(redactor);

        let out1 = streaming.process("Email: user@");
        let out2 = streaming.process("example.com\n");
        let out3 = streaming.flush();

        let combined = format!("{out1}{out2}{out3}");
        assert!(!combined.contains("user@example.com"));
    }

    #[test]
    fn redact_custom_pattern() {
        let detector = PiiDetector::new().add_pattern(
            "employee_id",
            r"EMP-\d{6}",
            "[EMPLOYEE ID REDACTED]",
            0.9,
        );
        let redactor = PiiRedactor::with_detector(detector);

        let result = redactor.redact("Contact EMP-123456 for assistance");
        assert!(result.contains("[EMPLOYEE ID REDACTED]"));
        assert!(!result.contains("EMP-123456"));
    }

    #[test]
    fn redact_custom_with_builtin() {
        let detector =
            PiiDetector::new().add_pattern("project", r"PROJ-[A-Z]{4}", "[PROJECT]", 0.9);
        let redactor = PiiRedactor::with_detector(detector);

        let result = redactor.redact("PROJ-ABCD owner: user@example.com");
        assert!(result.contains("[PROJECT]"));
        assert!(result.contains("[EMAIL REDACTED]"));
        assert!(!result.contains("PROJ-ABCD"));
        assert!(!result.contains("user@example.com"));
    }

    #[test]
    fn redact_custom_asterisks() {
        let detector = PiiDetector::custom_only().add_pattern("code", r"CODE-\d{4}", "[CODE]", 0.9);
        let redactor = PiiRedactor::with_detector(detector).style(RedactionStyle::Asterisks);

        let result = redactor.redact("Use CODE-1234 to access");
        assert!(result.contains("*********")); // 9 asterisks for "CODE-1234"
        assert!(!result.contains("CODE-1234"));
    }
}
