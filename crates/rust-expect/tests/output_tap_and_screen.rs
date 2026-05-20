//! Integration tests for the additive output-tap and screen-integration
//! primitives added to `Session`: `add_output_tap`, `attach_screen`,
//! `expect_screen_contains`, `wait_screen_stable`, and `send_paste`.
//!
//! These tests spawn `/bin/sh` with small fixed scripts and verify the
//! primitives observe and assert on the resulting byte stream / screen.

#![cfg(all(unix, feature = "screen"))]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use rust_expect::{ExpectError, Session, SessionBuilder};

fn build(script: &str) -> (String, Vec<String>, rust_expect::config::SessionConfig) {
    let cmd = "/bin/sh".to_string();
    let args = vec!["-c".to_string(), script.to_string()];
    let config = SessionBuilder::new()
        .command(&cmd)
        .args(args.iter().cloned())
        .dimensions(80, 24)
        .timeout(Duration::from_secs(10))
        .build();
    (cmd, args, config)
}

/// `add_output_tap` should observe every byte the matcher buffer sees.
#[tokio::test]
async fn output_tap_observes_all_bytes() {
    let (cmd, args, config) = build("printf 'tap-saw-this\\n'; exit 0");
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config)
        .await
        .unwrap();

    let captured: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let buf = captured.clone();
    session.add_output_tap(move |chunk| {
        buf.lock().unwrap().extend_from_slice(chunk);
    });

    session
        .expect_timeout("tap-saw-this", Duration::from_secs(3))
        .await
        .unwrap();
    session.wait_timeout(Duration::from_secs(2)).await.ok();

    let bytes_snapshot = captured.lock().unwrap().clone();
    assert!(
        std::str::from_utf8(&bytes_snapshot)
            .map(|s| s.contains("tap-saw-this"))
            .unwrap_or(false),
        "tap should have captured the literal output; got {bytes_snapshot:?}"
    );
}

/// Multiple taps fire in registration order on every chunk.
#[tokio::test]
async fn multiple_taps_each_receive_chunks() {
    let (cmd, args, config) = build("printf 'fan-out\\n'; exit 0");
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config)
        .await
        .unwrap();

    let counters: Vec<Arc<Mutex<u32>>> = (0..3).map(|_| Arc::new(Mutex::new(0))).collect();
    for c in &counters {
        let c = c.clone();
        session.add_output_tap(move |_chunk| {
            *c.lock().unwrap() += 1;
        });
    }

    session
        .expect_timeout("fan-out", Duration::from_secs(3))
        .await
        .unwrap();
    session.wait_timeout(Duration::from_secs(2)).await.ok();

    let counts: Vec<u32> = counters.iter().map(|c| *c.lock().unwrap()).collect();
    assert!(
        counts.iter().all(|&n| n >= 1),
        "every tap should have fired; got {counts:?}"
    );
    assert!(
        counts.windows(2).all(|w| w[0] == w[1]),
        "taps should have fired the same number of times; got {counts:?}"
    );
}

/// An attached screen accumulates rendered text as bytes arrive.
#[tokio::test]
async fn attach_screen_renders_emitted_text() {
    let (cmd, args, config) = build("printf 'rendered-via-screen\\n'; exit 0");
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config)
        .await
        .unwrap();

    session.attach_screen();

    session
        .expect_timeout("rendered-via-screen", Duration::from_secs(3))
        .await
        .unwrap();
    session.wait_timeout(Duration::from_secs(2)).await.ok();

    let text = session
        .screen()
        .expect("screen should be attached")
        .lock()
        .unwrap()
        .text();
    assert!(
        text.contains("rendered-via-screen"),
        "screen text should contain the output; got {text:?}"
    );
}

/// Screen integration handles multi-byte UTF-8 cleanly (regression test for
/// the parser fix in screen/parser.rs).
#[tokio::test]
async fn attach_screen_preserves_utf8() {
    let (cmd, args, config) = build("printf 'box ╭─╮ rocket 🚀\\n'; exit 0");
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config)
        .await
        .unwrap();

    session.attach_screen();
    session
        .expect_timeout("rocket", Duration::from_secs(3))
        .await
        .unwrap();
    session.wait_timeout(Duration::from_secs(2)).await.ok();

    let text = session.screen().unwrap().lock().unwrap().text();
    assert!(
        text.contains("╭─╮"),
        "box-drawing should round-trip; got {text:?}"
    );
    assert!(text.contains('🚀'), "emoji should round-trip; got {text:?}");
}

