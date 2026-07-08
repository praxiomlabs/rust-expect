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

/// Regression for #40: on macOS, reaping the child (`waitpid`) tears down the
/// terminal and discards any output still buffered on the PTY master. A fast
/// exiting child's final output must therefore survive a reap that happens
/// before the session's first read.
///
/// This reaps deterministically (`is_running()` reaps through the child handle)
/// *before* the first `expect`, which pre-fix loses the output and yields
/// `Eof { buffer: "" }` — the exact intermittent CI symptom, made deterministic.
/// The fix salvages the buffered bytes ahead of every reap, so they remain
/// matchable.
#[tokio::test]
async fn output_survives_reap_before_first_read() {
    let mut session = Session::spawn("/bin/echo", &["hello"])
        .await
        .expect("spawn should succeed");

    // Let the child write "hello\n" and exit, then reap it (is_running ->
    // try_wait) before ever reading the master.
    tokio::time::sleep(Duration::from_millis(50)).await;
    while session.is_running() {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    let m = session
        .expect_timeout("hello", Duration::from_secs(5))
        .await
        .expect("final output must survive a reap-before-read on macOS");
    assert!(m.matched.contains("hello"));
}

/// Regression for #40, looped: hammer the fast-exit-then-expect path to catch
/// the reap/read race under scheduling jitter. Each iteration spawns
/// `echo hello`, forces a reap before reading (as above), and requires the
/// output to survive.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn fast_exit_output_survives_reap_looped() {
    for i in 0..100 {
        let mut session = Session::spawn("/bin/echo", &["hello"])
            .await
            .expect("spawn should succeed");
        tokio::time::sleep(Duration::from_millis(5)).await;
        while session.is_running() {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        let m = session
            .expect_timeout("hello", Duration::from_secs(5))
            .await
            .unwrap_or_else(|e| panic!("iter {i}: output lost after reap: {e:?}"));
        assert!(m.matched.contains("hello"), "iter {i}");
    }
}

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
/// slave device rather than reporting "not a tty". The device is named
/// `/dev/ttys*` on macOS/BSD and `/dev/pts/*` on Linux.
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
    // macOS/BSD: /dev/ttysNNN ; Linux: /dev/pts/N. Both mean the child has a
    // controlling pts; `tty(1)` prints "not a tty" when it has none.
    let names_pts = text.contains("/dev/ttys") || text.contains("/dev/pts/");
    assert!(
        names_pts,
        "child should have a controlling pts (/dev/ttys* or /dev/pts/*), got: {text:?}"
    );
    assert!(
        !text.contains("not a tty"),
        "child unexpectedly had no controlling terminal: {text:?}"
    );
}

/// Manual, macOS-only PTY-allocation stress test. Ignored so CI never runs it:
///
/// ```text
/// cargo test -p rust-expect --test macos_bsd_pty -- --ignored --nocapture
/// ```
///
/// macOS caps system-wide PTYs at `kern.tty.ptmx_max` (511 by default). This
/// spawns large concurrent batches of immediately-exiting sessions — a far
/// heavier concurrent-allocation load than [`concurrent_spawns_all_allocate`]
/// — waiting on and reaping each (so no zombies accumulate), and asserts every
/// spawn succeeds. It stays bounded well under the cap, and each wave fully
/// drains before the next, so it never sustains exhaustion or starves other
/// processes.
///
/// Relationship to the retry path: `open_pty_pair_with_retry` retries when
/// `openpty` transiently fails as the PTY table brushes its cap. That happens
/// here when the machine is *already* near the cap — e.g. run this while the
/// full test suite runs, which is exactly the workload that surfaced the
/// original flakiness. Note it is **not** possible to force a *recoverable*
/// retry deterministically in isolation on macOS: holding sessions gives a
/// hard step at the cap (over-subscription fails outright rather than
/// transiently), and PTY teardown lags allocation, so a sustained push past
/// the cap produces unrecoverable failures instead of transient ones. This
/// test therefore verifies robustness under heavy concurrent allocation rather
/// than asserting a retry fired.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "macOS PTY-cap stress; manual only: cargo test -- --ignored"]
async fn pty_allocation_stress_under_concurrency() {
    // 250 concurrent < the ~511 cap (minus baseline usage), so a single wave
    // never sustains exhaustion; several drained waves keep the pressure up.
    const BATCH: usize = 250;
    const WAVES: usize = 4;

    for wave in 0..WAVES {
        let mut handles = Vec::with_capacity(BATCH);
        for _ in 0..BATCH {
            handles.push(tokio::spawn(async {
                let mut s = Session::spawn("/bin/sh", &["-c", "exit 0"]).await?;
                // wait_timeout reaps the child, so no zombies pile up.
                s.wait_timeout(Duration::from_secs(10)).await.ok();
                Ok::<(), rust_expect::ExpectError>(())
            }));
        }
        for h in handles {
            h.await
                .expect("spawn task panicked")
                .unwrap_or_else(|e| panic!("wave {wave}: every spawn should allocate a PTY: {e}"));
        }
    }
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
