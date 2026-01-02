//! Integration tests for configuration handling.

use rust_expect::{
    BufferConfig, HumanTypingConfig, LineEnding, LogFormat, LoggingConfig, SessionConfig,
    TimeoutConfig,
};
use std::time::Duration;

#[test]
fn session_config_default() {
    let config = SessionConfig::default();
    assert_eq!(config.timeout.default, Duration::from_secs(30));
    assert!(!config.logging.log_user);
}

#[test]
fn session_config_new() {
    let config = SessionConfig::new("bash");
    assert_eq!(config.command, "bash");
}

#[test]
fn session_config_builder_pattern() {
    let config = SessionConfig::new("bash")
        .args(["-l", "-i"])
        .env("MY_VAR", "value")
        .dimensions(120, 40)
        .timeout(Duration::from_secs(60));

    assert_eq!(config.command, "bash");
    assert_eq!(config.args, vec!["-l", "-i"]);
    assert_eq!(config.env.get("MY_VAR"), Some(&"value".to_string()));
    assert_eq!(config.dimensions, (120, 40));
    assert_eq!(config.timeout.default, Duration::from_secs(60));
}

#[test]
fn timeout_config_default() {
    let timeout = TimeoutConfig::default();
    assert_eq!(timeout.default, Duration::from_secs(30));
    assert_eq!(timeout.spawn, Duration::from_secs(60));
    assert_eq!(timeout.close, Duration::from_secs(10));
}

#[test]
fn timeout_config_new() {
    let timeout = TimeoutConfig::new(Duration::from_millis(500));
    assert_eq!(timeout.default.as_millis(), 500);
}

#[test]
fn timeout_config_builder() {
    let timeout = TimeoutConfig::new(Duration::from_secs(10))
        .spawn(Duration::from_secs(30))
        .close(Duration::from_secs(5));

    assert_eq!(timeout.default, Duration::from_secs(10));
    assert_eq!(timeout.spawn, Duration::from_secs(30));
    assert_eq!(timeout.close, Duration::from_secs(5));
}

#[test]
fn buffer_config_default() {
    let buffer = BufferConfig::default();
    assert_eq!(buffer.max_size, 100 * 1024 * 1024); // 100 MB
    assert!(buffer.ring_buffer);
}

#[test]
fn buffer_config_new() {
    let buffer = BufferConfig::new(1024 * 1024);
    assert_eq!(buffer.max_size, 1024 * 1024);
}

#[test]
fn buffer_config_builder() {
    let buffer = BufferConfig::new(4096)
        .search_window(1024)
        .ring_buffer(false);

    assert_eq!(buffer.max_size, 4096);
    assert_eq!(buffer.search_window, Some(1024));
    assert!(!buffer.ring_buffer);
}

#[test]
fn line_ending_values() {
    assert_eq!(LineEnding::Lf.as_str(), "\n");
    assert_eq!(LineEnding::CrLf.as_str(), "\r\n");
    assert_eq!(LineEnding::Cr.as_str(), "\r");
}

#[test]
fn line_ending_default() {
    let default = LineEnding::default();
    assert_eq!(default, LineEnding::Lf);
}

#[test]
fn human_typing_config_default() {
    let config = HumanTypingConfig::default();
    assert_eq!(config.base_delay, Duration::from_millis(100));
    assert_eq!(config.variance, Duration::from_millis(50));
}

#[test]
fn human_typing_config_builder() {
    let config = HumanTypingConfig::new(Duration::from_millis(50), Duration::from_millis(25))
        .typo_chance(0.05)
        .correction_chance(0.9);

    assert_eq!(config.base_delay, Duration::from_millis(50));
    assert_eq!(config.variance, Duration::from_millis(25));
}

#[test]
fn logging_config_default() {
    let config = LoggingConfig::default();
    assert!(!config.log_user);
    assert_eq!(config.format, LogFormat::Raw);
}

#[test]
fn logging_config_builder() {
    let config = LoggingConfig::new()
        .log_file("/tmp/test.log")
        .log_user(true)
        .format(LogFormat::Ndjson)
        .redact("password");

    assert!(config.log_file.is_some());
    assert!(config.log_user);
    assert_eq!(config.format, LogFormat::Ndjson);
    assert_eq!(config.redact_patterns, vec!["password"]);
}

#[test]
fn log_format_variants() {
    let _ = LogFormat::Raw;
    let _ = LogFormat::Timestamped;
    let _ = LogFormat::Ndjson;
    let _ = LogFormat::Asciicast;
}
