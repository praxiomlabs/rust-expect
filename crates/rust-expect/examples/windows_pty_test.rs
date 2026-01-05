//! Windows PTY verification test
//!
//! This example demonstrates basic Windows `ConPTY` functionality.
//!
//! Note: When running inside Windows Terminal, text output may appear in the
//! terminal window rather than being captured through the PTY pipe. This is
//! a known Windows Terminal behavior with nested `ConPTY` sessions. VT escape
//! sequences (initialization, cursor control, window title) are still captured.

#[cfg(not(windows))]
fn main() {
    println!("This example only runs on Windows.");
    println!("Run it on a Windows system to test ConPTY functionality.");
}

#[cfg(windows)]
use std::time::Duration;

#[cfg(windows)]
use rust_pty::{PtyConfig, PtySystem, WindowsPtySystem};
#[cfg(windows)]
use tokio::io::AsyncReadExt;

#[cfg(windows)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing Windows PTY (ConPTY)...\n");

    let config = PtyConfig::default();
    let (mut master, mut child) =
        WindowsPtySystem::spawn("cmd.exe", ["/c", "echo Hello from ConPTY"], &config).await?;

    println!("Spawned: cmd /c echo Hello from ConPTY");
    println!("Reading PTY output...\n");

    let mut output = Vec::new();
    let start = std::time::Instant::now();

    while start.elapsed() < Duration::from_secs(3) {
        let mut buf = [0u8; 4096];
        match tokio::time::timeout(Duration::from_millis(200), master.read(&mut buf)).await {
            Ok(Ok(0)) => {
                println!("EOF reached");
                break;
            }
            Ok(Ok(n)) => {
                output.extend_from_slice(&buf[..n]);
            }
            Ok(Err(e)) => {
                println!("Read error: {e:?}");
                break;
            }
            Err(_) => {
                if let Some(status) = child.try_wait()? {
                    println!("Process exited: {status:?}");
                    break;
                }
            }
        }
    }

    println!("Captured {} bytes of PTY output", output.len());

    // Show what we captured
    if !output.is_empty() {
        let text = String::from_utf8_lossy(&output);
        println!("Content: {text:?}");
    }

    println!("\n[OK] Windows PTY test complete");
    Ok(())
}
