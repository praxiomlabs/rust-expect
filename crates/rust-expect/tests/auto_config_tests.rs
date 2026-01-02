//! Integration tests for auto-configuration.

use rust_expect::auto_config::locale::{detect_locale, is_utf8_environment, locale_env};
use rust_expect::auto_config::shell::{detect_from_path, default_shell, ShellConfig};
use rust_expect::{detect_shell, LocaleInfo, ShellType};

#[test]
fn detect_shell_returns_known_type() {
    // This may vary by environment, but should not panic
    let shell = detect_shell();

    // Should be one of the known types
    match shell {
        ShellType::Bash
        | ShellType::Zsh
        | ShellType::Fish
        | ShellType::Sh
        | ShellType::Ksh
        | ShellType::Tcsh
        | ShellType::Dash
        | ShellType::PowerShell
        | ShellType::Cmd
        | ShellType::Unknown => {}
    }
}

#[test]
fn shell_type_display() {
    assert!(!format!("{:?}", ShellType::Bash).is_empty());
    assert!(!format!("{:?}", ShellType::Zsh).is_empty());
    assert!(!format!("{:?}", ShellType::Fish).is_empty());
    assert!(!format!("{:?}", ShellType::Unknown).is_empty());
}

#[test]
fn shell_type_equality() {
    assert_eq!(ShellType::Bash, ShellType::Bash);
    assert_ne!(ShellType::Bash, ShellType::Zsh);
}

#[test]
fn shell_type_name() {
    assert_eq!(ShellType::Bash.name(), "bash");
    assert_eq!(ShellType::Zsh.name(), "zsh");
    assert_eq!(ShellType::Fish.name(), "fish");
    assert_eq!(ShellType::Sh.name(), "sh");
    assert_eq!(ShellType::PowerShell.name(), "powershell");
    assert_eq!(ShellType::Cmd.name(), "cmd");
}

#[test]
fn shell_type_supports_ansi() {
    assert!(ShellType::Bash.supports_ansi());
    assert!(ShellType::Zsh.supports_ansi());
    assert!(!ShellType::Cmd.supports_ansi());
}

#[test]
fn shell_type_prompt_pattern() {
    // Each shell should have a non-empty prompt pattern
    assert!(!ShellType::Bash.prompt_pattern().is_empty());
    assert!(!ShellType::Zsh.prompt_pattern().is_empty());
    assert!(!ShellType::Fish.prompt_pattern().is_empty());
}

#[test]
fn shell_type_exit_command() {
    assert_eq!(ShellType::Bash.exit_command(), "exit");
    assert_eq!(ShellType::PowerShell.exit_command(), "exit");
}

#[test]
fn detect_from_path_bash() {
    assert_eq!(detect_from_path("/bin/bash"), ShellType::Bash);
    assert_eq!(detect_from_path("/usr/bin/bash"), ShellType::Bash);
    assert_eq!(detect_from_path("/usr/local/bin/bash"), ShellType::Bash);
}

#[test]
fn detect_from_path_zsh() {
    assert_eq!(detect_from_path("/bin/zsh"), ShellType::Zsh);
    assert_eq!(detect_from_path("/usr/bin/zsh"), ShellType::Zsh);
}

#[test]
fn detect_from_path_fish() {
    assert_eq!(detect_from_path("/usr/bin/fish"), ShellType::Fish);
}

#[test]
fn detect_from_path_sh() {
    assert_eq!(detect_from_path("/bin/sh"), ShellType::Sh);
}

#[test]
fn detect_from_path_unknown() {
    assert_eq!(detect_from_path("/custom/shell"), ShellType::Unknown);
}

#[test]
fn default_shell_returns_path() {
    let shell = default_shell();
    assert!(!shell.is_empty());
}

#[test]
fn shell_config_new() {
    let config = ShellConfig::new();
    assert!(!config.path.is_empty());
}

#[test]
fn shell_config_with_path() {
    let config = ShellConfig::new().with_path("/bin/zsh");

    assert_eq!(config.path, "/bin/zsh");
    assert_eq!(config.shell_type, ShellType::Zsh);
}

#[test]
fn shell_config_with_args() {
    let config = ShellConfig::new()
        .arg("-l")
        .arg("-i");

    assert_eq!(config.args, vec!["-l", "-i"]);
}

#[test]
fn shell_config_with_env() {
    let config = ShellConfig::new()
        .env("MY_VAR", "value")
        .env("OTHER_VAR", "other");

    assert_eq!(config.env.get("MY_VAR"), Some(&"value".to_string()));
    assert_eq!(config.env.get("OTHER_VAR"), Some(&"other".to_string()));
}

#[test]
fn shell_config_command() {
    let config = ShellConfig::new()
        .with_path("/bin/bash")
        .arg("-l");

    let (path, args) = config.command();
    assert_eq!(path, "/bin/bash");
    assert_eq!(args, &["-l".to_string()]);
}

#[test]
fn locale_info_parse_full() {
    let info = LocaleInfo::parse("en_US.UTF-8");

    assert_eq!(info.language, Some("en".to_string()));
    assert_eq!(info.territory, Some("US".to_string()));
    assert_eq!(info.codeset, Some("UTF-8".to_string()));
    assert!(info.is_utf8());
}

#[test]
fn locale_info_parse_with_modifier() {
    let info = LocaleInfo::parse("de_DE.UTF-8@euro");

    assert_eq!(info.language, Some("de".to_string()));
    assert_eq!(info.territory, Some("DE".to_string()));
    assert_eq!(info.codeset, Some("UTF-8".to_string()));
    assert_eq!(info.modifier, Some("euro".to_string()));
}

#[test]
fn locale_info_parse_c_locale() {
    let info = LocaleInfo::parse("C");

    assert_eq!(info.language, Some("C".to_string()));
    assert!(!info.is_utf8());
}

#[test]
fn locale_info_parse_posix() {
    let info = LocaleInfo::parse("POSIX");

    assert_eq!(info.language, Some("C".to_string()));
}

#[test]
fn locale_info_default() {
    let locale = LocaleInfo::default();
    // Default locale has all fields as None
    assert!(!format!("{:?}", locale).is_empty());
}

#[test]
fn locale_info_is_utf8() {
    let utf8_locale = LocaleInfo::parse("en_US.UTF-8");
    assert!(utf8_locale.is_utf8());

    let non_utf8 = LocaleInfo::parse("en_US.ISO-8859-1");
    assert!(!non_utf8.is_utf8());
}

#[test]
fn locale_info_to_string() {
    let info = LocaleInfo::parse("en_US.UTF-8");
    assert_eq!(info.to_string(), "en_US.UTF-8");
}

#[test]
fn detect_locale_does_not_panic() {
    // Should not panic regardless of environment
    let locale = detect_locale();
    assert!(!format!("{:?}", locale).is_empty());
}

#[test]
fn locale_env_returns_map() {
    let env = locale_env();
    // Should return a map (may be empty if no locale vars set)
    assert!(!format!("{:?}", env).is_empty());
}

#[test]
fn is_utf8_environment_returns_bool() {
    // Should return a boolean without panicking
    let _is_utf8 = is_utf8_environment();
}
