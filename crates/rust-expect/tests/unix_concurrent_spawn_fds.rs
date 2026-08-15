//! Concurrently spawned sessions must not inherit each other's PTY slave fds.
//!
//! The three stdio descriptors handed to a child were duplicated with `dup`,
//! which does not carry `FD_CLOEXEC` over. They sat in the parent as
//! inheritable fds until `spawn`, so any process forked by another thread in
//! that window inherited them.
//!
//! The consequence is worse than a leaked descriptor. A session's master never
//! reaches EOF while some other session's child holds its slave open, so
//! `wait`, `wait_timeout` and `expect_eof` hang for as long as that unrelated
//! child lives — a wedge that lasts as long as the longest-running sibling.

#![cfg(unix)]

#[cfg(target_os = "linux")]
use std::collections::BTreeSet;
use std::time::Duration;

use rust_expect::{Pattern, Session};

/// The `/dev/pts/*` devices a process currently holds open.
///
/// Reads `/proc`, so it exists only where `/proc` does.
#[cfg(target_os = "linux")]
fn pts_devices(pid: u32) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let Ok(entries) = std::fs::read_dir(format!("/proc/{pid}/fd")) else {
        return found;
    };
    for entry in entries.flatten() {
        if let Ok(target) = std::fs::read_link(entry.path()) {
            let target = target.to_string_lossy().into_owned();
            if target.starts_with("/dev/pts/") {
                found.insert(target);
            }
        }
    }
    found
}

/// Each child should hold exactly one pts — its own.
///
/// Linux-only: it reads `/proc/<pid>/fd`. The defect is not Linux-specific, but
/// this is the portable-enough way to observe it directly.
#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn concurrently_spawned_children_do_not_hold_each_others_pts() {
    let mut spawning = Vec::new();
    for _ in 0..6 {
        spawning.push(tokio::spawn(async {
            Session::spawn("/bin/sh", &["-c", "echo up; sleep 300"])
                .await
                .expect("spawn")
        }));
    }

    let mut sessions = Vec::new();
    for handle in spawning {
        let mut session = handle.await.expect("join");
        session
            .expect_timeout(Pattern::literal("up"), Duration::from_secs(5))
            .await
            .expect("child should announce itself");
        sessions.push(session);
    }

    let held: Vec<(u32, BTreeSet<String>)> = sessions
        .iter()
        .map(|s| {
            let pid = s.pid().expect("pid");
            (pid, pts_devices(pid))
        })
        .collect();

    // Drop kills them; do it before asserting so a failure does not leak.
    drop(sessions);

    for (pid, ptss) in &held {
        assert!(
            ptss.len() <= 1,
            "child {pid} holds {} pts devices, so it has another session's \
             slave open and is wedging that session's EOF: {ptss:?}",
            ptss.len()
        );
    }
}

/// The behaviour that leak produced: an earlier session could not reach EOF
/// while a concurrently spawned, longer-lived child held its slave.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn a_sibling_child_does_not_wedge_another_sessions_eof() {
    let mut spawning = Vec::new();
    for _ in 0..6 {
        spawning.push(tokio::spawn(async {
            Session::spawn("/bin/sh", &["-c", "echo up; sleep 300"])
                .await
                .expect("spawn")
        }));
    }

    let mut sessions = Vec::new();
    for handle in spawning {
        let mut session = handle.await.expect("join");
        session
            .expect_timeout(Pattern::literal("up"), Duration::from_secs(5))
            .await
            .expect("child should announce itself");
        sessions.push(session);
    }

    // End one of them. Its own child is gone, so its master must reach EOF —
    // the others are still running and must not be able to hold it open.
    //
    // An already-exited child is an acceptable starting point and not a
    // failure: what this test measures is what happens *after* the victim's
    // child is gone, however it went. (A slow runner once took long enough
    // over the six `expect`s that the children outlived their own sleep, which
    // is why the sleep is now far longer than any plausible run.)
    let mut victim = sessions.remove(0);
    match victim.kill() {
        Ok(()) | Err(rust_expect::ExpectError::SessionClosed) => {}
        Err(e) => panic!("kill failed: {e}"),
    }

    let status = tokio::time::timeout(
        Duration::from_secs(3),
        victim.wait_timeout(Duration::from_secs(2)),
    )
    .await
    .expect("wait_timeout hung: a sibling is holding the slave open")
    .expect("the session never reached EOF while its siblings ran");

    assert!(
        !matches!(status, rust_expect::ProcessExitStatus::Unknown),
        "expected a real exit status, got {status:?}"
    );
}
