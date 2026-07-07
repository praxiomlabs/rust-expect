//! Parity tests for the F1 migration: the Unix spawn now runs on rust-pty's
//! `tokio::process` path instead of a hand-rolled `fork`/`exec`.
//!
//! Every test runs under a **multi-threaded** runtime (`worker_threads = 4`) —
//! the exact scenario the old hand-rolled fork was unsafe in (non-async-signal-
//! safe work between `fork` and `exec`). These assert the migrated path behaves
//! correctly under that runtime.

#![cfg(unix)]

use std::time::Duration;

use rust_expect::{ProcessExitStatus, Session, SessionConfig};

const SIGTERM: i32 = 15;

/// F1 regression: on the hand-rolled fork path, a child spawned under a
/// multi-threaded runtime **intermittently produced no output at all** (an
/// empty expect buffer — see the closed PR #37), consistent with the
/// fork-in-threaded-tokio hazard. Spawn repeatedly under 4 workers and require
/// the child's output to actually arrive every time.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn env_var_delivered_and_output_received_under_multi_thread() {
    for i in 0..5 {
        let config = SessionConfig::new("/bin/sh").env("RUST_EXPECT_F1_PROBE", "delivered");
        let mut session = Session::spawn_with_config(
            "/bin/sh",
            &["-c", "printf 'probe=%s\\n' \"$RUST_EXPECT_F1_PROBE\""],
            config,
        )
        .await
        .expect("spawn should succeed");

        let m = session
            .expect_timeout("probe=delivered", Duration::from_secs(10))
            .await
            .unwrap_or_else(|e| panic!("iteration {i}: child produced no output: {e:?}"));
        assert!(
            m.matched.contains("delivered"),
            "iteration {i}: {:?}",
            m.matched
        );
    }
}

/// The child must have a controlling terminal: `tty(1)` prints the slave device
/// path when stdin is a tty, or "not a tty" otherwise. This validates that
/// rust-pty's `setsid` + `TIOCSCTTY` pre_exec wiring took effect.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn controlling_tty_established_under_multi_thread() {
    let mut session = Session::spawn("/bin/sh", &["-c", "tty"])
        .await
        .expect("spawn should succeed");

    let m = session
        .expect_timeout("/dev/", Duration::from_secs(10))
        .await
        .expect("child should have a controlling tty (tty(1) printed a device path)");
    assert!(
        m.matched.contains("/dev/"),
        "expected a tty path, got: {:?}",
        m.matched
    );
}

/// A normally-exiting child reports its real exit code (not `Unknown`) under a
/// multi-threaded runtime.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn exit_code_reported_under_multi_thread() {
    let mut session = Session::spawn("/bin/sh", &["-c", "exit 7"])
        .await
        .expect("spawn should succeed");

    let status = tokio::time::timeout(Duration::from_secs(10), session.wait())
        .await
        .expect("wait() hung")
        .expect("wait() errored");
    assert_eq!(status, ProcessExitStatus::Exited(7));
}

/// A signal-terminated child is distinguished from a normal exit under a
/// multi-threaded runtime.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn signal_exit_reported_under_multi_thread() {
    let mut session = Session::spawn("/bin/sh", &["-c", "kill -TERM $$"])
        .await
        .expect("spawn should succeed");

    let status = tokio::time::timeout(Duration::from_secs(10), session.wait())
        .await
        .expect("wait() hung")
        .expect("wait() errored");
    assert_eq!(status, ProcessExitStatus::Signaled(SIGTERM));
}

/// S1 under a multi-threaded runtime: after the child is reaped, `signal`/`kill`
/// must refuse (PID-reuse guard) rather than target a recycled PID.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reap_then_signal_rejected_under_multi_thread() {
    let mut session = Session::spawn("/bin/sh", &["-c", "exit 0"])
        .await
        .expect("spawn should succeed");

    // Reap the child.
    let _ = tokio::time::timeout(Duration::from_secs(10), session.wait())
        .await
        .expect("wait() hung")
        .expect("wait() errored");

    // A post-reap signal must be rejected, not delivered to a recycled PID.
    let result = session.signal(SIGTERM);
    assert!(
        result.is_err(),
        "post-reap signal should be rejected, got {result:?}"
    );
}