/// `expect_screen_contains` returns as soon as the substring appears.
#[tokio::test]
async fn expect_screen_contains_succeeds_on_match() {
    let (cmd, args, config) = build("printf 'go-no-go\\n'; sleep 0.5; exit 0");
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config)
        .await
        .unwrap();
    session.attach_screen();

    session
        .expect_screen_contains("go-no-go", Duration::from_secs(3))
        .await
        .expect("should find substring on screen");
}

/// `expect_screen_contains` times out cleanly when the needle never appears.
#[tokio::test]
async fn expect_screen_contains_times_out_when_absent() {
    let (cmd, args, config) = build("printf 'something\\n'; sleep 0.5; exit 0");
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config)
        .await
        .unwrap();
    session.attach_screen();

    let r = session
        .expect_screen_contains("never-appears-on-screen", Duration::from_millis(500))
        .await;
    assert!(r.is_err(), "expected timeout error, got {r:?}");
}

/// `wait_screen_stable` returns successfully once output stops changing.
#[tokio::test]
async fn wait_screen_stable_returns_after_quiet_period() {
    // Emit a few lines quickly, then go silent.
    let (cmd, args, config) =
        build("printf 'one\\n'; printf 'two\\n'; printf 'three\\n'; sleep 1; exit 0");
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config)
        .await
        .unwrap();
    session.attach_screen();

    let start = std::time::Instant::now();
    session
        .wait_screen_stable(Duration::from_millis(300), Duration::from_secs(5))
        .await
        .expect("screen should stabilize after the three prints");
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(3),
        "wait_screen_stable should return promptly after output goes quiet; elapsed={elapsed:?}"
    );

    let text = session.screen().unwrap().lock().unwrap().text();
    for s in ["one", "two", "three"] {
        assert!(text.contains(s), "screen missing {s:?}: text={text:?}");
    }
}

/// A panic in one tap does not kill subsequent taps and does not propagate
/// out of the read loop.
#[tokio::test]
async fn panicking_tap_does_not_break_other_taps() {
    let (cmd, args, config) = build("printf 'observed\\n'; exit 0");
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config)
        .await
        .unwrap();

    let saw_after: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));

    // First tap panics on every chunk.
    session.add_output_tap(|_| panic!("intentional tap panic"));
    // Second tap should still receive chunks.
    let sa = saw_after.clone();
    session.add_output_tap(move |chunk| {
        if std::str::from_utf8(chunk)
            .unwrap_or("")
            .contains("observed")
        {
            *sa.lock().unwrap() = true;
        }
    });

    session
        .expect_timeout("observed", Duration::from_secs(3))
        .await
        .expect("matcher should still see the bytes after the tap panic");
    assert!(
        *saw_after.lock().unwrap(),
        "second tap must still observe chunks despite first tap panicking"
    );

    session.wait_timeout(Duration::from_secs(2)).await.ok();
}

/// `remove_output_tap` returns `true` for a known id and stops firing the
/// callback; `false` for an unknown id.
#[tokio::test]
async fn remove_output_tap_stops_invocations() {
    let (cmd, args, config) = build("printf 'before\\n'; sleep 0.5; printf 'after\\n'; exit 0");
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config)
        .await
        .unwrap();

    let count: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
    let cc = count.clone();
    let id = session.add_output_tap(move |_| *cc.lock().unwrap() += 1);

    session
        .expect_timeout("before", Duration::from_secs(3))
        .await
        .unwrap();
    let after_first = *count.lock().unwrap();
    assert!(after_first >= 1);

    assert!(
        session.remove_output_tap(id),
        "remove should succeed for known id"
    );
    assert!(
        !session.remove_output_tap(id),
        "remove should be idempotent"
    );

    session
        .expect_timeout("after", Duration::from_secs(3))
        .await
        .unwrap();
    let after_second = *count.lock().unwrap();
    assert_eq!(
        after_first, after_second,
        "tap count must not advance after removal (before={after_first}, after={after_second})"
    );

    session.wait_timeout(Duration::from_secs(2)).await.ok();
}

