//! PII (Personally Identifiable Information) detection and redaction.
//!
//! This module provides utilities for detecting and redacting sensitive
//! information from terminal output, including:
//!
//! - Social Security Numbers
//! - Credit card numbers
//! - Email addresses
//! - Phone numbers
//! - API keys and tokens
//! - Custom patterns (user-defined)
//!
//! # Example
//!
//! ```rust
//! use rust_expect::pii::{PiiDetector, PiiRedactor};
//!
//! let redactor = PiiRedactor::new();
//! let safe_text = redactor.redact("SSN: 123-45-6789, Email: user@example.com");
//! assert!(!safe_text.contains("123-45-6789"));
//! ```
//!
//! # Custom Patterns
//!
//! You can add custom detection patterns for organization-specific data:
//!
//! ```rust
//! use rust_expect::pii::{PiiDetector, PiiRedactor};
//!
//! let detector = PiiDetector::new()
//!     .add_pattern("employee_id", r"EMP-\d{6}", "[EMPLOYEE ID]", 0.9)
//!     .add_pattern("project_code", r"PROJ-[A-Z]{4}", "[PROJECT]", 0.85);
//!
//! let redactor = PiiRedactor::with_detector(detector);
//! let safe = redactor.redact("Contact EMP-123456 about PROJ-DEMO");
//! assert!(safe.contains("[EMPLOYEE ID]"));
//! assert!(safe.contains("[PROJECT]"));
//! ```

pub mod api_key;
pub mod credit_card;
pub mod detector;
pub mod redactor;
pub mod ssn;

pub use detector::{CustomPattern, PiiDetector, PiiMatch, PiiType};
pub use redactor::{PiiRedactor, RedactionStyle, StreamingRedactor};

/// Quick check if text contains any PII.
#[must_use]
pub fn contains_pii(text: &str) -> bool {
    PiiDetector::new().contains_pii(text)
}

/// Quick redaction with default settings.
#[must_use]
pub fn redact(text: &str) -> String {
    PiiRedactor::new().redact(text)
}

/// Redact with asterisks instead of placeholders.
#[must_use]
pub fn redact_asterisks(text: &str) -> String {
    PiiRedactor::new()
        .style(RedactionStyle::Asterisks)
        .redact(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_contains() {
        assert!(contains_pii("SSN: 123-45-6789"));
        assert!(contains_pii("Email: test@example.com"));
        assert!(!contains_pii("No PII here"));
    }

    #[test]
    fn quick_redact() {
        let result = redact("Card: 4111-1111-1111-1111");
        assert!(!result.contains("4111"));
    }
}
