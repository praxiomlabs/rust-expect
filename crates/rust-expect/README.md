# rust-expect

A modern, async-first terminal automation library for Rust, inspired by the classic Expect tool.

## Features

- **Async/Await First**: Built on Tokio for efficient, non-blocking I/O
- **Pattern Matching**: Literal strings, regular expressions, and glob patterns
- **PTY Support**: Full pseudo-terminal support on Unix systems
- **SSH Integration**: Built-in SSH session management (optional)
- **Screen Emulation**: Virtual terminal with ANSI escape sequence support
- **PII Redaction**: Automatic sensitive data masking
- **Dialog Scripting**: Declarative conversation flows
- **Human-like Typing**: Configurable typing simulation

## Usage

```rust
use rust_expect::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = Session::spawn("bash")?;

    session.expect("$ ").await?;
    session.send("echo 'Hello!'\n").await?;
    session.expect("Hello!").await?;
    session.send("exit\n").await?;

    Ok(())
}
```

## Feature Flags

- `ssh` - SSH session support
- `mock` - Mock sessions for testing
- `screen` - Virtual terminal emulation
- `pii-redaction` - Automatic PII masking
- `test-utils` - Testing utilities
- `metrics` - Performance metrics
- `full` - All features

## License

Licensed under MIT or Apache-2.0.
