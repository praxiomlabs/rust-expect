//! Windows-specific platform tests.
//!
//! These tests verify Windows-specific functionality including:
//! - Windows shell configuration (cmd.exe, PowerShell)
//! - Windows line ending handling (CRLF)
//! - Windows path handling in patterns

#![cfg(windows)]

use rust_expect::prelude::*;
use rust_expect::{Dialog, DialogStep};
use std::time::Duration;

/// Test Windows line ending detection.
#[test]
fn windows_line_endings() {
    use rust_expect::encoding::{detect_line_ending, LineEndingStyle};

    // Windows-style line endings
    let crlf_text = "line1\r\nline2\r\nline3";
    assert_eq!(detect_line_ending(crlf_text), Some(LineEndingStyle::CrLf));

    // Mixed line endings should detect based on first occurrence
    let mixed_text = "line1\r\nline2\nline3";
    assert_eq!(detect_line_ending(mixed_text), Some(LineEndingStyle::CrLf));
}

/// Test CRLF normalization for Windows.
#[test]
fn crlf_normalization() {
    use rust_expect::encoding::{normalize_line_endings, LineEndingStyle};

    // Normalize LF to CRLF (for sending to Windows programs)
    let unix_text = "line1\nline2\nline3";
    let windows_text = normalize_line_endings(unix_text, LineEndingStyle::CrLf);
    assert_eq!(windows_text, "line1\r\nline2\r\nline3");

    // Already CRLF should remain unchanged
    let crlf_text = "line1\r\nline2\r\nline3";
    let normalized = normalize_line_endings(crlf_text, LineEndingStyle::CrLf);
    assert_eq!(normalized, crlf_text);
}

/// Test SessionBuilder with Windows-specific configuration.
#[test]
fn session_builder_windows() {
    let builder = SessionBuilder::new()
        .command("cmd.exe")
        .arg("/c")
        .arg("echo hello")
        .timeout(Duration::from_secs(30))
        .env("PROMPT", "$P$G");

    let config = builder.build();
    assert_eq!(config.command, "cmd.exe");
    assert!(config.args.contains(&"/c".to_string()));
}

/// Test that Windows control characters are handled correctly.
#[test]
fn windows_control_chars() {
    use rust_expect::ControlChar;

    // Ctrl+C (interrupt)
    let ctrl_c = ControlChar::CtrlC;
    assert_eq!(ctrl_c.as_byte(), 0x03);

    // Ctrl+D (EOF on Unix, also works on Windows)
    let ctrl_d = ControlChar::CtrlD;
    assert_eq!(ctrl_d.as_byte(), 0x04);

    // Ctrl+Z (SIGTSTP on Unix, EOF on Windows console)
    let ctrl_z = ControlChar::CtrlZ;
    assert_eq!(ctrl_z.as_byte(), 0x1A);
}

/// Test Windows environment variable handling.
#[test]
fn windows_env_vars() {
    let builder = SessionBuilder::new()
        .command("cmd.exe")
        .env("COMPUTERNAME", "TESTPC")
        .env("USERNAME", "testuser")
        .env("USERPROFILE", "C:\\Users\\testuser");

    let config = builder.build();
    assert_eq!(config.env.get("COMPUTERNAME"), Some(&"TESTPC".to_string()));
    assert_eq!(config.env.get("USERNAME"), Some(&"testuser".to_string()));
}

/// Test Pattern matching with Windows paths.
#[test]
fn pattern_windows_paths() {
    // Windows paths with backslashes
    let pattern = Pattern::literal("C:\\Users\\");
    let text = "Current directory: C:\\Users\\testuser";
    assert!(pattern.matches(text).is_some());

    // UNC paths
    let unc_pattern = Pattern::literal("\\\\server\\share");
    let unc_text = "Mapped drive: \\\\server\\share\\folder";
    assert!(unc_pattern.matches(unc_text).is_some());
}

/// Test regex patterns with Windows-specific content.
#[test]
fn regex_windows_patterns() {
    // Match Windows drive letters
    let drive_pattern = Pattern::regex(r"[A-Z]:\\").unwrap();
    assert!(drive_pattern.matches("C:\\Windows\\System32").is_some());
    assert!(drive_pattern.matches("D:\\Data\\Files").is_some());

    // Match Windows process names
    let process_pattern = Pattern::regex(r"\w+\.exe").unwrap();
    assert!(process_pattern.matches("Running: notepad.exe").is_some());
}

/// Test Dialog with Windows-style prompts.
#[test]
fn dialog_windows_prompts() {
    let dialog = Dialog::named("windows_cmd")
        .step(DialogStep::new("prompt")
            .with_expect(">")
            .with_send("dir\r\n"))
        .step(DialogStep::new("output")
            .with_expect("Directory of"));

    assert_eq!(dialog.len(), 2);
    assert_eq!(dialog.name, "windows_cmd");
}

/// Test QuickSession Windows helpers.
#[test]
fn quick_session_windows() {
    // cmd.exe helper
    let cmd_config = QuickSession::cmd();
    assert_eq!(cmd_config.command, "cmd.exe");
    assert_eq!(cmd_config.line_ending, LineEnding::CrLf);

    // PowerShell helper
    let ps_config = QuickSession::powershell();
    assert_eq!(ps_config.command, "powershell.exe");
    assert!(ps_config.args.contains(&"-NoLogo".to_string()));
}
