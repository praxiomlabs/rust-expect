//! Interactive terminal session example.
//!
//! This example demonstrates the interact module for building
//! interactive terminal applications with hooks and filters.
//!
//! Run with: `cargo run --example interactive`

use rust_expect::interact::{
    HookBuilder, HookManager, InputFilter, InteractionMode, OutputFilter, Terminal, TerminalMode,
    TerminalSize, TerminalState,
};
use rust_expect::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    println!("rust-expect Interactive Session Example");
    println!("=======================================\n");

    // Example 1: Terminal state management
    println!("1. Terminal state...");

    let state = TerminalState::default();
    println!("   Default terminal state created");
    println!("   Mode: {:?}", state.mode);
    println!("   Echo: {}", state.echo);
    println!("   Canonical: {}", state.canonical);

    // Example 2: Terminal modes
    println!("\n2. Terminal modes...");

    let terminal_modes = [
        TerminalMode::Raw,
        TerminalMode::Cooked,
        TerminalMode::Cbreak,
    ];

    for mode in terminal_modes {
        println!("   Mode: {mode:?}");
    }

    // Example 3: Terminal size
    println!("\n3. Terminal size...");

    let size = TerminalSize::default();
    println!("   Default size: {}x{}", size.cols, size.rows);

    let custom_size = TerminalSize::new(120, 40);
    println!("   Custom size: {}x{}", custom_size.cols, custom_size.rows);

    // Example 4: Terminal handle
    println!("\n4. Terminal handle...");

    let terminal = Terminal::new();
    println!("   Terminal created");
    println!("   Is running: {}", terminal.is_running());
    println!("   Current mode: {:?}", terminal.mode());

    terminal.set_running(true);
    println!("   Set running: {}", terminal.is_running());

    // Example 5: Interaction mode configuration
    println!("\n5. Interaction modes...");

    let mode = InteractionMode::new()
        .with_local_echo(true)
        .with_crlf(true)
        .with_exit_char(Some(0x1d)); // Ctrl+]

    println!("   Local echo: {}", mode.local_echo);
    println!("   CRLF: {}", mode.crlf);
    println!("   Exit char: {:?}", mode.exit_char);

    // Example 6: Hook management
    println!("\n6. Setting up interaction hooks...");

    let mut hook_manager = HookManager::new();

    // Add custom input hook
    hook_manager.add_input_hook(|data| {
        // Example: convert to uppercase
        data.iter().map(u8::to_ascii_uppercase).collect()
    });

    // Add custom output hook
    hook_manager.add_output_hook(|data| {
        // Example: filter control characters
        data.iter()
            .copied()
            .filter(|&b| b >= 0x20 || b == b'\n')
            .collect()
    });

    println!("   Input hook added (uppercase conversion)");
    println!("   Output hook added (control char filter)");

    // Test the hooks
    let result = hook_manager.process_input(b"hello".to_vec());
    println!("   Test: 'hello' -> '{}'", String::from_utf8_lossy(&result));

    // Example 7: Hook builder pattern
    println!("\n7. Hook builder pattern...");

    let _manager = HookBuilder::new()
        .with_crlf() // Add CRLF translation
        .with_echo() // Add local echo
        .with_logging() // Add event logging
        .build();

    println!("   Built hook manager with CRLF, echo, and logging");

    // Example 8: Input/Output filters
    println!("\n8. Input/Output filters...");

    // Input filter - filter out certain characters
    let input_filter = InputFilter::new()
        .filter(b"xyz") // Filter out x, y, z
        .with_control(false); // Block control characters

    let filtered = input_filter.apply(b"abcxyz123");
    println!(
        "   Input filter applied: 'abcxyz123' -> '{}'",
        String::from_utf8_lossy(&filtered)
    );

    // Output filter - normalize and strip
    let output_filter = OutputFilter::new()
        .with_strip_ansi(true)
        .with_normalize_newlines(true);

    let normalized = output_filter.apply(b"line1\r\nline2\r\n");
    println!(
        "   Output filter (normalize CRLF): {:?}",
        String::from_utf8_lossy(&normalized)
    );

    // Example 9: Real interactive-style session
    println!("\n9. Semi-interactive automation...");

    let mut session = Session::spawn("/bin/sh", &[]).await?;
    session
        .expect_timeout(Pattern::regex(r"[$#>]").unwrap(), Duration::from_secs(2))
        .await?;

    // Simulate an interactive workflow
    let commands = [
        ("pwd", "Print working directory"),
        ("whoami", "Show current user"),
        ("date", "Display current date"),
    ];

    for (cmd, description) in commands {
        println!("   {cmd} -> {description}");
        session.send_line(cmd).await?;
        // Wait for prompt to return
        session
            .expect_timeout(Pattern::regex(r"[$#>]").unwrap(), Duration::from_secs(2))
            .await?;
    }

    // Clean up
    session.send_line("exit").await?;
    session.wait().await?;

    println!("\nInteractive session examples completed successfully!");
    Ok(())
}
