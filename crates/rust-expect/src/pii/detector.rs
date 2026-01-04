//! PII detection utilities.
//!
//! This module provides pattern-based detection of personally
//! identifiable information (PII) in text.
//!
//! # Custom Patterns
//!
//! In addition to built-in PII types, you can register custom patterns:
//!
//! ```rust
//! use rust_expect::pii::PiiDetector;
//!
//! let detector = PiiDetector::new()
//!     .add_pattern("employee_id", r"EMP-\d{6}", "[EMPLOYEE ID]", 0.9)
//!     .add_pattern("project_code", r"PROJ-[A-Z]{4}", "[PROJECT CODE]", 0.85);
//!
//! let matches = detector.detect("Contact EMP-123456 about PROJ-ABCD");
//! assert_eq!(matches.len(), 2);
//! ```

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
    /// Custom pattern (use `PiiMatch::custom_name()` for pattern name).
    Custom,
}

impl PiiType {
    /// Get a human-readable name for this PII type.
    ///
    /// For `Custom` types, this returns "Custom". Use `PiiMatch::custom_name()`
    /// to get the actual pattern name.
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
            Self::Custom => "Custom",
        }
    }

    /// Get the default redaction placeholder for this PII type.
    ///
    /// For `Custom` types, this returns `"[REDACTED]"`. Use `PiiMatch::placeholder()`
    /// to get the actual custom placeholder.
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
            Self::Custom => "[REDACTED]",
        }
    }

    /// Check if this is a custom pattern type.
    #[must_use]
    pub const fn is_custom(&self) -> bool {
        matches!(self, Self::Custom)
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
    /// Custom pattern name (only set for `PiiType::Custom`).
    custom_name: Option<String>,
    /// Custom placeholder (only set for `PiiType::Custom`).
    custom_placeholder: Option<String>,
}

impl PiiMatch {
    /// Create a new PII match for a built-in type.
    #[must_use]
    pub const fn new(
        pii_type: PiiType,
        start: usize,
        end: usize,
        text: String,
        confidence: f32,
    ) -> Self {
        Self {
            pii_type,
            start,
            end,
            text,
            confidence,
            custom_name: None,
            custom_placeholder: None,
        }
    }

    /// Create a new PII match for a custom pattern.
    #[must_use]
    pub fn custom(
        start: usize,
        end: usize,
        text: String,
        confidence: f32,
        name: impl Into<String>,
        placeholder: impl Into<String>,
    ) -> Self {
        Self {
            pii_type: PiiType::Custom,
            start,
            end,
            text,
            confidence,
            custom_name: Some(name.into()),
            custom_placeholder: Some(placeholder.into()),
        }
    }

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

    /// Get the custom pattern name, if this is a custom match.
    #[must_use]
    pub fn custom_name(&self) -> Option<&str> {
        self.custom_name.as_deref()
    }

    /// Get the effective name for this match.
    ///
    /// Returns the custom name for custom patterns, or the built-in type name.
    #[must_use]
    pub fn name(&self) -> &str {
        self.custom_name
            .as_deref()
            .unwrap_or_else(|| self.pii_type.name())
    }

    /// Get the effective placeholder for this match.
    ///
    /// Returns the custom placeholder for custom patterns, or the built-in placeholder.
    #[must_use]
    pub fn placeholder(&self) -> &str {
        self.custom_placeholder
            .as_deref()
            .unwrap_or_else(|| self.pii_type.placeholder())
    }

    /// Check if this match is from a custom pattern.
    #[must_use]
    pub const fn is_custom(&self) -> bool {
        self.pii_type.is_custom()
    }
}

/// Compiled patterns for PII detection.
/// These are compile-time constant patterns that are validated during development.
static SSN_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").expect("SSN pattern is a valid regex"));

static CREDIT_CARD_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:\d{4}[- ]?){3}\d{4}\b").expect("Credit card pattern is a valid regex")
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
    Regex::new(r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b").expect("AWS key pattern is a valid regex")
});

static IP_ADDRESS_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b")
        .expect("IP address pattern is a valid regex")
});

/// A custom pattern for PII detection.
///
/// Custom patterns allow you to define your own regex-based detection rules
/// with custom names and placeholders.
#[derive(Debug, Clone)]
pub struct CustomPattern {
    /// Name of the pattern (e.g., `"employee_id"`).
    name: String,
    /// Compiled regex pattern.
    regex: Regex,
    /// Placeholder for redaction.
    placeholder: String,
    /// Confidence score for matches.
    confidence: f32,
}

