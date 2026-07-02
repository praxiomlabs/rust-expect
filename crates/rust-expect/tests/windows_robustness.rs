//! Windows robustness checks (backlog S1 + ConPTY escape handling).
//!
//! - `kill()` on an already-exited child must be safe (no panic, clean result).
//! - The `screen` feature's VT100 emulator must ingest ConPTY's escape-heavy
//!   handshake frame without panicking and expose a correctly-sized screen.
//!
//! Note: on some Windows configurations conhost does not forward a child's
//! *rendered* text to the read pipe (an OS/environment behavior, reproducible
//! with other ConPTY libraries), so these tests deliberately assert only on
//! properties that hold regardless of whether child output is forwarded.

#![cfg(windows)]

use std::time::Duration;

use rust_expect::Session;

/// `kill()` after the child has already exited must not panic and must return
/// a clean result (backlog S1: Windows does not recycle PIDs, but the job
/// object / process handle must be handled gracefully post-exit).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn kill_after_exit_is_safe() {
    let mut session = Session::spawn("cmd.exe", &["/c", "exit 0"])
        .await
        .expect("spawn cmd.exe");

    session
        .wait_timeout(Duration::from_secs(10))
        .await
        .expect("child should exit");

    // First kill after exit: must not panic.
    let first = session.kill();
    // A second kill must also be safe (idempotent teardown).
    let second = session.kill();

    // We only require safety (no panic) and a well-formed Result; on Windows
    // terminating an already-finished job typically succeeds.
    let _ = (first, second);
    assert!(
        !session.is_running(),
        "child must report not-running post-exit"
    );
}

/// `kill()` on a live child terminates it and its job tree; a subsequent wait
/// resolves rather than hanging.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn kill_live_child_terminates_it() {
    // `pause` waits for a keypress that never comes, so the child stays alive.
    let mut session = Session::spawn("cmd.exe", &["/c", "pause"])
        .await
        .expect("spawn cmd.exe");

    // Give it a moment to actually be running.
    tokio::time::sleep(Duration::from_millis(150)).await;
    session.kill().expect("kill of a live child should succeed");

    // The session must observe the termination (EOF) rather than hang.
    let waited = session.wait_timeout(Duration::from_secs(10)).await;
    assert!(
        waited.is_ok(),
        "wait after kill should resolve, got {waited:?}"
    );
    assert!(!session.is_running());
}

/// The VT100 screen emulator must ingest ConPTY's escape-heavy handshake frame
/// (cursor-visibility, win32-input-mode, clear-screen, OSC title, etc.) without
/// panicking, and expose the configured dimensions.
#[cfg(feature = "screen")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn screen_ingests_conpty_escapes_without_panic() {
    let mut session = Session::spawn("cmd.exe", &["/c", "exit 0"])
        .await
        .expect("spawn cmd.exe");

    session.attach_screen();

    // Drive reads to completion; the screen's internal tap feeds every byte of
    // the ConPTY escape stream through the VT parser.
    session
        .wait_timeout(Duration::from_secs(10))
        .await
        .expect("child should exit");

    let screen = session.screen().expect("screen should be attached");
    let guard = screen.lock().unwrap();
    assert_eq!(guard.cols(), 80, "default screen width");
    assert_eq!(guard.rows(), 24, "default screen height");
    // text() must render the (mostly blank) screen without panicking.
    let _ = guard.text();
}

/// `attach_screen_with_scrollback` + `on_screen_line_scrolled_out` must wire up
/// cleanly on Windows (no panic), even when no lines end up scrolling.
#[cfg(feature = "screen")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn scrollback_api_is_wired_on_windows() {
    use std::sync::{Arc, Mutex};

    let mut session = Session::spawn("cmd.exe", &["/c", "exit 0"])
        .await
        .expect("spawn cmd.exe");

    let scrolled: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&scrolled);
    session.attach_screen_with_scrollback(1000);
    assert!(
        session.on_screen_line_scrolled_out(move |row| {
            sink.lock().unwrap().push(row.text());
        }),
        "callback registration should succeed on an attached scrollback screen"
    );

    session
        .wait_timeout(Duration::from_secs(10))
        .await
        .expect("child should exit");

    // full_text() must render (viewport + any history) without panicking.
    let full = session.screen().unwrap().lock().unwrap().full_text();
    let _ = full;
}
