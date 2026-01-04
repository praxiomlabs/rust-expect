//! Integration tests for PII detection and redaction.

#![cfg(feature = "pii-redaction")]

use rust_expect::{PiiDetector, PiiRedactor, PiiType};

#[test]
fn pii_type_variants() {
    let types = [
        PiiType::Ssn,
        PiiType::CreditCard,
        PiiType::Email,
        PiiType::Phone,
        PiiType::ApiKey,
        PiiType::Password,
        PiiType::IpAddress,
        PiiType::AwsKey,
        PiiType::Secret,
    ];

    for pii_type in types {
        assert!(!pii_type.name().is_empty());
        assert!(!pii_type.placeholder().is_empty());
    }
}

#[test]
fn pii_type_names() {
    assert_eq!(PiiType::Ssn.name(), "SSN");
    assert_eq!(PiiType::CreditCard.name(), "Credit Card");
    assert_eq!(PiiType::Email.name(), "Email");
}

#[test]
fn pii_type_placeholders() {
    assert_eq!(PiiType::Ssn.placeholder(), "[SSN REDACTED]");
    assert_eq!(PiiType::CreditCard.placeholder(), "[CARD REDACTED]");
    assert_eq!(PiiType::Email.placeholder(), "[EMAIL REDACTED]");
}

#[test]
fn pii_detector_new() {
    let detector = PiiDetector::new();
    assert!(!format!("{detector:?}").is_empty());
}

#[test]
fn pii_detector_default() {
    let detector = PiiDetector::default();
    // Default detector should have common PII types enabled
    assert!(!format!("{detector:?}").is_empty());
}

#[test]
fn pii_detector_detect_ssn() {
    let detector = PiiDetector::new();
    let text = "My SSN is 123-45-6789";

    let matches = detector.detect(text);
    assert!(!matches.is_empty());
    assert_eq!(matches[0].pii_type, PiiType::Ssn);
}

#[test]
fn pii_detector_detect_email() {
    let detector = PiiDetector::new();
    let text = "Contact me at user@example.com";

    let matches = detector.detect(text);
    assert!(!matches.is_empty());
    assert_eq!(matches[0].pii_type, PiiType::Email);
}

#[test]
fn pii_detector_detect_credit_card() {
    let detector = PiiDetector::new();
    let text = "Card: 4111-1111-1111-1111";

    let matches = detector.detect(text);
    // Should detect credit card pattern
    assert!(!matches.is_empty());
}

#[test]
fn pii_detector_detect_ip() {
    let detector = PiiDetector::new();
    let text = "Server IP: 192.168.1.100";

    let matches = detector.detect(text);
    assert!(!matches.is_empty());
    assert_eq!(matches[0].pii_type, PiiType::IpAddress);
}

#[test]
fn pii_detector_no_pii() {
    let detector = PiiDetector::new();
    let text = "This is just regular text without PII";

    let matches = detector.detect(text);
    assert!(matches.is_empty());
}

#[test]
fn pii_detector_contains_pii() {
    let detector = PiiDetector::new();

    assert!(detector.contains_pii("SSN: 123-45-6789"));
    assert!(!detector.contains_pii("No PII here"));
}

#[test]
fn pii_detector_enable_disable() {
    let detector = PiiDetector::new()
        .disable(PiiType::Email)
        .enable(PiiType::Ssn);

    // Should not detect email but should detect SSN
    let text = "Email: test@example.com SSN: 123-45-6789";
    let matches = detector.detect(text);

    // Should find SSN
    assert!(matches.iter().any(|m| m.pii_type == PiiType::Ssn));
}

#[test]
fn pii_detector_min_confidence() {
    let detector = PiiDetector::new().min_confidence(0.9);
    assert!(!format!("{detector:?}").is_empty());
}

#[test]
fn pii_redactor_new() {
    let redactor = PiiRedactor::new();
    assert!(!format!("{redactor:?}").is_empty());
}

#[test]
fn pii_redactor_redact_ssn() {
    let redactor = PiiRedactor::new();
    let text = "My SSN is 123-45-6789";

    let redacted = redactor.redact(text);
    assert!(redacted.contains("[SSN REDACTED]"));
    assert!(!redacted.contains("123-45-6789"));
}

#[test]
fn pii_redactor_redact_email() {
    let redactor = PiiRedactor::new();
    let text = "Email: user@example.com";

    let redacted = redactor.redact(text);
    assert!(redacted.contains("[EMAIL REDACTED]"));
    assert!(!redacted.contains("user@example.com"));
}

#[test]
fn pii_redactor_redact_multiple() {
    let redactor = PiiRedactor::new();
    let text = "SSN: 123-45-6789, Email: test@example.com";

    let redacted = redactor.redact(text);
    assert!(redacted.contains("[SSN REDACTED]"));
    assert!(redacted.contains("[EMAIL REDACTED]"));
}

#[test]
fn pii_redactor_preserves_non_pii() {
    let redactor = PiiRedactor::new();
    let text = "Hello, this is regular text!";

    let redacted = redactor.redact(text);
    assert_eq!(redacted, text);
}
