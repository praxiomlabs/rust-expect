//! Dialog automation example.
//!
//! This example demonstrates dialog-based automation for scripting
//! interactive sessions.
//!
//! Run with: `cargo run --example dialog`

use rust_expect::dialog::{DialogBuilder, DialogStep};
use std::time::Duration;

fn main() {
    println!("Dialog Automation Examples\n");

    // Simple login dialog
    println!("=== Simple Login Dialog ===");
    let login_dialog = DialogBuilder::new()
        .step(DialogStep::expect("login:").then_send("admin"))
        .step(DialogStep::expect("password:").then_send("secret123"))
        .step(DialogStep::expect("$"))
        .build();

    println!("Created login dialog with {} steps", login_dialog.len());
    for (i, step) in login_dialog.steps().iter().enumerate() {
        if let Some(pattern) = step.expect_pattern() {
            print!("  Step {}: expect '{}'", i + 1, pattern);
            if let Some(send) = step.send_text() {
                println!(", send '{send}'");
            } else {
                println!();
            }
        }
    }

    // Dialog with variables
    println!("\n=== Dialog with Variables ===");
    let dialog = DialogBuilder::new()
        .var("username", "alice")
        .var("password", "p@ssw0rd")
        .var("command", "ls -la")
        .step(DialogStep::expect("login:").then_send("${username}"))
        .step(DialogStep::expect("password:").then_send("${password}"))
        .step(DialogStep::expect("$").then_send("${command}"))
        .step(DialogStep::expect("$"))
        .build();

    println!("Variables:");
    for (key, value) in dialog.variables() {
        println!("  {key} = {value}");
    }

    println!("\nSubstitution examples:");
    println!("  '${{username}}' -> '{}'", dialog.substitute("${username}"));
    println!("  'Hello, ${{username}}!' -> '{}'", dialog.substitute("Hello, ${username}!"));

    // Dialog with timeouts
    println!("\n=== Dialog with Custom Timeouts ===");
    let dialog_with_timeouts = DialogBuilder::new()
        .step(DialogStep::expect("slow prompt").timeout(Duration::from_secs(60)))
        .step(DialogStep::expect("fast prompt").timeout(Duration::from_secs(5)))
        .build();

    for (i, step) in dialog_with_timeouts.steps().iter().enumerate() {
        if let Some(timeout) = step.get_timeout() {
            println!("  Step {}: timeout = {:?}", i + 1, timeout);
        }
    }

    // Complex dialog example
    println!("\n=== Complex Dialog Example ===");
    let complex_dialog = DialogBuilder::new()
        .var("env", "production")
        // Initial login
        .step(DialogStep::expect("login:").then_send("deployer"))
        .step(DialogStep::expect("password:").then_send("deploy123"))
        // Check environment
        .step(DialogStep::expect("$").then_send("echo $ENVIRONMENT"))
        .step(DialogStep::expect("${env}"))
        // Run deployment
        .step(DialogStep::expect("$").then_send("./deploy.sh"))
        .step(DialogStep::expect("Deployment complete"))
        .build();

    println!("Complex deployment dialog: {} steps", complex_dialog.len());

    println!("\nDialog examples completed!");
}
