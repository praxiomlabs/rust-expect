//! Zero-configuration example.
//!
//! This example demonstrates the `auto_config` module for automatic
//! detection of terminal settings, shell types, and prompts.
//!
//! Run with: `cargo run --example zero_config`

use rust_expect::auto_config::{
    default_shell, detect_from_path, detect_locale, detect_shell, is_utf8_environment,
    PromptConfig, ShellConfig, ShellType,
};
use rust_expect::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    println!("rust-expect Zero Configuration Example");
    println!("======================================\n");

    // Example 1: Detect default shell
    println!("1. Detecting default shell...");

    let shell = default_shell();
    println!("   Default shell: {shell}");

    // Example 2: Shell type detection from path
    println!("\n2. Detecting shell type from path...");

    let shell_types = [
        ("/bin/bash", ShellType::Bash),
        ("/bin/zsh", ShellType::Zsh),
        ("/bin/sh", ShellType::Sh),
        ("/usr/bin/fish", ShellType::Fish),
    ];

    for (path, _expected) in shell_types {
        let detected = detect_from_path(path);
        println!("   {path} -> {detected:?}");
    }

    // Example 3: Detect shell from environment
    println!("\n3. Detecting shell from environment...");
    let env_shell = detect_shell();
    println!("   Current shell: {env_shell:?}");

    // Example 4: Locale detection
    println!("\n4. Detecting locale...");

    let locale = detect_locale();
    println!("   Detected locale: {}", locale.to_string());
    println!("   UTF-8 environment: {}", is_utf8_environment());

    // Show relevant environment variables
    let locale_vars = ["LANG", "LC_ALL", "LC_CTYPE"];
    for var in locale_vars {
        if let Ok(value) = std::env::var(var) {
            println!("   {var}: {value}");
        }
    }

    // Example 5: Shell configuration
    println!("\n5. Shell configuration...");

    let _config = ShellConfig::default();
    println!("   Default shell config created");
    println!("   Useful for setting up sessions with sensible defaults");

    // Example 6: Prompt patterns
    println!("\n6. Common prompt patterns...");

    // Common prompt patterns
    let prompts = [
        ("$ ", "POSIX shell"),
        ("# ", "Root shell"),
        ("> ", "Generic prompt"),
        (">>> ", "Python REPL"),
        ("mysql> ", "MySQL client"),
    ];

    for (prompt, name) in prompts {
        println!("   '{prompt}' - {name}");
    }

    // Example 7: Automatic session setup
    println!("\n7. Automatic session setup...");

    // Spawn a shell with automatic configuration
    let mut session = Session::spawn(&default_shell(), &[]).await?;

    // Detect the prompt pattern automatically
    let prompt_pattern = Pattern::regex(r"[$#>]\s*$").unwrap();
    session.expect_timeout(prompt_pattern.clone(), Duration::from_secs(5)).await?;

    println!("   Session started with auto-detected shell");
    println!("   Shell: {}", default_shell());

    // Example 8: Environment-aware automation
    println!("\n8. Environment-aware automation...");

    // Check if we're in a UTF-8 environment
    if is_utf8_environment() {
        println!("   UTF-8 environment detected - safe to use Unicode");
        session.send_line("echo 'Unicode: ✓ 日本語'").await?;
    } else {
        println!("   Non-UTF-8 environment - using ASCII only");
        session.send_line("echo 'ASCII only'").await?;
    }

    session.expect_timeout(prompt_pattern.clone(), Duration::from_secs(2)).await?;

    // Example 9: Cross-platform considerations
    println!("\n9. Cross-platform detection...");

    #[cfg(unix)]
    println!("   Platform: Unix/Linux/macOS");

    #[cfg(windows)]
    println!("   Platform: Windows");

    // Line ending detection
    let line_ending = if cfg!(windows) { "CRLF" } else { "LF" };
    println!("   Expected line ending: {line_ending}");

    // Example 10: Prompt configuration
    println!("\n10. Prompt configuration...");

    let _prompt_config = PromptConfig::default();
    println!("   Prompt config created");
    println!("   Can be used to customize prompt detection behavior");

    // Example 11: Putting it all together
    println!("\n11. Zero-config session workflow...");

    // This demonstrates the ideal zero-config experience
    println!("   1. Detect shell: {}", default_shell());
    println!("   2. Detect locale: {}", detect_locale().to_string());
    println!("   3. Detect UTF-8: {}", is_utf8_environment());
    println!("   4. Configure session automatically");
    println!("   5. Start automation with sensible defaults");

    // Run a simple command to demonstrate
    session.send_line("echo 'Zero-config success!'").await?;
    let m = session.expect("Zero-config success!").await?;
    println!("\n   Result: {}", m.matched.trim());

    // Clean up
    session.send_line("exit").await?;
    session.wait().await?;

    println!("\nZero-configuration examples completed successfully!");
    Ok(())
}
