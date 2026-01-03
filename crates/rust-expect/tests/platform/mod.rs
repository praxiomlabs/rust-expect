//! Platform-specific tests.
//!
//! This module contains tests that are specific to Unix or Windows platforms,
//! as well as cross-platform tests that verify platform configurations work
//! regardless of the host platform.

#[cfg(unix)]
mod unix;

#[cfg(windows)]
mod windows;

// Cross-platform tests that verify Windows configuration without requiring Windows
mod cross_platform {
    use rust_expect::prelude::*;
    use rust_expect::backend::{BackendType, PtyConfig};
    use std::time::Duration;

    /// Verify Windows line ending configuration works on any platform.
    #[test]
    fn windows_line_ending_config() {
        let config = SessionBuilder::new()
            .command("cmd.exe")
            .windows_line_endings()
            .build();

        assert_eq!(config.line_ending, LineEnding::CrLf);
    }

    /// Verify CRLF normalization works on any platform.
    #[test]
    fn crlf_normalization_cross_platform() {
        use rust_expect::encoding::{normalize_line_endings, LineEndingStyle};

        // This test verifies the encoding functions work regardless of platform
        let windows_text = "line1\r\nline2\r\n";
        let unix_text = normalize_line_endings(windows_text, LineEndingStyle::Lf);
        assert_eq!(unix_text, "line1\nline2\n");

        let unix_text = "line1\nline2\n";
        let windows_text = normalize_line_endings(unix_text, LineEndingStyle::CrLf);
        assert_eq!(windows_text, "line1\r\nline2\r\n");
    }

    /// Verify Windows QuickSession configs are correct on any platform.
    #[test]
    fn windows_quicksession_cross_platform() {
        // cmd.exe config should work on any platform
        let cmd = QuickSession::cmd();
        assert_eq!(cmd.command, "cmd.exe");
        assert_eq!(cmd.line_ending, LineEnding::CrLf);

        // PowerShell config (uses platform-specific binary name)
        let ps = QuickSession::powershell();
        #[cfg(windows)]
        assert_eq!(ps.command, "powershell.exe");
        #[cfg(not(windows))]
        assert_eq!(ps.command, "pwsh");
        assert!(ps.args.contains(&"-NoLogo".to_string()));
        assert!(ps.args.contains(&"-NoProfile".to_string()));
    }

    /// Verify Windows path patterns work on any platform.
    #[test]
    fn windows_path_patterns_cross_platform() {
        // These patterns should match Windows-style paths regardless of host platform
        let drive_pattern = Pattern::regex(r"[A-Z]:\\").unwrap();
        assert!(drive_pattern.matches("C:\\Users\\test").is_some());
        assert!(drive_pattern.matches("D:\\Projects").is_some());

        // UNC paths
        let unc_pattern = Pattern::literal("\\\\server\\");
        assert!(unc_pattern.matches("\\\\server\\share").is_some());
    }

    /// Verify BackendType is available (PTY should be available on both platforms).
    #[test]
    fn backend_availability_cross_platform() {
        // PTY should be available on both Unix and Windows
        assert!(BackendType::Pty.is_available());
        assert_eq!(BackendType::Pty.name(), "pty");
    }

    /// Verify PtyConfig defaults are consistent.
    #[test]
    fn pty_config_cross_platform() {
        let config = PtyConfig::default();
        // Default terminal dimensions are VT100 standard (cols, rows)
        assert_eq!(config.dimensions, (80, 24));
    }

    /// Verify control characters work consistently across platforms.
    #[test]
    fn control_chars_cross_platform() {
        use rust_expect::ControlChar;

        // These values should be the same on all platforms
        assert_eq!(ControlChar::CtrlC.as_byte(), 0x03);
        assert_eq!(ControlChar::CtrlD.as_byte(), 0x04);
        assert_eq!(ControlChar::CtrlZ.as_byte(), 0x1A);
        assert_eq!(ControlChar::Escape.as_byte(), 0x1B);
    }

    /// Verify timeout configuration works cross-platform.
    #[test]
    fn timeout_config_cross_platform() {
        use rust_expect::util::TimeoutConfig;

        let uniform = TimeoutConfig::uniform(Duration::from_secs(30));
        assert_eq!(uniform.expect, Duration::from_secs(30));
        assert_eq!(uniform.read, Duration::from_secs(30));
        assert_eq!(uniform.write, Duration::from_secs(30));
        assert_eq!(uniform.connect, Duration::from_secs(30));
        assert_eq!(uniform.close, Duration::from_secs(30));

        // Default timeout (check that default() works correctly)
        let default = TimeoutConfig::default();
        // Default expect timeout is 30 seconds
        assert_eq!(default.expect, Duration::from_secs(30));
    }

    /// Verify pattern matching with Windows-style content works cross-platform.
    #[test]
    fn windows_content_patterns_cross_platform() {
        // Windows error patterns
        let not_recognized = Pattern::regex(r"is not recognized").unwrap();
        assert!(not_recognized.matches("'foo' is not recognized as an internal or external command").is_some());

        // Windows prompt patterns
        let cmd_prompt = Pattern::regex(r"[A-Z]:\\[^>]*>").unwrap();
        assert!(cmd_prompt.matches("C:\\Windows>").is_some());

        // PowerShell prompt patterns
        let ps_prompt = Pattern::regex(r"PS [A-Z]:\\").unwrap();
        assert!(ps_prompt.matches("PS C:\\Users\\test>").is_some());
    }

    /// Verify session configuration for Windows applications.
    #[test]
    fn windows_session_config_cross_platform() {
        let config = SessionBuilder::new()
            .command("cmd.exe")
            .arg("/c")
            .arg("dir")
            .timeout(Duration::from_secs(60))
            .env("PROMPT", "$P$G")
            .windows_line_endings()
            .build();

        assert_eq!(config.command, "cmd.exe");
        assert!(config.args.contains(&"/c".to_string()));
        assert!(config.args.contains(&"dir".to_string()));
        assert_eq!(config.env.get("PROMPT"), Some(&"$P$G".to_string()));
        assert_eq!(config.line_ending, LineEnding::CrLf);
    }
}
