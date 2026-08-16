//! Regression tests for Unix process ownership: what a session owns, what it
//! signals, and what survives it.
//!
//! Dropping a session closes the PTY master, and the kernel sends `SIGHUP` to
//! the foreground process group of a controlling terminal that hangs up. That
//! alone cleans up an ordinary child, which is why this gap stayed hidden — but
//! it is a side effect of drop order, not an owned guarantee, and it does
//! nothing for a child that ignores the signal. These tests pin both the
//! hangup and the ownership that now backs it.

#![cfg(unix)]

use std::time::Duration;

use rust_expect::{ProcessExitStatus, Session, SessionConfig, TimeoutConfig};

/// Whether a pid still names a live (or zombie) process.
fn alive(pid: u32) -> bool {
    #[allow(unsafe_code)]
    // SAFETY: signal 0 performs error checking only; it never delivers.
    let result = unsafe { libc::kill(pid as i32, 0) };
    result == 0
}

/// Poll for up to a second for `pid` to disappear.
async fn gone_within(pid: u32, limit: Duration) -> bool {
    let deadline = std::time::Instant::now() + limit;
    while std::time::Instant::now() < deadline {
        if !alive(pid) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    !alive(pid)
}

/// A child with no `SIGHUP` handler dies when the session drops, because
/// closing the master hangs up its controlling terminal.
#[tokio::test]
async fn dropping_a_session_hangs_up_an_ordinary_child() {
    let session = Session::spawn("/bin/sh", &["-c", "sleep 300"])
        .await
        .expect("spawn");
    let pid = session.pid().expect("pid");
    assert!(alive(pid), "child should be running before the drop");

    drop(session);

    assert!(
        gone_within(pid, Duration::from_secs(2)).await,
        "pid {pid} survived the drop"
    );
}

/// A child that ignores `SIGHUP` outlives the session that owns it. Nothing
/// else is holding it: the session is gone and no one will ever reap it.
#[tokio::test]
async fn a_child_ignoring_sighup_outlives_its_session() {
    let mut session = Session::spawn("/bin/sh", &["-c", "trap '' HUP; echo ready; sleep 300"])
        .await
        .expect("spawn");
    session
        .expect_timeout(
            rust_expect::Pattern::literal("ready"),
            Duration::from_secs(5),
        )
        .await
        .expect("child should announce itself");
    let pid = session.pid().expect("pid");

    drop(session);

    assert!(
        gone_within(pid, Duration::from_secs(2)).await,
        "pid {pid} ignored SIGHUP and outlived its session"
    );
}

/// `kill()` must reach the whole process group, not just its leader. A
/// background job in the session's process group is exactly what a shell
/// leaves behind.
///
/// The descendant ignores `SIGHUP` so that this measures `kill()`'s reach and
/// not the terminal hangup that follows the leader's death — with an ordinary
/// descendant, the hangup cleans up and the test passes whatever `kill()` does.
#[tokio::test]
async fn kill_reaches_the_childs_descendants() {
    let mut session = Session::spawn(
        "/bin/sh",
        &[
            "-c",
            "trap '' HUP; (trap '' HUP; sleep 300) & echo $!; wait",
        ],
    )
    .await
    .expect("spawn");

    let m = session
        .expect_timeout(
            rust_expect::Pattern::regex(r"(\d+)").unwrap(),
            Duration::from_secs(5),
        )
        .await
        .expect("child should report the background pid");
    let grandchild: u32 = m
        .matched
        .trim()
        .parse()
        .unwrap_or_else(|e| panic!("bad pid {:?}: {e}", m.matched));
    assert!(alive(grandchild), "background job should be running");

    session.kill().expect("kill");

    assert!(
        gone_within(grandchild, Duration::from_secs(2)).await,
        "descendant {grandchild} survived kill() of the session leader"
    );
}

/// A detached child is the caller's problem, not the session's. This is the
/// opt-out from the drop kill, and the only way to launch something meant to
/// outlive its session.
#[tokio::test]
async fn a_detached_child_outlives_its_session() {
    let mut session = Session::spawn("/bin/sh", &["-c", "trap '' HUP; echo ready; sleep 300"])
        .await
        .expect("spawn");
    session
        .expect_timeout(
            rust_expect::Pattern::literal("ready"),
            Duration::from_secs(5),
        )
        .await
        .expect("child should announce itself");
    let pid = session.pid().expect("pid");

    session.detach();
    drop(session);

    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        alive(pid),
        "detached pid {pid} was killed anyway; detach() did not take"
    );

    // Not this test's business to leak it.
    #[allow(unsafe_code)]
    // SAFETY: pid names the child this test spawned and just confirmed alive.
    unsafe {
        libc::kill(pid as i32, libc::SIGKILL);
    }
}

/// `shutdown()` ends a cooperative child and reports its status, without
/// waiting out the grace period.
#[tokio::test]
async fn shutdown_ends_a_cooperative_child_promptly() {
    let mut session = Session::spawn("/bin/sh", &["-c", "sleep 300"])
        .await
        .expect("spawn");
    let pid = session.pid().expect("pid");

    let started = std::time::Instant::now();
    let status = tokio::time::timeout(Duration::from_secs(5), session.shutdown())
        .await
        .expect("shutdown() hung")
        .expect("shutdown() errored");

    assert!(
        started.elapsed() < Duration::from_secs(2),
        "shutdown took {:?}; it waited out the grace period on a child that took SIGTERM",
        started.elapsed()
    );
    assert!(
        matches!(
            status,
            ProcessExitStatus::Signaled(_) | ProcessExitStatus::Exited(_)
        ),
        "expected a real exit status, got {status:?}"
    );
    assert!(!alive(pid), "pid {pid} survived shutdown()");
}

/// A child that refuses to take the hint is killed anyway. The grace period is
/// shortened so the test does not sit through the 10s default.
#[tokio::test]
async fn shutdown_kills_a_child_that_ignores_sigterm() {
    let config = SessionConfig {
        timeout: TimeoutConfig {
            close: Duration::from_millis(300),
            ..TimeoutConfig::default()
        },
        ..SessionConfig::default()
    };
    let mut session = Session::spawn_with_config(
        "/bin/sh",
        &["-c", "trap '' TERM; echo ready; sleep 300"],
        config,
    )
    .await
    .expect("spawn");
    session
        .expect_timeout(
            rust_expect::Pattern::literal("ready"),
            Duration::from_secs(5),
        )
        .await
        .expect("child should announce itself");
    let pid = session.pid().expect("pid");

    tokio::time::timeout(Duration::from_secs(5), session.shutdown())
        .await
        .expect("shutdown() hung")
        .expect("shutdown() errored");

    assert!(
        !alive(pid),
        "pid {pid} ignored SIGTERM and survived shutdown()"
    );
}

/// `shutdown()` on an already-exited child reports its status rather than
/// erroring on a signal that cannot be delivered.
#[tokio::test]
async fn shutdown_is_a_no_op_on_a_child_that_already_exited() {
    let mut session = Session::spawn("/bin/sh", &["-c", "exit 3"])
        .await
        .expect("spawn");

    tokio::time::sleep(Duration::from_millis(200)).await;

    let status = tokio::time::timeout(Duration::from_secs(5), session.shutdown())
        .await
        .expect("shutdown() hung")
        .expect("shutdown() errored");

    assert_eq!(status, ProcessExitStatus::Exited(3));
}
