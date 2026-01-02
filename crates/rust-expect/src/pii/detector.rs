//! PII detection utilities.
//!
//! This module provides pattern-based detection of personally
//! identifiable information (PII) in text.

use regex::Regex;
use std::sync::LazyLock;

/// Type of PII detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PiiType {
    /// Social Security Number.
    Ssn,
    /// Credit card number.
    CreditCard,
    /// Email address.
    Email,
    /// Phone number.
    Phone,
    /// API key or token.
    ApiKey,
    /// Password (based on context).
    Password,
    /// IP address.
    IpAddress,
    /// AWS access key.
    AwsKey,
    /// Generic secret or token.
    Secret,
}

impl PiiType {
    /// Get a human-readable name for this PII type.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Ssn => "SSN",
            Self::CreditCard => "Credit Card",
            Self::Email => "Email",
            Self::Phone => "Phone",
            Self::ApiKey => "API Key",
            Self::Password => "Password",
            Self::IpAddress => "IP Address",
            Self::AwsKey => "AWS Key",
            Self::Secret => "Secret",
        }
    }

    /// Get the default redaction placeholder for this PII type.
    #[must_use]
    pub const fn placeholder(&self) -> &'static str {
        match self {
            Self::Ssn => "[SSN REDACTED]",
            Self::CreditCard => "[CARD REDACTED]",
            Self::Email => "[EMAIL REDACTED]",
            Self::Phone => "[PHONE REDACTED]",
            Self::ApiKey => "[API KEY REDACTED]",
            Self::Password => "[PASSWORD REDACTED]",
            Self::IpAddress => "[IP REDACTED]",
            Self::AwsKey => "[AWS KEY REDACTED]",
            Self::Secret => "[SECRET REDACTED]",
        }
    }
}

/// A detected PII match.
#[derive(Debug, Clone)]
pub struct PiiMatch {
    /// Type of PII detected.
    pub pii_type: PiiType,
    /// Start position in the text.
    pub start: usize,
    /// End position in the text.
    pub end: usize,
    /// The matched text.
    pub text: String,
    /// Confidence level (0.0 - 1.0).
    pub confidence: f32,
}

impl PiiMatch {
    /// Get the length of the match.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.end - self.start
    }

    /// Check if the match is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// Compiled patterns for PII detection.
/// These are compile-time constant patterns that are validated during development.
static SSN_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b\d{3}-\d{2}-\d{4}\b")
        .expect("SSN pattern is a valid regex")
});

static CREDIT_CARD_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:\d{4}[- ]?){3}\d{4}\b")
        .expect("Credit card pattern is a valid regex")
});

static EMAIL_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b")
        .expect("Email pattern is a valid regex")
});

static PHONE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:\+?1[-. ]?)?(?:\([0-9]{3}\)|[0-9]{3})[-. ]?[0-9]{3}[-. ]?[0-9]{4}\b")
        .expect("Phone pattern is a valid regex")
});

static API_KEY_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\b(?:api[_-]?key|apikey|token|secret|password|auth)['"]?\s*[:=]\s*['"]?([A-Za-z0-9_-]{16,})['"]?"#)
        .expect("API key pattern is a valid regex")
});

static AWS_KEY_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b")
        .expect("AWS key pattern is a valid regex")
});

static IP_ADDRESS_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b")
        .expect("IP address pattern is a valid regex")
});

/// A PII detector.
#[derive(Debug, Clone)]
pub struct PiiDetector {
    /// Types of PII to detect.
    enabled_types: Vec<PiiType>,
    /// Minimum confidence threshold.
    min_confidence: f32,
}

