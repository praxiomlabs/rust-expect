# rust-expect User Guide

A comprehensive guide to terminal automation with rust-expect.

---

## Table of Contents

1. [Introduction](#introduction)
2. [Installation](#installation)
3. [Quick Start](#quick-start)
4. [Core Concepts](#core-concepts)
5. [Pattern Matching](#pattern-matching)
6. [Dialog Automation](#dialog-automation)
7. [Multi-Session Management](#multi-session-management)
8. [Screen Buffer](#screen-buffer)
9. [PII Redaction](#pii-redaction)
10. [SSH Sessions](#ssh-sessions)
11. [Transcript Recording](#transcript-recording)
12. [Testing with Mock Sessions](#testing-with-mock-sessions)
13. [Metrics and Observability](#metrics-and-observability)
14. [Error Handling](#error-handling)
15. [Best Practices](#best-practices)
16. [Migration Guide](#migration-guide)

---

## Introduction

rust-expect is a modern, async-first terminal automation library for Rust, inspired by the classic Expect tool. It enables you to:

- Spawn and control terminal processes
- Wait for specific output patterns (literal, regex, glob)
- Send input including keystrokes and control characters
- Automate interactive sessions like SSH logins, CLI tools, and REPLs
- Record and replay terminal sessions
- Manage multiple concurrent sessions

### Why rust-expect?

| Feature | rust-expect | pexpect (Python) | expectrl (Rust) |
|---------|-------------|------------------|-----------------|
| Async-first | Native Tokio | Blocking | Added later |
| Windows support | Full ConPTY | Limited | Limited |
| Type safety | Strong types | Dynamic | Basic |
| PII redaction | Built-in | Manual | Not available |
| SSH resilience | Auto-reconnect | Manual | Manual |
| Screen emulation | VT100 + diff | Basic | Basic |

---

## Installation

Add rust-expect to your `Cargo.toml`:

```toml
[dependencies]
rust-expect = "0.1"
tokio = { version = "1", features = ["full"] }
```

### Feature Flags

Enable optional features based on your needs:

```toml
[dependencies]
rust-expect = { version = "0.1", features = ["ssh", "screen", "pii-redaction"] }
```

| Feature | Description |
|---------|-------------|
| `ssh` | SSH session support via russh |
| `mock` | Mock sessions for testing |
| `screen` | Virtual terminal with VT100 emulation |
| `pii-redaction` | Automatic sensitive data masking |
| `metrics` | Prometheus/OpenTelemetry integration |
| `test-utils` | Testing utilities and fixtures |
| `full` | All features above |

---

## Quick Start

### Basic Example

```rust
use rust_expect::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    // Spawn a shell session
    let mut session = Session::spawn("/bin/sh", &[]).await?;

    // Wait for the prompt
    session.expect_timeout(
        Pattern::regex(r"[$#>]").unwrap(),
        Duration::from_secs(5)
    ).await?;

    // Send a command
    session.send_line("echo 'Hello, rust-expect!'").await?;

    // Wait for the output
    let result = session.expect("Hello, rust-expect!").await?;
    println!("Matched: {}", result.matched);

    // Clean exit
    session.send_line("exit").await?;
    session.wait().await?;

    Ok(())
}
```

### Running a Simple Command

```rust
use rust_expect::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Run a command and capture output
    let mut session = Session::spawn("echo", &["Hello, World!"]).await?;

    let m = session.expect("Hello").await?;
    println!("Found: {}", m.matched);

    Ok(())
}
```

---

## Core Concepts

### Session

A `Session` represents a connection to a terminal process. It provides methods to:

- **spawn**: Create a new process with a pseudo-terminal
- **send**: Write bytes to the terminal
- **send_line**: Write a line (with newline appended)
- **expect**: Wait for a pattern to appear in output
- **read**: Read available output
- **wait**: Wait for the process to exit

```rust
// Create a session
let mut session = Session::spawn("bash", &[]).await?;

// Get session info
println!("PID: {}", session.pid());
println!("Dimensions: {:?}", session.config().dimensions);

// Resize the terminal
session.resize_pty(120, 40).await?;

// Send input
session.send(b"ls\n").await?;
session.send_line("pwd").await?;

// Wait for output
let m = session.expect("home").await?;

// Clean up
session.kill().await?;
// or
session.wait().await?;
```

### Pattern

A `Pattern` defines what to look for in terminal output:

```rust
use rust_expect::expect::Pattern;
use std::time::Duration;

// Literal string match
let pattern = Pattern::literal("login:");

// Regular expression
let pattern = Pattern::regex(r"\d{3}-\d{4}").unwrap();

// Glob pattern
let pattern = Pattern::glob("Error: *");

// EOF pattern (process exited)
let pattern = Pattern::eof();

// Timeout pattern
let pattern = Pattern::timeout(Duration::from_secs(30));
```

### Match Result

When a pattern matches, you get a `Match` containing:

```rust
let m = session.expect("hello").await?;

// What was matched
println!("Matched text: {}", m.matched);

// Text before the match
println!("Before match: {}", m.before);

// Text after the match
println!("After match: {}", m.after);

// For regex patterns, captured groups
if !m.captures.is_empty() {
    println!("Captures: {:?}", m.captures);
}
```

### PatternSet

Use `PatternSet` to wait for one of several patterns:

```rust
use rust_expect::expect::{Pattern, PatternSet};
use std::time::Duration;

let mut patterns = PatternSet::new();
patterns.add(Pattern::literal("success"));
patterns.add(Pattern::literal("failure"));
patterns.add(Pattern::literal("error"));
patterns.add(Pattern::timeout(Duration::from_secs(10)));

let m = session.expect_any(&patterns).await?;

if m.matched.contains("success") {
    println!("Operation succeeded!");
} else if m.matched.contains("failure") || m.matched.contains("error") {
    println!("Operation failed!");
}
```

---

## Pattern Matching

### Literal Patterns

Fast, exact string matching using memchr:

```rust
let pattern = Pattern::literal("login:");

// Matches anywhere in the buffer
assert!(pattern.matches("Please enter login:").is_some());
assert!(pattern.matches("username:").is_none());
```

### Regex Patterns

Full regex support with capture groups:

```rust
// Phone number pattern
let pattern = Pattern::regex(r"\d{3}-\d{4}").unwrap();

// With capture groups
let pattern = Pattern::regex(r"user: (\w+)").unwrap();
if let Some(m) = pattern.matches("user: alice") {
    println!("Username: {}", m.captures.get(1).unwrap());
}

// Case insensitive
let pattern = Pattern::regex(r"(?i)error").unwrap();
```

### Glob Patterns

Shell-style wildcards:

```rust
let pattern = Pattern::glob("*.txt");
assert!(pattern.matches("file.txt").is_some());
assert!(pattern.matches("file.rs").is_none());

let pattern = Pattern::glob("Error: *");
assert!(pattern.matches("Error: file not found").is_some());
```

### Timeout Patterns

Handle cases where expected output never arrives:

```rust
use std::time::Duration;

let mut patterns = PatternSet::new();
patterns.add(Pattern::literal("ready"));
patterns.add(Pattern::timeout(Duration::from_secs(30)));

match session.expect_any(&patterns).await {
    Ok(m) if m.matched.contains("ready") => {
        println!("System is ready");
    }
    Ok(_) => {
        println!("Timed out waiting for ready state");
    }
    Err(e) => {
        println!("Error: {}", e);
    }
}
```

---

## Dialog Automation

Dialogs provide a declarative way to script interactive sessions:

### Basic Dialog

```rust
use rust_expect::dialog::{DialogBuilder, DialogStep};
use rust_expect::prelude::*;

let dialog = DialogBuilder::new()
    .step(DialogStep::expect("Username:").then_send("admin\n"))
    .step(DialogStep::expect("Password:").then_send("secret\n"))
    .step(DialogStep::expect("$ "))
    .build();

let mut session = Session::spawn("login_program", &[]).await?;
session.run_dialog(&dialog).await?;
```

### Dialog with Variables

```rust
let username = std::env::var("SSH_USER").unwrap();
let password = std::env::var("SSH_PASS").unwrap();

let dialog = DialogBuilder::new()
    .var("user", &username)
    .var("pass", &password)
    .step(DialogStep::expect("login:").then_send("${user}\n"))
    .step(DialogStep::expect("password:").then_send("${pass}\n"))
    .build();
```

### Named Steps with Timeouts

```rust
use std::time::Duration;

let dialog = DialogBuilder::named("ssh-login")
    .step(
        DialogStep::new("username")
            .with_expect("login:")
            .with_send("myuser\n")
    )
    .step(
        DialogStep::new("password")
            .with_expect("password:")
            .with_send("mypass\n")
            .timeout(Duration::from_secs(30))
    )
    .step(
        DialogStep::new("prompt")
            .with_expect(r"[$#>]")
    )
    .build();
```

### Using expect_send Shorthand

```rust
let dialog = DialogBuilder::new()
    .expect_send("step1", "First prompt:", "response1\n")
    .expect_send("step2", "Second prompt:", "response2\n")
    .expect_send("done", r"[$#>]", "")
    .build();
```

### Control Characters

```rust
use rust_expect::dialog::ControlChar;

// Available control characters
let ctrl_c = ControlChar::CtrlC;  // 0x03 - Interrupt
let ctrl_d = ControlChar::CtrlD;  // 0x04 - EOF
let ctrl_z = ControlChar::CtrlZ;  // 0x1A - Suspend
let ctrl_m = ControlChar::CtrlM;  // 0x0D - Carriage return

// Send control character
session.send(&[ctrl_c.as_byte()]).await?;
```

---

## Multi-Session Management

### Managing Multiple Sessions

```rust
use rust_expect::multi::{MultiSessionManager, SessionGroup};
use rust_expect::prelude::*;

// Create sessions
let session1 = Session::spawn("/bin/sh", &[]).await?;
let session2 = Session::spawn("/bin/sh", &[]).await?;

// Add to manager with labels
let mut manager = MultiSessionManager::new();
let id1 = manager.add(session1, "web-server");
let id2 = manager.add(session2, "db-server");

println!("Managing {} sessions", manager.len());
```

### Waiting for All Sessions

```rust
// Wait for prompt on ALL sessions
let results = manager.expect_all(Pattern::regex(r"[$#>]").unwrap()).await?;

for result in results {
    println!("Session {} ready", result.session_id);
}
```

### Waiting for Any Session

```rust
// Wait for the FIRST session to match
let first = manager.expect_any("ready").await?;
println!("Session {} responded first", first.session_id);
```

### Sending to All Sessions

```rust
// Broadcast a command to all sessions
manager.send_all(b"uptime\n").await;
```

### Session Groups

```rust
use rust_expect::multi::{GroupBuilder, GroupManager};

// Create groups for different server types
let mut gm = GroupManager::new();

let web_group = gm.create("web");
web_group.add("nginx-1");
web_group.add("nginx-2");

let db_group = gm.create("database");
db_group.add("postgres-primary");
db_group.add("postgres-replica");

println!("Groups: {:?}", gm.names());
println!("Total sessions: {}", gm.total_sessions());
```

---

## Screen Buffer

The screen buffer feature provides VT100 terminal emulation for parsing TUI application output.

**Requires:** `features = ["screen"]`

### Basic Screen Usage

```rust
use rust_expect::screen::{Screen, ScreenQueryExt};

// Create a virtual screen (rows, cols)
let mut screen = Screen::new(24, 80);

// Process terminal output
screen.process_str("Hello, World!\n");
screen.process_str("Second line");

// Get cursor position
let cursor = screen.cursor();
println!("Cursor at ({}, {})", cursor.row, cursor.col);

// Get all text content
let text = screen.text();
println!("{}", text);
```

### ANSI Escape Sequences

```rust
let mut screen = Screen::new(10, 40);

// Cursor movement
screen.process_str("\x1b[1;1H");  // Move to row 1, col 1
screen.process_str("Modified content");

// Colors (parsed but not rendered in text output)
screen.process_str("\x1b[31mRed text\x1b[0m");
screen.process_str("\x1b[1;32mBold green\x1b[0m");

// Clear screen
screen.process_str("\x1b[2J\x1b[H");  // Clear and home
```

### Querying Screen Content

```rust
let query = screen.query();

// Check for text presence
if query.contains("Error") {
    println!("Error detected!");
}

// Get specific regions
let line1 = screen.get_line(0);
let region = screen.get_region(0, 0, 5, 20);
```

### Parsing TUI Output

```rust
let mut screen = Screen::new(24, 80);

// Process output from a TUI application (like htop, vim, etc.)
screen.process_str(&tui_output);

// Now query the "rendered" state
if screen.text().contains("CPU:") {
    println!("Found CPU stats");
}
```

---

## PII Redaction

Automatically detect and redact sensitive information from terminal output.

**Requires:** `features = ["pii-redaction"]`

### Quick Detection and Redaction

```rust
use rust_expect::pii::{contains_pii, redact, redact_asterisks};

// Check if text contains PII
if contains_pii("SSN: 123-45-6789") {
    println!("PII detected!");
}

// Redact with placeholders
let safe = redact("Email: user@example.com");
// Result: "Email: [EMAIL]"

// Redact with asterisks
let masked = redact_asterisks("Card: 4111-1111-1111-1111");
// Result: "Card: ****-****-****-1111"
```

### PII Types Detected

```rust
use rust_expect::pii::PiiType;

// Supported PII types:
// - PiiType::Ssn          - Social Security Numbers
// - PiiType::CreditCard   - Credit card numbers (with Luhn validation)
// - PiiType::Email        - Email addresses
// - PiiType::Phone        - Phone numbers
// - PiiType::ApiKey       - API keys and tokens
```

### Configurable Redactor

```rust
use rust_expect::pii::{PiiRedactor, RedactionStyle};

// Default style (placeholders)
let redactor = PiiRedactor::new();
let result = redactor.redact("Call 555-123-4567");
// Result: "Call [PHONE]"

// Asterisk style
let redactor = PiiRedactor::new().style(RedactionStyle::Asterisks);
let result = redactor.redact("Call 555-123-4567");
// Result: "Call ***-***-****"
```

### Streaming Redaction

For processing terminal output in real-time:

```rust
use rust_expect::pii::{PiiRedactor, StreamingRedactor};

let redactor = PiiRedactor::new();
let mut streaming = StreamingRedactor::new(redactor);

// Process chunks as they arrive
loop {
    let data = session.read().await?;
    let safe = streaming.process(&String::from_utf8_lossy(&data));

    if !safe.is_empty() {
        // Safe to log
        log::info!("{}", safe);
    }
}

// Don't forget to flush at the end
let remaining = streaming.flush();
log::info!("{}", remaining);
```

---

## SSH Sessions

Automate remote server access over SSH.

**Requires:** `features = ["ssh"]`

### SSH Connection

```rust
use rust_expect::ssh::{SshConfig, SshSession, SshAuth};

let config = SshConfig::new()
    .host("server.example.com")
    .port(22)
    .user("admin")
    .auth(SshAuth::PrivateKey {
        path: "/home/user/.ssh/id_ed25519".into(),
        passphrase: None,
    });

let mut session = SshSession::connect(config).await?;

// Use like a regular session
session.expect("$ ").await?;
session.send_line("hostname").await?;
let output = session.expect("$ ").await?;
println!("Hostname: {}", output.before.trim());
```

### Authentication Methods

```rust
// Password authentication
SshAuth::Password("secret".to_string())

// Private key (recommended: Ed25519)
SshAuth::PrivateKey {
    path: PathBuf::from("/home/user/.ssh/id_ed25519"),
    passphrase: Some("keypassword".to_string()),
}

// SSH agent
SshAuth::Agent
```

### Host Key Verification

```rust
use rust_expect::ssh::HostKeyVerification;

// Use known_hosts file (default, recommended)
HostKeyVerification::KnownHosts

// Accept and add new hosts to known_hosts
HostKeyVerification::AcceptAndAdd

// DANGEROUS: Accept all (requires 'insecure-skip-verify' feature)
// Only use for testing!
HostKeyVerification::AcceptAll
```

### Connection Pooling

```rust
use rust_expect::ssh::ConnectionPool;

let pool = ConnectionPool::new()
    .max_connections(10)
    .idle_timeout(Duration::from_secs(300));

// Get a session from the pool
let session = pool.get(&config).await?;

// Session is returned to pool when dropped
```

### Resilient Sessions

```rust
use rust_expect::ssh::{ResilientSession, RetryPolicy};

let policy = RetryPolicy::exponential()
    .max_attempts(5)
    .initial_delay(Duration::from_secs(1))
    .max_delay(Duration::from_secs(30));

let session = ResilientSession::new(config)
    .retry_policy(policy)
    .connect()
    .await?;

// Automatically reconnects on connection loss
```

---

## Transcript Recording

Record terminal sessions for replay, debugging, or documentation.

### Recording a Session

```rust
use rust_expect::transcript::{RecorderBuilder, Transcript};

let recorder = RecorderBuilder::new()
    .title("Deployment Script")
    .command("/bin/bash")
    .size(80, 24)
    .build();

// Record events as they happen
recorder.record_output(b"$ ");
recorder.record_input(b"./deploy.sh\n");
recorder.record_output(b"Deploying...\n");
recorder.record_output(b"Done!\n$ ");

// Get the transcript
let transcript = recorder.into_transcript();
println!("Recorded {} events", transcript.events.len());
println!("Duration: {:?}", transcript.duration());
```

### Playback

```rust
use rust_expect::transcript::{Player, PlaybackOptions, PlaybackSpeed};

let options = PlaybackOptions::new()
    .with_speed(PlaybackSpeed::Speed(2.0));  // 2x speed

let player = Player::new(&transcript);
// player.play_to(&mut stdout)?;
```

### Saving and Loading

```rust
// Save as JSON
let json = serde_json::to_string_pretty(&transcript)?;
std::fs::write("session.json", json)?;

// Save as asciicast v2 (asciinema compatible)
use rust_expect::transcript::asciicast::write_asciicast;
let mut file = File::create("session.cast")?;
write_asciicast(&mut file, &transcript)?;

// Play with asciinema:
// $ asciinema play session.cast
```

---

## Testing with Mock Sessions

Create reproducible tests without spawning real processes.

**Requires:** `features = ["mock"]`

### Basic Mock Session

```rust
use rust_expect::mock::MockSession;

let mut mock = MockSession::new();

// Define expected output
mock.expect_output("login: ");
mock.on_input("admin\n", "password: ");
mock.on_input("secret\n", "Welcome!\n$ ");

// Use like a real session
let mut session = mock.into_session();
session.expect("login:").await?;
session.send_line("admin").await?;
session.expect("password:").await?;
session.send_line("secret").await?;
session.expect("Welcome!").await?;
```

### Mock with Delays

```rust
mock.expect_output_delayed("Connecting...", Duration::from_millis(100));
mock.expect_output_delayed("Connected!", Duration::from_millis(500));
```

### Scripted Mock

```rust
use rust_expect::mock::MockScript;

let script = MockScript::new()
    .output("Username: ")
    .input("user\n")
    .output("Password: ")
    .input("pass\n")
    .output("Login successful\n");

let session = script.into_session();
```

---

## Metrics and Observability

Export metrics for monitoring automation performance.

**Requires:** `features = ["metrics"]`

### Prometheus Metrics

```rust
use rust_expect::metrics::{MetricsExporter, PrometheusExporter};

// Create exporter
let exporter = PrometheusExporter::new();

// Metrics are automatically collected during session operations
let mut session = Session::spawn("bash", &[]).await?;
session.expect("$ ").await?;

// Export metrics
let output = exporter.export();
println!("{}", output);
// # HELP rust_expect_session_spawn_total Total sessions spawned
// # TYPE rust_expect_session_spawn_total counter
// rust_expect_session_spawn_total 1
```

### OpenTelemetry Tracing

```rust
use rust_expect::metrics::OtelExporter;

// Configure OpenTelemetry
let exporter = OtelExporter::new()
    .endpoint("http://localhost:4317")
    .service_name("my-automation");

exporter.init()?;

// Operations are automatically traced
let mut session = Session::spawn("bash", &[]).await?;
// Creates span: rust_expect.session.spawn

session.expect("$ ").await?;
// Creates span: rust_expect.session.expect
```

---

## Error Handling

### Error Types

```rust
use rust_expect::error::{Error, ErrorKind};

match session.expect("pattern").await {
    Ok(m) => println!("Matched: {}", m.matched),
    Err(e) => match e.kind() {
        ErrorKind::Timeout => println!("Timed out"),
        ErrorKind::Eof => println!("Process exited"),
        ErrorKind::PatternNotFound => println!("Pattern not in buffer"),
        ErrorKind::IoError(io_err) => println!("IO error: {}", io_err),
        _ => println!("Error: {}", e),
    }
}
```

### Timeout Handling

```rust
use std::time::Duration;

// With explicit timeout
let result = session.expect_timeout(
    Pattern::literal("ready"),
    Duration::from_secs(30)
).await;

match result {
    Ok(m) => println!("Ready!"),
    Err(e) if e.is_timeout() => println!("Timed out waiting for ready"),
    Err(e) => return Err(e),
}
```

### EOF Handling

```rust
// Wait for process to exit
session.send_line("exit").await?;

match session.expect_eof().await {
    Ok(_) => println!("Process exited cleanly"),
    Err(e) => println!("Error waiting for EOF: {}", e),
}

// Or with timeout
session.expect_eof_timeout(Duration::from_secs(5)).await?;
```

### Error Context

Errors include helpful context:

```rust
// Error message includes:
// - What was expected
// - Buffer contents (snippet)
// - Line count
// - Actionable suggestions

// Example error:
// ExpectError: Pattern 'login:' not found
//   Buffer (42 chars, 3 lines):
//   "Welcome to the system\nPlease wait...\n..."
//   Tip: Check if the expected prompt appears later
```

---

## Best Practices

### 1. Use Timeouts

Always use timeouts to prevent hanging:

```rust
// Good: explicit timeout
session.expect_timeout(pattern, Duration::from_secs(30)).await?;

// Better: configure default timeout
let config = SessionConfig::default()
    .timeout(Duration::from_secs(30));
let session = Session::spawn_with_config("bash", &[], config).await?;
```

### 2. Handle Process Exit

Check for EOF when processes might exit:

```rust
let mut patterns = PatternSet::new();
patterns.add(Pattern::literal("ready"));
patterns.add(Pattern::eof());

match session.expect_any(&patterns).await? {
    m if m.is_eof() => bail!("Process exited unexpectedly"),
    m => println!("Matched: {}", m.matched),
}
```

### 3. Clean Up Sessions

Always clean up sessions properly:

```rust
// Option 1: Send exit command
session.send_line("exit").await?;
session.wait().await?;

// Option 2: Kill the process
session.kill().await?;

// Option 3: Use Drop (automatic, but less controlled)
drop(session);
```

### 4. Use Dialogs for Complex Flows

For multi-step interactions, dialogs are cleaner:

```rust
// Instead of:
session.expect("login:").await?;
session.send_line("user").await?;
session.expect("password:").await?;
session.send_line("pass").await?;
session.expect("$ ").await?;

// Use:
let dialog = DialogBuilder::new()
    .expect_send("login", "login:", "user\n")
    .expect_send("password", "password:", "pass\n")
    .expect_send("prompt", "$ ", "")
    .build();

session.run_dialog(&dialog).await?;
```

### 5. Redact Sensitive Output

When logging session output:

```rust
use rust_expect::pii::PiiRedactor;

let redactor = PiiRedactor::new();

// Before logging
let output = session.read().await?;
let safe_output = redactor.redact(&String::from_utf8_lossy(&output));
log::info!("Session output: {}", safe_output);
```

### 6. Use Mock Sessions for Tests

Don't spawn real processes in unit tests:

```rust
#[cfg(test)]
mod tests {
    use rust_expect::mock::MockSession;

    #[tokio::test]
    async fn test_login_flow() {
        let mock = MockSession::new()
            .on_output("login:")
            .on_input("admin\n", "password:")
            .on_input("secret\n", "Welcome!\n");

        let mut session = mock.into_session();
        // Test your login logic...
    }
}
```

---

## Migration Guide

### From pexpect (Python)

| pexpect | rust-expect |
|---------|-------------|
| `pexpect.spawn("cmd")` | `Session::spawn("cmd", &[]).await?` |
| `child.expect("pattern")` | `session.expect("pattern").await?` |
| `child.sendline("text")` | `session.send_line("text").await?` |
| `child.before` | `match_result.before` |
| `child.after` | `match_result.after` |
| `pexpect.EOF` | `Pattern::eof()` |
| `pexpect.TIMEOUT` | `Pattern::timeout(duration)` |

### From expectrl

| expectrl | rust-expect |
|----------|-------------|
| `Session::spawn("cmd")` | `Session::spawn("cmd", &[]).await?` |
| `session.expect(Regex("..."))` | `session.expect(Pattern::regex("...")).await?` |
| `session.send("text")` | `session.send(b"text").await?` |
| `Expect::check()` | `session.check("pattern").await?` |

See [MIGRATION.md](../MIGRATION.md) for detailed migration examples.

---

## Further Reading

- [API Documentation](https://docs.rs/rust-expect)
- [Examples](../crates/rust-expect/examples/)
- [Architecture Guide](../ARCHITECTURE.md)
- [Contributing Guide](../CONTRIBUTING.md)
- [Security Policy](../SECURITY.md)
