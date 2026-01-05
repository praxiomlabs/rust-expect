//! Comprehensive error handling example.
//!
//! This example demonstrates proper error handling patterns in rust-expect,
//! including error recovery, pattern matching on errors, and extracting
//! diagnostic information from error types.
//!
//! Run with: `cargo run --example error_handling`

use std::time::Duration;

use rust_expect::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("rust-expect Error Handling Example");
    println!("===================================\n");

    // Example 1: Handling timeout errors
    println!("1. Handling timeout errors...");
    demonstrate_timeout_handling().await?;

    // Example 2: Pattern matching on error types
    println!("\n2. Pattern matching on error types...");
    demonstrate_error_pattern_matching().await?;

    // Example 3: Error recovery with retries
    println!("\n3. Error recovery with retries...");
    demonstrate_retry_logic().await?;

    // Example 4: Extracting diagnostic info from errors
    println!("\n4. Extracting diagnostic information...");
    demonstrate_error_diagnostics().await?;

    // Example 5: Handling spawn errors
    println!("\n5. Handling spawn errors...");
    demonstrate_spawn_error_handling().await;

    // Example 6: Using error context
    println!("\n6. Using error context...");
    demonstrate_error_context().await?;

    // Example 7: Graceful degradation
    println!("\n7. Graceful degradation strategies...");
    demonstrate_graceful_degradation().await?;

    println!("\nError handling examples completed successfully!");
    Ok(())
}

/// Demonstrate timeout error handling with proper recovery
async fn demonstrate_timeout_handling() -> Result<()> {
    let mut session = Session::spawn("/bin/sh", &[]).await?;
    session
        .expect_timeout(Pattern::shell_prompt(), Duration::from_secs(2))
        .await?;

    // Intentionally trigger a timeout
    session.send_line("echo 'output'").await?;

    match session
        .expect_timeout("nonexistent pattern", Duration::from_millis(500))
        .await
    {
        Ok(_) => println!("   Pattern found (unexpected)"),
        Err(ExpectError::Timeout {
            duration, buffer, ..
        }) => {
            println!("   Timeout after {duration:?} as expected");
            println!("   Buffer had {} bytes of data", buffer.len());
            println!("   Recovery: Will wait for actual prompt instead");

            // Recovery: clear buffer and wait for actual prompt
            session.clear_buffer();
            session
                .expect_timeout(Pattern::shell_prompt(), Duration::from_secs(2))
                .await?;
            println!("   Recovered successfully");
        }
        Err(e) => println!("   Unexpected error: {e}"),
    }

    session.send_line("exit").await?;
    let _ = session.wait().await;
    Ok(())
}

/// Demonstrate pattern matching on different error types
async fn demonstrate_error_pattern_matching() -> Result<()> {
    let errors: Vec<ExpectError> = vec![
        ExpectError::timeout(Duration::from_secs(5), "password:", "Enter username:"),
        ExpectError::pattern_not_found("expected_output", "actual output here"),
        ExpectError::eof("last output before EOF"),
        ExpectError::SessionClosed,
        ExpectError::invalid_pattern("regex error in [pattern"),
    ];

    for err in errors {
        let recovery = match &err {
            ExpectError::Timeout {
                duration, pattern, ..
            } => {
                format!(
                    "Retry with longer timeout (was {duration:?} for pattern '{pattern}')"
                )
            }
            ExpectError::PatternNotFound { pattern, .. } => {
                format!("Verify pattern '{pattern}' is correct, or wait longer")
            }
            ExpectError::Eof { .. } => "Process terminated - restart session".to_string(),
            ExpectError::SessionClosed => "Create new session".to_string(),
            ExpectError::InvalidPattern { message } => {
                format!("Fix pattern syntax: {message}")
            }
            ExpectError::ProcessExited { exit_status, .. } => {
                format!("Process exited with {exit_status:?} - check command")
            }
            ExpectError::Io(io_err) => {
                format!("I/O error ({}): check system resources", io_err.kind())
            }
            _ => "Investigate error".to_string(),
        };
        println!("   {:20} -> {}", error_type_name(&err), recovery);
    }

    Ok(())
}

/// Get a short name for an error type
fn error_type_name(err: &ExpectError) -> &'static str {
    match err {
        ExpectError::Timeout { .. } => "Timeout",
        ExpectError::PatternNotFound { .. } => "PatternNotFound",
        ExpectError::Eof { .. } => "Eof",
        ExpectError::SessionClosed => "SessionClosed",
        ExpectError::InvalidPattern { .. } => "InvalidPattern",
        ExpectError::ProcessExited { .. } => "ProcessExited",
        ExpectError::Io(_) => "Io",
        ExpectError::Spawn(_) => "Spawn",
        ExpectError::Regex(_) => "Regex",
        _ => "Other",
    }
}

/// Demonstrate retry logic with exponential backoff
async fn demonstrate_retry_logic() -> Result<()> {
    let mut session = Session::spawn("/bin/sh", &[]).await?;
    session
        .expect_timeout(Pattern::shell_prompt(), Duration::from_secs(2))
        .await?;

    // Simulate a command that might fail initially
    session
        .send_line("echo 'ready after delay'")
        .await?;

    // Retry with exponential backoff - inline for clarity
    let max_attempts = 3u32;
    let base_delay = Duration::from_millis(100);
    let mut last_error = None;

    for attempt in 1..=max_attempts {
        match session
            .expect_timeout("ready", Duration::from_millis(500))
            .await
        {
            Ok(m) => {
                if attempt > 1 {
                    println!("   Succeeded on attempt {attempt}");
                }
                println!("   Success: '{}'", m.matched.trim());
                last_error = None;
                break;
            }
            Err(e) if attempt < max_attempts && e.is_timeout() => {
                let delay = base_delay * 2u32.pow(attempt - 1);
                println!("   Attempt {attempt} failed, retrying in {delay:?}...");
                tokio::time::sleep(delay).await;
                last_error = Some(e);
            }
            Err(e) => {
                last_error = Some(e);
                break;
            }
        }
    }

    if let Some(e) = last_error {
        println!("   All retries failed: {}", error_type_name(&e));
    }

    session.send_line("exit").await?;
    let _ = session.wait().await;
    Ok(())
}