impl CustomPattern {
    /// Create a new custom pattern.
    ///
    /// # Errors
    ///
    /// Returns an error if the regex pattern is invalid.
    pub fn new(
        name: impl Into<String>,
        pattern: &str,
        placeholder: impl Into<String>,
        confidence: f32,
    ) -> Result<Self, regex::Error> {
        Ok(Self {
            name: name.into(),
            regex: Regex::new(pattern)?,
            placeholder: placeholder.into(),
            confidence: confidence.clamp(0.0, 1.0),
        })
    }

    /// Get the pattern name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the placeholder for redaction.
    #[must_use]
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Get the confidence score.
    #[must_use]
    pub const fn confidence(&self) -> f32 {
        self.confidence
    }

    /// Find all matches in the given text.
    fn find_matches(&self, text: &str) -> Vec<PiiMatch> {
        self.regex
            .find_iter(text)
            .map(|m| {
                PiiMatch::custom(
                    m.start(),
                    m.end(),
                    m.as_str().to_string(),
                    self.confidence,
                    &self.name,
                    &self.placeholder,
                )
            })
            .collect()
    }
}

/// A PII detector.
#[derive(Debug, Clone)]
pub struct PiiDetector {
    /// Types of PII to detect.
    enabled_types: Vec<PiiType>,
    /// Minimum confidence threshold.
    min_confidence: f32,
    /// Custom patterns.
    custom_patterns: Vec<CustomPattern>,
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
            custom_patterns: Vec::new(),
        }
    }

    /// Create a detector with only custom patterns (no built-in types).
    #[must_use]
    pub const fn custom_only() -> Self {
        Self {
            enabled_types: Vec::new(),
            min_confidence: 0.5,
            custom_patterns: Vec::new(),
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

    /// Add a custom pattern for detection.
    ///
    /// # Panics
    ///
    /// Panics if the regex pattern is invalid. Use `try_add_pattern` for
    /// fallible pattern registration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rust_expect::pii::PiiDetector;
    ///
    /// let detector = PiiDetector::new()
    ///     .add_pattern("employee_id", r"EMP-\d{6}", "[EMPLOYEE ID]", 0.9);
    /// ```
    #[must_use]
    pub fn add_pattern(
        mut self,
        name: impl Into<String>,
        pattern: &str,
        placeholder: impl Into<String>,
        confidence: f32,
    ) -> Self {
        let custom = CustomPattern::new(name, pattern, placeholder, confidence)
            .expect("invalid regex pattern");
        self.custom_patterns.push(custom);
        self
    }

    /// Try to add a custom pattern for detection.
    ///
    /// Returns an error if the regex pattern is invalid.
    ///
    /// # Errors
    ///
    /// Returns `regex::Error` if the pattern is not a valid regex.
    pub fn try_add_pattern(
        mut self,
        name: impl Into<String>,
        pattern: &str,
        placeholder: impl Into<String>,
        confidence: f32,
    ) -> Result<Self, regex::Error> {
        let custom = CustomPattern::new(name, pattern, placeholder, confidence)?;
        self.custom_patterns.push(custom);
        Ok(self)
    }

    /// Add a pre-built custom pattern.
    #[must_use]
    pub fn with_pattern(mut self, pattern: CustomPattern) -> Self {
        self.custom_patterns.push(pattern);
        self
    }

    /// Get the number of custom patterns registered.
    #[must_use]
    pub fn custom_pattern_count(&self) -> usize {
        self.custom_patterns.len()
    }

    /// Get the names of all registered custom patterns.
    #[must_use]
    pub fn custom_pattern_names(&self) -> Vec<&str> {
        self.custom_patterns
            .iter()
            .map(CustomPattern::name)
            .collect()
    }

    /// Detect PII in the given text.
    #[must_use]
    pub fn detect(&self, text: &str) -> Vec<PiiMatch> {
        let mut matches = Vec::new();

        // Detect built-in types
        for pii_type in &self.enabled_types {
            matches.extend(self.detect_type(text, *pii_type));
        }

        // Detect custom patterns
        for pattern in &self.custom_patterns {
            let pattern_matches = pattern.find_matches(text);
            for m in pattern_matches {
                if m.confidence >= self.min_confidence {
                    matches.push(m);
                }
            }
        }

        // Sort by position
        matches.sort_by_key(|m| m.start);

        // Remove overlapping matches (keep higher confidence)
        Self::remove_overlaps(&mut matches);

        matches
    }

    /// Remove overlapping matches, keeping the higher confidence one.
    fn remove_overlaps(matches: &mut Vec<PiiMatch>) {
        if matches.len() < 2 {
            return;
        }

        let mut i = 0;
        while i < matches.len() - 1 {
            let current_end = matches[i].end;
            let next_start = matches[i + 1].start;

            if next_start < current_end {
                // Overlap detected - keep the one with higher confidence
                if matches[i].confidence >= matches[i + 1].confidence {
                    matches.remove(i + 1);
                } else {
                    matches.remove(i);
                }
            } else {
                i += 1;
            }
        }
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
            // Custom patterns are handled in detect(), Password/Secret have no built-in patterns
            PiiType::Password | PiiType::Secret | PiiType::Custom => return Vec::new(),
        };

        pattern
            .find_iter(text)
            .filter_map(|m| {
                let confidence = self.calculate_confidence(pii_type, m.as_str());
                if confidence >= self.min_confidence {
                    Some(PiiMatch::new(
                        pii_type,
                        m.start(),
                        m.end(),
                        m.as_str().to_string(),
                        confidence,
                    ))
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
                if text.len() == 11 { 0.9 } else { 0.5 }
            }
            PiiType::CreditCard => {
                // Luhn check would increase confidence
                if luhn_check(text) { 0.95 } else { 0.4 }
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
                if doubled > 9 { doubled - 9 } else { doubled }
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

    #[test]
    fn custom_pattern_basic() {
        let detector =
            PiiDetector::new().add_pattern("employee_id", r"EMP-\d{6}", "[EMPLOYEE ID]", 0.9);

        let matches = detector.detect("Contact EMP-123456 for help");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pii_type, PiiType::Custom);
        assert_eq!(matches[0].text, "EMP-123456");
        assert_eq!(matches[0].custom_name(), Some("employee_id"));
        assert_eq!(matches[0].placeholder(), "[EMPLOYEE ID]");
    }

    #[test]
    fn custom_pattern_multiple() {
        let detector = PiiDetector::custom_only()
            .add_pattern("employee_id", r"EMP-\d{6}", "[EMPLOYEE ID]", 0.9)
            .add_pattern("project_code", r"PROJ-[A-Z]{4}", "[PROJECT CODE]", 0.85);

        let matches = detector.detect("EMP-123456 is working on PROJ-ABCD");
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].custom_name(), Some("employee_id"));
        assert_eq!(matches[1].custom_name(), Some("project_code"));
    }

    #[test]
    fn custom_pattern_with_builtin() {
        let detector =
            PiiDetector::new().add_pattern("employee_id", r"EMP-\d{6}", "[EMPLOYEE ID]", 0.9);

        let matches = detector.detect("EMP-123456 can be reached at user@example.com");
        assert_eq!(matches.len(), 2);

        // First should be employee ID (starts earlier)
        assert!(matches[0].is_custom());
        assert_eq!(matches[0].name(), "employee_id");

        // Second should be email
        assert!(!matches[1].is_custom());
        assert_eq!(matches[1].pii_type, PiiType::Email);
    }

    #[test]
    fn custom_pattern_try_add() {
        let result = PiiDetector::new().try_add_pattern("test", r"[a-z]+", "[TEST]", 0.8);
        assert!(result.is_ok());

        let result = PiiDetector::new().try_add_pattern("invalid", r"[invalid", "[INVALID]", 0.8);
        assert!(result.is_err());
    }

    #[test]
    fn custom_pattern_count() {
        let detector = PiiDetector::new()
            .add_pattern("one", r"one", "[1]", 0.9)
            .add_pattern("two", r"two", "[2]", 0.9);

        assert_eq!(detector.custom_pattern_count(), 2);
        assert_eq!(detector.custom_pattern_names(), vec!["one", "two"]);
    }

    #[test]
    fn custom_pattern_confidence_threshold() {
        let detector = PiiDetector::custom_only()
            .min_confidence(0.95)
            .add_pattern("low_conf", r"test", "[TEST]", 0.5)
            .add_pattern("high_conf", r"demo", "[DEMO]", 0.99);

        let matches = detector.detect("test and demo");
        // Only high_conf pattern should match due to threshold
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].text, "demo");
    }

    #[test]
    fn pii_match_methods() {
        let builtin = PiiMatch::new(PiiType::Email, 0, 16, "user@example.com".to_string(), 0.9);
        assert!(!builtin.is_custom());
        assert_eq!(builtin.name(), "Email");
        assert_eq!(builtin.placeholder(), "[EMAIL REDACTED]");
        assert_eq!(builtin.custom_name(), None);

        let custom = PiiMatch::custom(0, 10, "EMP-123456".to_string(), 0.9, "employee_id", "[EMP]");
        assert!(custom.is_custom());
        assert_eq!(custom.name(), "employee_id");
        assert_eq!(custom.placeholder(), "[EMP]");
        assert_eq!(custom.custom_name(), Some("employee_id"));
    }

    #[test]
    fn custom_pattern_struct() {
        let pattern = CustomPattern::new("test", r"\d+", "[NUMBER]", 0.8).unwrap();
        assert_eq!(pattern.name(), "test");
        assert_eq!(pattern.placeholder(), "[NUMBER]");
        assert!((pattern.confidence() - 0.8).abs() < 0.001);
    }
}
