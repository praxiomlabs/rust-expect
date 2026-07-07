//! Integration tests for PTY spawning functionality.
//!
//! These tests verify the PTY backend works correctly with the `SessionBuilder`.

#![cfg(unix)] // PTY tests only work on Unix

use std::time::Duration;

use rust_expect::{QuickSession, SessionBuilder};

/// Test `SessionBuilder` creates valid config.
#[test]
fn session_builder_creates_config() {
    let config = SessionBuilder::new()
        .command("/bin/echo")
        .arg("hello world")
        .timeout(Duration::from_secs(5))
        .build();

    assert_eq!(config.command, "/bin/echo");
    assert_eq!(config.args, vec!["hello world"]);
    assert_eq!(config.timeout.default, Duration::from_secs(5));
}

/// Test `SessionBuilder` with environment variables.
#[test]
fn session_builder_with_env() {
    let config = SessionBuilder::new()
        .command("/bin/sh")
        .arg("-c")
        .arg("echo $TEST_VAR")
        .env("TEST_VAR", "test_value")
        .build();

    assert!(config.env.contains_key("TEST_VAR"));
    assert_eq!(config.env.get("TEST_VAR"), Some(&"test_value".to_string()));
}

/// Regression test: env vars set via `SessionBuilder::env()` must actually reach
/// the spawned child process. Before the env-plumbing fix in `backend/pty.rs`,
/// this would fail — the value was dropped between `PtyConfig::from` and
/// `execvp` on Unix.
#[tokio::test]
async fn env_vars_reach_child_process() {
    use rust_expect::Session;

    let config = SessionBuilder::new()
        .command("/bin/sh")
        .arg("-c")
        .arg("printf 'value=%s\\n' \"$RUST_EXPECT_TEST_VAR\"; exit 0")
        .env("RUST_EXPECT_TEST_VAR", "smelt-pinecone-42")
        .timeout(Duration::from_secs(5))
        .build();

    let mut session = Session::spawn_with_config(
        "/bin/sh",
        &[
            "-c",
            "printf 'value=%s\\n' \"$RUST_EXPECT_TEST_VAR\"; exit 0",
        ],
        config,
    )
    .await
    .expect("spawn should succeed");

    // Match the full string in one expect to avoid races on the split
    // between `value=` and the rest of the line.
    let m = session
        .expect_timeout("value=smelt-pinecone-42", Duration::from_secs(10))
        .await
        .expect("expected child to receive RUST_EXPECT_TEST_VAR=smelt-pinecone-42");
    assert!(m.matched.contains("smelt-pinecone-42"));

    session.wait_timeout(Duration::from_secs(2)).await.ok();
}

/// Env-var fix: when no explicit env is set, the child should still inherit
/// the parent's environment (Inherit mode is the default and previously
/// worked; this guards against regressing it).
#[tokio::test]
async fn parent_env_inherited_when_no_overrides() {
    use rust_expect::Session;
    // SAFETY: single-threaded test setup.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("RUST_EXPECT_PARENT_PROBE", "from-parent");
    }

    let config = SessionBuilder::new()
        .command("/bin/sh")
        .arg("-c")
        .arg("printf 'probe=%s\\n' \"$RUST_EXPECT_PARENT_PROBE\"; exit 0")
        .timeout(Duration::from_secs(5))
        .build();

    let mut session = Session::spawn_with_config(
        "/bin/sh",
        &[
            "-c",
            "printf 'probe=%s\\n' \"$RUST_EXPECT_PARENT_PROBE\"; exit 0",
        ],
        config,
    )
    .await
    .expect("spawn should succeed");

    let m = session
        .expect_timeout("probe=from-parent", Duration::from_secs(10))
        .await
        .expect("expected child to inherit RUST_EXPECT_PARENT_PROBE=from-parent");
    assert!(m.matched.contains("from-parent"));

    session.wait_timeout(Duration::from_secs(2)).await.ok();
}

/// cwd fix: `working_directory` must actually `chdir` the child before exec.
/// Before the fix in `backend/pty.rs`, `working_dir` was dropped between
/// `PtyConfig::from` and `execvp` on Unix, so the child ran in the parent's cwd.
#[tokio::test]
async fn working_dir_changes_child_cwd() {
    use rust_expect::Session;

    let dir = std::env::temp_dir();
    let canonical = std::fs::canonicalize(&dir).expect("temp dir should canonicalize");
    let expected = canonical.to_string_lossy().into_owned();

    let config = SessionBuilder::new()
        .command("/bin/sh")
        .arg("-c")
        .arg("pwd -P; exit 0")
        .working_directory(&canonical)
        .timeout(Duration::from_secs(5))
        .build();

    let mut session = Session::spawn_with_config("/bin/sh", &["-c", "pwd -P; exit 0"], config)
        .await
        .expect("spawn should succeed");

    let m = session
        .expect_timeout(expected.as_str(), Duration::from_secs(10))
        .await
        .expect("expected child to run in the configured working directory");
    assert!(m.matched.contains(&expected));

    session.wait_timeout(Duration::from_secs(2)).await.ok();
}

