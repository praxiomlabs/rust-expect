//! Error handling tests.
//!
//! Tests for the error types, error creation helpers, and error introspection.

use std::io;
use std::time::Duration;

use rust_expect::error::{ExpectError, SpawnError};

// =============================================================================
// ExpectError creation and introspection
// =============================================================================

#[test]
fn timeout_error_creation() {
    let err = ExpectError::timeout(Duration::from_secs(5), "pattern", "buffer contents");

    assert!(err.is_timeout());
    assert!(!err.is_eof());

    if let ExpectError::Timeout {
        duration,
        pattern,
        buffer,
    } = err
    {
        assert_eq!(duration, Duration::from_secs(5));
        assert_eq!(pattern, "pattern");
        assert_eq!(buffer, "buffer contents");
    } else {
        panic!("Expected Timeout variant");
    }
}

#[test]
fn eof_error_creation() {
    let err = ExpectError::eof("last buffer contents");

    assert!(err.is_eof());
    assert!(!err.is_timeout());

    if let ExpectError::Eof { buffer } = err {
        assert_eq!(buffer, "last buffer contents");
    } else {
        panic!("Expected Eof variant");
    }
}

#[test]
fn pattern_not_found_error_creation() {
    let err = ExpectError::pattern_not_found("expected pattern", "actual buffer");

    if let ExpectError::PatternNotFound { pattern, buffer } = err {
        assert_eq!(pattern, "expected pattern");
        assert_eq!(buffer, "actual buffer");
    } else {
        panic!("Expected PatternNotFound variant");
    }
}

#[test]
fn invalid_pattern_error_creation() {
    let err = ExpectError::invalid_pattern("unclosed bracket");

    if let ExpectError::InvalidPattern { message } = err {
        assert_eq!(message, "unclosed bracket");
    } else {
        panic!("Expected InvalidPattern variant");
    }
}

#[test]
fn session_closed_error() {
    let err = ExpectError::SessionClosed;
    assert!(!err.is_timeout());
    assert!(!err.is_eof());
}

#[test]
fn config_error_creation() {
    let err = ExpectError::config("invalid configuration value");
    assert!(err.to_string().contains("invalid configuration value"));
}

// =============================================================================
// Error buffer extraction
// =============================================================================

#[test]
fn buffer_extraction_from_timeout() {
    let err = ExpectError::timeout(Duration::from_secs(1), "pat", "my buffer");
    assert_eq!(err.buffer(), Some("my buffer"));
}

#[test]
fn buffer_extraction_from_eof() {
    let err = ExpectError::eof("eof buffer");
    assert_eq!(err.buffer(), Some("eof buffer"));
}

#[test]
fn buffer_extraction_from_pattern_not_found() {
    let err = ExpectError::pattern_not_found("pat", "search buffer");
    assert_eq!(err.buffer(), Some("search buffer"));
}

#[test]
fn buffer_extraction_from_session_closed() {
    let err = ExpectError::SessionClosed;
    assert_eq!(err.buffer(), None);
}

// =============================================================================
// Error Display formatting
// =============================================================================

#[test]
fn timeout_error_display_includes_helpful_info() {
    let err = ExpectError::timeout(Duration::from_secs(10), "expected$", "actual output");
    let msg = err.to_string();

    // Should mention it's a timeout
    assert!(msg.to_lowercase().contains("timeout"));
    // Should include the duration
    assert!(msg.contains("10"));
}

#[test]
fn eof_error_display_mentions_eof() {
    let err = ExpectError::eof("buffer");
    let msg = err.to_string();
    assert!(msg.to_lowercase().contains("eof") || msg.to_lowercase().contains("end"));
}

#[test]
fn pattern_not_found_display_shows_pattern() {
    let err = ExpectError::pattern_not_found("my_pattern", "buffer");
    let msg = err.to_string();
    assert!(msg.contains("my_pattern") || msg.to_lowercase().contains("pattern"));
}

// =============================================================================
// SpawnError tests
// =============================================================================

