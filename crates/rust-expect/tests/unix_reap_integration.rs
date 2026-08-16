//! Regression tests for Unix PTY child exit detection ("reaping") and liveness.
//!
//! These are the Unix counterpart to `windows_reap_integration.rs`. They lock
//! in the behavior added alongside the Windows `ConPTY` exit-watcher fix:
//!
//!   1. `wait`/`wait_timeout` report the child's *real* exit status
//!      (`Exited(code)` / `Signaled(sig)`), not `ProcessExitStatus::Unknown`.
//!   2. `is_running()` reflects the truth as soon as the child exits.
//!   3. A `send` to an already-exited child fails cleanly with `SessionClosed`
//!      rather than surfacing a raw `EIO`.

#![cfg(unix)]

use std::time::{Duration, Instant};

use rust_expect::{ExpectError, ProcessExitStatus, Session};

/// `wait` reports the real exit code of a normally-exiting child.
#[tokio::test]
async fn wait_reports_real_exit_code() {
    let mut session = Session::spawn("/bin/sh", &["-c", "exit 7"])
        .await
        .expect("spawn");

    let status = tokio::time::timeout(Duration::from_secs(5), session.wait())
        .await
        .expect("wait() hung")
        .expect("wait() errored");

    assert_eq!(
        status,
        ProcessExitStatus::Exited(7),
        "expected the real exit code, got {status:?}"
    );
}

/// `wait` distinguishes a signal-terminated child from a normal exit.
#[tokio::test]
async fn wait_reports_terminating_signal() {
    // The shell terminates itself with SIGTERM (15).
    let mut session = Session::spawn("/bin/sh", &["-c", "kill -TERM $$"])
        .await
        .expect("spawn");

    let status = tokio::time::timeout(Duration::from_secs(5), session.wait())
        .await
        .expect("wait() hung")
        .expect("wait() errored");

    assert_eq!(
        status,
        ProcessExitStatus::Signaled(libc::SIGTERM),
        "expected Signaled(SIGTERM), got {status:?}"
    );
}

/// `wait_timeout` returns as soon as the child exits, well before its own
/// timeout, and carries the real status.
#[tokio::test]
async fn wait_timeout_observes_exit_promptly() {
    let mut session = Session::spawn("/bin/sh", &["-c", "exit 0"])
        .await
        .expect("spawn");

    let start = Instant::now();
    let status = session
        .wait_timeout(Duration::from_secs(10))
        .await
        .expect("wait_timeout errored");
    let elapsed = start.elapsed();

    assert_eq!(status, ProcessExitStatus::Exited(0));
    assert!(
        elapsed < Duration::from_secs(2),
        "wait_timeout took {elapsed:?}; it should return right after exit"
    );
}

/// `is_running()` flips to false once the child exits — without any explicit
/// `wait`, since it performs a non-blocking reap itself.
#[tokio::test]
async fn is_running_reflects_exit() {
    let session = Session::spawn("/bin/sh", &["-c", "sleep 10"])
        .await
        .expect("spawn");

    assert_eq!(
        session.is_running(),
        Some(true),
        "child should be alive right after spawn"
    );

    session.kill().expect("kill");

    // Poll briefly for the liveness flag to flip after the kill is delivered.
    let mut became_dead = false;
    for _ in 0..50 {
        if session.is_running() == Some(false) {
            became_dead = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(became_dead, "is_running() never observed the child's exit");
}

/// A write to an already-exited child surfaces as a clean `SessionClosed`,
/// not a raw OS error, and never silently succeeds forever.
#[tokio::test]
async fn send_after_exit_returns_session_closed() {
    let mut session = Session::spawn("/bin/sh", &["-c", "exit 0"])
        .await
        .expect("spawn");

    // Observe the exit so the session knows the child is gone.
    let _ = tokio::time::timeout(Duration::from_secs(5), session.wait()).await;

    match session.send_line("noop").await {
        Err(ExpectError::SessionClosed) => {}
        other => panic!("expected SessionClosed after exit, got {other:?}"),
    }
}

/// Variant that does not call `wait` first: the very first post-exit write must
/// still resolve to a clean error within a bounded number of attempts, never
/// buffering silently into a dead PTY.
#[tokio::test]
async fn raw_send_after_exit_eventually_errors() {
    let mut session = Session::spawn("/bin/sh", &["-c", "exit 0"])
        .await
        .expect("spawn");

    // Let the child exit and the slave close, without calling wait().
    tokio::time::sleep(Duration::from_millis(300)).await;

    let mut closed = false;
    for _ in 0..1000 {
        match session.send_line("noop").await {
            Ok(()) => {}
            Err(ExpectError::SessionClosed) => {
                closed = true;
                break;
            }
            Err(other) => panic!("expected SessionClosed, got raw error: {other:?}"),
        }
    }
    assert!(
        closed,
        "1000 writes to an exited child all succeeded — exit was never surfaced"
    );
}

/// After the child has been reaped its PID is eligible for reuse, so
/// `signal`/`kill` must NOT fire `libc::kill` at the (possibly recycled) PID.
/// Both report `SessionClosed` instead. Regression test for S1.
#[tokio::test]
async fn reap_then_signal_is_rejected() {
    let mut session = Session::spawn("/bin/sh", &["-c", "exit 0"])
        .await
        .expect("spawn");

    // Reap the child so the guard has an authoritative "already exited" state.
    let _ = tokio::time::timeout(Duration::from_secs(5), session.wait())
        .await
        .expect("wait() hung")
        .expect("wait() errored");

    match session.signal(libc::SIGTERM) {
        Err(ExpectError::SessionClosed) => {}
        other => panic!("expected SessionClosed from signal() after reap, got {other:?}"),
    }
    match session.kill() {
        Err(ExpectError::SessionClosed) => {}
        other => panic!("expected SessionClosed from kill() after reap, got {other:?}"),
    }
}

/// The guard must not over-reject: a still-running child still receives the
/// signal and terminates with it. Regression test for S1.
#[tokio::test]
async fn signal_running_child_still_delivers() {
    let mut session = Session::spawn("/bin/sh", &["-c", "sleep 30"])
        .await
        .expect("spawn");

    session
        .signal(libc::SIGTERM)
        .expect("signalling a live child should succeed");

    let status = tokio::time::timeout(Duration::from_secs(5), session.wait())
        .await
        .expect("wait() hung")
        .expect("wait() errored");
    assert_eq!(
        status,
        ProcessExitStatus::Signaled(libc::SIGTERM),
        "expected SIGTERM termination, got {status:?}"
    );
}
