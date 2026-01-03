//! Windows-specific platform tests.
//!
//! These tests verify Windows-specific functionality including:
//! - Windows shell configuration (cmd.exe, PowerShell)
//! - Windows line ending handling (CRLF)
//! - Windows path handling in patterns
//! - Windows PTY backend configuration
//! - Windows-specific QuickSession helpers

#![cfg(windows)]

use rust_expect::prelude::*;
use rust_expect::{Dialog, DialogStep};
use rust_expect::backend::{BackendType, PtyConfig};
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

/// Test PTY backend availability on Windows.
#[test]
fn pty_backend_windows() {
    assert!(BackendType::Pty.is_available());
    assert_eq!(BackendType::Pty.name(), "pty");
}

/// Test PtyConfig for Windows.
#[test]
fn pty_config_windows() {
    let config = PtyConfig::default();
    assert_eq!(config.rows, 24);
    assert_eq!(config.cols, 80);
}

/// Test Windows-specific session timeouts.
#[test]
fn windows_session_timeout() {
    let config = SessionBuilder::new()
        .command("cmd.exe")
        .timeout(Duration::from_secs(60))
        .read_timeout(Duration::from_secs(30))
        .build();

    assert_eq!(config.timeout.default, Duration::from_secs(60));
    assert_eq!(config.timeout.read, Some(Duration::from_secs(30)));
}

/// Test Windows-style script patterns.
#[test]
fn windows_script_patterns() {
    // Batch file patterns
    let batch_pattern = Pattern::regex(r"\.bat|\.cmd").unwrap();
    assert!(batch_pattern.matches("script.bat").is_some());
    assert!(batch_pattern.matches("install.cmd").is_some());

    // PowerShell script patterns
    let ps_pattern = Pattern::regex(r"\.ps1|\.psm1").unwrap();
    assert!(ps_pattern.matches("setup.ps1").is_some());
    assert!(ps_pattern.matches("module.psm1").is_some());
}

/// Test Windows error message patterns.
#[test]
fn windows_error_patterns() {
    // Common Windows error messages
    let error_pattern = Pattern::regex(r"'[^']+' is not recognized").unwrap();
    assert!(error_pattern.matches("'foo' is not recognized as an internal or external command").is_some());

    let access_denied = Pattern::literal("Access is denied");
    assert!(access_denied.matches("Error: Access is denied.").is_some());
}

/// Test Windows-specific prompt patterns.
#[test]
fn windows_prompt_patterns() {
    // Standard cmd.exe prompt
    let cmd_prompt = Pattern::regex(r"[A-Z]:\\[^>]*>").unwrap();
    assert!(cmd_prompt.matches("C:\\Users\\test>").is_some());
    assert!(cmd_prompt.matches("D:\\Projects\\app>").is_some());

    // PowerShell prompt
    let ps_prompt = Pattern::regex(r"PS [A-Z]:\\[^>]+> ").unwrap();
    assert!(ps_prompt.matches("PS C:\\Users\\test> ").is_some());
}

/// Test Windows registry path patterns.
#[test]
fn windows_registry_patterns() {
    let hklm_pattern = Pattern::literal("HKEY_LOCAL_MACHINE");
    assert!(hklm_pattern.matches("HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft").is_some());

    let reg_pattern = Pattern::regex(r"HKEY_(LOCAL_MACHINE|CURRENT_USER|CLASSES_ROOT)\\").unwrap();
    assert!(reg_pattern.matches("HKEY_LOCAL_MACHINE\\SOFTWARE").is_some());
    assert!(reg_pattern.matches("HKEY_CURRENT_USER\\Software").is_some());
}

/// Test Python configuration on Windows.
#[test]
fn quick_session_python_windows() {
    let config = QuickSession::python();
    assert_eq!(config.command, "python");
    assert!(config.args.contains(&"-i".to_string()));
}

/// Test default shell detection on Windows.
#[test]
fn windows_default_shell() {
    let shell = QuickSession::default_shell();
    // On Windows without SHELL env var, should default to cmd.exe
    if std::env::var("SHELL").is_err() {
        assert_eq!(shell, "cmd.exe");
    }
}

/// Test Windows-specific control sequences in Dialog.
#[test]
fn dialog_windows_control() {
    use rust_expect::ControlChar;

    let dialog = Dialog::named("windows_interrupt")
        .step(DialogStep::new("running")
            .with_expect("Running...")
            .with_send_control(ControlChar::CtrlC))
        .step(DialogStep::new("stopped")
            .with_expect("Terminated"));

    assert_eq!(dialog.len(), 2);
}

/// Test Windows-specific timeout configuration.
#[test]
fn windows_timeout_config() {
    use rust_expect::util::TimeoutConfig;

    let config = TimeoutConfig::uniform(Duration::from_secs(30));
    assert_eq!(config.default, Duration::from_secs(30));
    assert_eq!(config.read, Some(Duration::from_secs(30)));
    assert_eq!(config.expect, Some(Duration::from_secs(30)));
}
