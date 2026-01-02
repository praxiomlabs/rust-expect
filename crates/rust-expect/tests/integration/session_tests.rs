//! Session lifecycle tests.

use rust_expect::config::SessionConfig;
use rust_expect::session::SessionBuilder;
use std::time::Duration;

#[test]
fn session_config_defaults() {
    let config = SessionConfig::default();
    assert_eq!(config.dimensions, (80, 24));
    assert!(config.inherit_env);
}

#[test]
fn session_builder_configuration() {
    let builder = SessionBuilder::new()
        .command("echo")
        .arg("hello")
        .env("TEST_VAR", "value")
        .dimensions(120, 40)
        .timeout(Duration::from_secs(30));

    // Builder should have the correct config
    let config = builder.config();
    assert_eq!(config.dimensions, (120, 40));
    assert!(config.env.contains_key("TEST_VAR"));
}

#[test]
fn session_builder_shell_mode() {
    let builder = SessionBuilder::new().shell();
    let config = builder.config();

    // Shell mode should set command to /bin/sh or similar
    assert!(!config.command.is_empty());
}

#[test]
#[cfg(unix)]
fn session_config_with_working_dir() {
    let config = SessionConfig {
        working_dir: Some("/tmp".into()),
        ..Default::default()
    };

    assert_eq!(config.working_dir, Some("/tmp".into()));
}
