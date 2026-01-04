# Migration Guide

This guide helps you migrate from other Expect-like libraries to rust-expect.

## From Python pexpect

[pexpect](https://pexpect.readthedocs.io/) is the popular Python implementation of Expect. Here's how to translate common patterns.

### Spawning a Process

**pexpect (Python):**
```python
import pexpect

child = pexpect.spawn('/usr/bin/ftp speedtest.example.com')
# or with separate args
child = pexpect.spawn('/usr/bin/ssh', ['user@example.com'])
```

**rust-expect (Rust):**
```rust
use rust_expect::prelude::*;

let mut session = Session::spawn("ftp speedtest.example.com")?;
// or with arguments
let mut session = Session::spawn_args("ssh", &["user@example.com"])?;
```

### Expecting Patterns

**pexpect (Python):**
```python
child.expect('Name \(.*\):')     # Regex pattern
child.expect_exact('Password:')  # Literal string
child.expect([pexpect.EOF, pexpect.TIMEOUT, 'prompt'])  # Multiple patterns
```

**rust-expect (Rust):**
```rust
use std::time::Duration;

session.expect_regex(r"Name \(.*\):")?;  // Regex pattern
session.expect("Password:")?;             // Literal string

// Multiple patterns
let result = session.expect_any(&[
    Pattern::eof(),
    Pattern::timeout(Duration::from_secs(10)),
    Pattern::literal("prompt"),
]).await?;

match result.index() {
    0 => println!("EOF received"),
    1 => println!("Timeout"),
    2 => println!("Got prompt"),
    _ => unreachable!(),
}
```

### Sending Input

**pexpect (Python):**
```python
child.send('hello')          # Send without newline
child.sendline('hello')      # Send with newline
child.sendcontrol('c')       # Send Ctrl+C
child.sendeof()              # Send EOF (Ctrl+D)
```

**rust-expect (Rust):**
```rust
session.send("hello").await?;       // Send without newline
session.send_line("hello").await?;  // Send with newline
session.send_control('c').await?;   // Send Ctrl+C
session.send_eof().await?;          // Send EOF (Ctrl+D)
```

### Timeout Handling

**pexpect (Python):**
```python
child = pexpect.spawn('cmd', timeout=30)  # Default timeout
child.expect('pattern', timeout=10)        # Per-operation timeout

try:
    child.expect('pattern')
except pexpect.TIMEOUT:
    print("Timeout!")
```

**rust-expect (Rust):**
```rust
use std::time::Duration;

// Default timeout in config
let config = SessionConfig::default()
    .with_timeout(Duration::from_secs(30));
let mut session = Session::spawn_with_config("cmd", config)?;

// Per-operation timeout
session.expect_timeout("pattern", Duration::from_secs(10)).await?;

// Handle timeout error
match session.expect("pattern").await {
    Ok(result) => println!("Matched: {}", result.matched()),
    Err(e) if e.is_timeout() => println!("Timeout!"),
    Err(e) => return Err(e.into()),
}
```

### Interactive Mode

**pexpect (Python):**
```python
child.interact()  # Pass control to user
```

**rust-expect (Rust):**
```rust
use rust_expect::interact::InteractOptions;

// Basic interactive mode
session.interact().await?;

// With hooks for pattern matching
let options = InteractOptions::new()
    .on_output("password:", |ctx| {
        eprintln!("Password prompt detected!");
        Ok(())
    });
session.interact_with(options).await?;
```

### Reading Output

**pexpect (Python):**
```python
child.before   # Text before the match
child.after    # Text that matched
child.match    # The match object
```

**rust-expect (Rust):**
```rust
let result = session.expect("pattern").await?;

result.before()   // Text before the match
result.matched()  // Text that matched
result.after()    // Text after the match (in buffer)
```

### Checking for EOF

**pexpect (Python):**
```python
if child.eof():
    print("Process ended")

child.expect(pexpect.EOF)  # Wait for EOF
```

**rust-expect (Rust):**
```rust
if session.is_eof() {
    println!("Process ended");
}

session.expect_eof().await?;  // Wait for EOF
```

---

## From Rust expectrl

[expectrl](https://crates.io/crates/expectrl) is another Rust Expect library. Here's how to migrate.

### Spawning a Process

**expectrl:**
```rust
use expectrl::spawn;

let mut session = spawn("bash")?;
// SSH
let mut session = expectrl::spawn("ssh user@host")?;
```

**rust-expect:**
```rust
use rust_expect::prelude::*;

let mut session = Session::spawn("bash")?;
// SSH with dedicated backend
let mut session = SshSessionBuilder::new("host")
    .username("user")
    .connect()?;
```

### Pattern Matching

**expectrl:**
```rust
use expectrl::{Regex, Eof};

session.expect(Regex("\\$ $"))?;
session.expect("literal string")?;
session.expect(Eof)?;
```

**rust-expect:**
```rust
use rust_expect::expect::Pattern;

session.expect_regex(r"\$ $").await?;
session.expect("literal string").await?;
session.expect_eof().await?;
```

### Sending Input

**expectrl:**
```rust
session.send_line("echo hello")?;
session.send("raw bytes")?;
```

**rust-expect:**
```rust
session.send_line("echo hello").await?;
session.send("raw bytes").await?;
```

### Async Support

**expectrl:**
```rust
// Must enable async feature, uses different API
#[cfg(feature = "async")]
use expectrl::AsyncSession;

let mut session = AsyncSession::spawn("bash").await?;
session.expect("$ ").await?;
```

**rust-expect:**
```rust
// Async is the default, sync is opt-in
use rust_expect::prelude::*;

let mut session = Session::spawn("bash")?;
session.expect("$ ").await?;

// For sync contexts
use rust_expect::sync::SyncSession;
let mut session = SyncSession::spawn("bash")?;
session.expect("$ ")?;  // Blocking
```

### Interact Mode

**expectrl:**
```rust
use expectrl::interact::InteractSession;

let mut interact = InteractSession::new(&mut session, stream::stdin());
interact.spawn()?;
```

**rust-expect:**
```rust
session.interact().await?;

// Or with more control
use rust_expect::interact::InteractOptions;

let options = InteractOptions::new()
    .on_input("exit", |ctx| {
        println!("User typed exit");
        Ok(())
    });
session.interact_with(options).await?;
```

### Checking Match Results

**expectrl:**
```rust
let captures = session.expect(Regex("user: (\\w+)"))?;
let matched = captures.get(1).unwrap();
```

**rust-expect:**
```rust
let result = session.expect_regex(r"user: (\w+)").await?;
let matched = result.matched();
// For captures, use the pattern directly
if let Some(caps) = result.captures() {
    let user = caps.get(1).map(|m| m.as_str());
}
```

### Screen Buffer

**expectrl:**
```rust
// Limited screen support
```

**rust-expect:**
```rust
use rust_expect::screen::ScreenBuffer;

let mut screen = ScreenBuffer::new(80, 24);
screen.feed(&output);

// Query screen content
let text = screen.get_text(0, 0, 80, 1);  // First line
let found = screen.find_text("pattern");

// Visual diff
let diff = screen.diff(&other_screen);
```

---

## Key Differences Summary

| Feature | pexpect | expectrl | rust-expect |
|---------|---------|----------|-------------|
| Language | Python | Rust | Rust |
| Async default | No | No | **Yes** |
| Sync API | Yes | Yes | Optional |
| Windows support | Limited | Yes | **Full ConPTY** |
| SSH backend | External | Basic | **Full (pooling, resilience)** |
| Screen emulation | No | Basic | **VT100 + visual diff** |
| PII redaction | No | No | **Built-in** |
| Metrics | No | No | **Prometheus/OTLP** |
| Connection pooling | N/A | No | **Yes** |
| Mock testing | No | No | **Built-in** |

---

## Common Migration Patterns

### Error Handling

**pexpect/expectrl pattern:**
```rust
// expectrl uses Result with custom error types
match session.expect("pattern") {
    Ok(m) => { /* handle match */ }
    Err(e) => { /* handle error */ }
}
```

**rust-expect pattern:**
```rust
use rust_expect::error::ExpectError;

match session.expect("pattern").await {
    Ok(result) => {
        println!("Matched: {}", result.matched());
    }
    Err(ExpectError::Timeout { duration, pattern, buffer }) => {
        // Rich error context with buffer snippet
        eprintln!("Timeout after {:?} waiting for '{}'", duration, pattern);
        eprintln!("Buffer: {}", buffer);
    }
    Err(ExpectError::Eof { buffer }) => {
        eprintln!("Process ended unexpectedly");
    }
    Err(e) => return Err(e.into()),
}
```

### Dialog/Script Automation

**pexpect:**
```python
child.expect('login:')
child.sendline('admin')
child.expect('password:')
child.sendline('secret')
child.expect('$')
```

**rust-expect with Dialog:**
```rust
use rust_expect::dialog::{Dialog, DialogStep};

let dialog = Dialog::new()
    .step(DialogStep::expect("login:").then_send("admin\n"))
    .step(DialogStep::expect("password:").then_send("secret\n"))
    .step(DialogStep::expect("$"));

session.run_dialog(&dialog).await?;
```

### Multi-Session Management

**pexpect:**
```python
# Manual management of multiple sessions
sessions = [pexpect.spawn(f'ssh host{i}') for i in range(3)]
for s in sessions:
    s.expect('$')
```

**rust-expect:**
```rust
use rust_expect::multi::{MultiSession, expect_all, expect_any};

let mut multi = MultiSession::new();
for i in 0..3 {
    let session = Session::spawn(&format!("ssh host{}", i))?;
    multi.add(session);
}

// Wait for all to match
let results = expect_all(&mut multi.sessions_mut(), "$").await?;

// Or wait for first match
let first = expect_any(&mut multi.sessions_mut(), "$").await?;
```

---

## Getting Help

- [API Documentation](https://docs.rs/rust-expect)
- [GitHub Issues](https://github.com/praxiomlabs/rust-expect/issues)
- [Examples](https://github.com/praxiomlabs/rust-expect/tree/main/crates/rust-expect/examples)
