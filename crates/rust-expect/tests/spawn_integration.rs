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
