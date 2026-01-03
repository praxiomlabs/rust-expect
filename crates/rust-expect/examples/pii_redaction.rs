//! PII redaction example.
//!
//! This example demonstrates detecting and redacting personally
//! identifiable information from terminal output.
//!
//! Run with: `cargo run --example pii_redaction --features pii-redaction`

#[cfg(feature = "pii-redaction")]
use rust_expect::pii::{
    contains_pii, redact, redact_asterisks, PiiDetector, PiiRedactor, PiiType, RedactionStyle,
    StreamingRedactor,
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

    // Example 9: Streaming PII redaction
    println!("\n9. Streaming PII redaction...");
    demonstrate_streaming_redaction();

    // Example 10: Real-time session integration
    println!("\n10. Real-time session integration pattern...");
    demonstrate_session_integration();

    println!("\nPII redaction examples completed successfully!");
}

#[cfg(feature = "pii-redaction")]
fn demonstrate_streaming_redaction() {
    println!("   Streaming redaction processes data chunk by chunk,");
    println!("   buffering until safe boundaries are found.");
    println!();

    // Create a streaming redactor with custom buffer size
    let redactor = PiiRedactor::new();
    let mut streaming = StreamingRedactor::new(redactor).max_buffer(256);

    // Simulate receiving data in chunks (like from a terminal)
    let chunks = [
        "Connecting to database...\n",
        "User authenticated: admin@",   // Email split across chunks!
        "company.com\n",
        "Loading profile for SSN: 123",  // SSN split across chunks!
        "-45-6789\n",
        "Session complete.\n",
    ];

    println!("   Processing {} chunks:", chunks.len());
    let mut combined_output = String::new();

    for (i, chunk) in chunks.iter().enumerate() {
        let output = streaming.process(chunk);
        if !output.is_empty() {
            println!("   Chunk {}: Emitted {} chars", i + 1, output.len());
            combined_output.push_str(&output);
        } else {
            println!("   Chunk {}: Buffering (waiting for boundary)", i + 1);
        }
    }

    // Flush remaining buffer
    let final_output = streaming.flush();
    if !final_output.is_empty() {
        println!("   Flush: Emitted {} chars", final_output.len());
        combined_output.push_str(&final_output);
    }

    println!();
    println!("   Combined redacted output:");
    for line in combined_output.lines() {
        println!("     {}", line);
    }

    // Verify PII was redacted even when split across chunks
    assert!(!combined_output.contains("admin@company.com"));
    assert!(!combined_output.contains("123-45-6789"));
    println!();
    println!("   Verified: PII redacted even when split across chunks");
}

#[cfg(feature = "pii-redaction")]
fn demonstrate_session_integration() {
    println!("   Pattern for integrating with rust-expect sessions:");
    println!();
    println!("   ```rust");
    println!("   let redactor = PiiRedactor::new();");
    println!("   let mut streaming = StreamingRedactor::new(redactor);");
    println!();
    println!("   // In your session read loop:");
    println!("   loop {{");
    println!("       let data = session.read().await?;");
    println!("       let safe = streaming.process(&data);");
    println!("       if !safe.is_empty() {{");
    println!("           log::info!(\"{{safe}}\");  // Safe to log");
    println!("       }}");
    println!("   }}");
    println!();
    println!("   // On session end:");
    println!("   let remaining = streaming.flush();");
    println!("   log::info!(\"{{remaining}}\");");
    println!("   ```");
}
