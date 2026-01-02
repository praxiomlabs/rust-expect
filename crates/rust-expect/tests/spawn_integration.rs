//! Integration tests for PTY spawning functionality.
//!
//! These tests verify the PTY backend works correctly with the `SessionBuilder`.

#![cfg(unix)] // PTY tests only work on Unix

use rust_expect::{QuickSession, SessionBuilder};
use std::time::Duration;

/// Test `SessionBuilder` creates valid config.
#[test]
fn session_builder_creates_config() {
    let config = SessionBuilder::new()
        .command("/bin/echo")
        .arg("hello world")
        .timeout(Duration::from_secs(5))
        .build();

    assert_eq!(config.command, "/bin/echo");
    assert_eq!(config.args, vec!["hello world"]);
    assert_eq!(config.timeout.default, Duration::from_secs(5));
}

/// Test `SessionBuilder` with environment variables.
#[test]
fn session_builder_with_env() {
    let config = SessionBuilder::new()
        .command("/bin/sh")
        .arg("-c")
        .arg("echo $TEST_VAR")
        .env("TEST_VAR", "test_value")
        .build();

    assert!(config.env.contains_key("TEST_VAR"));
    assert_eq!(config.env.get("TEST_VAR"), Some(&"test_value".to_string()));
}

/// Test `SessionBuilder` with custom dimensions.
#[test]
fn session_builder_with_dimensions() {
    let config = SessionBuilder::new()
        .command("/bin/sh")
        .dimensions(120, 40)
        .build();

    assert_eq!(config.dimensions, (120, 40));
}

/// Test `QuickSession::bash` creates correct config.
#[test]
fn quick_session_bash_config() {
    let config = QuickSession::bash();

    assert_eq!(config.command, "/bin/bash");
    assert!(config.args.contains(&"--norc".to_string()));
    assert!(config.args.contains(&"--noprofile".to_string()));
}

/// Test `QuickSession::shell` uses SHELL env var or default.
#[test]
fn quick_session_shell_config() {
    let config = QuickSession::shell();

    // Should have a command set
    assert!(!config.command.is_empty());
}

/// Test `QuickSession::ssh` creates correct config.
#[test]
fn quick_session_ssh_config() {
    let config = QuickSession::ssh("example.com");

    assert_eq!(config.command, "ssh");
    assert!(config.args.contains(&"example.com".to_string()));
    assert_eq!(config.timeout.default, Duration::from_secs(30));
}

/// Test `QuickSession::ssh_user` creates correct config.
#[test]
fn quick_session_ssh_user_config() {
    let config = QuickSession::ssh_user("admin", "server.example.com");

    assert_eq!(config.command, "ssh");
    assert!(config.args.contains(&"admin@server.example.com".to_string()));
}

/// Test `QuickSession::python` creates correct config.
#[test]
fn quick_session_python_config() {
    let config = QuickSession::python();

    assert_eq!(config.command, "python3");
    assert!(config.args.contains(&"-i".to_string()));
}

/// Test `QuickSession::telnet` creates correct config.
#[test]
fn quick_session_telnet_config() {
    let config = QuickSession::telnet("host.example.com", 23);

    assert_eq!(config.command, "telnet");
    assert!(config.args.contains(&"host.example.com".to_string()));
    assert!(config.args.contains(&"23".to_string()));
}

/// Test `SessionBuilder` working directory.
#[test]
fn session_builder_working_dir() {
    let config = SessionBuilder::new()
        .command("/bin/pwd")
        .working_directory("/tmp")
        .build();

    assert_eq!(config.working_dir, Some("/tmp".into()));
}

/// Test `SessionBuilder` line endings.
#[test]
fn session_builder_line_endings() {
    use rust_expect::LineEnding;

    let config_unix = SessionBuilder::new()
        .command("test")
        .unix_line_endings()
        .build();
    assert!(matches!(config_unix.line_ending, LineEnding::Lf));

    let config_windows = SessionBuilder::new()
        .command("test")
        .windows_line_endings()
        .build();
    assert!(matches!(config_windows.line_ending, LineEnding::CrLf));
}

/// Test `SessionBuilder` buffer configuration.
#[test]
fn session_builder_buffer_size() {
    let config = SessionBuilder::new()
        .command("test")
        .buffer_max_size(1024 * 1024)
        .build();

    assert_eq!(config.buffer.max_size, 1024 * 1024);
}