/// Re-attaching a screen replaces the previous one and does NOT leak the
/// previous attach's internal tap.
#[tokio::test]
async fn attach_screen_replacement_does_not_leak_taps() {
    let (cmd, args, config) = build("sleep 2; exit 0");
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config)
        .await
        .unwrap();

    session.attach_screen();
    assert_eq!(session.output_tap_callbacks().count(), 1);

    // Add a user tap so we can verify the count reflects the user tap +
    // exactly one screen tap (not two screen taps).
    let count: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
    let cc = count.clone();
    session.add_output_tap(move |_| *cc.lock().unwrap() += 1);
    assert_eq!(session.output_tap_callbacks().count(), 2);

    // Re-attach. The previous screen's tap must be removed.
    session.attach_screen();
    assert_eq!(
        session.output_tap_callbacks().count(),
        2,
        "re-attach should replace, not stack, the screen tap (user tap + 1 screen tap)"
    );

    let _ = session.send_control(rust_expect::ControlChar::CtrlC).await;
    session.wait_timeout(Duration::from_secs(2)).await.ok();
}

/// If the screen mutex becomes poisoned after the screen has been
/// populated, subsequent `expect_screen_contains` calls still succeed via
/// the poison-recovery path in `lock_screen()` instead of silently
/// always-missing on the poisoned lock.
#[tokio::test]
async fn screen_mutex_poison_recovers() {
    let (cmd, args, config) =
        build("printf 'POISON-MARKER\\n'; sleep 0.3; printf 'AFTER\\n'; sleep 0.3; exit 0");
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config)
        .await
        .unwrap();

    session.attach_screen();

    // Let the first marker render so the screen has real content.
    session
        .expect_screen_contains("POISON-MARKER", Duration::from_secs(2))
        .await
        .expect("baseline: marker should render before we poison the lock");

    // Now poison the mutex by panicking while holding it on another thread.
    let screen_handle = session.screen().unwrap().clone();
    let _ = std::thread::spawn(move || {
        let _guard = screen_handle.lock().unwrap();
        panic!("intentional panic to poison the screen mutex");
    })
    .join();
    assert!(
        session.screen().unwrap().lock().is_err(),
        "test setup failed: screen mutex should be poisoned by now"
    );

    // expect_screen_contains must still see the rendered content via the
    // poison-recovery path in lock_screen().
    session
        .expect_screen_contains("POISON-MARKER", Duration::from_secs(1))
        .await
        .expect("must recover from poisoning and still see the marker");

    // The tap closure also recovers from poisoning, so new bytes continue
    // to update the screen and `AFTER` should land too.
    session
        .expect_screen_contains("AFTER", Duration::from_secs(3))
        .await
        .expect("tap must continue feeding screen across poison recovery");

    session.wait_timeout(Duration::from_secs(2)).await.ok();
}

/// `detach_screen` removes its tap and `screen()` returns None.
#[tokio::test]
async fn detach_screen_removes_internal_tap() {
    let (cmd, args, config) = build("sleep 2; exit 0");
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config)
        .await
        .unwrap();
    session.attach_screen();
    assert!(session.screen().is_some());
    assert_eq!(session.output_tap_callbacks().count(), 1);

    assert!(session.detach_screen());
    assert!(session.screen().is_none());
    assert_eq!(
        session.output_tap_callbacks().count(),
        0,
        "screen's internal tap should be removed"
    );
    // Idempotent.
    assert!(!session.detach_screen());

    let _ = session.send_control(rust_expect::ControlChar::CtrlC).await;
    session.wait_timeout(Duration::from_secs(2)).await.ok();
}

