//! Dropping a session must never panic, and must still end its child, when it
//! happens outside the runtime that made it.
//!
//! `Session`'s drop reaps through tokio's child handle before signalling, so it
//! cannot deliver to a recycled pid — and a panic on that path would be far
//! worse than the leak it prevents.
//!
//! In its own test binary on purpose. Creating and dropping a runtime disturbs
//! tokio's process-wide `SIGCHLD` reaping, which breaks any `#[tokio::test]`
//! reaping a child in parallel — observed while writing these tests.

#![cfg(unix)]

use rust_expect::Session;

/// Reap `pid` if it has terminated, returning whether it had.
///
/// The test process is the child's parent, so it can reap directly. This is
/// what distinguishes "dead" from "alive" here: with the runtime gone nothing
/// reaps orphans, so a killed child stays a zombie and `kill(pid, 0)` — which
/// succeeds for zombies — would report it as alive.
/// Polls, because `SIGKILL` is delivered asynchronously: the child has not
/// necessarily terminated by the time the signalling call returns.
fn reaped_within(pid: u32, limit: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + limit;
    loop {
        let mut status: libc::c_int = 0;
        #[allow(unsafe_code)]
        // SAFETY: pid names a child of this process; WNOHANG makes this
        // non-blocking, and `status` is a valid out-pointer.
        let result = unsafe { libc::waitpid(pid as i32, &raw mut status, libc::WNOHANG) };
        if result == pid as i32 {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
fn dropping_a_session_outside_its_runtime_kills_its_child() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let session = runtime.block_on(async {
        Session::spawn("/bin/sh", &["-c", "trap '' HUP; sleep 300"])
            .await
            .expect("spawn")
    });
    let pid = session.pid().expect("pid");

    drop(runtime);
    // Panicking here is the failure this test exists for.
    drop(session);

    assert!(
        reaped_within(pid, std::time::Duration::from_secs(2)),
        "pid {pid} was still running after a drop outside its runtime"
    );
}