impl Default for PiiDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl PiiDetector {
    /// Create a new detector with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            enabled_types: vec![
                PiiType::Ssn,
                PiiType::CreditCard,
                PiiType::Email,
                PiiType::Phone,
                PiiType::ApiKey,
                PiiType::AwsKey,
                PiiType::IpAddress,
            ],
            min_confidence: 0.5,
        }
    }

    /// Enable detection of a specific PII type.
    #[must_use]
    pub fn enable(mut self, pii_type: PiiType) -> Self {
        if !self.enabled_types.contains(&pii_type) {
            self.enabled_types.push(pii_type);
        }
        self
    }

    /// Disable detection of a specific PII type.
    #[must_use]
    pub fn disable(mut self, pii_type: PiiType) -> Self {
        self.enabled_types.retain(|t| t != &pii_type);
        self
    }

    /// Set the minimum confidence threshold.
    #[must_use]
    pub const fn min_confidence(mut self, threshold: f32) -> Self {
        self.min_confidence = threshold;
        self
    }

    /// Detect PII in the given text.
    #[must_use]
    pub fn detect(&self, text: &str) -> Vec<PiiMatch> {
        let mut matches = Vec::new();

        for pii_type in &self.enabled_types {
            matches.extend(self.detect_type(text, *pii_type));
        }

        // Sort by position and remove overlaps
        matches.sort_by_key(|m| m.start);
        matches
    }

    /// Detect a specific type of PII.
    fn detect_type(&self, text: &str, pii_type: PiiType) -> Vec<PiiMatch> {
        let pattern = match pii_type {
            PiiType::Ssn => &*SSN_PATTERN,
            PiiType::CreditCard => &*CREDIT_CARD_PATTERN,
            PiiType::Email => &*EMAIL_PATTERN,
            PiiType::Phone => &*PHONE_PATTERN,
            PiiType::ApiKey => &*API_KEY_PATTERN,
            PiiType::AwsKey => &*AWS_KEY_PATTERN,
            PiiType::IpAddress => &*IP_ADDRESS_PATTERN,
            PiiType::Password | PiiType::Secret => return Vec::new(),
        };

        pattern
            .find_iter(text)
            .filter_map(|m| {
                let confidence = self.calculate_confidence(pii_type, m.as_str());
                if confidence >= self.min_confidence {
                    Some(PiiMatch {
                        pii_type,
                        start: m.start(),
                        end: m.end(),
                        text: m.as_str().to_string(),
                        confidence,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Calculate confidence for a match.
    fn calculate_confidence(&self, pii_type: PiiType, text: &str) -> f32 {
        match pii_type {
            PiiType::Ssn => {
                // Validate SSN format
                if text.len() == 11 {
                    0.9
                } else {
                    0.5
                }
            }
            PiiType::CreditCard => {
                // Luhn check would increase confidence
                if luhn_check(text) {
                    0.95
                } else {
                    0.4
                }
            }
            PiiType::Email => {
                // Most email patterns are high confidence
                0.9
            }
            PiiType::Phone => {
                // Phone numbers are context-dependent
                0.7
            }
            PiiType::ApiKey | PiiType::AwsKey => {
                // API keys are high confidence when matched
                0.85
            }
            PiiType::IpAddress => {
                // IP addresses could be public
                0.6
            }
            _ => 0.5,
        }
    }

    /// Check if text contains any PII.
    #[must_use]
    pub fn contains_pii(&self, text: &str) -> bool {
        !self.detect(text).is_empty()
    }
}

/// Perform Luhn check on a credit card number.
fn luhn_check(number: &str) -> bool {
    let digits: Vec<u32> = number
        .chars()
        .filter(char::is_ascii_digit)
        .filter_map(|c| c.to_digit(10))
        .collect();

    if digits.len() < 13 || digits.len() > 19 {
        return false;
    }

    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| {
            if i % 2 == 1 {
                let doubled = d * 2;
                if doubled > 9 {
                    doubled - 9
                } else {
                    doubled
                }
            } else {
                d
            }
        })
        .sum();

    sum % 10 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_ssn() {
        let detector = PiiDetector::new();
        let matches = detector.detect("My SSN is 123-45-6789");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pii_type, PiiType::Ssn);
    }

    #[test]
    fn detect_email() {
        let detector = PiiDetector::new();
        let matches = detector.detect("Contact me at user@example.com");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pii_type, PiiType::Email);
    }

    #[test]
    fn detect_credit_card() {
        let detector = PiiDetector::new();
        let matches = detector.detect("Card: 4111-1111-1111-1111");
        assert!(!matches.is_empty());
    }

    #[test]
    fn luhn_valid() {
        assert!(luhn_check("4111111111111111")); // Test Visa
        assert!(luhn_check("5500000000000004")); // Test MasterCard
    }

    #[test]
    fn luhn_invalid() {
        assert!(!luhn_check("1234567890123456"));
    }
}
