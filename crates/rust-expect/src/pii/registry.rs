//! Custom PII pattern registry.
//!
//! This module provides a centralized registry for managing custom PII patterns.
//! It includes pre-defined pattern sets for common compliance scenarios.
//!
//! # Example
//!
//! ```rust
//! use rust_expect::pii::{PatternRegistry, PiiDetector};
//!
//! // Load pre-built patterns
//! let registry = PatternRegistry::new()
//!     .with_healthcare()
//!     .with_financial();
//!
//! // Apply to detector
//! let detector = registry.apply(PiiDetector::new());
//! ```

use std::collections::HashMap;

use super::detector::{CustomPattern, PiiDetector};

/// A pattern set name for common compliance scenarios.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PatternSet {
    /// Healthcare identifiers (MRN, NPI, DEA).
    Healthcare,
    /// Financial identifiers (IBAN, SWIFT, routing numbers).
    Financial,
    /// Government identifiers (passport, driver license, national ID).
    Government,
    /// Network identifiers (MAC addresses, UUIDs).
    Network,
    /// Authentication (JWT tokens, OAuth tokens).
    Authentication,
    /// Cloud provider identifiers (GCP, Azure keys).
    Cloud,
    /// Cryptocurrency (wallet addresses).
    Cryptocurrency,
    /// Employee identifiers (typical corporate patterns).
    Corporate,
}

/// Entry for a pattern in the registry.
#[derive(Debug, Clone)]
pub struct PatternEntry {
    /// Pattern name.
    pub name: String,
    /// Regex pattern.
    pub pattern: String,
    /// Redaction placeholder.
    pub placeholder: String,
    /// Confidence score (0.0 - 1.0).
    pub confidence: f32,
    /// Description of what this pattern detects.
    pub description: String,
    /// Pattern set this belongs to (if pre-built).
    pub set: Option<PatternSet>,
}

impl PatternEntry {
    /// Create a new pattern entry.
    pub fn new(
        name: impl Into<String>,
        pattern: impl Into<String>,
        placeholder: impl Into<String>,
        confidence: f32,
    ) -> Self {
        Self {
            name: name.into(),
            pattern: pattern.into(),
            placeholder: placeholder.into(),
            confidence,
            description: String::new(),
            set: None,
        }
    }

    /// Add a description.
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Mark as part of a pattern set.
    #[must_use]
    pub const fn with_set(mut self, set: PatternSet) -> Self {
        self.set = Some(set);
        self
    }
}

/// Registry of custom PII patterns.
///
/// The registry provides a centralized way to manage custom patterns,
/// including pre-built pattern sets for common compliance scenarios.
#[derive(Debug, Clone, Default)]
pub struct PatternRegistry {
    /// All registered patterns.
    patterns: Vec<PatternEntry>,
    /// Named collections of patterns.
    collections: HashMap<String, Vec<String>>,
}

