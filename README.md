# rust-expect

[![Crates.io](https://img.shields.io/crates/v/rust-expect.svg)](https://crates.io/crates/rust-expect)
[![Documentation](https://docs.rs/rust-expect/badge.svg)](https://docs.rs/rust-expect)
[![License](https://img.shields.io/crates/l/rust-expect.svg)](LICENSE)
[![CI](https://github.com/praxiomlabs/rust-expect/workflows/CI/badge.svg)](https://github.com/praxiomlabs/rust-expect/actions)

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
rust-expect = "0.4"
```

### Basic Example

```rust,no_run
use rust_expect::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Spawn a new session
    let mut session = Session::spawn("/bin/bash", &[]).await?;

    // Wait for prompt and send command
    session.expect("$ ").await?;
    session.send_line("echo 'Hello, World!'").await?;

    // Expect the output
    let result = session.expect("Hello, World!").await?;
    println!("Matched: {}", result.matched);

    // Clean exit
    session.send_line("exit").await?;
    session.expect_eof().await?;

    Ok(())
}
```

### Using Dialogs

```rust,no_run
use rust_expect::prelude::*;
use rust_expect::dialog::{Dialog, DialogStep};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dialog = Dialog::new()
        .step(DialogStep::expect("login: ").then_send("admin\n"))
        .step(DialogStep::expect("password: ").then_send("secret\n"))
        .step(DialogStep::expect("$ "));

    let mut session = Session::spawn("login_program", &[]).await?;
    session.run_dialog(&dialog).await?;

    Ok(())
}
```

### Driving TUI Applications

Many automation targets are not line-oriented CLIs but full-screen TUIs
(vim, lazygit, k9s, fzf, Claude Code, …) where the byte stream is dominated
by ANSI escape sequences and literal substring matching on the raw buffer
is impractical. The `screen` feature plus the screen-aware expect methods
let you anchor on what the TUI actually *renders*:

```rust
use rust_expect::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = Session::spawn("/usr/bin/htop", &["--no-color"]).await?;
    session.attach_screen();                            // virtual terminal

    // Anchor on rendered content, not the byte stream.
    session.expect_screen_contains("CPU%", Duration::from_secs(5)).await?;

    // Wait for the TUI to stop redrawing.
    session.wait_screen_stable(
        Duration::from_millis(500),
        Duration::from_secs(5),
    ).await?;

    // Send a paste-mode-wrapped command so a leading `/` doesn't trigger
    // an autocomplete popup or slash-command picker.
    session.send_paste("/some-command").await?;
    session.send(b"\r").await?;

    // Inspect what's on screen.
    let text = session.screen().unwrap().lock().unwrap().text();
    assert!(text.contains("/some-command"));

    session.send(b"q").await?;
    session.wait().await?;
    Ok(())
}
```

Companion primitives:

- `Session::add_output_tap` / `remove_output_tap` / `output_tap_callbacks`
  register synchronous observers for every chunk of bytes read — the
  foundation for transcript recording, asciinema export, and live-view
  tees that don't interfere with `expect`.
- `Session::expect_screen_contains` / `wait_screen_not_contains` /
  `wait_screen_stable` poll the rendered screen rather than the raw
  buffer. Polling interval is configurable via
  `Session::set_screen_poll_interval`.
- `Session::send_paste` wraps text in bracketed-paste markers
  (DECSET 2004); `Session::send_shift_tab` emits CSI Z for TUIs that
  use back-tab for reverse focus traversal.

Both `examples/drive_less.rs` (alt-screen + paging) and
`examples/drive_htop.rs` (continuous redraws) demonstrate end-to-end TUI
driving against third-party applications.

### Pattern Matching

```rust,no_run
use rust_expect::expect::{Pattern, PatternSet};

# fn run() -> Result<(), Box<dyn std::error::Error>> {
// Literal string
let _pattern = Pattern::literal("hello");

// Regular expression
let _pattern = Pattern::regex(r"\d{3}-\d{4}")?;

// Glob pattern
let _pattern = Pattern::glob("Error: *");

// Multiple patterns — timeout is supplied to `expect_timeout`, not as a Pattern.
let mut patterns = PatternSet::new();
patterns.add(Pattern::literal("success"));
patterns.add(Pattern::literal("failure"));
# Ok(()) }
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
rust-expect = { version = "0.4", features = ["ssh", "screen"] }
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

This project requires **Rust 1.88** or later.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Contributing

Contributions are welcome! Please read our [Contributing Guide](CONTRIBUTING.md) and [Code of Conduct](CODE_OF_CONDUCT.md).
