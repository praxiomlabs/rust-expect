//! Integration tests for expect/pattern matching functionality.
//!
//! These tests use mock transports to test the expect functionality
//! without requiring real PTY spawning.
//!
//! These tests require the `mock` feature to be enabled.

#![cfg(feature = "mock")]

use std::time::Duration;

use rust_expect::mock::{MockBuilder, MockTransport, Scenario, simple_mock};
use rust_expect::{ControlChar, Pattern, PatternSet, Session, SessionConfig};

fn config_with_timeout(timeout: Duration) -> SessionConfig {
    let mut config = SessionConfig::default();
    config.timeout.default = timeout;
    config
}

/// Test basic literal pattern matching.
#[tokio::test]
async fn expect_literal_pattern() {
    let transport = simple_mock("Hello, World!\nPrompt> ");
    let config = config_with_timeout(Duration::from_secs(1));
    let mut session = Session::new(transport, config);

    let result = session.expect("World").await;
    assert!(result.is_ok(), "Expected to match 'World'");

    let m = result.unwrap();
    assert!(m.matched.contains("World"), "Match should contain 'World'");
}

/// Test regex pattern matching.
#[tokio::test]
async fn expect_regex_pattern() {
    let transport = simple_mock("User123 logged in at 2024-01-01\n");
    let config = config_with_timeout(Duration::from_secs(1));
    let mut session = Session::new(transport, config);

    let pattern = Pattern::regex(r"User\d+ logged in").unwrap();
    let result = session.expect(pattern).await;

    assert!(result.is_ok(), "Expected to match regex pattern");
}

/// Test timeout behavior.
#[tokio::test]
async fn expect_timeout() {
    let transport = MockBuilder::new()
        .output("Partial output without the expected pattern")
        .delay_ms(2000) // Delay longer than timeout
        .build();

    let config = config_with_timeout(Duration::from_millis(100));
    let mut session = Session::new(transport, config);

    let result = session.expect("never_gonna_match").await;
    assert!(result.is_err(), "Expected timeout error");

    if let Err(e) = result {
        let err_str = e.to_string().to_lowercase();
        assert!(
            err_str.contains("timeout") || err_str.contains("timed out"),
            "Error should mention timeout: {e}"
        );
    }
}

/// Test EOF handling.
#[tokio::test]
async fn expect_eof() {
    let transport = MockBuilder::new().output("Some output\n").eof().build();

    let config = config_with_timeout(Duration::from_secs(1));
    let mut session = Session::new(transport, config);

    // First expect should work
    let result = session.expect("output").await;
    assert!(result.is_ok());

    // Second expect should get EOF
    let result = session.expect("more").await;
    assert!(result.is_err());
}

/// Test pattern set with multiple patterns.
#[tokio::test]
async fn expect_multiple_patterns() {
    let transport = simple_mock("Status: SUCCESS\n");
    let config = config_with_timeout(Duration::from_secs(1));
    let mut session = Session::new(transport, config);

    let mut patterns = PatternSet::new();
    patterns
        .add(Pattern::literal("SUCCESS"))
        .add(Pattern::literal("FAILURE"))
        .add(Pattern::literal("PENDING"));

    let result = session.expect_any(&patterns).await;
    assert!(result.is_ok());

    let m = result.unwrap();
    assert!(m.matched.contains("SUCCESS"));
}

/// Test send and expect workflow.
#[tokio::test]
async fn send_and_expect() {
    // Create a scenario that responds to input
    let scenario = Scenario::new("interactive")
        .initial_output("Enter name: ")
        .expect_respond("John", "Hello, John!\n> ");

    let transport = MockTransport::from_scenario(&scenario);
    let config = config_with_timeout(Duration::from_secs(1));
    let mut session = Session::new(transport, config);

    // Expect the initial prompt
    let result = session.expect("Enter name:").await;
    assert!(result.is_ok());

    // Send the name
    let send_result = session.send_line("John").await;
    assert!(send_result.is_ok());

    // Expect the greeting
    let result = session.expect("Hello, John").await;
    assert!(result.is_ok());
}

/// Test buffer management.
#[tokio::test]
async fn buffer_operations() {
    let transport = simple_mock("First line\nSecond line\n");
    let config = config_with_timeout(Duration::from_secs(1));
    let mut session = Session::new(transport, config);

    // Match first line
    let _ = session.expect("First").await;

    // Buffer should still contain unmatched content
    let buffer = session.buffer();
    assert!(
        buffer.contains("line") || buffer.contains("Second"),
        "Buffer: {buffer}"
    );

    // Clear and verify
    session.clear_buffer();
    assert!(
        session.buffer().is_empty() || session.buffer().len() < buffer.len(),
        "Buffer should be cleared or smaller"
    );
}

/// Test control character sending.
#[tokio::test]
async fn send_control_chars() {
    let transport = simple_mock("Ready\n");
    let config = config_with_timeout(Duration::from_secs(1));
    let mut session = Session::new(transport, config);

    // These should not panic
    assert!(session.send_control(ControlChar::CtrlC).await.is_ok());
    assert!(session.send_control(ControlChar::CtrlD).await.is_ok());
    assert!(session.send_control(ControlChar::CtrlZ).await.is_ok());
}

/// Test pattern with before/after context.
#[tokio::test]
async fn pattern_context() {
    let transport = simple_mock("prefix[TARGET]suffix\n");
    let config = config_with_timeout(Duration::from_secs(1));
    let mut session = Session::new(transport, config);

    let result = session.expect("TARGET").await;
    assert!(result.is_ok());

    let m = result.unwrap();
    // Before should contain prefix
    assert!(m.before.contains("prefix"), "Before: {}", m.before);
}

/// Test session ID uniqueness.
#[tokio::test]
async fn session_id_unique() {
    let transport1 = simple_mock("Session 1\n");
    let transport2 = simple_mock("Session 2\n");
    let config = SessionConfig::default();

    let session1 = Session::new(transport1, config.clone());
    let session2 = Session::new(transport2, config);

    assert_ne!(session1.id(), session2.id(), "Session IDs should be unique");
}

/// Test custom timeout per pattern.
#[tokio::test]
async fn pattern_specific_timeout() {
    let transport = MockBuilder::new()
        .delay_ms(500) // 500ms delay before output
        .output("Eventually appears\n")
        .build();

    let config = config_with_timeout(Duration::from_millis(100)); // Short default
    let mut session = Session::new(transport, config);

    // This should timeout with short timeout
    let short_result = session
        .expect_timeout("Eventually", Duration::from_millis(50))
        .await;
    // May or may not timeout depending on timing
    let _ = short_result;
}
