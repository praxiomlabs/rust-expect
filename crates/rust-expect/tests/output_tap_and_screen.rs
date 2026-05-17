//! Integration tests for the additive output-tap and screen-integration
//! primitives added to `Session`: `add_output_tap`, `attach_screen`,
//! `expect_screen_contains`, `wait_screen_stable`, and `send_paste`.
//!
//! These tests spawn `/bin/sh` with small fixed scripts and verify the
//! primitives observe and assert on the resulting byte stream / screen.

#![cfg(all(unix, feature = "screen"))]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use rust_expect::{Session, SessionBuilder};

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
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config).await.unwrap();

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

    let bytes = captured.lock().unwrap();
    assert!(
        std::str::from_utf8(&bytes)
            .map(|s| s.contains("tap-saw-this"))
            .unwrap_or(false),
        "tap should have captured the literal output; got {bytes:?}"
    );
}

/// Multiple taps fire in registration order on every chunk.
#[tokio::test]
async fn multiple_taps_each_receive_chunks() {
    let (cmd, args, config) = build("printf 'fan-out\\n'; exit 0");
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config).await.unwrap();

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
    assert!(counts.iter().all(|&n| n >= 1), "every tap should have fired; got {counts:?}");
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
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config).await.unwrap();

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
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config).await.unwrap();

    session.attach_screen();
    session
        .expect_timeout("rocket", Duration::from_secs(3))
        .await
        .unwrap();
    session.wait_timeout(Duration::from_secs(2)).await.ok();

    let text = session.screen().unwrap().lock().unwrap().text();
    assert!(text.contains("╭─╮"), "box-drawing should round-trip; got {text:?}");
    assert!(text.contains('🚀'), "emoji should round-trip; got {text:?}");
}

/// `expect_screen_contains` returns as soon as the substring appears.
#[tokio::test]
async fn expect_screen_contains_succeeds_on_match() {
    let (cmd, args, config) = build("printf 'go-no-go\\n'; sleep 0.5; exit 0");
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config).await.unwrap();
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
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config).await.unwrap();
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
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config).await.unwrap();
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
    let mut session = Session::spawn_with_config(&cmd, &arg_refs, config).await.unwrap();

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
