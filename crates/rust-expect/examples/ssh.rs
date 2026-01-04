//! SSH session example.
//!
//! This example demonstrates establishing SSH connections and
//! automating remote terminal sessions.
//!
//! Run with: `cargo run --example ssh --features ssh`
//!
//! Note: This example requires a real SSH server to connect to.
//! Modify the connection parameters to match your environment.

fn main() {
    println!("rust-expect SSH Session Example");
    println!("================================\n");

    #[cfg(not(feature = "ssh"))]
    {
        println!("This example requires the 'ssh' feature.");
        println!("Run with: cargo run --example ssh --features ssh");
        return;
    }

    #[cfg(feature = "ssh")]
    {
        println!("SSH support is enabled.");
        println!("\nTo use SSH automation with rust-expect:\n");

        print_ssh_examples();
    }
}

#[cfg(feature = "ssh")]
fn print_ssh_examples() {
    // Example 1: SSH connection concepts
    println!("1. SSH Connection Configuration...");
    println!("   ");
    println!("   // Example SSH session setup:");
    println!("   let config = SshConfig::new()");
    println!("       .host(\"server.example.com\")");
    println!("       .port(22)");
    println!("       .user(\"admin\")");
    println!("       .auth(SshAuth::Password(\"secret\"));");
    println!();

    // Example 2: Authentication methods
    println!("2. Authentication Methods...");
    println!("   ");
    println!("   // Password authentication:");
    println!("   SshAuth::Password(password)");
    println!("   ");
    println!("   // Key-based authentication:");
    println!("   SshAuth::PrivateKey {{");
    println!("       path: \"/home/user/.ssh/id_rsa\",");
    println!("       passphrase: None,");
    println!("   }}");
    println!("   ");
    println!("   // SSH agent authentication:");
    println!("   SshAuth::Agent");
    println!();

    // Example 3: Host key verification
    println!("3. Host Key Verification...");
    println!("   ");
    println!("   // Strict verification (recommended for production):");
    println!("   HostKeyVerification::Strict");
    println!("   ");
    println!("   // Known hosts file:");
    println!("   HostKeyVerification::KnownHosts(\"/path/to/known_hosts\")");
    println!("   ");
    println!("   // Accept and add to known hosts:");
    println!("   HostKeyVerification::AcceptAndAdd");
    println!("   ");
    println!("   // DANGEROUS - AcceptAll (requires 'insecure-skip-verify' feature):");
    println!("   // HostKeyVerification::AcceptAll");
    println!();

    // Example 4: Session workflow
    println!("4. SSH Session Workflow...");
    println!("   ");
    println!("   #[tokio::main]");
    println!("   async fn main() -> Result<()> {{");
    println!("       // Establish connection");
    println!("       let session = SshSession::connect(config).await?;");
    println!("       ");
    println!("       // Wait for shell prompt");
    println!("       session.expect(\"$ \").await?;");
    println!("       ");
    println!("       // Execute commands");
    println!("       session.send_line(\"hostname\").await?;");
    println!("       let output = session.expect(\"$ \").await?;");
    println!("       println!(\"Hostname: {{}}\", output.before);");
    println!("       ");
    println!("       // Disconnect");
    println!("       session.send_line(\"exit\").await?;");
    println!("       Ok(())");
    println!("   }}");
    println!();

    // Example 5: Remote command execution
    println!("5. Remote Command Execution...");
    println!("   ");
    println!("   // Execute a single command:");
    println!("   let output = session.exec(\"uptime\").await?;");
    println!("   ");
    println!("   // Execute with timeout:");
    println!("   let output = session");
    println!("       .exec_timeout(\"long-running-task\", Duration::from_secs(60))");
    println!("       .await?;");
    println!();

    // Example 6: File operations
    println!("6. File Operations over SSH...");
    println!("   ");
    println!("   // SFTP operations can be done via expect:");
    println!("   session.send_line(\"cat /etc/hostname\").await?;");
    println!("   let content = session.expect_regex(r\"[\\r\\n]+\").await?;");
    println!("   ");
    println!("   // Or using SCP-style commands:");
    println!("   session.send_line(\"scp file.txt remote:/path/\").await?;");
    println!();

    // Example 7: Jump hosts / bastion
    println!("7. Jump Host Configuration...");
    println!("   ");
    println!("   // Configure a jump host:");
    println!("   let config = SshConfig::new()");
    println!("       .host(\"internal-server\")");
    println!("       .user(\"admin\")");
    println!("       .jump_host(SshConfig::new()");
    println!("           .host(\"bastion.example.com\")");
    println!("           .user(\"jump-user\")");
    println!("       );");
    println!();

    // Example 8: Error handling
    println!("8. SSH Error Handling...");
    println!("   ");
    println!("   match session.connect().await {{");
    println!("       Ok(s) => println!(\"Connected!\"),");
    println!("       Err(SshError::ConnectionRefused) => {{");
    println!("           eprintln!(\"Server refused connection\");");
    println!("       }}");
    println!("       Err(SshError::AuthenticationFailed) => {{");
    println!("           eprintln!(\"Invalid credentials\");");
    println!("       }}");
    println!("       Err(SshError::HostKeyMismatch) => {{");
    println!("           eprintln!(\"Host key verification failed!\");");
    println!("       }}");
    println!("       Err(e) => eprintln!(\"Error: {{}}\", e),");
    println!("   }}");
    println!();

    // Example 9: Security considerations
    println!("9. Security Considerations...");
    println!("   ");
    println!("   - ALWAYS verify host keys in production");
    println!("   - Use key-based authentication over passwords");
    println!("   - Store credentials securely (not in code)");
    println!("   - Use short-lived sessions when possible");
    println!("   - Consider using SSH agent for key management");
    println!("   - Enable audit logging for automation");
    println!();

    // Example 10: Production pattern
    println!("10. Production Automation Pattern...");
    println!("    ");
    println!("    async fn deploy_to_server(host: &str) -> Result<()> {{");
    println!("        let config = SshConfig::from_env()?");
    println!("            .host(host)");
    println!("            .host_key_verification(HostKeyVerification::KnownHosts);");
    println!("        ");
    println!("        let session = SshSession::connect(config).await?;");
    println!("        session.expect(\"$ \").await?;");
    println!("        ");
    println!("        // Run deployment commands");
    println!("        for cmd in [\"cd /app\", \"git pull\", \"./restart.sh\"] {{");
    println!("            session.send_line(cmd).await?;");
    println!("            session.expect(\"$ \").await?;");
    println!("        }}");
    println!("        ");
    println!("        session.send_line(\"exit\").await?;");
    println!("        Ok(())");
    println!("    }}");

    println!("\nSSH examples completed!");
    println!("\nNote: To actually test SSH connections, you need:");
    println!("  - A running SSH server");
    println!("  - Valid credentials");
    println!("  - Proper network access");
}