/// `wait_screen_not_contains` returns once the substring disappears from the
/// screen — exercised here by waiting for an in-flight marker to clear after
/// the child exits.
#[tokio::test]
async fn wait_screen_not_contains_clears_when_substring_gone() {
    // Emit the marker, then a clear-screen sequence (ESC [ 2 J) that wipes it.
    // Using octal escapes for portability across /bin/sh implementations.
    let (cmd, args, config) =
        build("printf 'IN-FLIGHT\\n'; sleep 0.2; printf '\\033[2J\\033[H'; sleep 0.5; exit 0");
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config)
        .await
        .unwrap();
    session.attach_screen();

    // First confirm we saw the marker.
    session
        .expect_screen_contains("IN-FLIGHT", Duration::from_secs(2))
        .await
        .expect("marker should appear");

    // Then wait for it to clear.
    session
        .wait_screen_not_contains("IN-FLIGHT", Duration::from_secs(3))
        .await
        .expect("marker should be cleared by ESC [ 2 J");
}

/// `wait_screen_not_contains` times out when the substring sticks around.
#[tokio::test]
async fn wait_screen_not_contains_times_out_when_substring_persists() {
    let (cmd, args, config) = build("printf 'STICKY\\n'; sleep 2; exit 0");
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config)
        .await
        .unwrap();
    session.attach_screen();

    session
        .expect_screen_contains("STICKY", Duration::from_secs(2))
        .await
        .unwrap();
    let r = session
        .wait_screen_not_contains("STICKY", Duration::from_millis(500))
        .await;
    assert!(r.is_err(), "expected timeout, got {r:?}");
}

/// `resize_pty` also resizes the attached screen.
#[tokio::test]
async fn resize_pty_resizes_attached_screen() {
    let (cmd, args, config) = build("sleep 2; exit 0");
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config)
        .await
        .unwrap();
    session.attach_screen();

    let (c0, r0) = {
        let s = session.screen().unwrap().lock().unwrap();
        (s.cols(), s.rows())
    };
    assert_eq!(c0, 80);
    assert_eq!(r0, 24);

    session.resize_pty(132, 50).await.unwrap();

    let (c1, r1) = {
        let s = session.screen().unwrap().lock().unwrap();
        (s.cols(), s.rows())
    };
    assert_eq!(c1, 132, "screen cols should follow resize_pty");
    assert_eq!(r1, 50, "screen rows should follow resize_pty");

    let _ = session.send_control(rust_expect::ControlChar::CtrlC).await;
    session.wait_timeout(Duration::from_secs(2)).await.ok();
}

/// `send_paste` rejects input containing the closing paste marker.
#[tokio::test]
async fn send_paste_rejects_embedded_end_marker() {
    let cmd = "/bin/cat".to_string();
    let config = SessionBuilder::new()
        .command(&cmd)
        .timeout(Duration::from_secs(5))
        .build();
    let mut session = Session::spawn_with_config(&cmd, &[], config).await.unwrap();

    let evil = "ok then \x1b[201~ → DROP OUT ← \x1b[200~";
    let r = session.send_paste(evil).await;
    assert!(r.is_err(), "should reject embedded \\x1b[201~");

    // Clean text still works.
    session.send_paste("hello").await.unwrap();
    let _ = session.send_control(rust_expect::ControlChar::CtrlD).await;
    session.wait_timeout(Duration::from_secs(2)).await.ok();
}

/// Calling a screen-aware expect without attaching a screen returns the
/// dedicated `ScreenNotAttached` variant — not a runtime "pattern not found"
/// miss that would be easy to misinterpret.
#[tokio::test]
async fn screen_aware_methods_return_screen_not_attached() {
    let (cmd, args, config) = build("sleep 1; exit 0");
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config)
        .await
        .unwrap();
    // Intentionally do NOT call attach_screen.

    let r = session
        .expect_screen_contains("anything", Duration::from_millis(100))
        .await;
    assert!(
        matches!(r, Err(ExpectError::ScreenNotAttached)),
        "expect_screen_contains without attach should return ScreenNotAttached, got {r:?}"
    );

    let r = session
        .wait_screen_not_contains("anything", Duration::from_millis(100))
        .await;
    assert!(
        matches!(r, Err(ExpectError::ScreenNotAttached)),
        "wait_screen_not_contains without attach should return ScreenNotAttached, got {r:?}"
    );

    let r = session
        .wait_screen_stable(Duration::from_millis(50), Duration::from_millis(100))
        .await;
    assert!(
        matches!(r, Err(ExpectError::ScreenNotAttached)),
        "wait_screen_stable without attach should return ScreenNotAttached, got {r:?}"
    );

    let _ = session.send_control(rust_expect::ControlChar::CtrlC).await;
    session.wait_timeout(Duration::from_secs(2)).await.ok();
}

