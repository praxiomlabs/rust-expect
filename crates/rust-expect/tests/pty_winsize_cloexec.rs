//! Regression tests for initial PTY window size (W1) and master-fd
//! close-on-exec (L1).
//!
//! Before these fixes `openpty` was called with a null `winp`, so a freshly
//! spawned child saw a 0x0 terminal until an explicit `resize` (a TUI child
//! renders into nothing), and the PTY master fd lacked `FD_CLOEXEC`, so it
//! could leak into unrelated processes spawned concurrently.

#![cfg(unix)]

use std::time::Duration;

use rust_expect::{Session, SessionConfig};

/// A child spawned with custom dimensions sees them at startup via
/// `stty size` (which prints "rows cols"), not 0x0.
#[tokio::test]
async fn spawn_applies_configured_window_size() {
    let config = SessionConfig::new("/bin/sh").dimensions(90, 30);
    let mut session = Session::spawn_with_config("/bin/sh", &["-c", "stty size"], config)
        .await
        .expect("spawn should succeed");

    let m = session
        .expect_timeout("30 90", Duration::from_secs(5))
        .await
        .expect("expected stty size to report the configured 30 rows x 90 cols");
    assert!(m.matched.contains("30 90"), "got: {:?}", m.matched);
}

/// The default 80x24 config also reaches the child (regression guard against
/// the 0x0 window returning).
#[tokio::test]
async fn spawn_applies_default_window_size() {
    let mut session = Session::spawn("/bin/sh", &["-c", "stty size"])
        .await
        .expect("spawn should succeed");

    let m = session
        .expect_timeout("24 80", Duration::from_secs(5))
        .await
        .expect("expected stty size to report the default 24 rows x 80 cols");
    assert!(m.matched.contains("24 80"), "got: {:?}", m.matched);
}
