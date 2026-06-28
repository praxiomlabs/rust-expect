//! Regression tests for Windows `ConPTY` child exit detection ("reaping").
//!
//! `ConPTY` keeps the output pipe open for the lifetime of the pseudo console,
//! so a child's exit is not observable simply by reading the pipe. Before the
//! exit-watcher fix in `rust-pty`'s Windows backend, this caused two bugs:
//!
//!   1. `Session::wait`/`wait_timeout`/`expect_eof` never observed the child's
//!      exit — they blocked until their own timeout (or forever, for `wait`).
//!   2. Writes to an already-exited child's PTY returned `Ok` (silently
//!      buffered) instead of failing.
//!
//! These tests spawn real `ConPTY` processes and assert the corrected behavior.

#![cfg(windows)]

use std::time::{Duration, Instant};

use rust_expect::Session;

/// `wait_timeout` must return as soon as the child exits, not run out its own
/// timeout. The generous 10s budget is only a safety net — a working
/// implementation returns in well under a second.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wait_timeout_observes_child_exit() {
    let mut session = Session::spawn("cmd.exe", &["/c", "echo bye"])
        .await
        .expect("spawn cmd.exe");

    let start = Instant::now();
    let result = session.wait_timeout(Duration::from_secs(10)).await;
    let elapsed = start.elapsed();

    assert!(
        result.is_ok(),
        "wait_timeout should observe the child's exit (got {result:?})"
    );
    assert!(
        elapsed < Duration::from_secs(9),
        "wait_timeout returned only via its own timeout ({elapsed:?}); \
         the child's exit was never observed"
    );
}

/// `expect_eof` must complete once the child exits — EOF has to be delivered on
/// the output stream when the pseudo console is closed.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn expect_eof_returns_after_exit() {
    let mut session = Session::spawn("cmd.exe", &["/c", "echo bye"])
        .await
        .expect("spawn cmd.exe");

    let outcome = tokio::time::timeout(Duration::from_secs(10), session.expect_eof()).await;

    assert!(
        outcome.is_ok(),
        "expect_eof() blocked past 10s; EOF was never delivered on child exit"
    );
    assert!(
        outcome.unwrap().is_ok(),
        "expect_eof() should succeed when the child exits"
    );
}

/// `is_running()` must report `false` once the child has exited.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn is_running_flips_false_after_exit() {
    let mut session = Session::spawn("cmd.exe", &["/c", "echo bye"])
        .await
        .expect("spawn cmd.exe");

    session
        .wait_timeout(Duration::from_secs(10))
        .await
        .expect("child should exit");

    assert!(
        !session.is_running(),
        "is_running() must report false after the child has exited"
    );
}

/// Writing after the child has exited must fail rather than being silently
/// buffered into a dead PTY.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn write_after_exit_fails() {
    let mut session = Session::spawn("cmd.exe", &["/c", "exit 0"])
        .await
        .expect("spawn cmd.exe");

    session
        .wait_timeout(Duration::from_secs(10))
        .await
        .expect("child should exit");

    // The child is gone and the pseudo console is closed. A write must surface
    // an error; allow a few attempts to cover the brief window between the
    // child's exit and the watcher tearing the PTY down.
    let mut errored = false;
    for _ in 0..10 {
        if session
            .send_line("echo should-not-be-buffered")
            .await
            .is_err()
        {
            errored = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    assert!(
        errored,
        "writing after the child exited should fail, not return Ok"
    );
}
