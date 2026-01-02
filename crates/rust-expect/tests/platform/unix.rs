//! Unix-specific platform tests.
//!
//! These tests verify Unix-specific functionality including:
//! - Unix shell configuration (bash, zsh, fish)
//! - Unix line ending handling (LF)
//! - Unix path handling in patterns
//! - PTY operations

use rust_expect::prelude::*;
use rust_expect::{Dialog, DialogStep};
use std::time::Duration;

/// Test Unix line ending detection.
#[test]
fn unix_line_endings() {
    use rust_expect::encoding::{detect_line_ending, LineEndingStyle};

    // Unix-style line endings
    let lf_text = "line1\nline2\nline3";
    assert_eq!(detect_line_ending(lf_text), Some(LineEndingStyle::Lf));

    // No line endings returns None
    let no_newline = "single line";
    assert_eq!(detect_line_ending(no_newline), None);
}

/// Test LF normalization for Unix.
#[test]
fn lf_normalization() {
    use rust_expect::encoding::{normalize_line_endings, LineEndingStyle};

    // Normalize CRLF to LF (for processing Windows output)
    let windows_text = "line1\r\nline2\r\nline3";
    let unix_text = normalize_line_endings(windows_text, LineEndingStyle::Lf);
    assert_eq!(unix_text, "line1\nline2\nline3");

    // Already LF should remain unchanged
    let lf_text = "line1\nline2\nline3";
    let normalized = normalize_line_endings(lf_text, LineEndingStyle::Lf);
    assert_eq!(normalized, lf_text);
}

/// Test SessionBuilder with Unix-specific configuration.
#[test]
fn session_builder_unix() {
    let builder = SessionBuilder::new()
        .command("/bin/bash")
        .arg("-c")
        .arg("echo hello")
        .timeout(Duration::from_secs(30))
        .env("PS1", "$ ");

    let config = builder.build();
    assert_eq!(config.command, "/bin/bash");
    assert!(config.args.contains(&"-c".to_string()));
}

/// Test that Unix control characters are handled correctly.
#[test]
fn unix_control_chars() {
    use rust_expect::ControlChar;

    // Ctrl+C (SIGINT)
    let ctrl_c = ControlChar::CtrlC;
    assert_eq!(ctrl_c.as_byte(), 0x03);

    // Ctrl+D (EOF)
    let ctrl_d = ControlChar::CtrlD;
    assert_eq!(ctrl_d.as_byte(), 0x04);

    // Ctrl+Z (SIGTSTP)
    let ctrl_z = ControlChar::CtrlZ;
    assert_eq!(ctrl_z.as_byte(), 0x1A);

    // Ctrl+\ (SIGQUIT)
    let ctrl_backslash = ControlChar::CtrlBackslash;
    assert_eq!(ctrl_backslash.as_byte(), 0x1C);
}

/// Test Unix environment variable handling.
#[test]
fn unix_env_vars() {
    let builder = SessionBuilder::new()
        .command("/bin/bash")
        .env("HOME", "/home/testuser")
        .env("USER", "testuser")
        .env("SHELL", "/bin/bash")
        .env("TERM", "xterm-256color");

    let config = builder.build();
    assert_eq!(config.env.get("HOME"), Some(&"/home/testuser".to_string()));
    assert_eq!(config.env.get("TERM"), Some(&"xterm-256color".to_string()));
}

/// Test Pattern matching with Unix paths.
#[test]
fn pattern_unix_paths() {
    // Unix paths
    let pattern = Pattern::literal("/home/");
    let text = "Current directory: /home/testuser";
    assert!(pattern.matches(text).is_some());

    // Root paths
    let root_pattern = Pattern::literal("/");
    let root_text = "Mounted at /";
    assert!(root_pattern.matches(root_text).is_some());
}

/// Test regex patterns with Unix-specific content.
#[test]
fn regex_unix_patterns() {
    // Match Unix absolute paths
    let path_pattern = Pattern::regex(r"/[a-z]+(/[a-z]+)*").unwrap();
    assert!(path_pattern.matches("/usr/local/bin").is_some());
    assert!(path_pattern.matches("/home/user").is_some());

    // Match Unix process patterns
    let process_pattern = Pattern::regex(r"\d+\s+\S+").unwrap();
    assert!(process_pattern.matches("12345 /bin/bash").is_some());
}

/// Test Dialog with Unix-style prompts.
#[test]
fn dialog_unix_prompts() {
    let dialog = Dialog::named("unix_shell")
        .step(DialogStep::new("prompt")
            .with_expect("$ ")
            .with_send("ls -la\n"))
        .step(DialogStep::new("output")
            .with_expect("total"));

    assert_eq!(dialog.len(), 2);
    assert_eq!(dialog.name, "unix_shell");
}

/// Test QuickSession Unix helpers.
#[test]
fn quick_session_unix() {
    // bash helper
    let bash_config = QuickSession::bash();
    assert_eq!(bash_config.command, "/bin/bash");
    assert!(bash_config.args.contains(&"--norc".to_string()));

    // zsh helper
    let zsh_config = QuickSession::zsh();
    assert_eq!(zsh_config.command, "/bin/zsh");
    assert!(zsh_config.args.contains(&"--no-rcs".to_string()));

    // fish helper
    let fish_config = QuickSession::fish();
    assert_eq!(fish_config.command, "fish");
    assert!(fish_config.args.contains(&"--no-config".to_string()));

    // python helper
    let python_config = QuickSession::python();
    assert_eq!(python_config.command, "python3");
    assert!(python_config.args.contains(&"-i".to_string()));

    // PowerShell (pwsh on Unix)
    let pwsh_config = QuickSession::powershell();
    assert_eq!(pwsh_config.command, "pwsh");
}

/// Test default shell detection on Unix.
#[test]
fn default_shell_unix() {
    let shell = QuickSession::default_shell();
    // Should either be from SHELL env var or /bin/sh
    assert!(shell.contains("sh") || shell.contains("zsh") || shell.contains("bash") || shell.contains("fish"));
}

/// Test shell type name method.
#[test]
fn shell_type_names() {
    use rust_expect::ShellType;

    assert_eq!(ShellType::Bash.name(), "bash");
    assert_eq!(ShellType::Zsh.name(), "zsh");
    assert_eq!(ShellType::Sh.name(), "sh");
    assert_eq!(ShellType::Fish.name(), "fish");
}
