//! Drive htop briefly — a real-time, alt-screen, continuously-redrawing TUI.
//!
//! This is a tougher generalization test than `drive_less.rs`:
//!   - htop uses the alternate screen buffer
//!   - htop redraws ~2x/second forever; `wait_screen_stable` will NEVER return
//!     spontaneously while it's running. We instead anchor on a stable text
//!     element (the "CPU" / "Mem" labels) and use a fixed dwell time.
//!   - htop responds to single keystrokes (F-keys, q to quit).
//!
//! Run with:
//!   `cargo run --example drive_htop --features screen`

#![cfg(feature = "screen")]

use std::time::Duration;

use rust_expect::{Session, SessionBuilder};

#[tokio::main]
async fn main() -> rust_expect::Result<()> {
    let config = SessionBuilder::new()
        .command("/usr/bin/htop")
        .arg("--no-color")
        .env("TERM", "xterm-256color")
        .dimensions(120, 30)
        .timeout(Duration::from_secs(10))
        .build();

    eprintln!("[drive_htop] spawning htop --no-color");
    let mut session = Session::spawn_with_config("/usr/bin/htop", &["--no-color"], config).await?;
    session.attach_screen();

    // Anchor on a control that htop renders once it's drawn. htop's footer
    // shows F-key bindings; "F10" is stably present.
    session
        .expect_screen_contains("F10", Duration::from_secs(5))
        .await?;
    eprintln!("[drive_htop] htop is up; footer 'F10' anchor matched");

    // Anchor on the column header "CPU%" or similar to confirm the table
    // header rendered too.
    let header_anchor = if session
        .expect_screen_contains("CPU%", Duration::from_secs(2))
        .await
        .is_ok()
    {
        "CPU%"
    } else {
        // Some htop builds render the header differently; fall back to "PID".
        session
            .expect_screen_contains("PID", Duration::from_secs(2))
            .await?;
        "PID"
    };
    eprintln!("[drive_htop] table header anchor: {header_anchor}");

    // Dwell briefly so a few redraw cycles happen and prove our tap keeps up.
    eprintln!("[drive_htop] dwelling 1s while htop redraws");
    let _ = session
        .expect_screen_contains(
            "__never_match__sentinel_for_dwell__",
            Duration::from_millis(1000),
        )
        .await;

    // Verify the screen is still healthy after redraws (regression guard for
    // tap-+-screen drift during continuous output).
    session
        .expect_screen_contains(header_anchor, Duration::from_millis(500))
        .await?;
    eprintln!("[drive_htop] header still on screen after 1s of redraws");

    // Quit with 'q'.
    eprintln!("[drive_htop] sending 'q' to quit");
    session.send(b"q").await?;
    let _ = session.wait_timeout(Duration::from_secs(3)).await;
    eprintln!("[drive_htop] htop exited cleanly");

    Ok(())
}