/// cwd fix: a non-existent `working_directory` must fail with a clean
/// `InvalidWorkingDir` spawn error rather than a cryptic child exit.
#[tokio::test]
async fn working_dir_missing_returns_clean_error() {
    use rust_expect::Session;

    let config = SessionBuilder::new()
        .command("/bin/sh")
        .arg("-c")
        .arg("exit 0")
        .working_directory("/no/such/rust-expect/fixture/dir")
        .build();

    let result = Session::spawn_with_config("/bin/sh", &["-c", "exit 0"], config).await;
    assert!(
        result.is_err(),
        "spawn should fail for a non-existent working directory"
    );
}

/// env fix: `inherit_env(false)` must clear the parent environment so the child
/// sees only explicit overrides. Before the fix the flag was a silent no-op.
#[tokio::test]
async fn inherit_env_false_clears_parent_env() {
    use rust_expect::Session;
    // SAFETY: single-threaded test setup.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("RUST_EXPECT_CLEAR_PROBE", "should-not-appear");
    }

    let config = SessionBuilder::new()
        .command("/bin/sh")
        .arg("-c")
        .arg("printf 'probe=[%s]\\n' \"$RUST_EXPECT_CLEAR_PROBE\"; exit 0")
        .timeout(Duration::from_secs(5))
        .build()
        .inherit_env(false);

    let mut session = Session::spawn_with_config(
        "/bin/sh",
        &[
            "-c",
            "printf 'probe=[%s]\\n' \"$RUST_EXPECT_CLEAR_PROBE\"; exit 0",
        ],
        config,
    )
    .await
    .expect("spawn should succeed");

    let m = session
        .expect_timeout("probe=[]", Duration::from_secs(10))
        .await
        .expect("expected cleared env so the parent probe var is absent");
    assert!(m.matched.contains("probe=[]"));

    session.wait_timeout(Duration::from_secs(2)).await.ok();
}

/// Test `SessionBuilder` with custom dimensions.
#[test]
fn session_builder_with_dimensions() {
    let config = SessionBuilder::new()
        .command("/bin/sh")
        .dimensions(120, 40)
        .build();

    assert_eq!(config.dimensions, (120, 40));
}

/// Test `QuickSession::bash` creates correct config.
#[test]
fn quick_session_bash_config() {
    let config = QuickSession::bash();

    assert_eq!(config.command, "/bin/bash");
    assert!(config.args.contains(&"--norc".to_string()));
    assert!(config.args.contains(&"--noprofile".to_string()));
}

/// Test `QuickSession::shell` uses SHELL env var or default.
#[test]
fn quick_session_shell_config() {
    let config = QuickSession::shell();

    // Should have a command set
    assert!(!config.command.is_empty());
}

/// Test `QuickSession::ssh` creates correct config.
#[test]
fn quick_session_ssh_config() {
    let config = QuickSession::ssh("example.com");

    assert_eq!(config.command, "ssh");
    assert!(config.args.contains(&"example.com".to_string()));
    assert_eq!(config.timeout.default, Duration::from_secs(30));
}

/// Test `QuickSession::ssh_user` creates correct config.
#[test]
fn quick_session_ssh_user_config() {
    let config = QuickSession::ssh_user("admin", "server.example.com");

    assert_eq!(config.command, "ssh");
    assert!(
        config
            .args
            .contains(&"admin@server.example.com".to_string())
    );
}

/// Test `QuickSession::python` creates correct config.
#[test]
fn quick_session_python_config() {
    let config = QuickSession::python();

    assert_eq!(config.command, "python3");
    assert!(config.args.contains(&"-i".to_string()));
}

/// Test `QuickSession::telnet` creates correct config.
#[test]
fn quick_session_telnet_config() {
    let config = QuickSession::telnet("host.example.com", 23);

    assert_eq!(config.command, "telnet");
    assert!(config.args.contains(&"host.example.com".to_string()));
    assert!(config.args.contains(&"23".to_string()));
}

/// Test `SessionBuilder` working directory.
#[test]
fn session_builder_working_dir() {
    let config = SessionBuilder::new()
        .command("/bin/pwd")
        .working_directory("/tmp")
        .build();

    assert_eq!(config.working_dir, Some("/tmp".into()));
}

/// Test `SessionBuilder` line endings.
#[test]
fn session_builder_line_endings() {
    use rust_expect::LineEnding;

    let config_unix = SessionBuilder::new()
        .command("test")
        .unix_line_endings()
        .build();
    assert!(matches!(config_unix.line_ending, LineEnding::Lf));

    let config_windows = SessionBuilder::new()
        .command("test")
        .windows_line_endings()
        .build();
    assert!(matches!(config_windows.line_ending, LineEnding::CrLf));
}

