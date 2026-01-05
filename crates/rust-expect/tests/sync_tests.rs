//! Integration tests for synchronous API.

use rust_expect::block_on;

#[test]
fn block_on_simple_future() {
    let result = block_on(async { 42 });
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 42);
}

#[test]
fn block_on_async_operation() {
    let result = block_on(async {
        let x = 1;
        let y = 2;
        format!("sum: {}", x + y)
    });

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "sum: 3");
}

#[test]
fn block_on_nested_async() {
    #[allow(clippy::unused_async)]
    async fn inner() -> i32 {
        100
    }

    let result = block_on(async { inner().await + 1 });
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 101);
}

// =============================================================================
// SyncSession integration tests
// =============================================================================

#[cfg(unix)]
mod sync_session_tests {
    use rust_expect::SyncSession;

    #[test]
    fn sync_session_spawn_echo() {
        let mut session =
            SyncSession::spawn("/bin/echo", &["hello", "sync"]).expect("Failed to spawn echo");

        let m = session.expect("sync").expect("Failed to expect sync");
        assert!(m.matched.contains("sync"));
    }

    #[test]
    fn sync_session_send_and_expect() {
        let mut session = SyncSession::spawn("/bin/cat", &[]).expect("Failed to spawn cat");

        session.send_line("test line").expect("Failed to send");
        let m = session.expect("test line").expect("Failed to expect");
        assert!(m.matched.contains("test line"));
    }

    #[test]
    fn sync_session_has_pid() {
        let session = SyncSession::spawn("/bin/true", &[]).expect("Failed to spawn true");

        assert!(session.pid() > 0);
    }

    #[test]
    fn sync_session_buffer() {
        let mut session =
            SyncSession::spawn("/bin/echo", &["buffer_test"]).expect("Failed to spawn echo");

        // Wait for output
        session.expect("buffer_test").expect("Failed to expect");

        // Buffer operations should work
        session.clear_buffer();
        let buffer = session.buffer();
        assert!(buffer.is_empty() || !buffer.contains("buffer_test"));
    }

    #[test]
    fn sync_session_control_char() {
        use rust_expect::ControlChar;

        let mut session = SyncSession::spawn("/bin/cat", &[]).expect("Failed to spawn cat");

        // Send Ctrl-D (EOF)
        session
            .send_control(ControlChar::CtrlD)
            .expect("Failed to send ctrl-d");
    }
}
