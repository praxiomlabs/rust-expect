# rust-pty

Low-level pseudo-terminal (PTY) abstraction for Rust.

## Features

- Async I/O with Tokio
- PTY allocation and configuration
- Window size management
- Child process spawning
- Signal handling

## Usage

```rust
use rust_pty::{PtyConfig, UnixPtyMaster, spawn_with_pty};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = PtyConfig::default()
        .window_size(80, 24);

    let (master, child) = spawn_with_pty("bash", &config)?;

    // Use master for I/O
    // master.write_all(b"echo hello\n").await?;

    Ok(())
}
```

## Platform Support

- **Unix**: Linux, macOS, BSD (via rustix PTY)
- **Windows**: Windows 10 1809+ (via ConPTY)

## License

Licensed under MIT or Apache-2.0.