/// `Screen::revision()` bumps once per `process()` call so cheap stability
/// polls can detect activity without materializing the full screen text.
#[tokio::test]
async fn screen_revision_advances_on_input() {
    let (cmd, args, config) = build("printf 'a\\n'; sleep 0.3; printf 'b\\n'; exit 0");
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config)
        .await
        .unwrap();
    session.attach_screen();

    // Initial state: no input processed yet.
    let r0 = session.screen().unwrap().lock().unwrap().revision();
    assert_eq!(r0, 0);

    session
        .expect_timeout("a", Duration::from_secs(2))
        .await
        .unwrap();
    let r1 = session.screen().unwrap().lock().unwrap().revision();
    assert!(
        r1 > r0,
        "revision should advance after the first chunk: r0={r0}, r1={r1}"
    );

    session
        .expect_timeout("b", Duration::from_secs(2))
        .await
        .unwrap();
    let r2 = session.screen().unwrap().lock().unwrap().revision();
    assert!(
        r2 > r1,
        "revision should advance again after the second chunk: r1={r1}, r2={r2}"
    );

    session.wait_timeout(Duration::from_secs(2)).await.ok();
}

/// Parser malformed-UTF-8 recovery emits both `U+FFFD` and the recovery
/// byte's own emission in the correct visual order. Without the fix, the
/// recovery byte's effect was dropped.
#[tokio::test]
async fn malformed_utf8_emits_recovery_and_next_byte() {
    // E2 = lead byte of a 3-byte UTF-8 sequence (e.g. start of '╭').
    // Following it with 'X' (not a continuation) should produce U+FFFD
    // then 'X', rendered onto cells 0 and 1 respectively.
    //
    // Use `printf` with octal escapes so /bin/sh emits the raw bytes.
    let (cmd, args, config) = build("printf '\\342X\\n'; exit 0");
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config)
        .await
        .unwrap();
    session.attach_screen();

    session
        .expect_screen_contains("X", Duration::from_secs(3))
        .await
        .unwrap();
    session.wait_timeout(Duration::from_secs(2)).await.ok();

    let row0 = session
        .screen()
        .unwrap()
        .lock()
        .unwrap()
        .buffer()
        .row_text(0);
    let chars: Vec<char> = row0.chars().take(2).collect();
    assert_eq!(
        chars,
        vec![std::char::REPLACEMENT_CHARACTER, 'X'],
        "expected U+FFFD then 'X' in cells 0/1, got row0={row0:?}"
    );
}

/// `send_paste` wraps text in bracketed-paste markers.
#[tokio::test]
async fn send_paste_emits_bracketed_paste_markers() {
    // /bin/cat echoes stdin back unchanged. We send a paste, then look for
    // the wrapping markers in the echoed output.
    let cmd = "/bin/cat".to_string();
    let args: Vec<String> = vec![];
    let config = SessionBuilder::new()
        .command(&cmd)
        .dimensions(80, 24)
        .timeout(Duration::from_secs(5))
        .build();
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config)
        .await
        .unwrap();

    session.send_paste("hello-paste").await.unwrap();
    // Bracketed paste wraps as ESC [ 200 ~ ... ESC [ 201 ~.
    let m = session
        .expect_timeout("\x1b[200~hello-paste\x1b[201~", Duration::from_secs(3))
        .await;
    // PTY echo may rewrite ESC; accept either the exact sequence or the
    // payload framed by '~' markers.
    if m.is_err() {
        let m2 = session
            .expect_timeout("200~hello-paste", Duration::from_secs(1))
            .await;
        assert!(m2.is_ok(), "neither bracketed-paste form matched");
    }

    // Tear down the cat session.
    let _ = session.send_control(rust_expect::ControlChar::CtrlD).await;
    session.wait_timeout(Duration::from_secs(2)).await.ok();
}
