//! macOS/BSD PTY behavior regression tests.
//!
//! macOS runs rust-expect's hand-rolled Unix PTY backend under BSD semantics
//! that differ from the Linux-heavy CI in ways worth pinning down:
//!
//! - A read of the PTY master after the child exits returns a **0-byte read**
//!   (BSD) rather than `EIO` (Linux); both must be treated as EOF *without*
//!   dropping the child's final, unterminated output chunk.
//! - The PTY slave becomes the child's controlling terminal (`setsid` +
//!   `TIOCSCTTY`), so `tty(1)` in the child names a real `/dev/ttys*` device.
//! - macOS caps system-wide PTYs at `kern.tty.ptmx_max` (511), so concurrent
//!   allocation must be resilient (see `open_pty_pair_with_retry`).
//!
//! These are Unix-only; the assertions hold on Linux too but the BSD EOF and
//! controlling-tty paths are what CI's macOS `test` job exercises here.
#![cfg(unix)]

use std::time::Duration;

use rust_expect::Session;
use rust_expect::types::ProcessExitStatus;

/// The child's final chunk (no trailing newline, printed right before exit)
/// must survive the EOF transition and still be matchable.
#[tokio::test]
async fn eof_drain_preserves_final_chunk() {
    let mut session = Session::spawn("/bin/sh", &["-c", "printf 'first\\nlast-line'; exit 0"])
        .await
        .expect("spawn should succeed");

    let m = session
        .expect_timeout("last-line", Duration::from_secs(5))
        .await
        .expect("final chunk must not be lost on EOF");
    assert!(m.matched.contains("last-line"));
}

/// `wait()` must return promptly with the child's real exit status once the
/// child closes the PTY slave — no hang, and the code is preserved.
#[tokio::test]
async fn wait_reports_real_exit_status() {
    let mut session = Session::spawn("/bin/sh", &["-c", "exit 7"])
        .await
        .expect("spawn should succeed");

    let status = session
        .wait_timeout(Duration::from_secs(5))
        .await
        .expect("wait should return promptly, not hang");
    assert_eq!(status, ProcessExitStatus::Exited(7), "got {status:?}");
}

/// `expect_eof` must resolve promptly when the child exits.
#[tokio::test]
async fn expect_eof_resolves_promptly() {
    let mut session = Session::spawn("/bin/sh", &["-c", "printf done; exit 0"])
        .await
        .expect("spawn should succeed");

    session
        .expect_timeout("done", Duration::from_secs(5))
        .await
        .expect("should see child output");
    session
        .expect_eof_timeout(Duration::from_secs(5))
        .await
        .expect("expect_eof should resolve at child exit");
}

/// The child's controlling terminal is the PTY slave: `tty(1)` names a real
/// `/dev/ttys*` device rather than reporting "not a tty".
#[tokio::test]
async fn child_controlling_terminal_is_pts() {
    let mut session = Session::spawn("/bin/sh", &["-c", "tty; exit 0"])
        .await
        .expect("spawn should succeed");

    let out = session
        .expect_eof_timeout(Duration::from_secs(5))
        .await
        .expect("expect_eof");
    let text = out.before;
    assert!(
        text.contains("/dev/tty"),
        "child should have a controlling pts, got: {text:?}"
    );
    assert!(
        !text.contains("not a tty"),
        "child unexpectedly had no controlling terminal: {text:?}"
    );
}

/// Concurrent PTY allocation stays reliable under moderate load. Exercises the
/// transient-exhaustion retry path without approaching `ptmx_max` (511).
#[tokio::test]
async fn concurrent_spawns_all_allocate() {
    let mut handles = Vec::new();
    for _ in 0..32 {
        handles.push(tokio::spawn(async {
            let mut s = Session::spawn("/bin/sh", &["-c", "printf ok; exit 0"])
                .await
                .expect("concurrent spawn should allocate a PTY");
            s.expect_timeout("ok", Duration::from_secs(5))
                .await
                .expect("child output");
            s.wait_timeout(Duration::from_secs(5)).await.ok();
        }));
    }
    for h in handles {
        h.await.expect("task should not panic");
    }
}
