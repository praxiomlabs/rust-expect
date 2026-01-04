//! Interactive session with pattern hooks example.
//!
//! This example demonstrates the pattern hook system in rust-expect's
//! interactive mode, including:
//! - Output pattern hooks (matching output and responding)
//! - Input pattern hooks (intercepting input)
//! - Resize hooks (handling terminal resize events)
//!
//! Run with: `cargo run --example interact_hooks`

use rust_expect::interact::{InteractAction, ResizeContext, TerminalSize};
use rust_expect::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    println!("rust-expect Interactive Pattern Hooks Example");
    println!("==============================================\n");

    // Run non-interactive examples that demonstrate the API
    demonstrate_pattern_matching().await?;
    demonstrate_resize_context();
    demonstrate_action_types();

    // Note: The actual interactive session requires a TTY and cannot
    // run in automated tests. See the comment block at the end for
    // how to use interactive mode in a real application.

    println!("\nAll pattern hooks examples completed!");
    Ok(())
}

/// Demonstrate pattern matching in a semi-interactive way
async fn demonstrate_pattern_matching() -> Result<()> {
    println!("1. Pattern Matching Demonstration");
    println!("   --------------------------------");

    // Create a session
    let mut session = Session::spawn("/bin/sh", &[]).await?;

    // Wait for initial prompt
    session.expect_timeout(
        Pattern::regex(r"[$#>]").unwrap(),
        Duration::from_secs(2),
    ).await?;

    // Create counters to track pattern matches
    let output_match_count = Arc::new(AtomicUsize::new(0));
    let counter_clone = Arc::clone(&output_match_count);

    // Demonstrate what pattern hooks would do by manually matching
    println!("   Simulating pattern-based response to 'password:' prompts\n");

    // Send a command that echoes a password prompt pattern
    session.send_line("echo 'Enter password: test'").await?;

    // In real usage, we'd use interact() with hooks:
    // session.interact()
    //     .on_output("password:", |ctx| {
    //         counter_clone.fetch_add(1, Ordering::SeqCst);
    //         ctx.send("my_secret_password\n")
    //     })
    //     .on_output("$", |_| InteractAction::Stop)
    //     .start()
    //     .await?;

    // For this demo, we manually check for the pattern
    let result = session.expect_timeout(
        Pattern::literal("password:"),
        Duration::from_secs(2),
    ).await;

    match result {
        Ok(m) => {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            println!("   Matched pattern 'password:' in output");
            println!("   Text before match: {:?}", m.before.chars().take(30).collect::<String>());
            println!("   Would respond with: 'my_secret_password\\n'");
        }
        Err(_) => println!("   Pattern not found (expected in some shells)"),
    }

    // Wait for prompt
    let _ = session.expect_timeout(
        Pattern::regex(r"[$#>]").unwrap(),
        Duration::from_secs(2),
    ).await;

    // Clean up
    session.send_line("exit").await?;
    let _ = session.wait().await; // Ignore wait errors

    println!("   Pattern matches recorded: {}", output_match_count.load(Ordering::SeqCst));
    println!();

    Ok(())
}

/// Demonstrate the `ResizeContext` structure
fn demonstrate_resize_context() {
    println!("2. Resize Context Demonstration");
    println!("   -----------------------------");

    // Create terminal sizes
    let old_size = TerminalSize::new(80, 24);
    let new_size = TerminalSize::new(120, 40);

    // Create a resize context (as would be passed to on_resize callback)
    let ctx = ResizeContext {
        size: new_size,
        previous: Some(old_size),
    };

    println!("   Simulating terminal resize event:");
    println!("   Previous size: {}x{}", old_size.cols, old_size.rows);
    println!("   New size: {}x{}", ctx.size.cols, ctx.size.rows);

    // Demonstrate what a resize handler might do
    let action = example_resize_handler(&ctx);
    match action {
        InteractAction::Continue => println!("   Handler action: Continue (resize handled silently)"),
        InteractAction::Send(ref data) => println!("   Handler action: Send {} bytes", data.len()),
        InteractAction::Stop => println!("   Handler action: Stop interaction"),
        InteractAction::Error(ref msg) => println!("   Handler action: Error - {msg}"),
    }

    // Show resize without previous size (initial case)
    let initial_ctx = ResizeContext {
        size: TerminalSize::new(80, 24),
        previous: None,
    };
    println!("   Initial size (no previous): {}x{}", initial_ctx.size.cols, initial_ctx.size.rows);
    println!();
}

