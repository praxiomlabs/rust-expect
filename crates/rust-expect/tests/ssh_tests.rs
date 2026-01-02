//! Integration tests for SSH backend.
//!
//! Note: These tests verify API structure only.
//! Actual SSH connections require a running SSH server.

#![cfg(feature = "ssh")]

use rust_expect::backend::ssh::{AuthMethod, SshConfig, SshCredentials};
use std::time::Duration;

#[test]
fn ssh_credentials_with_password() {
    let creds = SshCredentials::new("testuser").with_password("testpass");

    assert_eq!(creds.username, "testuser");
    assert_eq!(creds.auth_methods.len(), 1);
    assert!(creds.auth_methods[0].is_password());
}

#[test]
fn ssh_credentials_with_key() {
    let creds = SshCredentials::new("testuser").with_key("/path/to/key");

    assert_eq!(creds.username, "testuser");
    assert_eq!(creds.auth_methods.len(), 1);
    assert!(creds.auth_methods[0].is_public_key());
}

#[test]
fn ssh_credentials_multiple_auth() {
    let creds = SshCredentials::new("testuser")
        .with_password("pass")
        .with_key("/path/to/key")
        .with_agent();

    assert_eq!(creds.username, "testuser");
    assert_eq!(creds.auth_methods.len(), 3);
}

#[test]
fn ssh_config_builder() {
    let config = SshConfig::new("example.com");

    assert_eq!(config.host, "example.com");
    assert_eq!(config.port, 22); // Default port
}

#[test]
fn ssh_config_with_port() {
    let config = SshConfig::new("example.com").port(2222);

    assert_eq!(config.port, 2222);
}

#[test]
fn ssh_config_with_timeout() {
    let config = SshConfig::new("example.com").connect_timeout(Duration::from_secs(60));

    assert_eq!(config.connect_timeout, Duration::from_secs(60));
}

#[test]
fn ssh_config_with_credentials() {
    let creds = SshCredentials::new("admin").with_password("secret");
    let config = SshConfig::new("example.com").credentials(creds);

    assert_eq!(config.credentials.username, "admin");
}

#[test]
fn ssh_config_with_username() {
    let config = SshConfig::new("example.com").username("admin");

    assert_eq!(config.credentials.username, "admin");
}

#[test]
fn ssh_config_display() {
    let config = SshConfig::new("example.com");
    let display = format!("{:?}", config);

    assert!(!display.is_empty());
    assert!(display.contains("example.com"));
}

#[test]
fn ssh_credentials_display() {
    let creds = SshCredentials::new("testuser").with_password("secret");
    let display = format!("{:?}", creds);

    // Should show username
    assert!(display.contains("testuser"));
}

#[test]
fn ssh_config_clone() {
    let config1 = SshConfig::new("example.com").port(2222);
    let config2 = config1.clone();

    assert_eq!(config1.host, config2.host);
    assert_eq!(config1.port, config2.port);
}

#[test]
fn ssh_config_address() {
    let config = SshConfig::new("example.com").port(2222);

    assert_eq!(config.address(), "example.com:2222");
}

#[test]
fn ssh_config_with_compression() {
    let config = SshConfig::new("example.com").with_compression();

    assert!(config.compression);
}

#[test]
fn auth_method_password() {
    let method = AuthMethod::password("secret");

    assert!(method.is_password());
    assert!(!method.is_public_key());
}

#[test]
fn auth_method_public_key() {
    let method = AuthMethod::public_key("/path/to/key");

    assert!(method.is_public_key());
    assert!(!method.is_password());
}

#[test]
fn auth_method_agent() {
    let method = AuthMethod::agent();

    assert!(!method.is_password());
    assert!(!method.is_public_key());
}
