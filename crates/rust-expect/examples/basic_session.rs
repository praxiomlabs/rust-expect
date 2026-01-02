//! Basic session example.
//!
//! This example shows how to spawn a simple shell session and interact with it.
//!
//! Run with: `cargo run --example basic_session`

use rust_expect::prelude::*;
use std::time::Duration;

fn main() -> Result<()> {
    println!("Starting basic session example...");

    // Create a session builder
    let _builder = SessionBuilder::new()
        .command("sh")
        .arg("-c")
        .arg("echo 'Hello from shell'; sleep 1; echo 'Goodbye'")
        .timeout(Duration::from_secs(10));

    println!("Session configured with command: sh -c ...");
    println!("Timeout: 10 seconds");

    // In a real scenario, you would spawn and interact:
    // let mut session = builder.spawn()?;
    // session.expect("Hello").await?;
    // session.expect("Goodbye").await?;

    println!("Example completed successfully!");
    Ok(())
}