/// Example resize handler that could be used with `on_resize`
fn example_resize_handler(ctx: &ResizeContext) -> InteractAction {
    // Log the resize (in a real application, might update UI)
    eprintln!("[resize] Terminal resized to {}x{}", ctx.size.cols, ctx.size.rows);

    // If the terminal got very small, might want to stop
    if ctx.size.cols < 40 || ctx.size.rows < 10 {
        return InteractAction::Error("Terminal too small".into());
    }

    // For SSH sessions, might want to send a window change notification
    // (handled automatically by rust-expect for PTY sessions)

    InteractAction::Continue
}

/// Demonstrate the different `InteractAction` types
fn demonstrate_action_types() {
    println!("3. InteractAction Types Demonstration");
    println!("   -----------------------------------");

    // InteractAction::Continue - keep processing
    let continue_action = InteractAction::Continue;
    println!("   Continue: {continue_action:?}");

    // InteractAction::Send - send data to session
    let send_action = InteractAction::send("hello\n");
    match &send_action {
        InteractAction::Send(data) => {
            println!("   Send: {} bytes - {:?}", data.len(), String::from_utf8_lossy(data));
        }
        _ => unreachable!(),
    }

    // InteractAction::send_bytes - send raw bytes
    let send_bytes_action = InteractAction::send_bytes(vec![0x03]); // Ctrl+C
    match &send_bytes_action {
        InteractAction::Send(data) => {
            println!("   Send bytes: {data:?} (Ctrl+C)");
        }
        _ => unreachable!(),
    }

    // InteractAction::Stop - stop the interaction
    let stop_action = InteractAction::Stop;
    println!("   Stop: {stop_action:?}");

    // InteractAction::Error - stop with an error message
    let error_action = InteractAction::Error("Something went wrong".into());
    match &error_action {
        InteractAction::Error(msg) => println!("   Error: {msg:?}"),
        _ => unreachable!(),
    }

    println!();
}

// ============================================================================
// REAL INTERACTIVE USAGE
// ============================================================================
//
// The following code shows how to use interactive mode with pattern hooks
// in a real terminal application. This requires a TTY and cannot run in
// automated tests.
//
// ```rust
// use rust_expect::interact::InteractAction;
// use rust_expect::prelude::*;
// use std::time::Duration;
//
// async fn interactive_example() -> Result<()> {
//     let mut session = Session::spawn("/bin/bash", &[]).await?;
//
//     // Wait for initial prompt
//     session.expect_timeout(
//         Pattern::regex(r"[$#>]").unwrap(),
//         Duration::from_secs(2),
//     ).await?;
//
//     // Start interactive session with hooks
//     let result = session.interact()
//         // Auto-respond to password prompts
//         .on_output("password:", |ctx| {
//             eprintln!("[hook] Password prompt detected");
//             ctx.send("my_password\n")
//         })
//         // Auto-respond to "yes/no" confirmations
//         .on_output("(yes/no)", |ctx| {
//             eprintln!("[hook] Confirmation prompt detected");
//             ctx.send("yes\n")
//         })
//         // Log when we see error messages
//         .on_output("error:", |ctx| {
//             eprintln!("[hook] Error detected: {}", ctx.matched);
//             InteractAction::Continue
//         })
//         // Stop on logout/exit
//         .on_output("logout", |_| {
//             eprintln!("[hook] Logout detected, stopping");
//             InteractAction::Stop
//         })
//         // Intercept certain input patterns
//         .on_input("exit", |ctx| {
//             eprintln!("[hook] Exit command detected");
//             ctx.send("exit\n")  // Allow it through
//         })
//         // Handle window resize
//         .on_resize(|ctx| {
//             eprintln!(
//                 "[hook] Terminal resized to {}x{}",
//                 ctx.size.cols, ctx.size.rows
//             );
//             InteractAction::Continue
//         })
//         // Configure interaction mode
//         .with_mode(
//             InteractionMode::new()
//                 .with_local_echo(false)
//                 .with_crlf(true)
//         )
//         // Set escape sequence (Ctrl+] to exit)
//         .with_escape(vec![0x1d])
//         // Set a timeout
//         .with_timeout(Duration::from_secs(300))
//         // Start the interaction
//         .start()
//         .await?;
//
//     println!("Interactive session ended: {:?}", result.reason);
//     println!("Final buffer: {:?}", result.buffer.chars().take(100).collect::<String>());
//
//     Ok(())
// }
// ```
//
// To run an interactive session, you need a terminal. This example can
// be adapted for:
// - SSH automation (auto-responding to host key prompts)
// - Password handling (auto-filling from environment or keychain)
// - Interactive CLI tools (responding to menus and prompts)
// - Log monitoring (triggering on specific output patterns)
// ============================================================================
