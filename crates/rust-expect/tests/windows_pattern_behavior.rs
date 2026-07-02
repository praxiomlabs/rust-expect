//! Windows ConPTY read/pattern behaviors.
//!
//! Covers two fixes verified on the ConPTY backend:
//!   1. `Pattern::Bytes(n)` now matches once `n` bytes are buffered (previously
//!      it returned `None` unconditionally and hung until timeout).
//!   2. The reader drains the output pipe until `ReadFile` reports broken-pipe,
//!      instead of truncating as soon as the exit watcher clears the `open`
//!      flag — so a child's buffered output is not discarded on exit.
//!
//! Note on ConPTY output visibility: on some Windows configurations (notably
//! headless/service contexts and certain preview builds) conhost does not
//! forward a child's *rendered* output to the read pipe at all — this is an
//! OS/environment behavior reproducible with other ConPTY libraries, not a
//! property of this crate. The assertions below only rely on the ConPTY
//! *handshake* frame, which conhost always emits, so they are robust to that.

#![cfg(windows)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use rust_expect::{Pattern, Session};

/// `expect(Pattern::bytes(n))` must resolve once at least `n` bytes have been
/// received. Previously it returned `None` unconditionally and hung until the
/// timeout. ConPTY emits its handshake frame (tens of bytes) immediately, so a
/// small `n` resolves quickly.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bytes_pattern_matches_once_enough_received() {
    let mut session = Session::spawn("cmd.exe", &["/c", "exit 0"])
        .await
        .expect("spawn cmd.exe");

    let m = session
        .expect_timeout(Pattern::bytes(3), Duration::from_secs(10))
        .await
        .expect("Bytes(3) should match once >=3 bytes arrive");

    assert_eq!(
        m.matched.len(),
        3,
        "Bytes(3) should consume exactly 3 bytes, got {:?}",
        m.matched
    );

    session.wait_timeout(Duration::from_secs(5)).await.ok();
}

/// `Bytes(n)` must wait for `n` bytes: an unreachable `n` must not match early,
/// it must time out (confirming the count is actually honored, not ignored).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bytes_pattern_waits_for_enough_data() {
    let mut session = Session::spawn("cmd.exe", &["/c", "exit 0"])
        .await
        .expect("spawn cmd.exe");

    // Far more than the handshake frame ever provides.
    let result = session
        .expect_timeout(Pattern::bytes(1_000_000), Duration::from_secs(2))
        .await;

    assert!(
        result.is_err(),
        "Bytes(1_000_000) must not match on a tiny output (got {result:?})"
    );
    session.wait_timeout(Duration::from_secs(5)).await.ok();
}

/// Regression test for the read-drain fix: buffered ConPTY output must not be
/// truncated when the child exits.
///
/// Before the fix, the exit watcher cleared the shared `open` flag the instant
/// the child exited, and `poll_read` short-circuited to EOF on `!open` — so any
/// bytes conhost had already written to the pipe but that the reader had not yet
/// consumed were discarded. Concretely, only the first ~18 bytes of the ConPTY
/// handshake frame survived. After the fix the reader drains until `ReadFile`
/// reports `ERROR_BROKEN_PIPE`, recovering the full frame (~85 bytes, including
/// conhost's clear-screen and title sequences).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn drains_buffered_output_on_exit() {
    let mut session = Session::spawn("cmd.exe", &["/c", "exit 0"])
        .await
        .expect("spawn cmd.exe");

    let captured: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&captured);
    session.add_output_tap(move |chunk: &[u8]| sink.lock().unwrap().extend_from_slice(chunk));

    session
        .wait_timeout(Duration::from_secs(10))
        .await
        .expect("child should exit");

    let bytes = captured.lock().unwrap().clone();
    assert!(
        bytes.len() > 40,
        "buffered ConPTY output was truncated on exit: drained only {} bytes ({:?})",
        bytes.len(),
        String::from_utf8_lossy(&bytes)
    );
}