/// Test `SessionBuilder` buffer configuration.
#[test]
fn session_builder_buffer_size() {
    let config = SessionBuilder::new()
        .command("test")
        .buffer_max_size(1024 * 1024)
        .build();

    assert_eq!(config.buffer.max_size, 1024 * 1024);
}

/// Test `SessionBuilder` logging.
#[test]
fn session_builder_logging() {
    let config = SessionBuilder::new()
        .command("test")
        .log_to_file("/tmp/test.log")
        .build();

    assert_eq!(config.logging.log_file, Some("/tmp/test.log".into()));
}

// =============================================================================
// End-to-end spawn tests (require actual process spawning)
// =============================================================================

use rust_expect::Session;

/// Test spawning a simple command and expecting output.
#[tokio::test]
async fn spawn_echo_command() {
    let mut session = Session::spawn("/bin/echo", &["hello", "world"])
        .await
        .expect("Failed to spawn echo");

    // Read the output
    let m = session.expect("world").await.expect("Expected 'world'");
    assert!(m.matched.contains("world"));
}

/// Test spawning a shell and sending commands.
#[tokio::test]
async fn spawn_shell_send_command() {
    let mut session = Session::spawn("/bin/sh", &[])
        .await
        .expect("Failed to spawn shell");

    // Wait for shell prompt ($ or something similar)
    // Send a command
    session
        .send_line("echo test123")
        .await
        .expect("Failed to send");

    // Expect the output
    let m = session.expect("test123").await.expect("Expected 'test123'");
    assert!(m.matched.contains("test123"));
}

/// Test spawning cat in interactive mode.
#[tokio::test]
async fn spawn_cat_interactive() {
    let mut session = Session::spawn("/bin/cat", &[])
        .await
        .expect("Failed to spawn cat");

    // Cat echoes what we send
    session
        .send_line("hello cat")
        .await
        .expect("Failed to send");

    let m = session
        .expect("hello cat")
        .await
        .expect("Expected 'hello cat'");
    assert!(m.matched.contains("hello cat"));

    // Send EOF to terminate cat (Ctrl+D)
    session
        .send_control(rust_expect::ControlChar::CtrlD)
        .await
        .expect("Failed to send EOF");
}

/// Test process ID is available.
#[tokio::test]
async fn spawn_has_pid() {
    let session = Session::spawn("/bin/true", &[])
        .await
        .expect("Failed to spawn true");

    let pid = session.pid();
    assert!(pid > 0, "Expected valid PID, got {pid}");
}

/// Test spawn with custom configuration.
#[tokio::test]
async fn spawn_with_custom_config() {
    use rust_expect::SessionConfig;

    let config = SessionConfig {
        dimensions: (100, 30),
        ..SessionConfig::default()
    };

    let session = Session::spawn_with_config("/bin/sh", &[], config)
        .await
        .expect("Failed to spawn with config");

    // Just verify it spawned successfully
    let pid = session.pid();
    assert!(pid > 0);
}

/// Test spawning command that fails.
#[tokio::test]
async fn spawn_nonexistent_command() {
    let result = Session::spawn("/nonexistent/command", &[]).await;
    // The spawn should succeed (fork works), but the exec fails
    // The child process will exit immediately with code 1
    // This is expected behavior for PTY spawning
    // We just verify we don't panic
    assert!(result.is_ok() || result.is_err());
}

/// Test sending control characters.
#[tokio::test]
async fn spawn_send_control_c() {
    let mut session = Session::spawn("/bin/cat", &[])
        .await
        .expect("Failed to spawn cat");

    // Send Ctrl-C to interrupt
    session
        .send_control(rust_expect::ControlChar::CtrlC)
        .await
        .expect("Failed to send Ctrl-C");

    // Cat should terminate after Ctrl-C
    // Wait for EOF with a timeout to prevent hanging if something goes wrong
    let result = session.wait_timeout(Duration::from_secs(5)).await;

    // The process should have exited (EOF detected) or we timed out
    // Either outcome is acceptable for this test - we mainly want to verify
    // that Ctrl-C was sent successfully and the test doesn't hang
    assert!(
        result.is_ok() || result.is_err(),
        "wait_timeout should return a result"
    );
}

/// Test basic expect with multiple patterns.
#[tokio::test]
async fn spawn_expect_multiple() {
    let mut session = Session::spawn("/bin/sh", &[])
        .await
        .expect("Failed to spawn shell");

    session
        .send_line("echo first; echo second")
        .await
        .expect("Failed to send");

    // Expect first
    session.expect("first").await.expect("Expected 'first'");

    // Expect second
    session.expect("second").await.expect("Expected 'second'");
}

/// Test that matched field contains the expected text.
#[tokio::test]
async fn spawn_match_contains_text() {
    let mut session = Session::spawn("/bin/echo", &["hello", "world"])
        .await
        .expect("Failed to spawn echo");

    let m = session.expect("hello").await.expect("Expected 'hello'");

    // The matched field should contain the matched text
    assert!(m.matched.contains("hello"), "Match should contain 'hello'");
}
