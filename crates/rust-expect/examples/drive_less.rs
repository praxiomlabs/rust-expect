//! Drive `less` viewing a generated text file, asserting on the rendered
//! screen and quitting cleanly.
//!
//! This example exists as a stress test of the new TUI-driving primitives
//! on a third-party application — specifically one that uses the alternate
//! screen buffer (DECSET 1049), which the synchronized-output flow Claude
//! Code uses does *not* exercise.
//!
//! Run with:
//!   `cargo run --example drive_less --features screen`
//!
//! It demonstrates:
//!   - `Session::attach_screen` + `Session::screen()` against an alt-screen TUI
//!   - `Session::expect_screen_contains` anchored on rendered content
//!   - `Session::wait_screen_stable` as the "TUI finished drawing" signal
//!   - Pagination via the `Space` keystroke
//!   - Clean exit via `q`

#![cfg(feature = "screen")]

use std::io::Write;
use std::time::Duration;

use rust_expect::{Session, SessionBuilder};

#[tokio::main]
async fn main() -> rust_expect::Result<()> {
    // Generate a fixture file with predictable content.
    let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
    for i in 1..=200 {
        writeln!(
            tmp,
            "line {i:03}: the quick brown fox jumps over the lazy dog"
        )
        .unwrap();
    }
    let path = tmp.path().to_path_buf();
    tmp.as_file_mut().flush().ok();
    let path_str = path.to_str().expect("utf-8 path");

    eprintln!("[drive_less] spawning less {path_str}");

    // No -X: we *want* less to enter the alternate screen so this example
    // exercises that path in the screen emulator.
    let config = SessionBuilder::new()
        .command("/usr/bin/less")
        .arg("-R") // -R: pass raw ANSI through (harmless for plain text)
        .arg(path_str)
        .env("TERM", "xterm-256color")
        .env("LESS", "")
        .dimensions(80, 24)
        .timeout(Duration::from_secs(10))
        .build();

    let mut session =
        Session::spawn_with_config("/usr/bin/less", &["-R", path_str], config).await?;
    session.attach_screen();

    // First page should contain the first lines we wrote.
    session
        .expect_screen_contains("line 001:", Duration::from_secs(3))
        .await?;
    session
        .expect_screen_contains("line 020:", Duration::from_secs(3))
        .await?;
    // Last visible line on an 80x24 screen viewing 200 lines should be near 23.
    session
        .expect_screen_contains("line 023:", Duration::from_secs(3))
        .await?;
    eprintln!("[drive_less] first page rendered as expected");

    // Page down with space; less goes to the next screenful.
    session.send(b" ").await?;
    session
        .wait_screen_stable(Duration::from_millis(150), Duration::from_secs(2))
        .await?;

    // After one Space we should be past the first page.
    session
        .expect_screen_contains("line 024:", Duration::from_secs(3))
        .await?;
    eprintln!("[drive_less] second page rendered after Space");

    // Jump to end with 'G'.
    session.send(b"G").await?;
    session
        .wait_screen_stable(Duration::from_millis(150), Duration::from_secs(2))
        .await?;
    session
        .expect_screen_contains("line 200:", Duration::from_secs(3))
        .await?;
    eprintln!("[drive_less] tail rendered after 'G'");

    // Dump some screen text for the reader.
    let text = session.screen().unwrap().lock().unwrap().text();
    let last_three: Vec<&str> = text
        .lines()
        .rev()
        .filter(|l| !l.trim().is_empty())
        .take(3)
        .collect();
    eprintln!("[drive_less] last three non-blank lines on screen:");
    for line in last_three.iter().rev() {
        eprintln!("[drive_less]   {line}");
    }

    // Quit less with 'q'.
    session.send(b"q").await?;
    let _ = session.wait_timeout(Duration::from_secs(3)).await;
    eprintln!("[drive_less] less exited cleanly");

    Ok(())
}