/// Demonstrate extracting diagnostic information from errors
async fn demonstrate_error_diagnostics() -> Result<()> {
    let mut session = Session::spawn("/bin/sh", &[]).await?;
    session
        .expect_timeout(Pattern::shell_prompt(), Duration::from_secs(2))
        .await?;

    // Generate some output
    session
        .send_line("echo 'line1'; echo 'line2'; echo 'line3'")
        .await?;

    // Create a timeout to demonstrate diagnostics
    let err = session
        .expect_timeout("will_not_match", Duration::from_millis(200))
        .await
        .unwrap_err();

    // Extract diagnostic information
    println!("   Error type: {:?}", error_type_name(&err));
    println!("   Is timeout: {}", err.is_timeout());
    println!("   Is EOF: {}", err.is_eof());

    if let Some(buffer) = err.buffer() {
        let lines: Vec<&str> = buffer.lines().collect();
        println!("   Buffer lines: {}", lines.len());
        println!("   Last line: {:?}", lines.last().unwrap_or(&"(empty)"));
    }

    // The formatted error message includes helpful tips
    let msg = err.to_string();
    println!("   Error includes tips: {}", msg.contains("Tip:"));

    session.send_line("exit").await?;
    let _ = session.wait().await;
    Ok(())
}

/// Demonstrate handling spawn errors
async fn demonstrate_spawn_error_handling() {
    // Try to spawn a nonexistent command
    let result = Session::spawn("/nonexistent/command", &[]).await;

    match result {
        Ok(_) => println!("   Session created (unexpected)"),
        Err(ExpectError::Spawn(spawn_err)) => {
            println!("   Spawn error: {spawn_err}");

            // Pattern match on specific spawn errors
            match &spawn_err {
                SpawnError::CommandNotFound { command } => {
                    println!("   Recovery: Check if '{command}' is installed");
                }
                SpawnError::PermissionDenied { path } => {
                    println!("   Recovery: Check permissions for '{path}'");
                }
                SpawnError::PtyAllocation { reason } => {
                    println!("   Recovery: PTY issue - {reason}");
                }
                SpawnError::Io(io_err) => {
                    println!("   Recovery: I/O error - {}", io_err.kind());
                }
                _ => println!("   Recovery: Investigate spawn failure"),
            }
        }
        Err(e) => println!("   Other error: {e}"),
    }
}

/// Demonstrate using error context for better diagnostics
async fn demonstrate_error_context() -> Result<()> {
    // Example: wrapping I/O errors with context
    let io_result: std::io::Result<()> = Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "configuration file missing",
    ));

    match ExpectError::with_io_context(io_result, "loading session config") {
        Ok(()) => println!("   Config loaded"),
        Err(ExpectError::IoWithContext { context, source }) => {
            println!("   Context: {context}");
            println!("   Cause: {source}");
            println!("   Error kind: {:?}", source.kind());
        }
        Err(e) => println!("   Other error: {e}"),
    }

    // Using io_context directly
    let contextualized = ExpectError::io_context(
        "writing transcript",
        std::io::Error::new(std::io::ErrorKind::PermissionDenied, "read-only filesystem"),
    );
    println!("   Contextualized error: {contextualized}");

    Ok(())
}

/// Demonstrate graceful degradation when errors occur
async fn demonstrate_graceful_degradation() -> Result<()> {
    // Try primary approach, fall back to alternatives
    let result = try_with_fallback(
        "Primary shell",
        Session::spawn("/bin/bash", &[]),
        |_| async {
            // Secondary: try sh
            try_with_fallback(
                "Fallback shell",
                Session::spawn("/bin/sh", &[]),
                |_| async {
                    // Tertiary: error out
                    Err(ExpectError::config("No shell available"))
                },
            )
            .await
        },
    )
    .await;

    match result {
        Ok(mut session) => {
            println!("   Session established");
            session.send_line("exit").await?;
            let _ = session.wait().await;
        }
        Err(e) => {
            println!("   All fallbacks failed: {e}");
        }
    }

    // Pattern: optional features with graceful fallback
    println!("\n   Pattern: Optional feature detection");
    let mut session = Session::spawn("/bin/sh", &[]).await?;
    session
        .expect_timeout(Pattern::shell_prompt(), Duration::from_secs(2))
        .await?;

    // Try to use an optional feature
    session.send_line("which jq 2>/dev/null || echo 'NOT_FOUND'").await?;
    let m = session
        .expect_timeout(Pattern::shell_prompt(), Duration::from_secs(2))
        .await?;

    if m.before.contains("NOT_FOUND") {
        println!("   jq not found - using fallback JSON parsing");
    } else {
        println!("   jq available - using native JSON parsing");
    }

    session.send_line("exit").await?;
    let _ = session.wait().await;

    Ok(())
}

/// Helper: try an operation with a fallback
async fn try_with_fallback<T, F, Fut>(
    name: &str,
    primary: impl std::future::Future<Output = Result<T>>,
    fallback: F,
) -> Result<T>
where
    F: FnOnce(ExpectError) -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    match primary.await {
        Ok(value) => {
            println!("   {name}: success");
            Ok(value)
        }
        Err(e) => {
            println!("   {name}: failed ({e}), trying fallback...");
            fallback(e).await
        }
    }
}