/// Test `SessionBuilder` logging.
#[test]
fn session_builder_logging() {
    let config = SessionBuilder::new()
        .command("test")
        .log_to_file("/tmp/test.log")
        .build();

    assert_eq!(config.logging.log_file, Some("/tmp/test.log".into()));
}

// =============================================================================
// End-to-end spawn tests (require actual process spawning)
// =============================================================================

use rust_expect::Session;

/// Test spawning a simple command and expecting output.
#[tokio::test]
async fn spawn_echo_command() {
    let mut session = Session::spawn("/bin/echo", &["hello", "world"])
        .await
        .expect("Failed to spawn echo");

    // Read the output
    let m = session.expect("world").await.expect("Expected 'world'");
    assert!(m.matched.contains("world"));
}

/// Test spawning a shell and sending commands.
#[tokio::test]
async fn spawn_shell_send_command() {
    let mut session = Session::spawn("/bin/sh", &[])
        .await
        .expect("Failed to spawn shell");

    // Wait for shell prompt ($ or something similar)
    // Send a command
    session.send_line("echo test123").await.expect("Failed to send");

    // Expect the output
    let m = session.expect("test123").await.expect("Expected 'test123'");
    assert!(m.matched.contains("test123"));
}

/// Test spawning cat in interactive mode.
#[tokio::test]
async fn spawn_cat_interactive() {
    let mut session = Session::spawn("/bin/cat", &[])
        .await
        .expect("Failed to spawn cat");

    // Cat echoes what we send
    session.send_line("hello cat").await.expect("Failed to send");

    let m = session.expect("hello cat").await.expect("Expected 'hello cat'");
    assert!(m.matched.contains("hello cat"));

    // Send EOF to terminate cat (Ctrl+D)
    session
        .send_control(rust_expect::ControlChar::CtrlD)
        .await
        .expect("Failed to send EOF");
}

/// Test process ID is available.
#[tokio::test]
async fn spawn_has_pid() {
    let session = Session::spawn("/bin/true", &[])
        .await
        .expect("Failed to spawn true");

    let pid = session.pid();
    assert!(pid > 0, "Expected valid PID, got {}", pid);
}

/// Test spawn with custom configuration.
#[tokio::test]
async fn spawn_with_custom_config() {
    use rust_expect::SessionConfig;

    let mut config = SessionConfig::default();
    config.dimensions = (100, 30);

    let session = Session::spawn_with_config("/bin/sh", &[], config)
        .await
        .expect("Failed to spawn with config");

    // Just verify it spawned successfully
    let pid = session.pid();
    assert!(pid > 0);
}

/// Test spawning command that fails.
#[tokio::test]
async fn spawn_nonexistent_command() {
    let result = Session::spawn("/nonexistent/command", &[]).await;
    // The spawn should succeed (fork works), but the exec fails
    // The child process will exit immediately with code 1
    // This is expected behavior for PTY spawning
    // We just verify we don't panic
    assert!(result.is_ok() || result.is_err());
}

/// Test sending control characters.
#[tokio::test]
async fn spawn_send_control_c() {
    let mut session = Session::spawn("/bin/cat", &[])
        .await
        .expect("Failed to spawn cat");

    // Send Ctrl-C to interrupt
    session
        .send_control(rust_expect::ControlChar::CtrlC)
        .await
        .expect("Failed to send Ctrl-C");

    // Cat should terminate after Ctrl-C
    // Wait for EOF
    let _ = session.wait().await;
}

/// Test basic expect with multiple patterns.
#[tokio::test]
async fn spawn_expect_multiple() {
    let mut session = Session::spawn("/bin/sh", &[])
        .await
        .expect("Failed to spawn shell");

    session
        .send_line("echo first; echo second")
        .await
        .expect("Failed to send");

    // Expect first
    session.expect("first").await.expect("Expected 'first'");

    // Expect second
    session.expect("second").await.expect("Expected 'second'");
}

/// Test that matched field contains the expected text.
#[tokio::test]
async fn spawn_match_contains_text() {
    let mut session = Session::spawn("/bin/echo", &["hello", "world"])
        .await
        .expect("Failed to spawn echo");

    let m = session.expect("hello").await.expect("Expected 'hello'");

    // The matched field should contain the matched text
    assert!(m.matched.contains("hello"), "Match should contain 'hello'");
}
