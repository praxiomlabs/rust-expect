//! SSH session example.
//!
//! This example demonstrates SSH session configuration.
//! Note: This requires the 'ssh' feature and a running SSH server.
//!
//! Run with: `cargo run --example ssh_session --features ssh`

#[cfg(feature = "ssh")]
fn main() {
    use rust_expect::backend::ssh::{SshCredentials, SshSessionBuilder};
    use std::time::Duration;

    println!("SSH Session Examples\n");

    // Password authentication
    println!("=== Password Authentication ===");
    let credentials = SshCredentials::new("admin").with_password("secret123");
    println!("Created password credentials for user 'admin'");

    // Key-based authentication
    println!("\n=== Key-Based Authentication ===");
    let _key_creds = SshCredentials::new("deploy").with_key("/home/user/.ssh/id_rsa");
    println!("Created key-file credentials for user 'deploy'");

    // Agent authentication
    println!("\n=== SSH Agent Authentication ===");
    let _agent_creds = SshCredentials::new("operator").with_agent();
    println!("Created agent-based credentials for user 'operator'");

    // Combined authentication (try multiple methods)
    println!("\n=== Combined Authentication ===");
    let _multi_creds = SshCredentials::new("user")
        .with_agent()
        .with_key("/home/user/.ssh/id_ed25519")
        .with_password("fallback123");
    println!("Created multi-method credentials");

    // Default credentials (uses agent and common key paths)
    println!("\n=== Default Credentials ===");
    let _default_creds = SshCredentials::new("user").with_defaults();
    println!("Created credentials with default auth methods");

    // Build an SSH session using the builder pattern
    println!("\n=== Building SSH Session ===");
    let _builder = SshSessionBuilder::new()
        .host("example.com")
        .port(22)
        .username("admin")
        .password("secret123")
        .connect_timeout(Duration::from_secs(30))
        .tcp_keepalive(Some(Duration::from_secs(60)));

    println!("SSH session configured:");
    println!("  Host: example.com:22");
    println!("  Timeout: 30s");
    println!("  Keepalive: 60s");

    // Alternative using credentials
    println!("\n=== Using Credentials Object ===");
    let _session_with_creds = SshSessionBuilder::new()
        .host("server.example.com")
        .username(credentials.username)
        .password("secret123");

    println!("\nSSH session examples completed!");
    println!("\nNote: To actually connect, you need a running SSH server.");
}

#[cfg(not(feature = "ssh"))]
fn main() {
    println!("This example requires the 'ssh' feature.");
    println!("Run with: cargo run --example ssh_session --features ssh");
}
