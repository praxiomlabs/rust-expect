//! PII redaction example.
//!
//! This example demonstrates detecting and redacting personally
//! identifiable information from terminal output.
//!
//! Run with: `cargo run --example pii_redaction --features pii-redaction`

#[cfg(feature = "pii-redaction")]
use rust_expect::pii::{
    contains_pii, redact, redact_asterisks, PiiDetector, PiiRedactor, PiiType, RedactionStyle,
};

fn main() {
    println!("rust-expect PII Redaction Example");
    println!("==================================\n");

    #[cfg(not(feature = "pii-redaction"))]
    {
        println!("This example requires the 'pii-redaction' feature.");
        println!("Run with: cargo run --example pii_redaction --features pii-redaction");
        return;
    }

    #[cfg(feature = "pii-redaction")]
    run_examples();
}

#[cfg(feature = "pii-redaction")]
fn run_examples() {
    // Example 1: Quick PII detection
    println!("1. Quick PII detection...");

    let texts = [
        "Hello, world!",
        "SSN: 123-45-6789",
        "Email: user@example.com",
        "Card: 4111-1111-1111-1111",
        "Phone: (555) 123-4567",
    ];

    for text in texts {
        let has_pii = contains_pii(text);
        println!("   '{}' -> PII: {}", text, has_pii);
    }

    // Example 2: Quick redaction
    println!("\n2. Quick redaction...");

    let sensitive = "Contact: john@example.com, SSN: 123-45-6789";
    let redacted = redact(sensitive);
    println!("   Original: {}", sensitive);
    println!("   Redacted: {}", redacted);

    // Example 3: Asterisk redaction
    println!("\n3. Asterisk style redaction...");

    let card_info = "Credit card: 4111-1111-1111-1111";
    let masked = redact_asterisks(card_info);
    println!("   Original: {}", card_info);
    println!("   Masked:   {}", masked);

    // Example 4: PII Detector
    println!("\n4. PII Detector with match details...");

    let detector = PiiDetector::new();
    let text = "User data: SSN 123-45-6789, email test@test.com";

    if detector.contains_pii(text) {
        println!("   Found PII in: '{}'", text);
        // The detector can identify specific PII types
        println!("   Types detected: SSN, Email");
    }

    // Example 5: Configurable redactor
    println!("\n5. Configurable PII Redactor...");

    // Default redactor with placeholders
    let redactor = PiiRedactor::new();
    let sample = "Email: admin@company.com, Phone: 555-1234";
    println!("   Original: {}", sample);
    println!("   Default:  {}", redactor.redact(sample));

    // Redactor with asterisks
    let asterisk_redactor = PiiRedactor::new().style(RedactionStyle::Asterisks);
    println!("   Asterisks: {}", asterisk_redactor.redact(sample));

    // Example 6: PII Types
    println!("\n6. PII types supported...");

    let pii_types = [
        (PiiType::Ssn, "Social Security Numbers (XXX-XX-XXXX)"),
        (PiiType::CreditCard, "Credit card numbers"),
        (PiiType::Email, "Email addresses"),
        (PiiType::Phone, "Phone numbers"),
        (PiiType::ApiKey, "API keys and tokens"),
    ];

    for (pii_type, description) in pii_types {
        println!("   {:?}: {}", pii_type, description);
    }

    // Example 7: Real-world log sanitization
    println!("\n7. Log sanitization example...");

    let log_entries = [
        "[INFO] User login: user@example.com",
        "[DEBUG] Processing payment for card 4111111111111111",
        "[WARN] Failed auth for SSN 987-65-4321",
        "[ERROR] API key exposed: sk_live_abc123xyz",
    ];

    let redactor = PiiRedactor::new();

    println!("   Sanitized logs:");
    for entry in log_entries {
        let safe = redactor.redact(entry);
        println!("   {}", safe);
    }

    // Example 8: Integration with session output
    println!("\n8. Session output sanitization...");

    // Simulate terminal output that might contain PII
    let terminal_output = r#"
        Database query results:
        - User: john.doe@company.com
        - SSN: 555-12-3456
        - Card ending: ****1234
        - Balance: $1,234.56
    "#;

    let safe_output = redactor.redact(terminal_output);
    println!("   Sanitized output would hide SSN and email");
    println!("   Original has {} characters", terminal_output.len());
    println!("   Redacted has {} characters", safe_output.len());

    println!("\nPII redaction examples completed successfully!");
}
