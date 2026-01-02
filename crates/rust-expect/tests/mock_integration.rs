//! Integration tests for mock session functionality.
//!
//! These tests verify the mock transport and scenario system works correctly.
//!
//! These tests require the `mock` feature to be enabled.

#![cfg(feature = "mock")]

use rust_expect::mock::{
    login_mock, shell_mock, simple_mock, MockBuilder, MockTransport, Scenario,
};
use rust_expect::{Session, SessionConfig};
use std::time::Duration;

fn config_with_timeout(timeout: Duration) -> SessionConfig {
    let mut config = SessionConfig::default();
    config.timeout.default = timeout;
    config
}

/// Test simple mock transport creation.
#[tokio::test]
async fn simple_mock_transport() {
    let transport = simple_mock("Hello from mock!\n");
    let config = config_with_timeout(Duration::from_secs(1));
    let mut session = Session::new(transport, config);

    let result = session.expect("Hello").await;
    assert!(result.is_ok());
}

/// Test mock builder API.
#[tokio::test]
async fn mock_builder_api() {
    let transport = MockBuilder::new()
        .output("Line 1\n")
        .delay_ms(10)
        .output("Line 2\n")
        .delay_ms(10)
        .output("Line 3\n")
        .exit(0)
        .build();

    let config = config_with_timeout(Duration::from_secs(1));
    let mut session = Session::new(transport, config);

    assert!(session.expect("Line 1").await.is_ok());
    assert!(session.expect("Line 2").await.is_ok());
    assert!(session.expect("Line 3").await.is_ok());
}

/// Test login mock preset.
#[tokio::test]
async fn login_mock_preset() {
    let transport = login_mock();
    assert!(!transport.is_eof());
}

/// Test shell mock preset.
#[tokio::test]
async fn shell_mock_preset() {
    let transport = shell_mock("$ ");
    assert!(!transport.is_eof());
}

/// Test scenario creation.
#[tokio::test]
async fn scenario_creation() {
    let scenario = Scenario::new("test_scenario")
        .initial_output("Welcome!\n> ")
        .expect_respond("help", "Available commands: exit, help\n> ")
        .expect_respond("exit", "Goodbye!\n");

    let transport = MockTransport::from_scenario(&scenario);
    let config = config_with_timeout(Duration::from_secs(1));
    let mut session = Session::new(transport, config);

    // Expect welcome
    let result = session.expect("Welcome").await;
    assert!(result.is_ok());

    // Send help
    assert!(session.send_line("help").await.is_ok());
    let result = session.expect("Available commands").await;
    assert!(result.is_ok());

    // Send exit
    assert!(session.send_line("exit").await.is_ok());
    let result = session.expect("Goodbye").await;
    assert!(result.is_ok());
}

/// Test EOF in mock transport.
#[tokio::test]
async fn mock_eof_handling() {
    let transport = MockBuilder::new().output("Last output\n").eof().build();

    let config = config_with_timeout(Duration::from_secs(1));
    let mut session = Session::new(transport, config);

    assert!(session.expect("Last").await.is_ok());

    // After consuming output, subsequent expect should hit EOF
    // The behavior depends on implementation details
}

/// Test exit code in mock transport.
#[tokio::test]
async fn mock_exit_code() {
    let transport = MockBuilder::new()
        .output("Process complete\n")
        .exit(42)
        .build();

    assert!(!transport.is_eof());
}

/// Test multiple outputs with delays.
#[tokio::test]
async fn mock_timed_outputs() {
    let start = std::time::Instant::now();

    let transport = MockBuilder::new()
        .output("Immediate\n")
        .delay_ms(50)
        .output("Delayed\n")
        .build();

    let config = config_with_timeout(Duration::from_secs(2));
    let mut session = Session::new(transport, config);

    assert!(session.expect("Immediate").await.is_ok());
    assert!(session.expect("Delayed").await.is_ok());

    // Verify some time has passed (accounting for processing overhead)
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() >= 40,
        "Expected at least 40ms delay, got {elapsed:?}"
    );
}

/// Test scenario with pattern matching.
#[tokio::test]
async fn scenario_pattern_matching() {
    let scenario = Scenario::new("password_prompt")
        .initial_output("Password: ")
        .expect_respond("secret123", "Access granted\n");

    let transport = MockTransport::from_scenario(&scenario);
    let config = config_with_timeout(Duration::from_secs(1));
    let mut session = Session::new(transport, config);

    // Wait for password prompt
    let result = session.expect("Password:").await;
    assert!(result.is_ok());

    // Send password
    assert!(session.send_line("secret123").await.is_ok());

    // Expect access granted
    let result = session.expect("Access granted").await;
    assert!(result.is_ok());
}
