//! SSH session and authentication strategies example.
//!
//! This example demonstrates SSH session configuration and various
//! authentication strategies available in rust-expect.
//!
//! Note: This requires the 'ssh' feature and a running SSH server for
//! actual connections.
//!
//! Run with: `cargo run --example ssh_session --features ssh`

#[cfg(feature = "ssh")]
fn main() {
    use std::time::Duration;

    use rust_expect::backend::ssh::{AuthMethod, HostKeyVerification, SshCredentials, SshSessionBuilder};

    println!("rust-expect SSH Authentication Strategies");
    println!("==========================================\n");

    // =========================================================================
    // Section 1: Authentication Methods
    // =========================================================================

    println!("1. Authentication Methods");
    println!("   ----------------------");

    // 1a. Password authentication
    println!("\n   a) Password Authentication");
    let _password_creds = SshCredentials::new("admin").with_password("secret123");
    println!("      SshCredentials::new(\"admin\").with_password(\"secret123\")");
    println!("      Use case: Simple, direct password authentication");
    println!("      Security: Avoid for production; prefer key-based auth");

    // 1b. Public key authentication
    println!("\n   b) Public Key Authentication");
    let _key_creds = SshCredentials::new("deploy")
        .with_key("/home/user/.ssh/id_ed25519");
    println!("      .with_key(\"/home/user/.ssh/id_ed25519\")");
    println!("      Use case: Most secure, standard for automation");
    println!("      Tip: Use Ed25519 keys for best security/performance");

    // 1c. Public key with passphrase
    println!("\n   c) Encrypted Key Authentication");
    let _encrypted_key_creds = SshCredentials::new("secure-deploy")
        .with_key_passphrase("/home/user/.ssh/id_rsa", "key_passphrase");
    println!("      .with_key_passphrase(path, passphrase)");
    println!("      Use case: Extra security for private keys");

    // 1d. SSH agent authentication
    println!("\n   d) SSH Agent Authentication");
    let _agent_creds = SshCredentials::new("operator").with_agent();
    println!("      .with_agent()");
    println!("      Use case: Interactive use, avoids storing keys");
    println!("      Tip: Works with ssh-agent, gpg-agent, or 1Password agent");

    // 1e. Keyboard-interactive authentication
    println!("\n   e) Keyboard-Interactive Authentication");
    let _kbd_creds = SshCredentials::new("user")
        .with_keyboard_interactive("my_password");
    println!("      .with_keyboard_interactive(password)");
    println!("      Use case: PAM-based servers, cloud instances");
    println!("      Note: Many servers use this instead of direct password");

    // 1f. Keyboard-interactive with MFA
    println!("\n   f) Multi-Factor Authentication (MFA)");
    let _mfa_creds = SshCredentials::new("secure-user")
        .with_keyboard_interactive_responses(vec![
            "password".to_string(),
            "123456".to_string(), // TOTP code
        ]);
    println!("      .with_keyboard_interactive_responses(vec![password, otp])");
    println!("      Use case: 2FA/MFA-enabled servers");
    println!("      Tip: Use TOTP libraries to generate codes dynamically");

    // =========================================================================
    // Section 2: Chaining Authentication Methods
    // =========================================================================

    println!("\n2. Chaining Multiple Authentication Methods");
    println!("   -----------------------------------------");

    // Methods are tried in order until one succeeds
    let fallback_creds = SshCredentials::new("user")
        .with_agent()                                    // Try agent first
        .with_key("/home/user/.ssh/id_ed25519")          // Then specific key
        .with_keyboard_interactive("password")           // Then kbd-interactive
        .with_password("fallback_password");             // Finally password

    println!("   Methods tried in order:");
    for (i, method) in fallback_creds.auth_methods.iter().enumerate() {
        let method_name = match method {
            AuthMethod::Agent => "SSH Agent",
            AuthMethod::PublicKey { .. } => "Public Key (id_ed25519)",
            AuthMethod::KeyboardInteractive { .. } => "Keyboard-Interactive",
            AuthMethod::Password(_) => "Password",
            AuthMethod::None => "None",
        };
        println!("   {}. {}", i + 1, method_name);
    }
    println!("   Benefit: Robust authentication for various server configs");

    // =========================================================================
    // Section 3: Default Credentials
    // =========================================================================

    println!("\n3. Default Credentials");
    println!("   -------------------");

    let _default_creds = SshCredentials::new("user").with_defaults();
    println!("   .with_defaults() adds:");
    println!("     - SSH Agent");
    println!("     - ~/.ssh/id_ed25519");
    println!("     - ~/.ssh/id_rsa");
    println!("   Use case: Quick setup, mimics standard SSH client behavior");

    // =========================================================================
    // Section 4: Host Key Verification
    // =========================================================================

    println!("\n4. Host Key Verification Policies");
    println!("   -------------------------------");

    let policies = [
        (HostKeyVerification::KnownHosts, "KnownHosts",
         "Check against ~/.ssh/known_hosts (DEFAULT, RECOMMENDED)"),
        (HostKeyVerification::Tofu, "Tofu",
         "Trust On First Use - accept new, verify known"),
        (HostKeyVerification::RejectUnknown, "RejectUnknown",
         "Reject any unknown hosts (strictest)"),
    ];

    for (policy, name, desc) in policies {
        println!("   {:15} - {}", name, desc);
        let _ = policy; // Just to use the variable
    }

    #[cfg(feature = "insecure-skip-verify")]
    {
        println!("   {:15} - DANGEROUS: Skip verification (testing only)",
            "AcceptAll");
    }

    // =========================================================================
    // Section 5: Builder Pattern Configuration
    // =========================================================================

    println!("\n5. Full Session Configuration");
    println!("   ---------------------------");

    let _builder = SshSessionBuilder::new()
        .host("server.example.com")
        .port(22)
        .username("admin")
        .private_key("/home/user/.ssh/id_ed25519")
        .connect_timeout(Duration::from_secs(30))
        .tcp_keepalive(Some(Duration::from_secs(60)))
        .host_key_verification(HostKeyVerification::KnownHosts);

    println!("   let session = SshSessionBuilder::new()");
    println!("       .host(\"server.example.com\")");
    println!("       .port(22)");
    println!("       .username(\"admin\")");
    println!("       .private_key(\"/home/user/.ssh/id_ed25519\")");
    println!("       .connect_timeout(Duration::from_secs(30))");
    println!("       .tcp_keepalive(Some(Duration::from_secs(60)))");
    println!("       .host_key_verification(HostKeyVerification::KnownHosts)");
    println!("       .connect()");
    println!("       .await?;");

    // =========================================================================
    // Section 6: Common Patterns
    // =========================================================================

    println!("\n6. Common Authentication Patterns");
    println!("   -------------------------------");

    println!("\n   a) GitHub/GitLab deployment:");
    let _git_creds = SshCredentials::new("git")
        .with_key("/home/deploy/.ssh/deploy_key");
    println!("      User: 'git', Key: deploy-specific key");

    println!("\n   b) AWS EC2 instance:");
    let _ec2_creds = SshCredentials::new("ec2-user")
        .with_key("/home/user/.ssh/aws-key.pem");
    println!("      User: 'ec2-user' or 'ubuntu', Key: .pem file");

    println!("\n   c) Cloud server with password (keyboard-interactive):");
    let _cloud_creds = SshCredentials::new("root")
        .with_keyboard_interactive("server_password");
    println!("      Many cloud providers use keyboard-interactive");

    println!("\n   d) Interactive development:");
    let _dev_creds = SshCredentials::default().with_defaults();
    println!("      Uses current $USER, agent, and default keys");

    println!("\n   e) High-security environment:");
    let _secure_creds = SshCredentials::new("secure-admin")
        .with_key_passphrase("/secure/admin_key", "key_passphrase")
        .with_keyboard_interactive_responses(vec![
            "account_password".to_string(),
            "mfa_token".to_string(),
        ]);
    println!("      Encrypted key + MFA for maximum security");

    println!("\nSSH authentication examples completed!");
    println!("\nNote: For actual connections, you need a running SSH server.");
    println!("See the rust-expect documentation for complete connection examples.");
}

#[cfg(not(feature = "ssh"))]
fn main() {
    println!("This example requires the 'ssh' feature.");
    println!("Run with: cargo run --example ssh_session --features ssh");
}
