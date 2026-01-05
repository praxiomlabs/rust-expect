//! Dialog automation example.
//!
//! This example demonstrates using the Dialog system for step-based
//! automation of interactive terminal sessions.
//!
//! Run with: `cargo run --example dialog`

use std::time::Duration;

use rust_expect::dialog::{DialogBuilder, DialogStep};
use rust_expect::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("rust-expect Dialog Automation Example");
    println!("======================================\n");

    // Example 1: Simple dialog with fixed responses
    println!("1. Simple login dialog definition...");

    // Build a dialog for a login scenario
    let dialog = DialogBuilder::new()
        .step(DialogStep::expect("Username:").then_send("testuser\n"))
        .step(DialogStep::expect("Password:").then_send("testpass\n"))
        .step(DialogStep::expect(r"[#$>]"))
        .build();

    println!("   Dialog built with {} steps", dialog.len());
    for (i, step) in dialog.steps().iter().enumerate() {
        println!(
            "   Step {}: expect '{:?}' -> send '{:?}'",
            i + 1,
            step.expect_pattern(),
            step.send_text()
        );
    }

    // Example 2: Dialog with variables
    println!("\n2. Dialog with variable substitution...");

    let username = "admin";
    let password = "secret123";

    let dialog = DialogBuilder::new()
        .var("user", username)
        .var("pass", password)
        .step(DialogStep::expect("login:").then_send("${user}\n"))
        .step(DialogStep::expect("password:").then_send("${pass}\n"))
        .build();

    println!("   Variables defined: user={username}, pass=****");
    println!(
        "   Variable substitution test: '{}'",
        dialog.substitute("Hello ${user}")
    );

    // Example 3: Named dialog with multiple steps
    println!("\n3. Named dialog with chained steps...");

    let dialog = DialogBuilder::named("ssh-login")
        .step(
            DialogStep::new("username")
                .with_expect("login:")
                .with_send("myuser\n"),
        )
        .step(
            DialogStep::new("password")
                .with_expect("password:")
                .with_send("mypass\n")
                .timeout(Duration::from_secs(30)),
        )
        .step(DialogStep::new("prompt").with_expect(r"[$#>]"))
        .build();

    println!("   Dialog '{}' with {} steps", dialog.name, dialog.len());

    // Example 4: Dialog with control characters
    println!("\n4. Dialog with control characters...");

    // Using actual ControlChar variants
    let ctrl_c = ControlChar::CtrlC; // Interrupt
    let ctrl_m = ControlChar::CtrlM; // Carriage return (Enter)
    let ctrl_d = ControlChar::CtrlD; // EOF

    println!("   Control characters:");
    println!("   - Ctrl+C (interrupt): 0x{:02x}", ctrl_c.as_byte());
    println!("   - Ctrl+M (enter): 0x{:02x}", ctrl_m.as_byte());
    println!("   - Ctrl+D (EOF): 0x{:02x}", ctrl_d.as_byte());

    // Build a dialog that uses newlines (Ctrl+M is carriage return)
    let _dialog = DialogBuilder::new()
        .step(DialogStep::expect("Continue? [y/n]").then_send("y"))
        .step(DialogStep::expect("Press Enter").then_send("\n"))
        .build();

    println!("   Dialog ready for interactive prompts");

    // Example 5: Using expect_send shorthand
    println!("\n5. Using expect_send shorthand...");

    let dialog = DialogBuilder::new()
        .expect_send("step1", "First prompt:", "response1\n")
        .expect_send("step2", "Second prompt:", "response2\n")
        .expect_send("done", r"[$#>]", "")
        .build();

    println!("   Created {} steps with expect_send()", dialog.len());

    // Example 6: Real dialog execution
    println!("\n6. Executing a real dialog...");

    // Create a simple interactive script
    let mut session = Session::spawn("/bin/sh", &[]).await?;
    session
        .expect_timeout(Pattern::shell_prompt(), Duration::from_secs(2))
        .await?;

    // Create a shell script that prompts
    session
        .send_line("read -p 'Enter name: ' name && echo \"Hello, $name!\"")
        .await?;

    // Wait for the prompt
    session.expect("Enter name:").await?;
    println!("   Saw prompt: 'Enter name:'");

    // Respond
    session.send_line("World").await?;
    println!("   Sent: 'World'");

    // Expect the response
    let m = session.expect("Hello, World").await?;
    println!("   Received: '{}'", m.matched.trim());

    // Example 7: Step timeout configuration
    println!("\n7. Step timeout configuration...");

    let step_with_timeout = DialogStep::new("wait_for_slow")
        .with_expect("Slow operation complete")
        .with_send("next\n")
        .timeout(Duration::from_secs(120))
        .continue_on_timeout(true);

    println!("   Step timeout: {:?}", step_with_timeout.get_timeout());
    println!(
        "   Continue on timeout: {}",
        step_with_timeout.continues_on_timeout()
    );

    // Clean up
    session.send_line("exit").await?;
    session.wait().await?;

    println!("\nDialog examples completed successfully!");
    Ok(())
}
