# rust-expect

[![Crates.io](https://img.shields.io/crates/v/rust-expect.svg)](https://crates.io/crates/rust-expect)
[![Documentation](https://docs.rs/rust-expect/badge.svg)](https://docs.rs/rust-expect)
[![License](https://img.shields.io/crates/l/rust-expect.svg)](LICENSE)
[![CI](https://github.com/jkindrix/rust-expect/workflows/CI/badge.svg)](https://github.com/jkindrix/rust-expect/actions)

A modern, async-first terminal automation library for Rust, inspired by the classic Expect tool.

## Features

- **Async/Await First**: Built on Tokio for efficient, non-blocking I/O
- **Pattern Matching**: Support for literal strings, regex, and glob patterns
- **PTY Support**: Full pseudo-terminal support on Unix and Windows
- **SSH Integration**: Built-in SSH session management (optional)
- **Screen Emulation**: Virtual terminal with ANSI escape sequence support
- **PII Redaction**: Automatic sensitive data masking for logs
- **Dialog Scripting**: Declarative conversation flows
- **Human-like Typing**: Configurable typing simulation
- **Comprehensive Testing**: Mock sessions and test utilities

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
rust-expect = "0.1"
```

### Basic Example

```rust
use rust_expect::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Spawn a new session
    let mut session = Session::spawn("bash")?;

    // Wait for prompt and send command
    session.expect("$ ").await?;
    session.send("echo 'Hello, World!'\n").await?;

    // Expect the output
    let result = session.expect("Hello, World!").await?;
    println!("Matched: {}", result.matched());

    // Clean exit
    session.send("exit\n").await?;
    session.expect_eof().await?;

    Ok(())
}
```

### Using Dialogs

```rust
use rust_expect::prelude::*;
use rust_expect::dialog::{Dialog, DialogStep};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dialog = Dialog::new()
        .step(DialogStep::expect("login: ").then_send("admin\n"))
        .step(DialogStep::expect("password: ").then_send("secret\n"))
        .step(DialogStep::expect("$ "));

    let mut session = Session::spawn("login_program")?;
    session.run_dialog(&dialog).await?;

    Ok(())
}
```

### Pattern Matching

```rust
use rust_expect::expect::{Pattern, PatternSet};

// Literal string
let pattern = Pattern::literal("hello");

// Regular expression
let pattern = Pattern::regex(r"\d{3}-\d{4}")?;

// Glob pattern
let pattern = Pattern::glob("Error: *");

// Multiple patterns
let mut patterns = PatternSet::new();
patterns.add(Pattern::literal("success"));
patterns.add(Pattern::literal("failure"));
patterns.add(Pattern::timeout(Duration::from_secs(10)));
```

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `ssh` | SSH session support via russh | No |
| `mock` | Mock sessions for testing | No |
| `screen` | Virtual terminal emulation | No |
| `pii-redaction` | Automatic PII masking | No |
| `test-utils` | Testing utilities | No |
| `metrics` | Performance metrics | No |
| `full` | All features | No |

Enable features in `Cargo.toml`:

```toml
[dependencies]
rust-expect = { version = "0.1", features = ["ssh", "screen"] }
```

## Crates

This workspace includes:

- **[rust-expect](crates/rust-expect)**: Main library with session management, expect operations, and optional features
- **[rust-expect-macros](crates/rust-expect-macros)**: Procedural macros for pattern definitions
- **[rust-pty](crates/rust-pty)**: Low-level PTY abstraction for Unix and Windows

## Examples

See the [examples](crates/rust-expect/examples) directory:

| Example | Description | Required Features |
|---------|-------------|-------------------|
| `basic.rs` | Core spawn/expect workflow | - |
| `dialog.rs` | Dialog-based automation | - |
| `patterns.rs` | Pattern matching capabilities | - |
| `screen_buffer.rs` | Virtual terminal with ANSI | `screen` |
| `pii_redaction.rs` | Sensitive data masking | `pii-redaction` |
| `ssh.rs` | SSH session concepts | `ssh` |
| `mock_testing.rs` | Mock backend for testing | `mock` |
| `metrics.rs` | Prometheus/OpenTelemetry | `metrics` |
| `transcript.rs` | Recording and playback | - |
| `interactive.rs` | Interactive terminal mode | - |
| `multi_session.rs` | Managing multiple sessions | - |
| `sync_api.rs` | Synchronous API usage | - |

Run examples with:

```bash
cargo run --example basic
cargo run --example screen_buffer --features screen
cargo run --example ssh --features ssh
```

## Documentation

- [API Documentation](https://docs.rs/rust-expect)
- [Architecture Guide](ARCHITECTURE.md)
- [Contributing Guide](CONTRIBUTING.md)

## Minimum Supported Rust Version

This project requires **Rust 1.85** or later.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Contributing

Contributions are welcome! Please read our [Contributing Guide](CONTRIBUTING.md) and [Code of Conduct](CODE_OF_CONDUCT.md).