#[test]
fn spawn_error_command_not_found() {
    let err = SpawnError::CommandNotFound {
        command: "/nonexistent/binary".to_string(),
    };

    let msg = err.to_string();
    assert!(msg.contains("/nonexistent/binary") || msg.to_lowercase().contains("not found"));
}

#[test]
fn spawn_error_permission_denied() {
    let err = SpawnError::PermissionDenied {
        path: "/root/secret".to_string(),
    };

    let msg = err.to_string();
    assert!(
        msg.contains("/root/secret")
            || msg.to_lowercase().contains("permission")
            || msg.to_lowercase().contains("denied")
    );
}

#[test]
fn spawn_error_pty_allocation() {
    let err = SpawnError::PtyAllocation {
        reason: "no PTY available".to_string(),
    };

    let msg = err.to_string();
    assert!(msg.to_lowercase().contains("pty") || msg.contains("no PTY available"));
}

#[test]
fn spawn_error_io_wrapper() {
    let io_err = io::Error::other("custom error");
    let err = SpawnError::Io(io_err);

    let msg = err.to_string();
    assert!(msg.contains("custom error") || msg.to_lowercase().contains("error"));
}

// =============================================================================
// Error conversion tests
// =============================================================================

#[test]
fn spawn_error_converts_to_expect_error() {
    let spawn_err = SpawnError::CommandNotFound {
        command: "test".to_string(),
    };
    let expect_err: ExpectError = spawn_err.into();

    if let ExpectError::Spawn(_) = expect_err {
        // Success - it wrapped correctly
    } else {
        panic!("Expected Spawn variant");
    }
}

#[test]
fn io_error_converts_to_expect_error() {
    let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
    let expect_err: ExpectError = io_err.into();

    if let ExpectError::Io(_) = expect_err {
        // Success
    } else {
        panic!("Expected Io variant");
    }
}

// =============================================================================
// Error context tests
// =============================================================================

#[test]
fn io_context_wraps_error_with_context() {
    let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
    let err = ExpectError::io_context("reading config file", io_err);

    if let ExpectError::IoWithContext { context, .. } = err {
        assert_eq!(context, "reading config file");
    } else {
        panic!("Expected IoWithContext variant");
    }
}

#[test]
fn with_io_context_helper() {
    let result: io::Result<()> = Err(io::Error::new(io::ErrorKind::NotFound, "missing"));
    let wrapped = ExpectError::with_io_context(result, "loading settings");

    assert!(wrapped.is_err());
    if let Err(ExpectError::IoWithContext { context, .. }) = wrapped {
        assert_eq!(context, "loading settings");
    }
}

#[test]
fn with_io_context_passes_ok() {
    let result: io::Result<i32> = Ok(42);
    let wrapped = ExpectError::with_io_context(result, "should not matter");

    assert!(wrapped.is_ok());
    assert_eq!(wrapped.unwrap(), 42);
}

// =============================================================================
// Cache error tests (via Pattern::regex)
// =============================================================================

#[test]
fn regex_cache_returns_error_for_invalid() {
    use rust_expect::get_regex;

    let result = get_regex(r"[unclosed");
    assert!(result.is_err());
}

#[test]
fn regex_cache_caches_valid_patterns() {
    use rust_expect::get_regex;

    let r1 = get_regex(r"\d+").unwrap();
    let r2 = get_regex(r"\d+").unwrap();

    // Should be the same Arc (cached)
    assert!(std::sync::Arc::ptr_eq(&r1, &r2));
}

// =============================================================================
// Error trait implementations
// =============================================================================

#[test]
fn expect_error_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ExpectError>();
}

#[test]
fn spawn_error_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<SpawnError>();
}

#[test]
fn expect_error_implements_std_error() {
    fn assert_error<T: std::error::Error>() {}
    assert_error::<ExpectError>();
}

#[test]
fn spawn_error_implements_std_error() {
    fn assert_error<T: std::error::Error>() {}
    assert_error::<SpawnError>();
}