impl PatternRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a custom pattern.
    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, entry: PatternEntry) -> Self {
        self.patterns.push(entry);
        self
    }

    /// Add a pattern with minimal parameters.
    #[must_use]
    pub fn add_pattern(
        self,
        name: impl Into<String>,
        pattern: impl Into<String>,
        placeholder: impl Into<String>,
        confidence: f32,
    ) -> Self {
        self.add(PatternEntry::new(name, pattern, placeholder, confidence))
    }

    /// Get all registered patterns.
    #[must_use]
    pub fn patterns(&self) -> &[PatternEntry] {
        &self.patterns
    }

    /// Get pattern count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    /// Check if registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// Get patterns by set.
    #[must_use]
    pub fn get_by_set(&self, set: PatternSet) -> Vec<&PatternEntry> {
        self.patterns
            .iter()
            .filter(|p| p.set == Some(set))
            .collect()
    }

    /// Create a named collection from current patterns.
    #[must_use]
    pub fn save_collection(mut self, name: impl Into<String>) -> Self {
        let names: Vec<String> = self.patterns.iter().map(|p| p.name.clone()).collect();
        self.collections.insert(name.into(), names);
        self
    }

    /// Apply all patterns to a detector.
    #[must_use]
    pub fn apply(&self, mut detector: PiiDetector) -> PiiDetector {
        for entry in &self.patterns {
            if let Ok(pattern) = CustomPattern::new(
                &entry.name,
                &entry.pattern,
                &entry.placeholder,
                entry.confidence,
            ) {
                detector = detector.with_pattern(pattern);
            }
        }
        detector
    }

    /// Create a new detector with only registry patterns.
    #[must_use]
    pub fn to_detector(&self) -> PiiDetector {
        self.apply(PiiDetector::custom_only())
    }

    // ======= Pre-built Pattern Sets =======

    /// Add healthcare patterns (HIPAA-relevant).
    #[must_use]
    pub fn with_healthcare(self) -> Self {
        self.add(
            PatternEntry::new("mrn", r"\bMRN[-:]?\s*\d{6,10}\b", "[MRN REDACTED]", 0.85)
                .with_description("Medical Record Number")
                .with_set(PatternSet::Healthcare),
        )
        .add(
            PatternEntry::new("npi", r"\b(?:NPI[-:]?\s*)?\d{10}\b", "[NPI REDACTED]", 0.75)
                .with_description("National Provider Identifier")
                .with_set(PatternSet::Healthcare),
        )
        .add(
            PatternEntry::new("dea", r"\b[A-Z]{2}\d{7}\b", "[DEA REDACTED]", 0.7)
                .with_description("DEA Number")
                .with_set(PatternSet::Healthcare),
        )
        .add(
            PatternEntry::new(
                "health_plan_id",
                r"\b(?:member[-_]?id|policy[-_]?(?:no|num|number)?)\s*[:=]?\s*[A-Z0-9]{8,15}\b",
                "[HEALTH PLAN ID]",
                0.8,
            )
            .with_description("Health Plan Beneficiary Number")
            .with_set(PatternSet::Healthcare),
        )
    }

    /// Add financial patterns.
    #[must_use]
    pub fn with_financial(self) -> Self {
        self.add(
            PatternEntry::new(
                "iban",
                r"\b[A-Z]{2}\d{2}[A-Z0-9]{4,30}\b",
                "[IBAN REDACTED]",
                0.8,
            )
            .with_description("International Bank Account Number")
            .with_set(PatternSet::Financial),
        )
        .add(
            PatternEntry::new(
                "swift_bic",
                r"\b[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}(?:[A-Z0-9]{3})?\b",
                "[SWIFT REDACTED]",
                0.75,
            )
            .with_description("SWIFT/BIC Code")
            .with_set(PatternSet::Financial),
        )
        .add(
            PatternEntry::new("routing_number", r"\b\d{9}\b", "[ROUTING REDACTED]", 0.5)
                .with_description("US Bank Routing Number")
                .with_set(PatternSet::Financial),
        )
        .add(
            PatternEntry::new(
                "account_number",
                r"(?i)\baccount\s*(?:no|num|number|#)?\s*[:=]?\s*(\d{8,17})\b",
                "[ACCOUNT REDACTED]",
                0.85,
            )
            .with_description("Bank Account Number")
            .with_set(PatternSet::Financial),
        )
    }

    /// Add government ID patterns.
    #[must_use]
    pub fn with_government(self) -> Self {
        self
            .add(
                PatternEntry::new(
                    "passport",
                    r"(?i)\bpassport\s*(?:no|num|number|#)?\s*[:=]?\s*([A-Z0-9]{6,12})\b",
                    "[PASSPORT REDACTED]",
                    0.85,
                )
                .with_description("Passport Number")
                .with_set(PatternSet::Government),
            )
            .add(
                PatternEntry::new(
                    "drivers_license",
                    r"(?i)\b(?:dl|driver'?s?\s*lic(?:ense)?)\s*(?:no|num|number|#)?\s*[:=]?\s*([A-Z0-9]{5,15})\b",
                    "[DL REDACTED]",
                    0.8,
                )
                .with_description("Driver's License Number")
                .with_set(PatternSet::Government),
            )
            .add(
                PatternEntry::new(
                    "national_id",
                    r"(?i)\b(?:national\s*id|id\s*card)\s*(?:no|num|number|#)?\s*[:=]?\s*([A-Z0-9]{8,20})\b",
                    "[NATIONAL ID REDACTED]",
                    0.8,
                )
                .with_description("National ID Number")
                .with_set(PatternSet::Government),
            )
    }

    /// Add network patterns.
    #[must_use]
    pub fn with_network(self) -> Self {
        self.add(
            PatternEntry::new(
                "mac_address",
                r"\b(?:[0-9A-Fa-f]{2}[:-]){5}[0-9A-Fa-f]{2}\b",
                "[MAC REDACTED]",
                0.9,
            )
            .with_description("MAC Address")
            .with_set(PatternSet::Network),
        )
        .add(
            PatternEntry::new(
                "uuid",
                r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b",
                "[UUID REDACTED]",
                0.9,
            )
            .with_description("UUID")
            .with_set(PatternSet::Network),
        )
        .add(
            PatternEntry::new(
                "ipv6",
                r"\b(?:[0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}\b",
                "[IPv6 REDACTED]",
                0.85,
            )
            .with_description("IPv6 Address")
            .with_set(PatternSet::Network),
        )
    }

    /// Add authentication token patterns.
    #[must_use]
    pub fn with_authentication(self) -> Self {
        self.add(
            PatternEntry::new(
                "jwt",
                r"\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b",
                "[JWT REDACTED]",
                0.95,
            )
            .with_description("JSON Web Token")
            .with_set(PatternSet::Authentication),
        )
        .add(
            PatternEntry::new(
                "bearer_token",
                r"(?i)Bearer\s+([A-Za-z0-9_-]{20,})",
                "[BEARER TOKEN REDACTED]",
                0.9,
            )
            .with_description("Bearer Token")
            .with_set(PatternSet::Authentication),
        )
        .add(
            PatternEntry::new(
                "basic_auth",
                r"(?i)Basic\s+([A-Za-z0-9+/=]{20,})",
                "[BASIC AUTH REDACTED]",
                0.9,
            )
            .with_description("Basic Authentication Header")
            .with_set(PatternSet::Authentication),
        )
        .add(
            PatternEntry::new(
                "oauth_token",
                r#"(?i)(?:access_token|refresh_token)\s*[:=]\s*['"]?([A-Za-z0-9_-]{20,})['"]?"#,
                "[OAUTH TOKEN REDACTED]",
                0.9,
            )
            .with_description("OAuth Token")
            .with_set(PatternSet::Authentication),
        )
    }

    /// Add cloud provider patterns.
    #[must_use]
    pub fn with_cloud(self) -> Self {
        self
            .add(
                PatternEntry::new(
                    "gcp_api_key",
                    r"\bAIza[0-9A-Za-z_-]{35}\b",
                    "[GCP KEY REDACTED]",
                    0.95,
                )
                .with_description("Google Cloud API Key")
                .with_set(PatternSet::Cloud),
            )
            .add(
                PatternEntry::new(
                    "azure_key",
                    r#"(?i)\b(?:azure|subscription)[-_]?(?:key|secret)\s*[:=]\s*['"]?([A-Za-z0-9+/=]{20,})['"]?"#,
                    "[AZURE KEY REDACTED]",
                    0.85,
                )
                .with_description("Azure Key")
                .with_set(PatternSet::Cloud),
            )
            .add(
                PatternEntry::new(
                    "github_token",
                    r"\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{36,}\b",
                    "[GITHUB TOKEN REDACTED]",
                    0.95,
                )
                .with_description("GitHub Token")
                .with_set(PatternSet::Cloud),
            )
            .add(
                PatternEntry::new(
                    "slack_token",
                    r"\bxox[baprs]-[A-Za-z0-9-]+\b",
                    "[SLACK TOKEN REDACTED]",
                    0.95,
                )
                .with_description("Slack Token")
                .with_set(PatternSet::Cloud),
            )
    }

    /// Add cryptocurrency patterns.
    #[must_use]
    pub fn with_cryptocurrency(self) -> Self {
        self.add(
            PatternEntry::new(
                "bitcoin_address",
                r"\b(?:bc1|[13])[a-zA-HJ-NP-Z0-9]{25,39}\b",
                "[BTC ADDR REDACTED]",
                0.9,
            )
            .with_description("Bitcoin Address")
            .with_set(PatternSet::Cryptocurrency),
        )
        .add(
            PatternEntry::new(
                "ethereum_address",
                r"\b0x[a-fA-F0-9]{40}\b",
                "[ETH ADDR REDACTED]",
                0.9,
            )
            .with_description("Ethereum Address")
            .with_set(PatternSet::Cryptocurrency),
        )
    }

    /// Add typical corporate patterns.
    #[must_use]
    pub fn with_corporate(self) -> Self {
        self.add(
            PatternEntry::new(
                "employee_id",
                r"\b(?:EMP|E)[-#]?\d{5,8}\b",
                "[EMPLOYEE ID REDACTED]",
                0.85,
            )
            .with_description("Employee ID")
            .with_set(PatternSet::Corporate),
        )
        .add(
            PatternEntry::new(
                "badge_number",
                r"(?i)\bbadge\s*(?:no|num|number|#)?\s*[:=]?\s*(\d{4,10})\b",
                "[BADGE REDACTED]",
                0.8,
            )
            .with_description("Badge Number")
            .with_set(PatternSet::Corporate),
        )
        .add(
            PatternEntry::new(
                "internal_project",
                r"\b(?:PROJ|PRJ|PROJECT)[-#]\d{3,6}\b",
                "[PROJECT ID REDACTED]",
                0.75,
            )
            .with_description("Internal Project Code")
            .with_set(PatternSet::Corporate),
        )
    }

    /// Add all pre-built patterns.
    #[must_use]
    pub fn with_all(self) -> Self {
        self.with_healthcare()
            .with_financial()
            .with_government()
            .with_network()
            .with_authentication()
            .with_cloud()
            .with_cryptocurrency()
            .with_corporate()
    }

    /// Create a HIPAA-focused registry.
    #[must_use]
    pub fn hipaa() -> Self {
        Self::new().with_healthcare()
    }

    /// Create a PCI-DSS-focused registry.
    #[must_use]
    pub fn pci_dss() -> Self {
        Self::new().with_financial()
    }

    /// Create a comprehensive security-focused registry.
    #[must_use]
    pub fn security() -> Self {
        Self::new().with_authentication().with_cloud()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_new() {
        let registry = PatternRegistry::new();
        assert!(registry.is_empty());
    }

    #[test]
    fn registry_add_pattern() {
        let registry = PatternRegistry::new().add_pattern("test", r"\btest\b", "[TEST]", 0.9);

        assert_eq!(registry.len(), 1);
        assert_eq!(registry.patterns()[0].name, "test");
    }

    #[test]
    fn registry_with_healthcare() {
        let registry = PatternRegistry::new().with_healthcare();
        assert!(!registry.is_empty());

        let healthcare_patterns = registry.get_by_set(PatternSet::Healthcare);
        assert!(!healthcare_patterns.is_empty());
        assert!(healthcare_patterns.iter().any(|p| p.name == "mrn"));
    }

    #[test]
    fn registry_apply_to_detector() {
        let registry = PatternRegistry::new().add_pattern("emp_id", r"EMP-\d{6}", "[EMP]", 0.9);

        let detector = registry.apply(PiiDetector::new());
        let matches = detector.detect("Contact EMP-123456");

        assert_eq!(matches.len(), 1);
        assert!(matches[0].is_custom());
        assert_eq!(matches[0].name(), "emp_id");
    }

    #[test]
    fn registry_to_detector() {
        let registry = PatternRegistry::new().add_pattern("test", r"\btest\b", "[TEST]", 0.9);

        let detector = registry.to_detector();
        let matches = detector.detect("this is a test");

        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn registry_with_all() {
        let registry = PatternRegistry::new().with_all();
        assert!(registry.len() > 20); // Should have many patterns
    }

    #[test]
    fn registry_get_by_set() {
        let registry = PatternRegistry::new().with_all();

        let financial = registry.get_by_set(PatternSet::Financial);
        assert!(!financial.is_empty());
        assert!(
            financial
                .iter()
                .all(|p| p.set == Some(PatternSet::Financial))
        );

        let healthcare = registry.get_by_set(PatternSet::Healthcare);
        assert!(!healthcare.is_empty());
    }

    #[test]
    fn pre_built_registries() {
        let hipaa = PatternRegistry::hipaa();
        assert!(!hipaa.is_empty());

        let pci = PatternRegistry::pci_dss();
        assert!(!pci.is_empty());

        let security = PatternRegistry::security();
        assert!(!security.is_empty());
    }

    #[test]
    fn pattern_entry_with_description() {
        let entry = PatternEntry::new("test", r"\btest\b", "[TEST]", 0.9)
            .with_description("A test pattern")
            .with_set(PatternSet::Corporate);

        assert_eq!(entry.description, "A test pattern");
        assert_eq!(entry.set, Some(PatternSet::Corporate));
    }

    #[test]
    fn registry_detects_jwt() {
        let registry = PatternRegistry::new().with_authentication();
        let detector = registry.to_detector();

        // Sample JWT (not valid, just pattern matching)
        let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
        let matches = detector.detect(jwt);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name(), "jwt");
    }

    #[test]
    fn registry_detects_github_token() {
        let registry = PatternRegistry::new().with_cloud();
        let detector = registry.to_detector();

        let text = "GITHUB_TOKEN=ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        let matches = detector.detect(text);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name(), "github_token");
    }

    #[test]
    fn registry_detects_mac_address() {
        let registry = PatternRegistry::new().with_network();
        let detector = registry.to_detector();

        let matches = detector.detect("MAC: 00:1B:44:11:3A:B7");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name(), "mac_address");
    }

    #[test]
    fn registry_detects_bitcoin_address() {
        let registry = PatternRegistry::new().with_cryptocurrency();
        let detector = registry.to_detector();

        // Sample BTC address
        let matches = detector.detect("Send to 1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name(), "bitcoin_address");
    }
}
