# rust-expect: Technical Architecture

**Version:** 1.1.0
**Date:** 2025-12-30
**Status:** Authoritative
**Dependency Versions:** As of December 2025
**Aligns With:** REQUIREMENTS.md v1.2.0

---

## Table of Contents

1. [Overview](#1-overview)
2. [Design Philosophy](#2-design-philosophy)
3. [Workspace Structure](#3-workspace-structure)
4. [System Architecture](#4-system-architecture)
5. [rust-pty Crate](#5-rust-pty-crate)
6. [rust-expect Crate](#6-rust-expect-crate)
7. [rust-expect-macros Crate](#7-rust-expect-macros-crate)
8. [Cross-Platform Strategy](#8-cross-platform-strategy)
9. [Async Architecture](#9-async-architecture)
10. [Error Handling](#10-error-handling)
11. [Feature Flags](#11-feature-flags)
12. [Dependencies](#12-dependencies)
13. [Data Flow](#13-data-flow)
14. [Testing Architecture](#14-testing-architecture)
15. [Security Considerations](#15-security-considerations)
16. [Performance Baselines](#16-performance-baselines)
17. [Encoding and Character Handling](#17-encoding-and-character-handling)
18. [Observability and Metrics](#18-observability-and-metrics)
19. [Configuration File Support](#19-configuration-file-support)
20. [Transcript Logging](#20-transcript-logging)
21. [Zero-Config Mode](#21-zero-config-mode)
22. [Mock Session Backend](#22-mock-session-backend)
23. [Supply Chain Security](#23-supply-chain-security)

**Appendices:**
- [Appendix A: Glossary](#appendix-a-glossary)
- [Appendix B: References](#appendix-b-references)
- [Appendix C: Migration Guide](#appendix-c-migration-guide)

---

## 1. Overview

rust-expect is a next-generation terminal automation library for Rust, designed to exceed all existing implementations (expectrl, rexpect, pexpect) in features, performance, cross-platform support, API ergonomics, and reliability.

### 1.1 System Context

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              User Application                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                rust-expect                                   │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │   Session   │  │   Pattern   │  │   Dialog    │  │       SSH           │ │
│  │   Manager   │  │   Matcher   │  │   System    │  │     Backend         │ │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────────────┘ │
├─────────────────────────────────────────────────────────────────────────────┤
│                                 rust-pty                                     │
│  ┌─────────────────────────────┐  ┌─────────────────────────────────────────┐│
│  │       Unix Backend          │  │          Windows Backend                ││
│  │    (rustix + AsyncFd)       │  │   (windows-sys + ConPTY)                ││
│  └─────────────────────────────┘  └─────────────────────────────────────────┘│
├─────────────────────────────────────────────────────────────────────────────┤
│                           Operating System                                   │
│  ┌─────────────────────────────┐  ┌─────────────────────────────────────────┐│
│  │   Linux/macOS PTY           │  │       Windows ConPTY                    ││
│  │   /dev/ptmx, posix_openpt   │  │   CreatePseudoConsole                   ││
│  └─────────────────────────────┘  └─────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Key Metrics

| Metric | Target |
|--------|--------|
| Spawn latency | < 50ms |
| Pattern match throughput | > 100 MB/s |
| Memory overhead | < 1.5x output size |
| Max output handled | 1 GB without crash |
| MSRV | 1.85 (Edition 2024) |

---

## 2. Design Philosophy

### 2.1 Core Principles

| Principle | Implementation |
|-----------|----------------|
| **Async-First** | Core implementation is async; sync API is a thin `block_on` wrapper |
| **Cross-Platform by Design** | Windows is first-class, not an afterthought; platform-specific code isolated in `rust-pty` |
| **Zero Surprises** | Behavior matches documentation; edge cases handled explicitly |
| **Fail-Fast with Context** | Rich error types with buffer contents, durations, and patterns |
| **Performance Without Compromise** | Zero-copy where possible; streaming pattern matching |
| **Batteries Included** | SSH, logging, screen buffer—all optional via feature flags |

### 2.2 Architectural Decisions

| Decision | Rationale |
|----------|-----------|
| Separate `rust-pty` crate | Ecosystem value; reusable by other projects |
| Trait-based abstraction | Enable testing, mocking, and future backends |
| Tokio as primary runtime | Industry standard; excellent process/I/O support |
| `rustix` over `nix` | Modern, maintained, better API; used by pty-process |
| `windows-sys` over `winapi` | Official Microsoft crate; better type safety |
| Edition 2024 / MSRV 1.85 | Async closures; modern patterns worth adoption trade-off |

**MSRV Adoption Considerations:**

The choice of MSRV 1.85 (Edition 2024) is intentional for a greenfield project, enabling:
- Native async closures (RFC 3668) for cleaner callback APIs
- `gen` blocks for pattern matching iterators
- Modern error handling patterns

However, organizations with conservative MSRV policies (typically stable-2 to stable-7) may require adaptation. For maximum compatibility:

| Alternative | MSRV | Trade-off |
|-------------|------|-----------|
| Edition 2021 / MSRV 1.70 | 1.70 | Sacrifices async closures; requires `Box<dyn Future>` wrappers |
| Edition 2021 / MSRV 1.75 | 1.75 | Adds async fn in traits; still requires closure workarounds |
| Edition 2024 / MSRV 1.85 | 1.85 | Full feature set; recommended for new projects |

The `proptest` crate (MSRV guaranteed ≤ stable-7) validates this approach. Organizations needing broader compatibility should fork with Edition 2021 and accept the API ergonomics trade-off.

### 2.3 Non-Goals

- GUI automation (use AccessKit, windows-rs)
- Web browser automation (use chromiumoxide, fantoccini)
- WebAssembly support (PTY requires native OS APIs)
- Serial port communication in core (use serialport crate; future consideration)

---

## 3. Workspace Structure

### 3.1 Directory Layout

```
rust-expect/
├── Cargo.toml                    # Workspace manifest
├── ARCHITECTURE.md               # This document
├── REQUIREMENTS.md               # Functional requirements
├── LIBRARY_ANALYSIS.md           # Competitive analysis
├── README.md                     # Project overview
├── LICENSE-MIT                   # MIT license
├── LICENSE-APACHE                # Apache 2.0 license
│
├── crates/
│   ├── rust-pty/                 # Cross-platform async PTY
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── config.rs         # PtyConfig, PtySignal
│   │   │   ├── error.rs          # PtyError types
│   │   │   ├── traits.rs         # PtyMaster, PtyChild, PtySystem
│   │   │   ├── unix/             # Unix backend
│   │   │   │   ├── mod.rs
│   │   │   │   ├── pty.rs        # PTY allocation via rustix
│   │   │   │   ├── child.rs      # Child process management
│   │   │   │   └── signals.rs    # SIGWINCH, SIGCHLD handling
│   │   │   └── windows/          # Windows backend
│   │   │       ├── mod.rs
│   │   │       ├── conpty.rs     # ConPTY management
│   │   │       ├── pipes.rs      # Pipe I/O handling
│   │   │       ├── child.rs      # Process + Job Object management
│   │   │       └── async_adapter.rs  # Thread-per-pipe / overlapped I/O
│   │   └── tests/
│   │
│   ├── rust-expect/              # Main expect library
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── session/          # Session management
│   │   │   │   ├── mod.rs
│   │   │   │   ├── builder.rs    # SessionBuilder
│   │   │   │   ├── handle.rs     # Session handle
│   │   │   │   └── lifecycle.rs  # Spawn, wait, kill
│   │   │   ├── expect/           # Pattern matching
│   │   │   │   ├── mod.rs
│   │   │   │   ├── pattern.rs    # Pattern types
│   │   │   │   ├── matcher.rs    # Matching engine
│   │   │   │   ├── buffer.rs     # Buffer management
│   │   │   │   └── result.rs     # Match results
│   │   │   ├── send/             # Output operations
│   │   │   │   ├── mod.rs
│   │   │   │   ├── basic.rs      # send, send_line, send_control
│   │   │   │   └── human.rs      # send_slow, send_human
│   │   │   ├── interact/         # Interactive mode
│   │   │   │   ├── mod.rs
│   │   │   │   ├── basic.rs      # Basic interact
│   │   │   │   ├── hooks.rs      # Pattern/input hooks
│   │   │   │   └── terminal.rs   # Raw mode via crossterm
│   │   │   ├── multi/            # Multi-session
│   │   │   │   ├── mod.rs
│   │   │   │   ├── group.rs      # Session groups
│   │   │   │   └── select.rs     # select_expect, expect_all
│   │   │   ├── ssh/              # SSH backend (feature-gated)
│   │   │   │   ├── mod.rs
│   │   │   │   ├── session.rs    # SSH session
│   │   │   │   ├── auth.rs       # Authentication
│   │   │   │   └── channel.rs    # Channel management
│   │   │   ├── screen/           # Terminal emulation (feature-gated)
│   │   │   │   ├── mod.rs
│   │   │   │   ├── parser.rs     # ANSI parsing via vte
│   │   │   │   ├── buffer.rs     # Virtual screen buffer
│   │   │   │   └── query.rs      # Screen queries
│   │   │   ├── dialog/           # Dialog system
│   │   │   │   ├── mod.rs
│   │   │   │   ├── definition.rs # Dialog definition
│   │   │   │   ├── executor.rs   # Dialog execution
│   │   │   │   └── common.rs     # Login, sudo, confirm dialogs
│   │   │   ├── logging/          # Transcript logging
│   │   │   │   ├── mod.rs
│   │   │   │   └── transcript.rs # Session recording
│   │   │   └── error.rs          # Error types
│   │   ├── tests/
│   │   └── examples/
│   │
│   └── rust-expect-macros/       # Procedural macros
│       ├── Cargo.toml
│       ├── src/
│       │   ├── lib.rs
│       │   ├── patterns.rs       # patterns! macro
│       │   ├── regex.rs          # regex! macro
│       │   └── timeout.rs        # timeout! macro
│       └── tests/
│
├── tests/                        # Integration tests
│   ├── common/                   # Shared test utilities
│   ├── spawn_tests.rs
│   ├── expect_tests.rs
│   ├── interact_tests.rs
│   ├── multi_session_tests.rs
│   └── platform_tests.rs
│
├── benches/                      # Benchmarks
│   ├── spawn_bench.rs
│   ├── pattern_bench.rs
│   └── throughput_bench.rs
│
├── examples/                     # Usage examples
│   ├── basic.rs
│   ├── ssh.rs
│   ├── interactive.rs
│   ├── multi_session.rs
│   ├── large_output.rs
│   ├── dialog.rs
│   └── logging.rs
│
└── test-utils/                   # Test fixture binaries
    ├── test-echo/
    ├── test-prompt/
    ├── test-output/
    ├── test-signals/
    └── test-hang/
```

### 3.2 Workspace Cargo.toml

```toml
[workspace]
members = [
    "crates/rust-pty",
    "crates/rust-expect",
    "crates/rust-expect-macros",
    "test-utils/test-echo",
    "test-utils/test-prompt",
    "test-utils/test-output",
    "test-utils/test-signals",
    "test-utils/test-hang",
]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.85"
license = "MIT OR Apache-2.0"
repository = "https://github.com/..."
keywords = ["expect", "pty", "terminal", "automation", "async"]
categories = ["command-line-utilities", "development-tools::testing", "asynchronous"]

[workspace.dependencies]
# Async runtime
tokio = { version = "~1.43", features = ["full"] }  # LTS release, supported until March 2026

# PTY operations
rustix = { version = "1.1", features = ["termios", "process", "pty", "fs"] }

# Pattern matching
regex = "1.12"

# Logging
tracing = "0.1"

# Error handling
thiserror = "2.0"

# Terminal manipulation
crossterm = { version = "0.29", features = ["event-stream"] }

# ANSI parsing
vte = "0.15"

# SSH (optional)
russh = "0.54"
russh-keys = "0.49"

# Testing
proptest = "1.9"

# Workspace crates
rust-pty = { path = "crates/rust-pty" }
rust-expect = { path = "crates/rust-expect" }
rust-expect-macros = { path = "crates/rust-expect-macros" }

[workspace.dependencies.windows-sys]
version = "0.61"
features = [
    "Win32_Foundation",
    "Win32_System_Console",
    "Win32_System_Threading",
    "Win32_System_Pipes",
    "Win32_Security",
    "Win32_System_JobObjects",
    "Win32_System_IO",
]

[workspace.lints.rust]
unsafe_code = "warn"
missing_docs = "warn"

[workspace.lints.clippy]
all = "warn"
pedantic = "warn"
nursery = "warn"

[profile.release]
lto = true
codegen-units = 1
```

### 3.3 Crate Dependencies

```
┌─────────────────────────────────────────────────────────────────┐
│                        rust-expect                               │
│                                                                  │
│  ┌──────────────────┐  ┌──────────────────┐  ┌────────────────┐ │
│  │ rust-expect-macros│  │     russh        │  │      vte       │ │
│  │   (proc macros)   │  │  (SSH, optional) │  │ (ANSI, optional│ │
│  └──────────────────┘  └──────────────────┘  └────────────────┘ │
│                                                                  │
│  ┌──────────────────┐  ┌──────────────────┐  ┌────────────────┐ │
│  │    crossterm     │  │     tracing      │  │     regex      │ │
│  │ (terminal input) │  │    (logging)     │  │   (patterns)   │ │
│  └──────────────────┘  └──────────────────┘  └────────────────┘ │
├─────────────────────────────────────────────────────────────────┤
│                          rust-pty                                │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                        tokio                              │   │
│  │              (async runtime, AsyncFd)                     │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌─────────────────────────┐  ┌────────────────────────────┐   │
│  │        rustix           │  │       windows-sys          │   │
│  │   (Unix PTY syscalls)   │  │   (Windows ConPTY API)     │   │
│  │   [cfg(unix)]           │  │   [cfg(windows)]           │   │
│  └─────────────────────────┘  └────────────────────────────┘   │
│                                                                  │
│  ┌─────────────────────────┐  ┌────────────────────────────┐   │
│  │      signal-hook        │  │       thiserror            │   │
│  │   (Unix signals)        │  │   (error types)            │   │
│  │   [cfg(unix)]           │  │                            │   │
│  └─────────────────────────┘  └────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

---

## 4. System Architecture

### 4.1 Layered Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           User API Layer                                     │
│   Session::builder(), expect(), send(), interact(), Dialog, SSH              │
│   • High-level ergonomic API                                                 │
│   • Builder pattern configuration                                            │
│   • Macro-based pattern definition                                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                        Pattern Matching Engine                               │
│   Regex, Glob, Exact, EOF, Timeout, Composite                               │
│   • Streaming pattern matching                                               │
│   • exp_continue, expect_before/after                                        │
│   • Configurable search window                                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                         Stream Abstraction                                   │
│   AsyncRead/AsyncWrite, Buffering, Logging                                  │
│   • Ring buffer with configurable size                                       │
│   • Transcript recording                                                     │
│   • Encoding handling (UTF-8 default)                                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                        Process Abstraction                                   │
│   Spawn, Environment, Signals, Cleanup                                      │
│   • Unified interface for PTY and SSH sessions                              │
│   • Automatic cleanup on drop                                                │
│   • Cancellation-safe operations                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                         PTY Backend Layer                                    │
│   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐       │
│   │   Linux     │  │   macOS     │  │  Windows    │  │    SSH      │       │
│   │    PTY      │  │    PTY      │  │   ConPTY    │  │   Channel   │       │
│   │  (rustix)   │  │  (rustix)   │  │(windows-sys)│  │  (russh)    │       │
│   └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘       │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Component Interaction

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                              User Code                                        │
│                                  │                                            │
│                                  ▼                                            │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │                           Session                                        │ │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐    │ │
│  │  │   Config    │  │   Buffer    │  │   State     │  │   Logger    │    │ │
│  │  │  (timeout,  │  │  (ring buf, │  │  (running,  │  │ (transcript,│    │ │
│  │  │   env, etc) │  │   encoding) │  │   exited)   │  │   tracing)  │    │ │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘    │ │
│  │         │                │                │                │            │ │
│  │         └────────────────┴────────────────┴────────────────┘            │ │
│  │                                  │                                       │ │
│  │                                  ▼                                       │ │
│  │  ┌─────────────────────────────────────────────────────────────────────┐│ │
│  │  │                        Backend Trait                                ││ │
│  │  │  impl SessionBackend for PtyBackend                                 ││ │
│  │  │  impl SessionBackend for SshBackend                                 ││ │
│  │  └─────────────────────────────────────────────────────────────────────┘│ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                  │                                            │
│                                  ▼                                            │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │                           rust-pty                                       │ │
│  │                                                                          │ │
│  │   ┌─────────────────────────────────────────────────────────────────┐   │ │
│  │   │                      PtySystem trait                             │   │ │
│  │   │  fn spawn(config) -> (PtyMaster, PtyChild)                       │   │ │
│  │   └─────────────────────────────────────────────────────────────────┘   │ │
│  │                        │                   │                             │ │
│  │            ┌───────────┴───────────┐       │                             │ │
│  │            ▼                       ▼       ▼                             │ │
│  │   ┌─────────────────┐    ┌─────────────────────┐                        │ │
│  │   │   UnixPty       │    │    WindowsPty       │                        │ │
│  │   │  (Linux/macOS)  │    │    (ConPTY)         │                        │ │
│  │   │                 │    │                     │                        │ │
│  │   │  rustix         │    │  windows-sys        │                        │ │
│  │   │  AsyncFd        │    │  Thread/Overlapped  │                        │ │
│  │   │  signal-hook    │    │  Job Objects        │                        │ │
│  │   └─────────────────┘    └─────────────────────┘                        │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. rust-pty Crate

### 5.1 Purpose

`rust-pty` provides a cross-platform async PTY abstraction. It is published as a standalone crate because:

1. No existing crate provides async + cross-platform PTY support
2. Other projects (terminal emulators, automation tools) can benefit
3. Clean separation of concerns between PTY mechanics and expect logic

### 5.2 Core Traits

```rust
// crates/rust-pty/src/traits.rs

use std::future::Future;
use std::io::Result;
use std::path::PathBuf;
use std::process::ExitStatus;
use tokio::io::{AsyncRead, AsyncWrite};

/// Configuration for spawning a PTY
#[derive(Debug, Clone)]
pub struct PtyConfig {
    /// Command to execute
    pub command: String,
    /// Command arguments
    pub args: Vec<String>,
    /// Environment variables (in addition to inherited)
    pub env: Vec<(String, String)>,
    /// Whether to inherit parent environment
    pub inherit_env: bool,
    /// Working directory for child process
    pub working_dir: Option<PathBuf>,
    /// Initial terminal dimensions (columns, rows)
    pub dimensions: (u16, u16),
    /// TERM environment variable value
    pub term: String,
}

impl Default for PtyConfig {
    fn default() -> Self {
        Self {
            command: String::new(),
            args: Vec::new(),
            env: Vec::new(),
            inherit_env: true,
            working_dir: None,
            dimensions: (80, 24),
            term: "xterm-256color".into(),
        }
    }
}

/// Signals that can be sent to a PTY child
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PtySignal {
    /// SIGINT / Ctrl+C
    Interrupt,
    /// SIGTERM / graceful shutdown request
    Terminate,
    /// SIGKILL / TerminateProcess (immediate, cannot be caught)
    Kill,
    /// SIGHUP (Unix only, no-op on Windows)
    Hangup,
    /// SIGWINCH (handled internally on resize, exposed for completeness)
    WindowChange,
}

/// Handle to the master side of a PTY
///
/// Implements `AsyncRead` and `AsyncWrite` for seamless tokio integration.
/// The master side is where we read output from and write input to the child.
pub trait PtyMaster: AsyncRead + AsyncWrite + Send + Sync + Unpin {
    /// Resize the PTY to the specified dimensions
    ///
    /// On Unix, this sends SIGWINCH to the child process group.
    /// On Windows, this calls `ResizePseudoConsole`.
    fn resize(&self, cols: u16, rows: u16) -> impl Future<Output = Result<()>> + Send;

    /// Get the current PTY dimensions
    fn dimensions(&self) -> (u16, u16);
}

/// Handle to the child process spawned in the PTY
pub trait PtyChild: Send + Sync {
    /// Check if the child process is still running
    fn is_running(&self) -> bool;

    /// Wait for the child process to exit
    ///
    /// Returns immediately if the child has already exited.
    fn wait(&mut self) -> impl Future<Output = Result<ExitStatus>> + Send;

    /// Send a signal to the child process
    ///
    /// On Windows, only `Interrupt`, `Terminate`, and `Kill` are supported.
    /// Other signals are no-ops on Windows.
    fn signal(&self, signal: PtySignal) -> Result<()>;

    /// Forcefully kill the child process
    ///
    /// Equivalent to `signal(PtySignal::Kill)` but ensures termination.
    fn kill(&mut self) -> Result<()>;

    /// Get the child's process ID
    fn pid(&self) -> u32;
}

/// Factory for creating PTY instances
///
/// Different implementations provide platform-specific behavior.
pub trait PtySystem: Send + Sync {
    /// The master type produced by this system
    type Master: PtyMaster;
    /// The child type produced by this system
    type Child: PtyChild;

    /// Spawn a new process in a PTY
    ///
    /// Returns handles to both the master (I/O) and child (process control).
    fn spawn(
        &self,
        config: PtyConfig,
    ) -> impl Future<Output = Result<(Self::Master, Self::Child)>> + Send;
}
```

### 5.3 Unix Backend Architecture

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                          Unix PTY Backend                                     │
│                                                                               │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │                         UnixPtySystem                                    │ │
│  │  impl PtySystem                                                          │ │
│  │                                                                          │ │
│  │  spawn() workflow:                                                       │ │
│  │  1. posix_openpt() → master_fd                                          │ │
│  │  2. grantpt(), unlockpt()                                               │ │
│  │  3. ptsname() → slave_path                                              │ │
│  │  4. fork()                                                               │ │
│  │     ├── Parent: close slave_fd, return master                           │ │
│  │     └── Child: setsid(), open slave as controlling terminal             │ │
│  │              dup2 to stdin/stdout/stderr, execvp command                │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                     │                                         │
│                    ┌────────────────┴────────────────┐                       │
│                    ▼                                  ▼                       │
│  ┌────────────────────────────────┐  ┌────────────────────────────────────┐ │
│  │        UnixPtyMaster           │  │        UnixPtyChild                │ │
│  │                                │  │                                    │ │
│  │  master_fd: OwnedFd            │  │  pid: Pid                          │ │
│  │  async_fd: AsyncFd<OwnedFd>    │  │  status: Option<ExitStatus>        │ │
│  │  dimensions: (u16, u16)        │  │                                    │ │
│  │                                │  │  signal():                         │ │
│  │  AsyncRead/AsyncWrite via      │  │    kill(pid, sig) via rustix       │ │
│  │  tokio's AsyncFd               │  │                                    │ │
│  │                                │  │  wait():                           │ │
│  │  resize():                     │  │    waitpid() via rustix            │ │
│  │    ioctl(TIOCSWINSZ)           │  │    or signal-hook for SIGCHLD      │ │
│  └────────────────────────────────┘  └────────────────────────────────────┘ │
│                                                                               │
│  Signal Handling (via signal-hook):                                          │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │  SIGCHLD → notify waiters that child state changed                       │ │
│  │  SIGWINCH → (optional) propagate to PTY if auto-resize enabled          │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────┘
```

#### 5.3.1 Unix PTY Syscalls via rustix

```rust
// Pseudocode for Unix PTY allocation

use rustix::pty::{openpt, grantpt, unlockpt, ptsname};
use rustix::termios::{tcgetattr, tcsetattr, Termios, OptionalActions};
use rustix::io::{dup2, close};
use rustix::process::{fork, setsid, ForkResult, Pid};

fn spawn_unix(config: &PtyConfig) -> Result<(UnixPtyMaster, UnixPtyChild)> {
    // 1. Open master PTY
    let master_fd = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY)?;

    // 2. Grant and unlock slave
    grantpt(&master_fd)?;
    unlockpt(&master_fd)?;

    // 3. Get slave path
    let slave_path = ptsname(&master_fd, /* buffer */)?;

    // 4. Set initial dimensions
    set_window_size(&master_fd, config.dimensions.0, config.dimensions.1)?;

    // 5. Fork
    match unsafe { fork()? } {
        ForkResult::Parent { child_pid } => {
            // Parent: wrap master in async and return
            let async_fd = AsyncFd::new(master_fd)?;
            Ok((
                UnixPtyMaster { async_fd, dimensions: config.dimensions },
                UnixPtyChild { pid: child_pid, status: None },
            ))
        }
        ForkResult::Child => {
            // Child: become session leader, open slave as controlling terminal
            setsid()?;
            let slave_fd = open(&slave_path, OFlags::RDWR)?;

            // Make slave the controlling terminal
            ioctl_tiocsctty(&slave_fd)?;

            // Redirect stdin/stdout/stderr
            dup2(&slave_fd, 0)?; // stdin
            dup2(&slave_fd, 1)?; // stdout
            dup2(&slave_fd, 2)?; // stderr
            close(slave_fd)?;

            // Set up environment
            for (key, value) in &config.env {
                std::env::set_var(key, value);
            }
            std::env::set_var("TERM", &config.term);

            // Change directory if specified
            if let Some(dir) = &config.working_dir {
                std::env::set_current_dir(dir)?;
            }

            // Execute command
            exec(&config.command, &config.args)?;
            unreachable!()
        }
    }
}
```

### 5.4 Windows Backend Architecture

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                        Windows ConPTY Backend                                 │
│                                                                               │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │                       WindowsPtySystem                                   │ │
│  │  impl PtySystem                                                          │ │
│  │                                                                          │ │
│  │  spawn() workflow:                                                       │ │
│  │  1. CreatePipe() × 2 → (stdin_read, stdin_write), (stdout_read, write)  │ │
│  │  2. CreatePseudoConsole(dims, stdin_read, stdout_write) → hPC           │ │
│  │  3. InitializeProcThreadAttributeList() with PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE │
│  │  4. CreateProcess() with attribute list                                 │ │
│  │  5. AssignProcessToJobObject() for process tree management              │ │
│  │  6. Create async adapter for pipe I/O                                   │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                     │                                         │
│                    ┌────────────────┴────────────────┐                       │
│                    ▼                                  ▼                       │
│  ┌────────────────────────────────────┐  ┌──────────────────────────────────┐│
│  │        WindowsPtyMaster            │  │      WindowsPtyChild             ││
│  │                                    │  │                                  ││
│  │  hpc: HPCON                        │  │  process_handle: HANDLE          ││
│  │  stdin_write: HANDLE               │  │  job_handle: HANDLE              ││
│  │  stdout_read: HANDLE               │  │  pid: u32                        ││
│  │  async_adapter: AsyncAdapter       │  │                                  ││
│  │  dimensions: (u16, u16)            │  │  signal():                       ││
│  │                                    │  │    GenerateConsoleCtrlEvent()    ││
│  │  AsyncRead/Write via adapter       │  │    or TerminateProcess()         ││
│  │                                    │  │                                  ││
│  │  resize():                         │  │  wait():                         ││
│  │    ResizePseudoConsole()           │  │    WaitForSingleObject()         ││
│  └────────────────────────────────────┘  └──────────────────────────────────┘│
│                                                                               │
│  Async Adapter (runtime-selected):                                           │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │                                                                          │ │
│  │  ┌─────────────────────────────┐  ┌─────────────────────────────────┐   │ │
│  │  │  Thread-Per-Pipe (Current)  │  │  Overlapped I/O (Future)        │   │ │
│  │  │  (All current Windows)      │  │  (Windows 26H2+, unconfirmed)   │   │ │
│  │  │                             │  │                                  │   │ │
│  │  │  read_thread:               │  │  overlapped_read:               │   │ │
│  │  │    loop { ReadFile() }      │  │    ReadFile(OVERLAPPED)         │   │ │
│  │  │    send to channel          │  │    GetOverlappedResult()        │   │ │
│  │  │                             │  │                                  │   │ │
│  │  │  write_thread:              │  │  overlapped_write:              │   │ │
│  │  │    recv from channel        │  │    WriteFile(OVERLAPPED)        │   │ │
│  │  │    WriteFile()              │  │    GetOverlappedResult()        │   │ │
│  │  │                             │  │                                  │   │ │
│  │  │  Channel ←→ tokio task      │  │  Direct tokio integration       │   │ │
│  │  └─────────────────────────────┘  └─────────────────────────────────┘   │ │
│  │                                                                          │ │
│  │  NOTE: ConPTY overlapped I/O (PR #17510) merged Aug 2024 but NOT        │ │
│  │  shipped in any Windows release including 24H2/25H2. Expected 26H2+.    │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                                                               │
│  Job Object Management:                                                       │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │  CreateJobObject() with JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE              │ │
│  │  → All child processes terminated when job handle closed                │ │
│  │  → Prevents orphaned child processes                                    │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────┘
```

#### 5.4.1 Windows Version Detection and Async Strategy

```rust
// Runtime Windows version detection for async strategy selection
//
// IMPORTANT: As of December 2025, ConPTY overlapped I/O is NOT available
// in any released Windows version, including Windows 11 24H2 and 25H2.
//
// Background:
// - PR #17510 (microsoft/terminal) added overlapped I/O support in August 2024
// - However, this was AFTER the feature cutoff for Windows 11 24H2 (build 26100)
// - Windows 11 25H2 (build 26200) is an enablement package over 24H2,
//   sharing the same kernel base - it does NOT include new ConPTY features
// - Expected availability: Windows 26H2 or a future major Windows release
//
// Reference: https://github.com/microsoft/terminal/discussions/19112

#[cfg(windows)]
fn supports_overlapped_conpty() -> bool {
    // Conservative default: return false until overlapped I/O is confirmed
    // in a released Windows version. When Microsoft ships this feature,
    // update this function with proper version detection.
    //
    // Future implementation (when available):
    // - Detect Windows version >= 26H2 or specific build number
    // - Or probe ConPTY capability directly via CreatePseudoConsole flags
    false
}

#[cfg(windows)]
enum AsyncAdapter {
    /// Thread-per-pipe pattern (current default for all Windows versions)
    ThreadPerPipe(ThreadPipeAdapter),
    /// Overlapped I/O pattern (reserved for future Windows releases)
    Overlapped(OverlappedAdapter),
}

#[cfg(windows)]
impl AsyncAdapter {
    fn new(stdin_write: HANDLE, stdout_read: HANDLE) -> Self {
        if supports_overlapped_conpty() {
            AsyncAdapter::Overlapped(OverlappedAdapter::new(stdin_write, stdout_read))
        } else {
            // Thread-per-pipe is required for ALL current Windows versions
            AsyncAdapter::ThreadPerPipe(ThreadPipeAdapter::new(stdin_write, stdout_read))
        }
    }
}
```

#### 5.4.2 Thread-Per-Pipe Pattern

```rust
// Thread-per-pipe async adapter for all current Windows versions
// This is the ONLY supported pattern until ConPTY overlapped I/O ships.
//
// SCALABILITY NOTE: This pattern creates 2 threads per session (read + write).
// For 50+ concurrent sessions, this means 100+ threads. For high-concurrency
// scenarios on current Windows, consider implementing a shared thread pool
// with work-stealing (planned for post-1.0 optimization).

use std::sync::mpsc;
use tokio::sync::mpsc as tokio_mpsc;

struct ThreadPipeAdapter {
    /// Sender for write requests
    write_tx: tokio_mpsc::Sender<Vec<u8>>,
    /// Receiver for read data
    read_rx: tokio_mpsc::Receiver<Vec<u8>>,
    /// Handles for cleanup
    read_thread: Option<std::thread::JoinHandle<()>>,
    write_thread: Option<std::thread::JoinHandle<()>>,
}

impl ThreadPipeAdapter {
    fn new(stdin_write: HANDLE, stdout_read: HANDLE) -> Self {
        let (write_tx, mut write_rx) = tokio_mpsc::channel::<Vec<u8>>(64);
        let (read_tx, read_rx) = tokio_mpsc::channel::<Vec<u8>>(64);

        // Read thread: blocking ReadFile → channel
        let read_thread = std::thread::spawn(move || {
            let mut buffer = [0u8; 4096];
            loop {
                let mut bytes_read = 0u32;
                let result = unsafe {
                    ReadFile(
                        stdout_read,
                        buffer.as_mut_ptr().cast(),
                        buffer.len() as u32,
                        &mut bytes_read,
                        std::ptr::null_mut(),
                    )
                };

                if result == 0 || bytes_read == 0 {
                    break; // Pipe closed or error
                }

                if read_tx.blocking_send(buffer[..bytes_read as usize].to_vec()).is_err() {
                    break; // Receiver dropped
                }
            }
        });

        // Write thread: channel → blocking WriteFile
        let write_thread = std::thread::spawn(move || {
            while let Some(data) = write_rx.blocking_recv() {
                let mut bytes_written = 0u32;
                unsafe {
                    WriteFile(
                        stdin_write,
                        data.as_ptr().cast(),
                        data.len() as u32,
                        &mut bytes_written,
                        std::ptr::null_mut(),
                    );
                }
            }
        });

        Self {
            write_tx,
            read_rx,
            read_thread: Some(read_thread),
            write_thread: Some(write_thread),
        }
    }
}

impl AsyncRead for ThreadPipeAdapter {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.read_rx.poll_recv(cx) {
            Poll::Ready(Some(data)) => {
                buf.put_slice(&data);
                Poll::Ready(Ok(()))
            }
            Poll::Ready(None) => Poll::Ready(Ok(())), // EOF
            Poll::Pending => Poll::Pending,
        }
    }
}

impl AsyncWrite for ThreadPipeAdapter {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.write_tx.try_send(buf.to_vec()) {
            Ok(()) => Poll::Ready(Ok(buf.len())),
            Err(tokio_mpsc::error::TrySendError::Full(_)) => Poll::Pending,
            Err(tokio_mpsc::error::TrySendError::Closed(_)) => {
                Poll::Ready(Err(io::Error::new(io::ErrorKind::BrokenPipe, "write pipe closed")))
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(())) // Writes are immediate
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}
```

**Scalability Note:** The thread-per-pipe pattern creates 2 threads per session (one for read, one for write). For applications managing many concurrent sessions (50+), a shared thread pool with work-stealing would be more efficient. This optimization is deferred to post-1.0 and can be implemented without API changes.

### 5.5 Error Types

```rust
// crates/rust-pty/src/error.rs

use thiserror::Error;
use std::io;

#[derive(Error, Debug)]
pub enum PtyError {
    #[error("failed to allocate PTY: {0}")]
    Allocation(#[source] io::Error),

    #[error("failed to spawn process: {0}")]
    Spawn(#[source] io::Error),

    #[error("failed to read from PTY: {0}")]
    Read(#[source] io::Error),

    #[error("failed to write to PTY: {0}")]
    Write(#[source] io::Error),

    #[error("failed to resize PTY: {0}")]
    Resize(#[source] io::Error),

    #[error("failed to send signal: {0}")]
    Signal(#[source] io::Error),

    #[error("child process exited unexpectedly with status {0}")]
    ChildExited(i32),

    #[error("operation timed out after {0:?}")]
    Timeout(std::time::Duration),

    #[error("PTY was closed")]
    Closed,

    #[cfg(windows)]
    #[error("Windows error: {0}")]
    Windows(#[from] windows_sys::core::Error),
}

impl From<PtyError> for io::Error {
    fn from(err: PtyError) -> Self {
        match err {
            PtyError::Allocation(e) | PtyError::Spawn(e) |
            PtyError::Read(e) | PtyError::Write(e) |
            PtyError::Resize(e) | PtyError::Signal(e) => e,
            PtyError::ChildExited(code) => {
                io::Error::new(io::ErrorKind::Other, format!("child exited: {code}"))
            }
            PtyError::Timeout(d) => {
                io::Error::new(io::ErrorKind::TimedOut, format!("timeout after {d:?}"))
            }
            PtyError::Closed => {
                io::Error::new(io::ErrorKind::NotConnected, "PTY closed")
            }
            #[cfg(windows)]
            PtyError::Windows(e) => io::Error::from_raw_os_error(e.code() as i32),
        }
    }
}
```

---

## 6. rust-expect Crate

### 6.1 Session Architecture

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                              Session<B: Backend>                              │
│                                                                               │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │                         SessionInner                                     │ │
│  │                                                                          │ │
│  │  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────┐  │ │
│  │  │     Backend     │  │     Buffer      │  │        Config           │  │ │
│  │  │  (PtyBackend or │  │  (ring buffer)  │  │  - timeout              │  │ │
│  │  │   SshBackend)   │  │  - max_size     │  │  - delaybeforesend      │  │ │
│  │  │                 │  │  - search_win   │  │  - line_ending          │  │ │
│  │  │  AsyncRead +    │  │  - encoding     │  │  - echo                 │  │ │
│  │  │  AsyncWrite     │  │                 │  │                         │  │ │
│  │  └─────────────────┘  └─────────────────┘  └─────────────────────────┘  │ │
│  │                                                                          │ │
│  │  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────┐  │ │
│  │  │   Persistent    │  │    Transcript   │  │        State            │  │ │
│  │  │    Patterns     │  │     Logger      │  │  - running: bool        │  │ │
│  │  │  - before: Vec  │  │  - file         │  │  - exit_status: Option  │  │ │
│  │  │  - after: Vec   │  │  - format       │  │  - last_match           │  │ │
│  │  │                 │  │  - redactions   │  │                         │  │ │
│  │  └─────────────────┘  └─────────────────┘  └─────────────────────────┘  │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                                                               │
│  Public API:                                                                  │
│  ├── expect(pattern) → MatchResult                                           │
│  ├── expect_any(&[Pattern]) → MatchResult                                    │
│  ├── send(data) → Result<()>                                                 │
│  ├── send_line(text) → Result<()>                                            │
│  ├── send_control(char) → Result<()>                                         │
│  ├── send_slow(text, delay) → Result<()>                                     │
│  ├── interact() → InteractHandle                                             │
│  ├── buffer() → &str                                                         │
│  ├── clear_buffer()                                                          │
│  ├── resize(cols, rows) → Result<()>                                         │
│  ├── is_running() → bool                                                     │
│  ├── wait() → Result<ExitStatus>                                             │
│  └── kill() → Result<()>                                                     │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Session Builder

```rust
// crates/rust-expect/src/session/builder.rs

use std::path::PathBuf;
use std::time::Duration;

pub struct SessionBuilder {
    command: Option<String>,
    args: Vec<String>,
    env: Vec<(String, String)>,
    inherit_env: bool,
    working_dir: Option<PathBuf>,
    dimensions: (u16, u16),
    term: String,
    timeout: Duration,
    buffer_size: usize,
    search_window: Option<usize>,
    delay_before_send: Duration,
    line_ending: LineEnding,
    log_file: Option<PathBuf>,
    log_user: bool,
}

impl SessionBuilder {
    pub fn new() -> Self {
        Self {
            command: None,
            args: Vec::new(),
            env: Vec::new(),
            inherit_env: true,
            working_dir: None,
            dimensions: (80, 24),
            term: "xterm-256color".into(),
            timeout: Duration::from_secs(30),
            buffer_size: 100 * 1024 * 1024, // 100 MB
            search_window: None,
            delay_before_send: Duration::from_millis(50),
            line_ending: LineEnding::Lf,
            log_file: None,
            log_user: false,
        }
    }

    pub fn command(mut self, cmd: impl Into<String>) -> Self {
        self.command = Some(cmd.into());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    pub fn dimensions(mut self, cols: u16, rows: u16) -> Self {
        self.dimensions = (cols, rows);
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    pub fn search_window(mut self, size: usize) -> Self {
        self.search_window = Some(size);
        self
    }

    pub fn log_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.log_file = Some(path.into());
        self
    }

    pub fn log_user(mut self, enable: bool) -> Self {
        self.log_user = enable;
        self
    }

    pub async fn spawn(self) -> Result<Session<PtyBackend>, Error> {
        let config = PtyConfig {
            command: self.command.ok_or(Error::NoCommand)?,
            args: self.args,
            env: self.env,
            inherit_env: self.inherit_env,
            working_dir: self.working_dir,
            dimensions: self.dimensions,
            term: self.term,
        };

        let pty_system = native_pty_system();
        let (master, child) = pty_system.spawn(config).await?;

        Ok(Session::new(
            PtyBackend::new(master, child),
            SessionConfig {
                timeout: self.timeout,
                buffer_size: self.buffer_size,
                search_window: self.search_window,
                delay_before_send: self.delay_before_send,
                line_ending: self.line_ending,
                log_file: self.log_file,
                log_user: self.log_user,
            },
        ))
    }
}

// Convenience function
impl Session<PtyBackend> {
    pub fn builder() -> SessionBuilder {
        SessionBuilder::new()
    }
}
```

### 6.3 Pattern Matching Engine

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                          Pattern Matching Engine                              │
│                                                                               │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │                            Pattern                                       │ │
│  │                                                                          │ │
│  │  enum Pattern {                                                          │ │
│  │      Exact(String),           // Exact string match                      │ │
│  │      Regex(Regex),            // Regex pattern                           │ │
│  │      Glob(GlobPattern),       // Glob pattern                            │ │
│  │      Eof,                     // End of output                           │ │
│  │      Timeout(Duration),       // Timeout trigger                         │ │
│  │      NBytes(usize),           // N bytes received                        │ │
│  │      Any(Vec<Pattern>),       // First of multiple patterns              │ │
│  │      All(Vec<Pattern>),       // All patterns (any order)                │ │
│  │  }                                                                       │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                     │                                         │
│                                     ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │                           Matcher                                        │ │
│  │                                                                          │ │
│  │  struct Matcher {                                                        │ │
│  │      patterns: Vec<Pattern>,                                             │ │
│  │      before_patterns: Vec<Pattern>,  // expect_before                    │ │
│  │      after_patterns: Vec<Pattern>,   // expect_after                     │ │
│  │      search_window: Option<usize>,   // Performance optimization         │ │
│  │      regex_cache: RegexCache,        // Compiled regex cache             │ │
│  │  }                                                                       │ │
│  │                                                                          │ │
│  │  Methods:                                                                │ │
│  │  ├── try_match(&buffer) → Option<MatchResult>                           │ │
│  │  ├── wait_match(stream, timeout) → MatchResult                          │ │
│  │  └── set_continue(bool)  // exp_continue behavior                       │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                     │                                         │
│                                     ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │                          MatchResult                                     │ │
│  │                                                                          │ │
│  │  struct MatchResult {                                                    │ │
│  │      pattern_index: usize,       // Which pattern matched                │ │
│  │      matched_text: String,       // The text that matched                │ │
│  │      before_match: String,       // Text before the match                │ │
│  │      captures: Vec<String>,      // Regex capture groups                 │ │
│  │      position: Range<usize>,     // Position in buffer                   │ │
│  │  }                                                                       │ │
│  │                                                                          │ │
│  │  enum MatchOutcome {                                                     │ │
│  │      Matched(MatchResult),                                               │ │
│  │      Eof { buffer: String },                                             │ │
│  │      Timeout { buffer: String, duration: Duration },                     │ │
│  │  }                                                                       │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────┘
```

#### 6.3.1 Streaming Pattern Match Algorithm

```rust
// Streaming pattern matching to handle large outputs efficiently

impl Matcher {
    /// Attempt to match patterns against the buffer
    /// Returns immediately if a match is found
    pub fn try_match(&self, buffer: &[u8]) -> Option<MatchResult> {
        // Apply search window optimization
        let search_slice = if let Some(window) = self.search_window {
            let start = buffer.len().saturating_sub(window);
            &buffer[start..]
        } else {
            buffer
        };

        // Check before_patterns first (global patterns)
        for (idx, pattern) in self.before_patterns.iter().enumerate() {
            if let Some(m) = pattern.try_match(search_slice) {
                return Some(MatchResult {
                    pattern_index: idx,
                    pattern_type: PatternType::Before,
                    ..m
                });
            }
        }

        // Check main patterns
        for (idx, pattern) in self.patterns.iter().enumerate() {
            if let Some(m) = pattern.try_match(search_slice) {
                return Some(MatchResult {
                    pattern_index: idx,
                    pattern_type: PatternType::Main,
                    ..m
                });
            }
        }

        // Check after_patterns last
        for (idx, pattern) in self.after_patterns.iter().enumerate() {
            if let Some(m) = pattern.try_match(search_slice) {
                return Some(MatchResult {
                    pattern_index: idx,
                    pattern_type: PatternType::After,
                    ..m
                });
            }
        }

        None
    }

    /// Wait for a pattern match with timeout
    /// Reads from stream incrementally to avoid buffering entire output
    pub async fn wait_match<S: AsyncRead + Unpin>(
        &self,
        stream: &mut S,
        buffer: &mut Buffer,
        timeout: Duration,
    ) -> Result<MatchOutcome, Error> {
        let deadline = Instant::now() + timeout;
        let mut read_buf = [0u8; 4096];

        loop {
            // Check for match before reading more
            if let Some(result) = self.try_match(buffer.as_bytes()) {
                // Consume matched portion from buffer
                buffer.consume(result.position.end);
                return Ok(MatchOutcome::Matched(result));
            }

            // Calculate remaining timeout
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(MatchOutcome::Timeout {
                    buffer: buffer.to_string(),
                    duration: timeout,
                });
            }

            // Read with timeout
            match tokio::time::timeout(remaining, stream.read(&mut read_buf)).await {
                Ok(Ok(0)) => {
                    // EOF
                    return Ok(MatchOutcome::Eof {
                        buffer: buffer.to_string(),
                    });
                }
                Ok(Ok(n)) => {
                    buffer.extend(&read_buf[..n]);
                }
                Ok(Err(e)) => return Err(Error::Io(e)),
                Err(_) => {
                    return Ok(MatchOutcome::Timeout {
                        buffer: buffer.to_string(),
                        duration: timeout,
                    });
                }
            }
        }
    }
}
```

#### 6.3.2 Regex Compilation Cache

```rust
// crates/rust-expect/src/expect/cache.rs

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use regex::Regex;

/// Thread-safe cache for compiled regex patterns
///
/// Regex compilation is expensive (~1ms for simple patterns, more for complex).
/// This cache prevents recompilation when the same pattern is used multiple times.
pub struct RegexCache {
    cache: RwLock<HashMap<String, Arc<Regex>>>,
    max_entries: usize,
}

impl RegexCache {
    /// Create a new cache with default capacity (1000 patterns)
    pub fn new() -> Self {
        Self::with_capacity(1000)
    }

    pub fn with_capacity(max_entries: usize) -> Self {
        Self {
            cache: RwLock::new(HashMap::with_capacity(max_entries / 4)),
            max_entries,
        }
    }

    /// Get a compiled regex, compiling and caching if not present
    pub fn get_or_compile(&self, pattern: &str) -> Result<Arc<Regex>, regex::Error> {
        // Try read lock first (fast path)
        if let Some(regex) = self.cache.read().unwrap().get(pattern) {
            return Ok(Arc::clone(regex));
        }

        // Compile and cache (slow path)
        let regex = Arc::new(Regex::new(pattern)?);

        let mut cache = self.cache.write().unwrap();

        // Evict if at capacity (simple LRU-ish: just clear half)
        if cache.len() >= self.max_entries {
            let to_remove: Vec<_> = cache.keys()
                .take(self.max_entries / 2)
                .cloned()
                .collect();
            for key in to_remove {
                cache.remove(&key);
            }
        }

        cache.insert(pattern.to_string(), Arc::clone(&regex));
        Ok(regex)
    }

    /// Clear the cache
    pub fn clear(&self) {
        self.cache.write().unwrap().clear();
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let cache = self.cache.read().unwrap();
        CacheStats {
            entries: cache.len(),
            capacity: self.max_entries,
        }
    }
}

/// Default global cache for pattern matching
static GLOBAL_REGEX_CACHE: std::sync::OnceLock<RegexCache> = std::sync::OnceLock::new();

pub fn global_regex_cache() -> &'static RegexCache {
    GLOBAL_REGEX_CACHE.get_or_init(RegexCache::new)
}
```

**Cache Performance Characteristics:**

| Operation | Without Cache | With Cache (hit) | Notes |
|-----------|---------------|------------------|-------|
| Regex compile | ~1-5ms | ~200ns (Arc clone) | 5000x improvement on cache hit |
| Pattern match | ~7µs | ~7µs | Match time unchanged |
| Memory overhead | 0 | ~1KB per pattern | Acceptable for most workloads |

The cache is thread-safe and uses a simple eviction strategy (clear half when full) that is appropriate for expect automation workloads where pattern diversity is typically bounded.

### 6.4 Buffer Management

```rust
// crates/rust-expect/src/expect/buffer.rs

use std::collections::VecDeque;

/// Ring buffer for session output with configurable max size
pub struct Buffer {
    /// Raw bytes stored
    data: VecDeque<u8>,
    /// Maximum size before oldest data is discarded
    max_size: usize,
    /// Bytes discarded due to overflow
    overflow_count: usize,
    /// Encoding for text conversion
    encoding: Encoding,
}

impl Buffer {
    pub fn new(max_size: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(max_size.min(64 * 1024)),
            max_size,
            overflow_count: 0,
            encoding: Encoding::Utf8,
        }
    }

    /// Append bytes, discarding oldest if over max_size
    pub fn extend(&mut self, bytes: &[u8]) {
        // If new data alone exceeds max, only keep the tail
        if bytes.len() >= self.max_size {
            self.data.clear();
            let start = bytes.len() - self.max_size;
            self.data.extend(&bytes[start..]);
            self.overflow_count += start;
            return;
        }

        // Remove old data if necessary
        let needed = (self.data.len() + bytes.len()).saturating_sub(self.max_size);
        if needed > 0 {
            self.data.drain(..needed);
            self.overflow_count += needed;
        }

        self.data.extend(bytes);
    }

    /// Consume bytes from the front (after a match)
    pub fn consume(&mut self, count: usize) {
        self.data.drain(..count.min(self.data.len()));
    }

    /// Get buffer as bytes
    pub fn as_bytes(&self) -> &[u8] {
        // VecDeque may be non-contiguous; make contiguous for matching
        self.data.make_contiguous()
    }

    /// Convert to string using configured encoding
    pub fn to_string(&self) -> String {
        match self.encoding {
            Encoding::Utf8 => {
                String::from_utf8_lossy(self.as_bytes()).into_owned()
            }
            // Other encodings would go here
        }
    }

    /// Clear all buffered data
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Number of bytes currently buffered
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Number of bytes discarded due to overflow
    pub fn overflow_count(&self) -> usize {
        self.overflow_count
    }
}

#[derive(Clone, Copy)]
pub enum Encoding {
    Utf8,
    Ascii,
    Latin1,
    // Add more as needed via feature flag
}
```

**Performance Notes for Large Buffers:**

1. **`VecDeque::make_contiguous()`**: For buffers exceeding ~10MB, this operation may cause performance degradation due to memory copies. For very large output scenarios (100MB+), consider:
   - Using the `search_window` option to limit pattern matching to recent output
   - Enabling discard mode for non-critical output
   - See "Large Buffer Strategy" below for 100MB+ handling

2. **Regex crate weight**: The full `regex` crate adds ~1MB to binary size. For simple patterns (exact string, prefix/suffix), the library uses optimized fast paths. For applications where binary size is critical, a future `regex-lite` feature flag may provide a lighter alternative.

3. **PTY buffer tuning**: OS-level PTY buffer sizes (typically 4KB-64KB) can impact throughput. The library does not currently tune these; high-throughput applications may benefit from platform-specific tuning.

#### 6.4.1 Large Buffer Strategy (100MB+ to 1GB Target)

For automation scenarios requiring capture of very large outputs (e.g., log dumps, database exports), the standard `VecDeque` buffer becomes inefficient. The library provides tiered buffer strategies:

```rust
// crates/rust-expect/src/expect/large_buffer.rs

/// Buffer strategy selection based on expected output size
pub enum BufferStrategy {
    /// Standard VecDeque-based buffer (default, optimal for <10MB)
    Standard(Buffer),
    /// Memory-mapped file backing for large outputs (10MB-1GB)
    MemoryMapped(MmapBuffer),
    /// Streaming with no retention (for unbounded output)
    Streaming(StreamingBuffer),
}

/// Memory-mapped buffer for large output handling
/// Uses temporary file backing to avoid heap pressure
pub struct MmapBuffer {
    /// Memory-mapped region
    mmap: memmap2::MmapMut,
    /// Current write position
    write_pos: usize,
    /// Current search start position
    search_start: usize,
    /// Total capacity
    capacity: usize,
    /// Backing file (kept open)
    _file: std::fs::File,
}

impl MmapBuffer {
    /// Create a new memory-mapped buffer with specified capacity
    pub fn new(capacity: usize) -> std::io::Result<Self> {
        use std::io::Write;

        // Create temporary file
        let file = tempfile::tempfile()?;
        file.set_len(capacity as u64)?;

        // Memory-map the file
        let mmap = unsafe { memmap2::MmapMut::map_mut(&file)? };

        Ok(Self {
            mmap,
            write_pos: 0,
            search_start: 0,
            capacity,
            _file: file,
        })
    }

    /// Extend buffer with new data
    pub fn extend(&mut self, bytes: &[u8]) {
        let available = self.capacity - self.write_pos;
        if bytes.len() <= available {
            self.mmap[self.write_pos..self.write_pos + bytes.len()]
                .copy_from_slice(bytes);
            self.write_pos += bytes.len();
        } else {
            // Wrap around: discard oldest data
            let shift = bytes.len() - available;
            self.search_start = self.search_start.saturating_sub(shift);
            self.mmap.copy_within(shift..self.write_pos, 0);
            self.write_pos -= shift;
            self.mmap[self.write_pos..self.write_pos + bytes.len()]
                .copy_from_slice(bytes);
            self.write_pos += bytes.len();
        }
    }

    /// Get searchable slice (always contiguous - mmap advantage)
    pub fn as_bytes(&self) -> &[u8] {
        &self.mmap[self.search_start..self.write_pos]
    }
}

/// Automatic strategy selection based on configured max size
impl BufferStrategy {
    pub fn new(max_size: usize) -> Self {
        match max_size {
            0..=10_000_000 => BufferStrategy::Standard(Buffer::new(max_size)),
            _ => match MmapBuffer::new(max_size) {
                Ok(mmap) => BufferStrategy::MemoryMapped(mmap),
                Err(_) => {
                    // Fallback to standard if mmap fails
                    tracing::warn!(
                        max_size,
                        "Failed to create mmap buffer, falling back to VecDeque"
                    );
                    BufferStrategy::Standard(Buffer::new(max_size))
                }
            },
        }
    }
}
```

**Strategy Comparison:**

| Size Range | Strategy | Memory Usage | Pattern Match Latency | Notes |
|------------|----------|--------------|----------------------|-------|
| < 10 MB | `VecDeque` | 1x (heap) | ~7µs | Best for typical automation |
| 10-1000 MB | `MmapBuffer` | 0.5x (file-backed) | ~10µs | Avoids heap pressure; OS manages paging |
| Unbounded | `Streaming` | O(window) | ~7µs | Only retains search window; old data discarded |

**Achieving 1GB Target:**

The 1GB target is achieved through `MmapBuffer`:
1. File-backed storage avoids 1GB heap allocation
2. OS virtual memory system handles paging efficiently
3. `as_bytes()` returns contiguous slice (no `make_contiguous()` overhead)
4. Works across all platforms (Linux tmpfs, Windows temp, macOS)

For outputs exceeding 1GB, use `StreamingBuffer` with `search_window` option.

### 6.5 Interactive Mode

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                           Interactive Mode                                    │
│                                                                               │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │                         InteractBuilder                                  │ │
│  │                                                                          │ │
│  │  session.interact()                                                      │ │
│  │      .escape_char('\x1d')        // Ctrl+]                              │ │
│  │      .on_output(pattern, callback)                                       │ │
│  │      .on_input(pattern, callback)                                        │ │
│  │      .timeout(Duration)                                                  │ │
│  │      .run()                                                              │ │
│  │      .await                                                              │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                     │                                         │
│                                     ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │                          InteractLoop                                    │ │
│  │                                                                          │ │
│  │  ┌─────────────────────┐          ┌─────────────────────┐               │ │
│  │  │    User Terminal    │          │    PTY/SSH Backend  │               │ │
│  │  │    (crossterm)      │          │                     │               │ │
│  │  │                     │          │                     │               │ │
│  │  │   stdin ──────────────────────────────► write        │               │ │
│  │  │   (raw mode)        │    │     │                     │               │ │
│  │  │                     │    │     │                     │               │ │
│  │  │   stdout ◄────────────────────────────── read        │               │ │
│  │  │                     │    │     │                     │               │ │
│  │  └─────────────────────┘    │     └─────────────────────┘               │ │
│  │                             │                                            │ │
│  │                             ▼                                            │ │
│  │                      Hook Processing                                     │ │
│  │                      ├── Input hooks (filter/modify user input)         │ │
│  │                      └── Output hooks (filter/modify process output)    │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                                                               │
│  Terminal State Management:                                                   │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │  1. Save current terminal state (via crossterm)                          │ │
│  │  2. Enter raw mode (disable line buffering, echo)                        │ │
│  │  3. Run interact loop                                                    │ │
│  │  4. Restore terminal state on:                                           │ │
│  │     - Normal exit (escape char or timeout)                               │ │
│  │     - Error                                                              │ │
│  │     - Panic (via Drop guard)                                             │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────┘
```

#### 6.5.1 Async Interact Implementation

```rust
// crates/rust-expect/src/interact/basic.rs

use crossterm::event::{Event, EventStream, KeyCode, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use futures::StreamExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct InteractHandle<'a, B: Backend> {
    session: &'a mut Session<B>,
    escape_char: char,
    output_hooks: Vec<(Pattern, OutputHook)>,
    input_hooks: Vec<(Pattern, InputHook)>,
    timeout: Option<Duration>,
}

impl<'a, B: Backend> InteractHandle<'a, B> {
    pub fn escape_char(mut self, c: char) -> Self {
        self.escape_char = c;
        self
    }

    pub fn on_output<F>(mut self, pattern: impl Into<Pattern>, callback: F) -> Self
    where
        F: Fn(&str) + Send + 'static,
    {
        self.output_hooks.push((pattern.into(), Box::new(callback)));
        self
    }

    pub fn on_input<F>(mut self, pattern: impl Into<Pattern>, callback: F) -> Self
    where
        F: Fn(&str) -> Option<String> + Send + 'static,
    {
        self.input_hooks.push((pattern.into(), Box::new(callback)));
        self
    }

    pub fn timeout(mut self, duration: Duration) -> Self {
        self.timeout = Some(duration);
        self
    }

    pub async fn run(self) -> Result<InteractResult, Error> {
        // Terminal state guard - restores on drop
        let _guard = TerminalGuard::new()?;

        enable_raw_mode()?;

        let mut event_stream = EventStream::new();
        let mut read_buf = [0u8; 1024];
        let deadline = self.timeout.map(|d| Instant::now() + d);

        loop {
            let remaining = deadline.map(|d| d.saturating_duration_since(Instant::now()));
            if remaining.is_some_and(|r| r.is_zero()) {
                return Ok(InteractResult::Timeout);
            }

            tokio::select! {
                // User input from terminal
                event = event_stream.next() => {
                    match event {
                        Some(Ok(Event::Key(key))) => {
                            // Check for escape character
                            if matches!(key.code, KeyCode::Char(c) if c == self.escape_char) {
                                return Ok(InteractResult::Escaped);
                            }

                            // Convert key event to bytes
                            let input = key_to_bytes(&key);

                            // Apply input hooks
                            let input = self.apply_input_hooks(&input);

                            // Send to process
                            if let Some(data) = input {
                                self.session.backend.write_all(&data).await?;
                            }
                        }
                        Some(Ok(Event::Resize(cols, rows))) => {
                            self.session.resize(cols, rows).await?;
                        }
                        Some(Err(e)) => return Err(Error::Terminal(e)),
                        None => return Ok(InteractResult::InputClosed),
                        _ => {}
                    }
                }

                // Output from process
                result = self.session.backend.read(&mut read_buf) => {
                    match result {
                        Ok(0) => return Ok(InteractResult::ProcessExited),
                        Ok(n) => {
                            let output = &read_buf[..n];

                            // Apply output hooks
                            self.apply_output_hooks(output);

                            // Write to user's terminal
                            let mut stdout = tokio::io::stdout();
                            stdout.write_all(output).await?;
                            stdout.flush().await?;
                        }
                        Err(e) => return Err(Error::Io(e)),
                    }
                }

                // Timeout
                _ = async {
                    if let Some(r) = remaining {
                        tokio::time::sleep(r).await;
                    } else {
                        std::future::pending::<()>().await;
                    }
                } => {
                    return Ok(InteractResult::Timeout);
                }
            }
        }
    }
}

/// RAII guard for terminal state
struct TerminalGuard {
    was_raw: bool,
}

impl TerminalGuard {
    fn new() -> Result<Self, Error> {
        Ok(Self { was_raw: crossterm::terminal::is_raw_mode_enabled()? })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if !self.was_raw {
            let _ = disable_raw_mode();
        }
    }
}

pub enum InteractResult {
    Escaped,
    Timeout,
    ProcessExited,
    InputClosed,
}
```

### 6.6 Multi-Session Management

```rust
// crates/rust-expect/src/multi/select.rs

use futures::future::select_all;

/// Wait for the first of multiple sessions to match a pattern
pub async fn select_expect<B: Backend>(
    sessions: &mut [&mut Session<B>],
    pattern: impl Into<Pattern>,
) -> Result<(usize, MatchResult), Error> {
    let pattern = pattern.into();

    let futures: Vec<_> = sessions
        .iter_mut()
        .enumerate()
        .map(|(idx, session)| {
            let pattern = pattern.clone();
            async move {
                let result = session.expect(pattern).await;
                (idx, result)
            }
        })
        .collect();

    let (result, _, _) = select_all(futures).await;
    let (idx, match_result) = result;
    Ok((idx, match_result?))
}

/// Wait for all sessions to match a pattern
pub async fn expect_all<B: Backend>(
    sessions: &mut [&mut Session<B>],
    pattern: impl Into<Pattern>,
) -> Result<Vec<MatchResult>, Error> {
    let pattern = pattern.into();

    let futures: Vec<_> = sessions
        .iter_mut()
        .map(|session| {
            let pattern = pattern.clone();
            session.expect(pattern)
        })
        .collect();

    let results = futures::future::try_join_all(futures).await?;
    Ok(results)
}

/// Session group for managing related sessions
pub struct SessionGroup<B: Backend> {
    sessions: HashMap<String, Session<B>>,
}

impl<B: Backend> SessionGroup<B> {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn add(&mut self, name: impl Into<String>, session: Session<B>) {
        self.sessions.insert(name.into(), session);
    }

    pub fn get(&self, name: &str) -> Option<&Session<B>> {
        self.sessions.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Session<B>> {
        self.sessions.get_mut(name)
    }

    pub fn remove(&mut self, name: &str) -> Option<Session<B>> {
        self.sessions.remove(name)
    }

    /// Send to all sessions in the group
    pub async fn broadcast(&mut self, data: &[u8]) -> Result<(), Error> {
        for session in self.sessions.values_mut() {
            session.send(data).await?;
        }
        Ok(())
    }
}
```

### 6.7 SSH Backend Architecture

The SSH backend provides the same `Session` API over SSH connections, enabling remote automation without a local PTY.

#### 6.7.1 SSH Session Structure

```rust
// crates/rust-expect/src/backend/ssh/session.rs

use russh::{client, Channel, ChannelId, Disconnect};
use russh_keys::key;
use std::sync::Arc;
use tokio::sync::Mutex;

/// SSH session backend implementing the Backend trait
pub struct SshBackend {
    /// SSH client handle
    client: Arc<Mutex<SshClientState>>,
    /// Active channel for the shell session
    channel: Channel<client::Msg>,
    /// Channel ID for tracking
    channel_id: ChannelId,
    /// Terminal dimensions (for SIGWINCH equivalent)
    dimensions: (u16, u16),
}

struct SshClientState {
    handle: client::Handle<SshClientHandler>,
    connected: bool,
}

/// SSH client handler implementing russh callbacks
struct SshClientHandler {
    /// Buffer for incoming data before it's read
    incoming: Arc<Mutex<Vec<u8>>>,
    /// Notification channel for new data
    data_notify: tokio::sync::Notify,
    /// Server's host key (for verification)
    server_key: Option<key::PublicKey>,
}

#[async_trait::async_trait]
impl client::Handler for SshClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &key::PublicKey,
    ) -> Result<bool, Self::Error> {
        // Store for verification; real implementation would check known_hosts
        self.server_key = Some(server_public_key.clone());
        // TODO: Implement known_hosts checking via callback
        Ok(true)
    }

    async fn data(
        &mut self,
        _channel: ChannelId,
        data: &[u8],
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        let mut incoming = self.incoming.lock().await;
        incoming.extend_from_slice(data);
        self.data_notify.notify_one();
        Ok(())
    }

    async fn extended_data(
        &mut self,
        _channel: ChannelId,
        _ext: u32,
        data: &[u8],
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        // Extended data (stderr) - merge with stdout for terminal emulation
        let mut incoming = self.incoming.lock().await;
        incoming.extend_from_slice(data);
        self.data_notify.notify_one();
        Ok(())
    }
}
```

#### 6.7.2 SSH Connection Builder

```rust
// crates/rust-expect/src/backend/ssh/builder.rs

use std::path::PathBuf;
use std::time::Duration;

/// Builder for SSH sessions
pub struct SshSessionBuilder {
    host: String,
    port: u16,
    username: String,
    auth: SshAuth,
    timeout: Duration,
    dimensions: (u16, u16),
    env: HashMap<String, String>,
    known_hosts: KnownHostsPolicy,
}

/// SSH authentication methods
pub enum SshAuth {
    /// Password authentication
    Password(String),
    /// Private key authentication
    PrivateKey {
        path: PathBuf,
        passphrase: Option<String>,
    },
    /// SSH agent authentication
    Agent,
    /// Private key from memory
    PrivateKeyMemory {
        key_pem: String,
        passphrase: Option<String>,
    },
}

/// Known hosts verification policy
pub enum KnownHostsPolicy {
    /// Check against system known_hosts file
    System,
    /// Check against custom known_hosts file
    File(PathBuf),
    /// Accept any host key (INSECURE - for testing only)
    AcceptAll,
    /// Custom verification callback
    Custom(Arc<dyn Fn(&key::PublicKey, &str) -> bool + Send + Sync>),
}

impl SshSessionBuilder {
    pub fn new(host: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            port: 22,
            username: String::new(),
            auth: SshAuth::Agent,
            timeout: Duration::from_secs(30),
            dimensions: (80, 24),
            env: HashMap::new(),
            known_hosts: KnownHostsPolicy::System,
        }
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn username(mut self, username: impl Into<String>) -> Self {
        self.username = username.into();
        self
    }

    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.auth = SshAuth::Password(password.into());
        self
    }

    pub fn private_key(mut self, path: impl Into<PathBuf>) -> Self {
        self.auth = SshAuth::PrivateKey {
            path: path.into(),
            passphrase: None,
        };
        self
    }

    pub fn private_key_with_passphrase(
        mut self,
        path: impl Into<PathBuf>,
        passphrase: impl Into<String>,
    ) -> Self {
        self.auth = SshAuth::PrivateKey {
            path: path.into(),
            passphrase: Some(passphrase.into()),
        };
        self
    }

    pub fn agent(mut self) -> Self {
        self.auth = SshAuth::Agent;
        self
    }

    pub fn known_hosts(mut self, policy: KnownHostsPolicy) -> Self {
        self.known_hosts = policy;
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn dimensions(mut self, cols: u16, rows: u16) -> Self {
        self.dimensions = (cols, rows);
        self
    }

    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Connect and spawn a shell session
    pub async fn spawn(self) -> Result<Session<SshBackend>, Error> {
        let config = Arc::new(client::Config {
            connection_timeout: Some(self.timeout),
            ..Default::default()
        });

        let handler = SshClientHandler {
            incoming: Arc::new(Mutex::new(Vec::new())),
            data_notify: tokio::sync::Notify::new(),
            server_key: None,
        };

        let addr = format!("{}:{}", self.host, self.port);
        let mut handle = client::connect(config, &addr, handler).await?;

        // Authenticate
        let authenticated = match &self.auth {
            SshAuth::Password(pwd) => {
                handle.authenticate_password(&self.username, pwd).await?
            }
            SshAuth::PrivateKey { path, passphrase } => {
                let key = russh_keys::load_secret_key(path, passphrase.as_deref())?;
                handle.authenticate_publickey(&self.username, Arc::new(key)).await?
            }
            SshAuth::Agent => {
                let mut agent = russh_keys::agent::client::AgentClient::connect_env().await?;
                let identities = agent.request_identities().await?;
                let mut auth_success = false;
                for identity in identities {
                    if handle.authenticate_publickey_with(&self.username, identity, &mut agent).await? {
                        auth_success = true;
                        break;
                    }
                }
                auth_success
            }
            SshAuth::PrivateKeyMemory { key_pem, passphrase } => {
                let key = russh_keys::decode_secret_key(key_pem, passphrase.as_deref())?;
                handle.authenticate_publickey(&self.username, Arc::new(key)).await?
            }
        };

        if !authenticated {
            return Err(Error::Ssh(russh::Error::NotAuthenticated));
        }

        // Open channel and request PTY
        let channel = handle.channel_open_session().await?;
        let (cols, rows) = self.dimensions;

        channel.request_pty(
            false,              // want_reply
            "xterm-256color",   // term
            cols as u32,
            rows as u32,
            0,                  // pixel width
            0,                  // pixel height
            &[],                // terminal modes
        ).await?;

        // Set environment variables
        for (key, value) in &self.env {
            channel.set_env(false, key, value).await?;
        }

        // Request shell
        channel.request_shell(false).await?;

        let backend = SshBackend {
            client: Arc::new(Mutex::new(SshClientState {
                handle,
                connected: true,
            })),
            channel,
            channel_id: ChannelId(0),
            dimensions: self.dimensions,
        };

        Ok(Session::new(backend))
    }
}
```

#### 6.7.3 SSH Backend Trait Implementation

```rust
// crates/rust-expect/src/backend/ssh/backend.rs

#[async_trait::async_trait]
impl Backend for SshBackend {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        // Read from channel's incoming buffer
        let data = self.channel.make_reader().read(buf).await?;
        Ok(data)
    }

    async fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        self.channel.data(buf).await?;
        Ok(buf.len())
    }

    async fn resize(&mut self, cols: u16, rows: u16) -> Result<(), Error> {
        self.channel.window_change(
            cols as u32,
            rows as u32,
            0,
            0,
        ).await?;
        self.dimensions = (cols, rows);
        Ok(())
    }

    async fn close(&mut self) -> Result<(), Error> {
        self.channel.eof().await?;
        self.channel.close().await?;

        let mut state = self.client.lock().await;
        if state.connected {
            state.handle.disconnect(
                Disconnect::ByApplication,
                "Session closed",
                "en",
            ).await?;
            state.connected = false;
        }

        Ok(())
    }

    fn dimensions(&self) -> (u16, u16) {
        self.dimensions
    }
}
```

#### 6.7.4 SSH Session Usage Example

```rust
use rust_expect::ssh::{SshSessionBuilder, KnownHostsPolicy};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect via SSH with key authentication
    let mut session = SshSessionBuilder::new("example.com")
        .port(22)
        .username("admin")
        .private_key("~/.ssh/id_ed25519")
        .known_hosts(KnownHostsPolicy::System)
        .dimensions(120, 40)
        .spawn()
        .await?;

    // Same API as local PTY sessions
    session.expect("$").await?;
    session.send_line("hostname").await?;
    let hostname = session.expect("$").await?;
    println!("Connected to: {}", hostname.before());

    session.close().await?;
    Ok(())
}
```

#### 6.7.5 SSH Retry and Reconnection Strategies

SSH connections can fail or disconnect due to network instability, server restarts, or other transient issues. A robust SSH session manager must handle these gracefully.

##### Connection State Machine

```rust
// crates/rust-expect/src/backend/ssh/connection.rs

use std::time::{Duration, Instant};
use tokio::time::sleep;

/// SSH connection states for the state machine
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not yet connected
    Disconnected,
    /// Actively attempting to connect
    Connecting { attempt: u32, started: Instant },
    /// Connected and operational
    Connected { established: Instant },
    /// Connection lost, preparing to reconnect
    Reconnecting { attempt: u32, last_error: String },
    /// Permanently failed after max retries
    Failed { reason: String },
    /// Gracefully closed by application
    Closed,
}

/// Connection health and keepalive tracking
#[derive(Debug, Clone)]
pub struct ConnectionHealth {
    /// Last successful data exchange
    pub last_activity: Instant,
    /// Last keepalive sent
    pub last_keepalive_sent: Option<Instant>,
    /// Last keepalive acknowledged
    pub last_keepalive_ack: Option<Instant>,
    /// Consecutive keepalive failures
    pub missed_keepalives: u32,
    /// Connection latency estimate (round-trip)
    pub latency: Option<Duration>,
}
```

##### Retry Configuration

```rust
// crates/rust-expect/src/backend/ssh/retry.rs

use std::time::Duration;

/// Configuration for SSH connection retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of connection attempts (0 = unlimited)
    pub max_attempts: u32,
    /// Initial delay between retry attempts
    pub initial_delay: Duration,
    /// Maximum delay between retry attempts
    pub max_delay: Duration,
    /// Backoff multiplier for exponential backoff (1.0 = linear)
    pub backoff_multiplier: f64,
    /// Add random jitter to prevent thundering herd
    pub jitter: bool,
    /// Connection timeout per attempt
    pub connection_timeout: Duration,
    /// Keepalive interval (None = disabled)
    pub keepalive_interval: Option<Duration>,
    /// Number of missed keepalives before considering dead
    pub keepalive_max_failures: u32,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter: true,
            connection_timeout: Duration::from_secs(30),
            keepalive_interval: Some(Duration::from_secs(15)),
            keepalive_max_failures: 3,
        }
    }
}

impl RetryConfig {
    /// Preset for interactive sessions (fail fast)
    pub const INTERACTIVE: Self = Self {
        max_attempts: 3,
        initial_delay: Duration::from_millis(500),
        max_delay: Duration::from_secs(5),
        backoff_multiplier: 1.5,
        jitter: true,
        connection_timeout: Duration::from_secs(10),
        keepalive_interval: Some(Duration::from_secs(10)),
        keepalive_max_failures: 2,
    };

    /// Preset for batch/automation (resilient)
    pub const AUTOMATION: Self = Self {
        max_attempts: 10,
        initial_delay: Duration::from_secs(2),
        max_delay: Duration::from_secs(120),
        backoff_multiplier: 2.0,
        jitter: true,
        connection_timeout: Duration::from_secs(60),
        keepalive_interval: Some(Duration::from_secs(30)),
        keepalive_max_failures: 5,
    };

    /// Preset for long-running connections (persistent)
    pub const PERSISTENT: Self = Self {
        max_attempts: 0, // Unlimited
        initial_delay: Duration::from_secs(5),
        max_delay: Duration::from_secs(300),
        backoff_multiplier: 2.0,
        jitter: true,
        connection_timeout: Duration::from_secs(30),
        keepalive_interval: Some(Duration::from_secs(60)),
        keepalive_max_failures: 10,
    };

    /// Calculate delay for attempt number (1-indexed)
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        if attempt <= 1 {
            return self.initial_delay;
        }

        let base_delay = self.initial_delay.as_secs_f64()
            * self.backoff_multiplier.powi((attempt - 1) as i32);

        let capped_delay = base_delay.min(self.max_delay.as_secs_f64());

        let final_delay = if self.jitter {
            // Add 0-25% jitter
            let jitter_factor = 1.0 + (rand::random::<f64>() * 0.25);
            capped_delay * jitter_factor
        } else {
            capped_delay
        };

        Duration::from_secs_f64(final_delay)
    }
}
```

##### Resilient SSH Session

```rust
// crates/rust-expect/src/backend/ssh/resilient.rs

use std::sync::Arc;
use tokio::sync::{Mutex, Notify, RwLock};

/// Callback for connection state changes
pub type StateChangeCallback = Arc<dyn Fn(ConnectionState, ConnectionState) + Send + Sync>;

/// Callback for reconnection events
pub type ReconnectCallback = Arc<dyn Fn(u32, Option<&str>) -> bool + Send + Sync>;

/// SSH session with automatic reconnection capability
pub struct ResilientSshSession {
    /// Current connection state
    state: RwLock<ConnectionState>,
    /// Connection configuration (preserved for reconnection)
    config: SshSessionConfig,
    /// Retry configuration
    retry_config: RetryConfig,
    /// Current active session (if connected)
    inner: Mutex<Option<Session<SshBackend>>>,
    /// Notification for state changes
    state_notify: Notify,
    /// State change callback
    on_state_change: Option<StateChangeCallback>,
    /// Pre-reconnect callback (return false to abort)
    on_reconnect: Option<ReconnectCallback>,
    /// Buffer of unsent data during reconnection (bounded)
    pending_writes: Mutex<BoundedWriteBuffer>,
}

/// Bounded buffer for pending writes during disconnection
/// Prevents unbounded memory growth during extended outages
struct BoundedWriteBuffer {
    /// Queued writes
    writes: Vec<Vec<u8>>,
    /// Current total size in bytes
    current_size: usize,
    /// Maximum total bytes to buffer (default: 1MB)
    max_size: usize,
    /// Bytes dropped due to overflow
    dropped_bytes: usize,
    /// Strategy when full
    overflow_strategy: WriteOverflowStrategy,
}

#[derive(Clone, Copy, Default)]
enum WriteOverflowStrategy {
    /// Drop oldest writes first (default)
    #[default]
    DropOldest,
    /// Drop newest writes (reject new data)
    DropNewest,
    /// Return error to caller
    Error,
}

impl BoundedWriteBuffer {
    fn new(max_size: usize) -> Self {
        Self {
            writes: Vec::new(),
            current_size: 0,
            max_size,
            dropped_bytes: 0,
            overflow_strategy: WriteOverflowStrategy::default(),
        }
    }

    fn push(&mut self, data: Vec<u8>) -> Result<(), Error> {
        let data_len = data.len();

        // Check if single write exceeds max
        if data_len > self.max_size {
            match self.overflow_strategy {
                WriteOverflowStrategy::DropOldest | WriteOverflowStrategy::DropNewest => {
                    self.dropped_bytes += data_len;
                    tracing::warn!(
                        data_len,
                        max_size = self.max_size,
                        "Single write exceeds max buffer size, dropping"
                    );
                    return Ok(());
                }
                WriteOverflowStrategy::Error => {
                    return Err(Error::BufferFull);
                }
            }
        }

        // Evict old data if necessary
        while self.current_size + data_len > self.max_size && !self.writes.is_empty() {
            match self.overflow_strategy {
                WriteOverflowStrategy::DropOldest => {
                    if let Some(old) = self.writes.first() {
                        self.dropped_bytes += old.len();
                        self.current_size -= old.len();
                    }
                    self.writes.remove(0);
                }
                WriteOverflowStrategy::DropNewest => {
                    self.dropped_bytes += data_len;
                    return Ok(());
                }
                WriteOverflowStrategy::Error => {
                    return Err(Error::BufferFull);
                }
            }
        }

        self.current_size += data_len;
        self.writes.push(data);
        Ok(())
    }

    fn drain(&mut self) -> Vec<Vec<u8>> {
        self.current_size = 0;
        std::mem::take(&mut self.writes)
    }

    fn stats(&self) -> (usize, usize, usize) {
        (self.current_size, self.max_size, self.dropped_bytes)
    }
}

/// Preserved configuration for reconnection
#[derive(Clone)]
struct SshSessionConfig {
    host: String,
    port: u16,
    username: String,
    auth: SshAuth,
    known_hosts: KnownHostsPolicy,
    dimensions: (u16, u16),
    env: HashMap<String, String>,
}

impl ResilientSshSession {
    /// Create a new resilient SSH session
    pub fn builder(host: impl Into<String>) -> ResilientSshSessionBuilder {
        ResilientSshSessionBuilder::new(host)
    }

    /// Connect with automatic retry
    pub async fn connect(&self) -> Result<(), Error> {
        let mut attempt = 0u32;

        loop {
            attempt += 1;

            // Check if we've exceeded max attempts
            if self.retry_config.max_attempts > 0
                && attempt > self.retry_config.max_attempts
            {
                self.set_state(ConnectionState::Failed {
                    reason: format!("Exceeded {} connection attempts", self.retry_config.max_attempts),
                }).await;
                return Err(Error::MaxRetriesExceeded);
            }

            // Update state to connecting
            self.set_state(ConnectionState::Connecting {
                attempt,
                started: Instant::now(),
            }).await;

            // Attempt connection with timeout
            let connect_result = tokio::time::timeout(
                self.retry_config.connection_timeout,
                self.try_connect(),
            ).await;

            match connect_result {
                Ok(Ok(session)) => {
                    // Connection successful
                    *self.inner.lock().await = Some(session);
                    self.set_state(ConnectionState::Connected {
                        established: Instant::now(),
                    }).await;

                    // Replay any pending writes
                    self.replay_pending_writes().await?;

                    return Ok(());
                }
                Ok(Err(e)) => {
                    // Connection failed
                    tracing::warn!(attempt, error = %e, "SSH connection attempt failed");

                    if attempt >= self.retry_config.max_attempts && self.retry_config.max_attempts > 0 {
                        self.set_state(ConnectionState::Failed {
                            reason: e.to_string(),
                        }).await;
                        return Err(e);
                    }

                    // Wait before retry
                    let delay = self.retry_config.delay_for_attempt(attempt);
                    tracing::info!(attempt, delay_ms = delay.as_millis(), "Waiting before retry");
                    sleep(delay).await;
                }
                Err(_timeout) => {
                    // Connection timed out
                    tracing::warn!(attempt, timeout_ms = self.retry_config.connection_timeout.as_millis(), "SSH connection attempt timed out");

                    if attempt >= self.retry_config.max_attempts && self.retry_config.max_attempts > 0 {
                        self.set_state(ConnectionState::Failed {
                            reason: "Connection timeout".to_string(),
                        }).await;
                        return Err(Error::Timeout);
                    }

                    let delay = self.retry_config.delay_for_attempt(attempt);
                    sleep(delay).await;
                }
            }
        }
    }

    /// Attempt reconnection after disconnect
    async fn reconnect(&self) -> Result<(), Error> {
        let current_state = self.state.read().await.clone();

        // Only reconnect from certain states
        match &current_state {
            ConnectionState::Connected { .. } |
            ConnectionState::Reconnecting { .. } => {}
            _ => return Err(Error::InvalidState),
        }

        // Invoke callback to allow abort
        if let Some(callback) = &self.on_reconnect {
            let attempt = match &current_state {
                ConnectionState::Reconnecting { attempt, .. } => *attempt,
                _ => 1,
            };
            if !callback(attempt, None) {
                self.set_state(ConnectionState::Closed).await;
                return Ok(());
            }
        }

        // Close existing session if any
        if let Some(mut session) = self.inner.lock().await.take() {
            let _ = session.close().await;
        }

        // Reconnect using connect logic
        self.connect().await
    }

    /// Update state and notify listeners
    async fn set_state(&self, new_state: ConnectionState) {
        let old_state = {
            let mut state = self.state.write().await;
            let old = state.clone();
            *state = new_state.clone();
            old
        };

        if let Some(callback) = &self.on_state_change {
            callback(old_state, new_state);
        }

        self.state_notify.notify_waiters();
    }

    /// Get current connection state
    pub async fn state(&self) -> ConnectionState {
        self.state.read().await.clone()
    }

    /// Wait for connection state to change
    pub async fn wait_state_change(&self) -> ConnectionState {
        self.state_notify.notified().await;
        self.state.read().await.clone()
    }
}
```

##### Keepalive and Health Monitoring

```rust
// crates/rust-expect/src/backend/ssh/keepalive.rs

/// Background keepalive task for SSH sessions
pub struct KeepaliveMonitor {
    session: Arc<ResilientSshSession>,
    config: RetryConfig,
    shutdown: tokio::sync::broadcast::Receiver<()>,
}

impl KeepaliveMonitor {
    /// Start the keepalive monitoring loop
    pub async fn run(mut self) {
        let Some(interval) = self.config.keepalive_interval else {
            return; // Keepalives disabled
        };

        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if let Err(e) = self.send_keepalive().await {
                        tracing::warn!(error = %e, "Keepalive failed");
                        if self.should_reconnect().await {
                            let _ = self.session.reconnect().await;
                        }
                    }
                }
                _ = self.shutdown.recv() => {
                    tracing::debug!("Keepalive monitor shutting down");
                    break;
                }
            }
        }
    }

    /// Send SSH keepalive request
    async fn send_keepalive(&self) -> Result<(), Error> {
        // SSH keepalive via channel request
        // russh sends keep-alive via `session.request()` with want_reply=true
        let session = self.session.inner.lock().await;
        if let Some(ref inner) = *session {
            // Send global request as keepalive
            // The server should respond, confirming connection is alive
            // Implementation depends on russh API
            Ok(())
        } else {
            Err(Error::NotConnected)
        }
    }

    /// Check if we should attempt reconnection
    async fn should_reconnect(&self) -> bool {
        matches!(
            self.session.state().await,
            ConnectionState::Connected { .. }
        )
    }
}
```

##### Usage Example

```rust
use rust_expect::ssh::{ResilientSshSession, RetryConfig, ConnectionState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create resilient SSH session with automation preset
    let session = ResilientSshSession::builder("example.com")
        .username("admin")
        .private_key("~/.ssh/id_ed25519")
        .retry_config(RetryConfig::AUTOMATION)
        .on_state_change(|old, new| {
            println!("SSH state: {:?} -> {:?}", old, new);
        })
        .on_reconnect(|attempt, error| {
            println!("Reconnecting (attempt {}): {:?}", attempt, error);
            attempt <= 5 // Allow up to 5 reconnection attempts
        })
        .build();

    // Connect with automatic retry
    session.connect().await?;

    // Use session - automatic reconnection on transient failures
    loop {
        match session.state().await {
            ConnectionState::Connected { .. } => {
                // Normal operation
                session.send_line("uptime").await?;
                let output = session.expect("$").await?;
                println!("Uptime: {}", output.before());
            }
            ConnectionState::Reconnecting { attempt, .. } => {
                println!("Reconnecting... attempt {}", attempt);
                session.wait_state_change().await;
            }
            ConnectionState::Failed { reason } => {
                eprintln!("Connection failed permanently: {}", reason);
                break;
            }
            ConnectionState::Closed => {
                println!("Session closed");
                break;
            }
            _ => {
                session.wait_state_change().await;
            }
        }
    }

    Ok(())
}
```

##### Design Rationale

| Decision | Rationale |
|----------|-----------|
| Exponential backoff with jitter | Prevents thundering herd on server recovery; industry standard pattern |
| Configurable presets (INTERACTIVE, AUTOMATION, PERSISTENT) | Different use cases have different tolerance for delays and retries |
| State machine model | Clear transitions enable reliable monitoring and recovery |
| Callback-based notifications | Enables logging, metrics, and application-specific recovery logic |
| Keepalive monitoring | Detects silent connection failures (half-open connections) |
| Pending write buffer | Allows session continuity across reconnection without data loss |

**References:**
- [autossh man page](https://linux.die.net/man/1/autossh) - SSH monitoring and restart patterns
- [Keep SSH Sessions Running After Disconnection](https://www.tecmint.com/keep-remote-ssh-sessions-running-after-disconnection/)

### 6.8 Screen Buffer Architecture

The screen buffer provides a virtual terminal state, enabling inspection of the current screen contents independent of the output stream.

#### 6.8.1 Virtual Screen Buffer

```rust
// crates/rust-expect/src/screen/buffer.rs

use vte::{Parser, Perform};

/// Virtual screen buffer tracking terminal state
pub struct ScreenBuffer {
    /// Screen contents as a 2D grid
    cells: Vec<Vec<Cell>>,
    /// Current cursor position
    cursor: CursorPosition,
    /// Screen dimensions
    dimensions: (u16, u16),
    /// VTE parser for ANSI processing
    parser: Parser,
    /// Scroll-back buffer (optional)
    scrollback: Option<Vec<Vec<Cell>>>,
    /// Maximum scrollback lines
    scrollback_limit: usize,
}

/// Single cell in the screen buffer
#[derive(Clone, Default)]
pub struct Cell {
    /// Character at this position
    pub character: char,
    /// Foreground color
    pub fg: Color,
    /// Background color
    pub bg: Color,
    /// Cell attributes (bold, italic, etc.)
    pub attrs: CellAttributes,
}

/// Cursor position and state
#[derive(Clone, Default)]
pub struct CursorPosition {
    pub row: u16,
    pub col: u16,
    pub visible: bool,
}

/// Color representation
#[derive(Clone, Copy, Default)]
pub enum Color {
    #[default]
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

bitflags::bitflags! {
    /// Cell display attributes
    #[derive(Clone, Copy, Default)]
    pub struct CellAttributes: u8 {
        const BOLD = 0b0000_0001;
        const ITALIC = 0b0000_0010;
        const UNDERLINE = 0b0000_0100;
        const BLINK = 0b0000_1000;
        const REVERSE = 0b0001_0000;
        const HIDDEN = 0b0010_0000;
        const STRIKETHROUGH = 0b0100_0000;
    }
}

impl ScreenBuffer {
    /// Create a new screen buffer with given dimensions
    pub fn new(cols: u16, rows: u16) -> Self {
        let cells = vec![vec![Cell::default(); cols as usize]; rows as usize];
        Self {
            cells,
            cursor: CursorPosition::default(),
            dimensions: (cols, rows),
            parser: Parser::new(),
            scrollback: None,
            scrollback_limit: 0,
        }
    }

    /// Enable scrollback with specified line limit
    pub fn with_scrollback(mut self, lines: usize) -> Self {
        self.scrollback = Some(Vec::with_capacity(lines.min(1000)));
        self.scrollback_limit = lines;
        self
    }

    /// Process incoming bytes through VTE parser
    pub fn process(&mut self, data: &[u8]) {
        let mut performer = ScreenPerformer { buffer: self };
        for byte in data {
            self.parser.advance(&mut performer, *byte);
        }
    }

    /// Get current screen contents as text
    pub fn text(&self) -> String {
        self.cells
            .iter()
            .map(|row| {
                row.iter()
                    .map(|cell| cell.character)
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Get a specific line (0-indexed)
    pub fn line(&self, row: usize) -> Option<String> {
        self.cells.get(row).map(|row| {
            row.iter()
                .map(|cell| cell.character)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
    }

    /// Get text in a rectangular region
    pub fn region(&self, start_row: u16, start_col: u16, end_row: u16, end_col: u16) -> String {
        let mut result = String::new();
        for row in start_row..=end_row {
            if let Some(cells) = self.cells.get(row as usize) {
                let start = start_col as usize;
                let end = (end_col as usize).min(cells.len());
                for cell in cells[start..end].iter() {
                    result.push(cell.character);
                }
                if row < end_row {
                    result.push('\n');
                }
            }
        }
        result.trim_end().to_string()
    }

    /// Get cursor position
    pub fn cursor(&self) -> &CursorPosition {
        &self.cursor
    }

    /// Resize the screen buffer
    pub fn resize(&mut self, cols: u16, rows: u16) {
        let new_cells = vec![vec![Cell::default(); cols as usize]; rows as usize];
        // Copy existing content
        for (row_idx, row) in self.cells.iter().enumerate() {
            if row_idx >= rows as usize {
                break;
            }
            for (col_idx, cell) in row.iter().enumerate() {
                if col_idx >= cols as usize {
                    break;
                }
                // Note: new_cells needs to be mutable for this
            }
        }
        self.cells = new_cells;
        self.dimensions = (cols, rows);
        // Clamp cursor to new dimensions
        self.cursor.col = self.cursor.col.min(cols.saturating_sub(1));
        self.cursor.row = self.cursor.row.min(rows.saturating_sub(1));
    }
}

/// VTE Perform implementation for screen buffer updates
struct ScreenPerformer<'a> {
    buffer: &'a mut ScreenBuffer,
}

impl<'a> Perform for ScreenPerformer<'a> {
    fn print(&mut self, c: char) {
        let (cols, rows) = self.buffer.dimensions;
        let cursor = &mut self.buffer.cursor;

        if cursor.col >= cols {
            // Wrap to next line
            cursor.col = 0;
            cursor.row += 1;
            if cursor.row >= rows {
                self.buffer.scroll_up();
                cursor.row = rows - 1;
            }
        }

        if let Some(row) = self.buffer.cells.get_mut(cursor.row as usize) {
            if let Some(cell) = row.get_mut(cursor.col as usize) {
                cell.character = c;
            }
        }
        cursor.col += 1;
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            0x08 => { // Backspace
                if self.buffer.cursor.col > 0 {
                    self.buffer.cursor.col -= 1;
                }
            }
            0x09 => { // Tab
                self.buffer.cursor.col = ((self.buffer.cursor.col / 8) + 1) * 8;
                self.buffer.cursor.col = self.buffer.cursor.col.min(self.buffer.dimensions.0 - 1);
            }
            0x0A | 0x0B | 0x0C => { // Line feed, vertical tab, form feed
                self.buffer.cursor.row += 1;
                if self.buffer.cursor.row >= self.buffer.dimensions.1 {
                    self.buffer.scroll_up();
                    self.buffer.cursor.row = self.buffer.dimensions.1 - 1;
                }
            }
            0x0D => { // Carriage return
                self.buffer.cursor.col = 0;
            }
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        _intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        // Handle CSI sequences (cursor movement, clearing, etc.)
        match action {
            'A' => { // Cursor Up
                let n = params.iter().next().and_then(|p| p.first().copied()).unwrap_or(1) as u16;
                self.buffer.cursor.row = self.buffer.cursor.row.saturating_sub(n);
            }
            'B' => { // Cursor Down
                let n = params.iter().next().and_then(|p| p.first().copied()).unwrap_or(1) as u16;
                self.buffer.cursor.row = (self.buffer.cursor.row + n).min(self.buffer.dimensions.1 - 1);
            }
            'C' => { // Cursor Forward
                let n = params.iter().next().and_then(|p| p.first().copied()).unwrap_or(1) as u16;
                self.buffer.cursor.col = (self.buffer.cursor.col + n).min(self.buffer.dimensions.0 - 1);
            }
            'D' => { // Cursor Back
                let n = params.iter().next().and_then(|p| p.first().copied()).unwrap_or(1) as u16;
                self.buffer.cursor.col = self.buffer.cursor.col.saturating_sub(n);
            }
            'H' | 'f' => { // Cursor Position
                let mut iter = params.iter();
                let row = iter.next().and_then(|p| p.first().copied()).unwrap_or(1) as u16;
                let col = iter.next().and_then(|p| p.first().copied()).unwrap_or(1) as u16;
                self.buffer.cursor.row = row.saturating_sub(1).min(self.buffer.dimensions.1 - 1);
                self.buffer.cursor.col = col.saturating_sub(1).min(self.buffer.dimensions.0 - 1);
            }
            'J' => { // Erase in Display
                let mode = params.iter().next().and_then(|p| p.first().copied()).unwrap_or(0);
                self.buffer.erase_display(mode as u8);
            }
            'K' => { // Erase in Line
                let mode = params.iter().next().and_then(|p| p.first().copied()).unwrap_or(0);
                self.buffer.erase_line(mode as u8);
            }
            'm' => { // SGR (Select Graphic Rendition) - colors and attributes
                // Process SGR parameters for cell attributes
                self.process_sgr(params);
            }
            _ => {}
        }
    }

    fn hook(&mut self, _params: &vte::Params, _intermediates: &[u8], _ignore: bool, _action: char) {}
    fn put(&mut self, _byte: u8) {}
    fn unhook(&mut self) {}
    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {}
    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}
}

impl ScreenBuffer {
    fn scroll_up(&mut self) {
        // Move first line to scrollback if enabled
        if let Some(ref mut scrollback) = self.scrollback {
            if scrollback.len() >= self.scrollback_limit {
                scrollback.remove(0);
            }
            scrollback.push(self.cells.remove(0));
        } else {
            self.cells.remove(0);
        }
        // Add new empty line at bottom
        self.cells.push(vec![Cell::default(); self.dimensions.0 as usize]);
    }

    fn erase_display(&mut self, mode: u8) {
        match mode {
            0 => { // From cursor to end
                self.erase_line(0);
                for row in (self.cursor.row as usize + 1)..self.cells.len() {
                    self.cells[row] = vec![Cell::default(); self.dimensions.0 as usize];
                }
            }
            1 => { // From start to cursor
                for row in 0..self.cursor.row as usize {
                    self.cells[row] = vec![Cell::default(); self.dimensions.0 as usize];
                }
                self.erase_line(1);
            }
            2 | 3 => { // Entire screen (3 also clears scrollback)
                for row in self.cells.iter_mut() {
                    *row = vec![Cell::default(); self.dimensions.0 as usize];
                }
                if mode == 3 {
                    if let Some(ref mut scrollback) = self.scrollback {
                        scrollback.clear();
                    }
                }
            }
            _ => {}
        }
    }

    fn erase_line(&mut self, mode: u8) {
        if let Some(row) = self.cells.get_mut(self.cursor.row as usize) {
            match mode {
                0 => { // From cursor to end
                    for cell in row[self.cursor.col as usize..].iter_mut() {
                        *cell = Cell::default();
                    }
                }
                1 => { // From start to cursor
                    for cell in row[..=self.cursor.col as usize].iter_mut() {
                        *cell = Cell::default();
                    }
                }
                2 => { // Entire line
                    *row = vec![Cell::default(); self.dimensions.0 as usize];
                }
                _ => {}
            }
        }
    }
}

impl<'a> ScreenPerformer<'a> {
    fn process_sgr(&mut self, _params: &vte::Params) {
        // SGR parameter processing for colors and attributes
        // Implementation handles parameters like:
        // 0 = reset, 1 = bold, 3 = italic, 4 = underline
        // 30-37 = foreground colors, 40-47 = background colors
        // 38;5;N = 256-color foreground, 48;5;N = 256-color background
        // 38;2;R;G;B = RGB foreground, 48;2;R;G;B = RGB background
    }
}
```

#### 6.8.2 Screen Buffer Integration with Session

```rust
// crates/rust-expect/src/session/screen_session.rs

/// Session with screen buffer tracking
pub struct ScreenSession<B: Backend> {
    inner: Session<B>,
    screen: ScreenBuffer,
}

impl<B: Backend> ScreenSession<B> {
    /// Create a screen-enabled session from existing session
    pub fn new(session: Session<B>, cols: u16, rows: u16) -> Self {
        Self {
            inner: session,
            screen: ScreenBuffer::new(cols, rows),
        }
    }

    /// Get current screen contents
    pub fn screen(&self) -> &ScreenBuffer {
        &self.screen
    }

    /// Read and update screen buffer
    pub async fn read_screen(&mut self) -> Result<&ScreenBuffer, Error> {
        let data = self.inner.read_available().await?;
        self.screen.process(&data);
        Ok(&self.screen)
    }

    /// Wait for screen to contain specific text
    pub async fn expect_screen(&mut self, text: &str) -> Result<(), Error> {
        loop {
            self.read_screen().await?;
            if self.screen.text().contains(text) {
                return Ok(());
            }
            // Small delay to prevent busy loop
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// Wait for specific text at cursor line
    pub async fn expect_at_cursor(&mut self, text: &str) -> Result<(), Error> {
        loop {
            self.read_screen().await?;
            if let Some(line) = self.screen.line(self.screen.cursor().row as usize) {
                if line.contains(text) {
                    return Ok(());
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}
```

### 6.9 Dialog System Architecture

The Dialog system enables declarative specification of multi-step interactive conversations, similar to original Expect's dialog capabilities.

#### 6.9.1 Dialog Definition

```rust
// crates/rust-expect/src/dialog/definition.rs

/// A dialog defines a conversation with expected prompts and responses
pub struct Dialog {
    /// Name for logging/debugging
    name: String,
    /// Ordered steps in the dialog
    steps: Vec<DialogStep>,
    /// Default timeout for all steps
    default_timeout: Duration,
    /// Error handling strategy
    on_error: ErrorStrategy,
}

/// A single step in a dialog
pub struct DialogStep {
    /// Pattern to expect
    pub expect: Pattern,
    /// Response to send when pattern matches
    pub respond: Response,
    /// Optional step-specific timeout
    pub timeout: Option<Duration>,
    /// Optional step name for debugging
    pub name: Option<String>,
    /// Whether this step is optional
    pub optional: bool,
    /// Maximum retries for this step
    pub retries: u32,
}

/// Response action for a dialog step
pub enum Response {
    /// Send a string
    Text(String),
    /// Send a string followed by newline
    Line(String),
    /// Send raw bytes
    Bytes(Vec<u8>),
    /// Send control character (e.g., Ctrl+C)
    Control(char),
    /// No response (just wait for pattern)
    None,
    /// Dynamic response based on match result
    Dynamic(Box<dyn Fn(&MatchResult) -> String + Send + Sync>),
}

/// Error handling strategy for dialogs
pub enum ErrorStrategy {
    /// Stop on first error
    FailFast,
    /// Skip failed steps and continue
    SkipOnError,
    /// Retry failed steps up to N times
    RetryOnError(u32),
    /// Custom error handler
    Custom(Box<dyn Fn(&Error, &DialogStep) -> ErrorAction + Send + Sync>),
}

/// Action to take on error
pub enum ErrorAction {
    /// Stop the dialog
    Stop,
    /// Skip this step
    Skip,
    /// Retry this step
    Retry,
}

impl Dialog {
    /// Create a new dialog builder
    pub fn builder(name: impl Into<String>) -> DialogBuilder {
        DialogBuilder::new(name)
    }
}

/// Builder for constructing dialogs
pub struct DialogBuilder {
    name: String,
    steps: Vec<DialogStep>,
    default_timeout: Duration,
    on_error: ErrorStrategy,
}

impl DialogBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            steps: Vec::new(),
            default_timeout: Duration::from_secs(30),
            on_error: ErrorStrategy::FailFast,
        }
    }

    /// Add an expect-respond step
    pub fn step(
        mut self,
        expect: impl Into<Pattern>,
        respond: impl Into<Response>,
    ) -> Self {
        self.steps.push(DialogStep {
            expect: expect.into(),
            respond: respond.into(),
            timeout: None,
            name: None,
            optional: false,
            retries: 0,
        });
        self
    }

    /// Add a named step
    pub fn named_step(
        mut self,
        name: impl Into<String>,
        expect: impl Into<Pattern>,
        respond: impl Into<Response>,
    ) -> Self {
        self.steps.push(DialogStep {
            expect: expect.into(),
            respond: respond.into(),
            timeout: None,
            name: Some(name.into()),
            optional: false,
            retries: 0,
        });
        self
    }

    /// Add an optional step (won't fail if pattern not found)
    pub fn optional_step(
        mut self,
        expect: impl Into<Pattern>,
        respond: impl Into<Response>,
    ) -> Self {
        self.steps.push(DialogStep {
            expect: expect.into(),
            respond: respond.into(),
            timeout: None,
            name: None,
            optional: true,
            retries: 0,
        });
        self
    }

    /// Set default timeout for all steps
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// Set error handling strategy
    pub fn on_error(mut self, strategy: ErrorStrategy) -> Self {
        self.on_error = strategy;
        self
    }

    /// Build the dialog
    pub fn build(self) -> Dialog {
        Dialog {
            name: self.name,
            steps: self.steps,
            default_timeout: self.default_timeout,
            on_error: self.on_error,
        }
    }
}

impl From<&str> for Response {
    fn from(s: &str) -> Self {
        Response::Line(s.to_string())
    }
}

impl From<String> for Response {
    fn from(s: String) -> Self {
        Response::Line(s)
    }
}
```

#### 6.9.2 Dialog Executor

```rust
// crates/rust-expect/src/dialog/executor.rs

/// Result of dialog execution
pub struct DialogResult {
    /// Whether the dialog completed successfully
    pub success: bool,
    /// Results from each step
    pub step_results: Vec<StepResult>,
    /// Total execution time
    pub duration: Duration,
}

/// Result of a single dialog step
pub struct StepResult {
    /// Step name (if provided)
    pub name: Option<String>,
    /// Whether the step succeeded
    pub success: bool,
    /// Match result (if pattern matched)
    pub match_result: Option<MatchResult>,
    /// Error (if step failed)
    pub error: Option<Error>,
    /// Step execution time
    pub duration: Duration,
}

/// Execute a dialog on a session
pub async fn execute_dialog<B: Backend>(
    session: &mut Session<B>,
    dialog: &Dialog,
) -> Result<DialogResult, Error> {
    let start = Instant::now();
    let mut step_results = Vec::with_capacity(dialog.steps.len());

    for step in &dialog.steps {
        let step_start = Instant::now();
        let timeout = step.timeout.unwrap_or(dialog.default_timeout);

        let result = execute_step(session, step, timeout, &dialog.on_error).await;

        let step_result = match result {
            Ok(match_result) => StepResult {
                name: step.name.clone(),
                success: true,
                match_result: Some(match_result),
                error: None,
                duration: step_start.elapsed(),
            },
            Err(e) if step.optional => StepResult {
                name: step.name.clone(),
                success: true, // Optional steps don't fail the dialog
                match_result: None,
                error: Some(e),
                duration: step_start.elapsed(),
            },
            Err(e) => {
                // Handle error based on strategy
                match &dialog.on_error {
                    ErrorStrategy::FailFast => {
                        step_results.push(StepResult {
                            name: step.name.clone(),
                            success: false,
                            match_result: None,
                            error: Some(e),
                            duration: step_start.elapsed(),
                        });
                        return Ok(DialogResult {
                            success: false,
                            step_results,
                            duration: start.elapsed(),
                        });
                    }
                    ErrorStrategy::SkipOnError => StepResult {
                        name: step.name.clone(),
                        success: false,
                        match_result: None,
                        error: Some(e),
                        duration: step_start.elapsed(),
                    },
                    _ => StepResult {
                        name: step.name.clone(),
                        success: false,
                        match_result: None,
                        error: Some(e),
                        duration: step_start.elapsed(),
                    },
                }
            }
        };

        step_results.push(step_result);
    }

    let success = step_results.iter().all(|r| r.success || r.error.is_some() && step_results.iter().find(|s| s.name == r.name).map(|_| true).unwrap_or(false));

    Ok(DialogResult {
        success: step_results.iter().filter(|r| !r.success).count() == 0,
        step_results,
        duration: start.elapsed(),
    })
}

async fn execute_step<B: Backend>(
    session: &mut Session<B>,
    step: &DialogStep,
    timeout: Duration,
    _on_error: &ErrorStrategy,
) -> Result<MatchResult, Error> {
    // Wait for the expected pattern
    let match_result = tokio::time::timeout(
        timeout,
        session.expect(step.expect.clone()),
    ).await.map_err(|_| Error::Timeout(timeout))??;

    // Send the response
    match &step.respond {
        Response::Text(text) => {
            session.send(text.as_bytes()).await?;
        }
        Response::Line(line) => {
            session.send_line(line).await?;
        }
        Response::Bytes(bytes) => {
            session.send(bytes).await?;
        }
        Response::Control(c) => {
            session.send(&[*c as u8 - b'@']).await?;
        }
        Response::None => {}
        Response::Dynamic(f) => {
            let response = f(&match_result);
            session.send_line(&response).await?;
        }
    }

    Ok(match_result)
}
```

#### 6.9.3 Dialog Usage Example

```rust
use rust_expect::{Session, Dialog, Response};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = Session::builder()
        .command("ssh user@server")
        .spawn()
        .await?;

    // Define a login dialog
    let login_dialog = Dialog::builder("ssh-login")
        .timeout(Duration::from_secs(60))
        .step("password:", "my_secure_password")
        .optional_step("trust this host", "yes")
        .step("$", Response::None)  // Wait for prompt, don't send anything
        .build();

    // Execute the dialog
    let result = session.dialog(&login_dialog).await?;

    if result.success {
        println!("Login successful!");

        // Continue with automation...
        session.send_line("ls -la").await?;
        session.expect("$").await?;
    } else {
        for step_result in &result.step_results {
            if !step_result.success {
                eprintln!(
                    "Step {:?} failed: {:?}",
                    step_result.name,
                    step_result.error
                );
            }
        }
    }

    session.close().await?;
    Ok(())
}
```

#### 6.9.4 Dialog Macro (from rust-expect-macros)

```rust
// Declarative dialog definition via macro
use rust_expect::dialog;

let login = dialog! {
    name: "ssh-login",
    timeout: 60s,

    "password:" => "my_password",
    "trust this host" => "yes" [optional],
    "$" => _,  // Wait only, no response
};

// Equivalent to the builder pattern above
```

---

## 7. rust-expect-macros Crate

### 7.1 Pattern DSL Macros

```rust
// crates/rust-expect-macros/src/lib.rs

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Expr, Token};

/// Pattern matching DSL
///
/// Usage:
/// ```
/// session.expect(patterns![
///     regex!(r"password:") => |s| { s.send_line("secret"); Continue },
///     exact!("$") => |s| Break(s.matched()),
///     timeout!(10s) => |s| Err(Error::Timeout),
/// ])?;
/// ```
#[proc_macro]
pub fn patterns(input: TokenStream) -> TokenStream {
    // Parse pattern => action pairs
    // Generate Vec<(Pattern, Box<dyn Fn>)>
    // ...
    todo!("Implement patterns! macro")
}

/// Compile-time regex validation
///
/// Usage: `regex!(r"\d+")`
#[proc_macro]
pub fn regex(input: TokenStream) -> TokenStream {
    let regex_str = parse_macro_input!(input as syn::LitStr);
    let value = regex_str.value();

    // Validate regex at compile time
    if let Err(e) = regex::Regex::new(&value) {
        return syn::Error::new(regex_str.span(), format!("Invalid regex: {e}"))
            .to_compile_error()
            .into();
    }

    quote! {
        ::rust_expect::Pattern::Regex(
            ::regex::Regex::new(#regex_str).expect("validated at compile time")
        )
    }
    .into()
}

/// Exact string pattern
///
/// Usage: `exact!("hello")`
#[proc_macro]
pub fn exact(input: TokenStream) -> TokenStream {
    let string = parse_macro_input!(input as syn::LitStr);

    quote! {
        ::rust_expect::Pattern::Exact(#string.to_string())
    }
    .into()
}

/// Timeout pattern with duration parsing
///
/// Usage: `timeout!(10s)`, `timeout!(500ms)`, `timeout!(1m)`
#[proc_macro]
pub fn timeout(input: TokenStream) -> TokenStream {
    // Parse duration literal like "10s", "500ms", "1m"
    // ...
    todo!("Implement timeout! macro")
}
```

---

## 8. Cross-Platform Strategy

### 8.1 Conditional Compilation

```rust
// Platform-specific code organization

// crates/rust-pty/src/lib.rs
#[cfg(unix)]
mod unix;

#[cfg(windows)]
mod windows;

// Re-export the native implementation
#[cfg(unix)]
pub use unix::{UnixPtyMaster, UnixPtyChild, UnixPtySystem};

#[cfg(windows)]
pub use windows::{WindowsPtyMaster, WindowsPtyChild, WindowsPtySystem};

/// Get the native PTY system for the current platform
pub fn native_pty_system() -> impl PtySystem {
    #[cfg(unix)]
    { unix::UnixPtySystem::new() }

    #[cfg(windows)]
    { windows::WindowsPtySystem::new() }
}
```

### 8.2 Platform Abstraction Points

| Abstraction | Unix | Windows |
|-------------|------|---------|
| PTY allocation | `posix_openpt()` via rustix | `CreatePseudoConsole()` |
| PTY resize | `ioctl(TIOCSWINSZ)` | `ResizePseudoConsole()` |
| PTY close | `close()` | `ClosePseudoConsole()` |
| Process spawn | `fork()` + `execvp()` | `CreateProcess()` with attributes |
| Signal: Interrupt | `SIGINT` | `GenerateConsoleCtrlEvent(CTRL_C_EVENT)` |
| Signal: Terminate | `SIGTERM` | `GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT)` |
| Signal: Kill | `SIGKILL` | `TerminateProcess()` |
| Process tree kill | `kill(-pgid, SIGKILL)` | Job Objects with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` |
| Async I/O | `AsyncFd` on file descriptor | Thread-per-pipe or Overlapped I/O |
| Terminal raw mode | `tcsetattr()` via crossterm | Console mode via crossterm |

### 8.3 Feature Detection

```rust
// crates/rust-pty/src/windows/mod.rs

#[cfg(windows)]
pub mod version {
    /// Minimum Windows version for ConPTY (Windows 10 1809, build 17763)
    pub const MIN_CONPTY_BUILD: u32 = 17763;

    // NOTE: ConPTY overlapped I/O is NOT available in any released Windows version
    // as of December 2025. PR #17510 was merged in August 2024 but missed the
    // feature cutoff for Windows 11 24H2. Windows 11 25H2 is an enablement package
    // over 24H2 (same kernel base) and does NOT include new ConPTY features.
    // Expected availability: Windows 26H2 or later (no confirmed date).
    // Reference: https://github.com/microsoft/terminal/discussions/19112

    /// Check if ConPTY is available
    pub fn has_conpty() -> bool {
        get_windows_build() >= MIN_CONPTY_BUILD
    }

    /// Check if ConPTY overlapped I/O is available
    ///
    /// IMPORTANT: As of December 2025, this always returns false.
    /// Overlapped I/O is not available in any released Windows version.
    /// This function is a placeholder for future Windows releases.
    pub fn has_overlapped_conpty() -> bool {
        // Conservative default until Microsoft ships this feature
        // in a publicly released Windows version
        false
    }

    fn get_windows_build() -> u32 {
        // Use RtlGetVersion for accurate version info
        // (GetVersionEx can be lied to via manifest)
        use windows_sys::Win32::System::SystemInformation::*;
        // ...
        todo!("Implement Windows build detection")
    }
}
```

---

## 9. Async Architecture

### 9.1 Tokio Integration

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                           Tokio Integration                                   │
│                                                                               │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │                         rust-expect Public API                           │ │
│  │                                                                          │ │
│  │  All public async methods return impl Future:                            │ │
│  │  ├── Session::spawn() -> impl Future<Output = Result<Session>>          │ │
│  │  ├── session.expect() -> impl Future<Output = Result<MatchResult>>      │ │
│  │  ├── session.send() -> impl Future<Output = Result<()>>                 │ │
│  │  ├── session.interact().run() -> impl Future<Output = Result<..>>       │ │
│  │  └── session.wait() -> impl Future<Output = Result<ExitStatus>>         │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                     │                                         │
│                                     ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │                          Tokio Runtime                                   │ │
│  │                                                                          │ │
│  │  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────┐  │ │
│  │  │    I/O Driver   │  │  Timer Driver   │  │   Signal Handler        │  │ │
│  │  │                 │  │                 │  │   (Unix only)           │  │ │
│  │  │  Unix: epoll    │  │  tokio::time    │  │   signal-hook-tokio     │  │ │
│  │  │  macOS: kqueue  │  │  for timeouts   │  │   SIGWINCH, SIGCHLD     │  │ │
│  │  │  Windows: IOCP  │  │                 │  │                         │  │ │
│  │  └─────────────────┘  └─────────────────┘  └─────────────────────────┘  │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                     │                                         │
│                                     ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │                         rust-pty Backend                                 │ │
│  │                                                                          │ │
│  │  Unix:                          Windows:                                 │ │
│  │  ┌─────────────────────┐       ┌─────────────────────────────────────┐  │ │
│  │  │ AsyncFd<OwnedFd>    │       │ Thread-per-pipe (all current)       │  │ │
│  │  │                     │       │                                     │  │ │
│  │  │ - Registers PTY fd  │       │ - Blocking threads → tokio channels │  │ │
│  │  │   with epoll/kqueue │       │   (required for all Windows today)  │  │ │
│  │  │ - Zero-copy async   │       │ - Native IOCP via Overlapped I/O    │  │ │
│  │  │   read/write        │       │   (future: Windows 26H2+)           │  │ │
│  │  └─────────────────────┘       └─────────────────────────────────────┘  │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 9.2 Cancellation Safety

All async operations are designed to be cancellation-safe:

```rust
// Cancellation-safe expect implementation

impl<B: Backend> Session<B> {
    /// Expect is cancellation-safe:
    /// - Buffer state preserved on cancellation
    /// - No partial matches left in inconsistent state
    /// - Session remains usable after cancellation
    pub async fn expect(&mut self, pattern: impl Into<Pattern>) -> Result<MatchResult, Error> {
        let pattern = pattern.into();

        loop {
            // Check for existing match in buffer FIRST
            // This ensures we don't lose data on cancellation
            if let Some(result) = self.matcher.try_match(self.buffer.as_bytes()) {
                self.buffer.consume(result.position.end);
                return Ok(result);
            }

            // Read more data
            // If cancelled here, buffer contains all previously read data
            let mut read_buf = [0u8; 4096];

            tokio::select! {
                biased; // Check timeout first for consistent behavior

                _ = tokio::time::sleep(self.config.timeout) => {
                    return Err(Error::Timeout {
                        duration: self.config.timeout,
                        buffer: self.buffer.to_string(),
                        pattern: pattern.to_string(),
                    });
                }

                result = self.backend.read(&mut read_buf) => {
                    match result {
                        Ok(0) => {
                            return Err(Error::Eof {
                                buffer: self.buffer.to_string(),
                            });
                        }
                        Ok(n) => {
                            self.buffer.extend(&read_buf[..n]);
                            // Loop to check for match
                        }
                        Err(e) => return Err(Error::Io(e)),
                    }
                }
            }
        }
    }
}
```

### 9.3 Sync API Wrapper

```rust
// crates/rust-expect/src/sync.rs

use tokio::runtime::Runtime;

/// Synchronous session wrapper
///
/// Uses a thread-local tokio runtime for blocking on async operations.
pub struct SyncSession {
    inner: Session<PtyBackend>,
    runtime: Runtime,
}

impl SyncSession {
    pub fn spawn(command: &str) -> Result<Self, Error> {
        let runtime = Runtime::new()?;
        let inner = runtime.block_on(
            Session::builder()
                .command(command)
                .spawn()
        )?;

        Ok(Self { inner, runtime })
    }

    pub fn expect(&mut self, pattern: impl Into<Pattern>) -> Result<MatchResult, Error> {
        self.runtime.block_on(self.inner.expect(pattern))
    }

    pub fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        self.runtime.block_on(self.inner.send(data))
    }

    pub fn send_line(&mut self, text: &str) -> Result<(), Error> {
        self.runtime.block_on(self.inner.send_line(text))
    }

    // ... other methods
}
```

### 9.4 Backpressure Handling

PTY output can arrive faster than the application can process it. rust-expect implements multiple strategies to handle backpressure without losing data or blocking the kernel.

#### 9.4.1 Producer-Consumer Model

```rust
// crates/rust-expect/src/async/backpressure.rs

use tokio::sync::mpsc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Backpressure-aware channel between PTY reader and buffer
pub struct BackpressureChannel {
    /// Bounded channel for data chunks
    sender: mpsc::Sender<Vec<u8>>,
    receiver: mpsc::Receiver<Vec<u8>>,
    /// Current queue depth (bytes)
    queue_bytes: AtomicUsize,
    /// Configuration
    config: BackpressureConfig,
}

/// Backpressure configuration
#[derive(Clone, Debug)]
pub struct BackpressureConfig {
    /// Maximum bytes to buffer before applying backpressure
    pub high_watermark: usize,
    /// Resume normal operation when buffer drops below this
    pub low_watermark: usize,
    /// Maximum channel capacity (messages)
    pub channel_capacity: usize,
    /// Strategy when high watermark exceeded
    pub strategy: BackpressureStrategy,
}

impl Default for BackpressureConfig {
    fn default() -> Self {
        Self {
            high_watermark: 16 * 1024 * 1024,  // 16 MB
            low_watermark: 4 * 1024 * 1024,    // 4 MB
            channel_capacity: 1024,
            strategy: BackpressureStrategy::Block,
        }
    }
}

/// Strategy for handling backpressure
#[derive(Clone, Copy, Debug, Default)]
pub enum BackpressureStrategy {
    /// Block the producer (PTY reader) until consumer catches up
    #[default]
    Block,
    /// Drop oldest data when buffer is full
    DropOldest,
    /// Drop newest data when buffer is full
    DropNewest,
    /// Adaptive: block briefly, then drop if still full
    Adaptive { block_ms: u64 },
}

impl BackpressureChannel {
    pub fn new(config: BackpressureConfig) -> Self {
        let (sender, receiver) = mpsc::channel(config.channel_capacity);
        Self {
            sender,
            receiver,
            queue_bytes: AtomicUsize::new(0),
            config,
        }
    }

    /// Send data with backpressure handling
    pub async fn send(&self, data: Vec<u8>) -> Result<(), BackpressureError> {
        let len = data.len();
        let current = self.queue_bytes.load(Ordering::Relaxed);

        // Check high watermark
        if current + len > self.config.high_watermark {
            match self.config.strategy {
                BackpressureStrategy::Block => {
                    // Wait for space - the send itself will block
                }
                BackpressureStrategy::DropOldest => {
                    // Signal consumer to discard old data
                    return Err(BackpressureError::DropOldest(len));
                }
                BackpressureStrategy::DropNewest => {
                    // Simply don't send this data
                    tracing::warn!(
                        bytes = len,
                        queue_bytes = current,
                        "dropping data due to backpressure"
                    );
                    return Ok(());
                }
                BackpressureStrategy::Adaptive { block_ms } => {
                    // Try to send with timeout
                    let timeout = std::time::Duration::from_millis(block_ms);
                    match tokio::time::timeout(timeout, self.sender.send(data.clone())).await {
                        Ok(Ok(())) => {
                            self.queue_bytes.fetch_add(len, Ordering::Relaxed);
                            return Ok(());
                        }
                        _ => {
                            tracing::warn!(
                                bytes = len,
                                "adaptive backpressure: dropping after timeout"
                            );
                            return Ok(());
                        }
                    }
                }
            }
        }

        self.sender.send(data).await
            .map_err(|_| BackpressureError::Closed)?;
        self.queue_bytes.fetch_add(len, Ordering::Relaxed);
        Ok(())
    }

    /// Receive data, updating queue depth
    pub async fn recv(&mut self) -> Option<Vec<u8>> {
        let data = self.receiver.recv().await?;
        self.queue_bytes.fetch_sub(data.len(), Ordering::Relaxed);
        Some(data)
    }

    /// Check if backpressure is currently active
    pub fn is_under_pressure(&self) -> bool {
        self.queue_bytes.load(Ordering::Relaxed) > self.config.high_watermark
    }

    /// Current queue depth in bytes
    pub fn queue_bytes(&self) -> usize {
        self.queue_bytes.load(Ordering::Relaxed)
    }
}

#[derive(Debug)]
pub enum BackpressureError {
    Closed,
    DropOldest(usize),
}
```

#### 9.4.2 Flow Control Signals

```rust
// Flow control between PTY reader and pattern matcher

/// Flow control state
pub struct FlowControl {
    /// Whether consumer is ready for more data
    ready: AtomicBool,
    /// Notify producer when consumer is ready
    ready_notify: tokio::sync::Notify,
}

impl FlowControl {
    /// Producer: wait until consumer is ready
    pub async fn wait_ready(&self) {
        while !self.ready.load(Ordering::Acquire) {
            self.ready_notify.notified().await;
        }
    }

    /// Consumer: signal readiness for more data
    pub fn signal_ready(&self) {
        self.ready.store(true, Ordering::Release);
        self.ready_notify.notify_one();
    }

    /// Consumer: signal busy (stop sending)
    pub fn signal_busy(&self) {
        self.ready.store(false, Ordering::Release);
    }
}
```

#### 9.4.3 Buffer Size Management

```rust
// Dynamic buffer sizing based on throughput

impl OutputBuffer {
    /// Adjust buffer limits based on observed throughput
    pub fn auto_tune(&mut self, bytes_per_second: usize) {
        // Target: buffer should hold ~5 seconds of output
        let target_size = bytes_per_second.saturating_mul(5);

        // Clamp to reasonable bounds
        let new_limit = target_size
            .max(self.config.min_buffer_size)
            .min(self.config.max_buffer_size);

        if new_limit != self.max_size {
            tracing::debug!(
                old_limit = self.max_size,
                new_limit = new_limit,
                bytes_per_second = bytes_per_second,
                "auto-tuning buffer size"
            );
            self.max_size = new_limit;
        }
    }
}
```

### 9.5 PTY Buffer Tuning

Both OS-level and application-level buffer sizes affect performance. This section covers tuning strategies.

#### 9.5.1 OS-Level PTY Buffers

```rust
// crates/rust-pty/src/unix/buffer.rs

#[cfg(target_os = "linux")]
pub fn get_pty_buffer_size(fd: RawFd) -> io::Result<usize> {
    // Linux: /proc/sys/fs/pipe-max-size (typically 1MB)
    // PTY uses similar buffering to pipes
    use rustix::fs::fcntl_getpipe_sz;
    fcntl_getpipe_sz(fd).map(|sz| sz as usize)
}

#[cfg(target_os = "linux")]
pub fn set_pty_buffer_size(fd: RawFd, size: usize) -> io::Result<usize> {
    // Requires CAP_SYS_RESOURCE for sizes > /proc/sys/fs/pipe-max-size
    use rustix::fs::fcntl_setpipe_sz;
    fcntl_setpipe_sz(fd, size as i32).map(|sz| sz as usize)
}

#[cfg(target_os = "macos")]
pub fn get_pty_buffer_size(_fd: RawFd) -> io::Result<usize> {
    // macOS: Fixed at 8KB for PTYs (HFS+ default)
    Ok(8192)
}

#[cfg(windows)]
pub fn get_conpty_buffer_size() -> usize {
    // ConPTY uses internal buffering; not directly tunable
    // Default is approximately 64KB
    65536
}
```

#### 9.5.2 Application Buffer Configuration

```rust
// Buffer sizing recommendations

/// Recommended buffer sizes for different use cases
pub struct BufferPresets;

impl BufferPresets {
    /// Interactive shell: low latency, moderate buffer
    pub const INTERACTIVE: BufferConfig = BufferConfig {
        initial_capacity: 64 * 1024,      // 64 KB
        max_size: 1024 * 1024,            // 1 MB
        chunk_size: 4096,                  // 4 KB reads
        search_window: Some(64 * 1024),   // Search last 64 KB
    };

    /// Log streaming: high throughput, large buffer
    pub const LOG_STREAMING: BufferConfig = BufferConfig {
        initial_capacity: 1024 * 1024,     // 1 MB
        max_size: 64 * 1024 * 1024,        // 64 MB
        chunk_size: 65536,                  // 64 KB reads
        search_window: Some(1024 * 1024),  // Search last 1 MB
    };

    /// Test automation: moderate everything
    pub const TEST_AUTOMATION: BufferConfig = BufferConfig {
        initial_capacity: 256 * 1024,      // 256 KB
        max_size: 16 * 1024 * 1024,        // 16 MB
        chunk_size: 8192,                   // 8 KB reads
        search_window: None,                // Search all
    };

    /// Memory constrained: minimal buffering
    pub const MEMORY_CONSTRAINED: BufferConfig = BufferConfig {
        initial_capacity: 16 * 1024,       // 16 KB
        max_size: 256 * 1024,              // 256 KB
        chunk_size: 1024,                   // 1 KB reads
        search_window: Some(16 * 1024),    // Search last 16 KB
    };
}

/// Buffer configuration
#[derive(Clone, Debug)]
pub struct BufferConfig {
    /// Initial allocation size
    pub initial_capacity: usize,
    /// Maximum buffer size before discarding old data
    pub max_size: usize,
    /// Read chunk size for PTY reads
    pub chunk_size: usize,
    /// Limit pattern matching to last N bytes (None = search all)
    pub search_window: Option<usize>,
}

impl SessionBuilder {
    /// Use a buffer preset
    pub fn buffer_preset(mut self, preset: BufferConfig) -> Self {
        self.buffer_config = preset;
        self
    }

    /// Configure buffer for high-throughput scenarios
    pub fn high_throughput(self) -> Self {
        self.buffer_preset(BufferPresets::LOG_STREAMING)
    }

    /// Configure buffer for memory-constrained environments
    pub fn low_memory(self) -> Self {
        self.buffer_preset(BufferPresets::MEMORY_CONSTRAINED)
    }
}
```

#### 9.5.3 Performance Monitoring

```rust
// Buffer performance metrics

#[derive(Debug, Default)]
pub struct BufferStats {
    /// Total bytes written to buffer
    pub bytes_written: u64,
    /// Total bytes read/consumed from buffer
    pub bytes_read: u64,
    /// Total bytes discarded due to overflow
    pub bytes_discarded: u64,
    /// Number of times buffer reached max capacity
    pub overflow_count: u64,
    /// Peak buffer utilization (bytes)
    pub peak_size: usize,
    /// Average buffer utilization
    pub avg_size: f64,
}

impl OutputBuffer {
    /// Get current buffer statistics
    pub fn stats(&self) -> BufferStats {
        BufferStats {
            bytes_written: self.total_written,
            bytes_read: self.total_read,
            bytes_discarded: self.total_discarded,
            overflow_count: self.overflow_count,
            peak_size: self.peak_size,
            avg_size: self.calculate_avg_size(),
        }
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.total_written = 0;
        self.total_read = 0;
        self.total_discarded = 0;
        self.overflow_count = 0;
        // Keep peak_size for debugging
    }
}
```

---

## 10. Error Handling

### 10.1 Error Type Hierarchy

```rust
// crates/rust-expect/src/error.rs

use thiserror::Error;
use std::time::Duration;
use std::process::ExitStatus;

#[derive(Error, Debug)]
pub enum Error {
    /// No command specified in builder
    #[error("no command specified")]
    NoCommand,

    /// Failed to spawn process
    #[error("failed to spawn process: {0}")]
    Spawn(#[source] rust_pty::PtyError),

    /// I/O error during session operation
    #[error("I/O error: {0}")]
    Io(#[source] std::io::Error),

    /// Pattern matching timed out
    #[error("timeout after {duration:?} waiting for pattern '{pattern}'")]
    Timeout {
        duration: Duration,
        pattern: String,
        /// Buffer contents at timeout for debugging
        buffer: String,
    },

    /// EOF reached before pattern matched
    #[error("EOF reached before pattern matched")]
    Eof {
        /// Buffer contents at EOF
        buffer: String,
    },

    /// Process exited before pattern matched
    #[error("process exited with {exit_status:?} before pattern matched")]
    ProcessExited {
        exit_status: ExitStatus,
        /// Buffer contents at exit
        buffer: String,
    },

    /// Pattern not found in buffer
    #[error("pattern '{pattern}' not found")]
    PatternNotFound {
        pattern: String,
        buffer: String,
    },

    /// Invalid pattern specification
    #[error("invalid pattern: {0}")]
    InvalidPattern(String),

    /// Terminal error during interact
    #[error("terminal error: {0}")]
    Terminal(#[source] crossterm::ErrorKind),

    /// SSH-specific error (feature-gated)
    #[cfg(feature = "ssh")]
    #[error("SSH error: {0}")]
    Ssh(#[source] russh::Error),

    /// Session is closed
    #[error("session is closed")]
    SessionClosed,

    /// Invalid configuration
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
}

impl Error {
    /// Get the buffer contents if available
    pub fn buffer(&self) -> Option<&str> {
        match self {
            Error::Timeout { buffer, .. } |
            Error::Eof { buffer } |
            Error::ProcessExited { buffer, .. } |
            Error::PatternNotFound { buffer, .. } => Some(buffer),
            _ => None,
        }
    }

    /// Check if this is a timeout error
    pub fn is_timeout(&self) -> bool {
        matches!(self, Error::Timeout { .. })
    }

    /// Check if this is an EOF error
    pub fn is_eof(&self) -> bool {
        matches!(self, Error::Eof { .. })
    }
}

// Convenient Result type alias
pub type Result<T> = std::result::Result<T, Error>;
```

### 10.2 Error Context with Tracing

```rust
// Error logging integration with tracing

use tracing::{error, warn, debug, instrument};

impl<B: Backend> Session<B> {
    #[instrument(skip(self), fields(session_id = %self.id))]
    pub async fn expect(&mut self, pattern: impl Into<Pattern>) -> Result<MatchResult> {
        let pattern = pattern.into();
        debug!(?pattern, "starting expect");

        match self.expect_inner(&pattern).await {
            Ok(result) => {
                debug!(?result, "pattern matched");
                Ok(result)
            }
            Err(e) => {
                // Log with appropriate level based on error type
                match &e {
                    Error::Timeout { duration, .. } => {
                        warn!(?duration, "expect timed out");
                    }
                    Error::Eof { .. } => {
                        debug!("EOF during expect");
                    }
                    _ => {
                        error!(?e, "expect failed");
                    }
                }
                Err(e)
            }
        }
    }
}
```

---

## 11. Feature Flags

### 11.1 rust-pty Cargo.toml

```toml
[package]
name = "rust-pty"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[features]
default = []
# No optional features - core PTY is always available

[dependencies]
tokio = { workspace = true, features = ["io-util", "sync", "time", "rt"] }
thiserror = { workspace = true }

[target.'cfg(unix)'.dependencies]
rustix = { workspace = true }
signal-hook = "0.3"
signal-hook-tokio = { version = "0.3", features = ["futures-v0_3"] }

[target.'cfg(windows)'.dependencies]
windows-sys = { workspace = true }
```

### 11.2 rust-expect Cargo.toml

```toml
[package]
name = "rust-expect"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[features]
default = ["sync", "tracing"]

## Synchronous API wrapper (uses internal tokio runtime)
sync = []

## Tokio async runtime support
async-tokio = ["tokio/full"]

## SSH integration via russh
ssh = ["russh", "russh-keys"]

## ANSI parsing and virtual screen buffer
screen = ["vte"]

## Structured logging via tracing
tracing = ["dep:tracing"]

## Observability metrics (Prometheus/OpenTelemetry)
metrics = ["dep:metrics", "dep:opentelemetry", "dep:prometheus"]

## Auto-detect and redact PII patterns in transcripts
pii-redaction = []

## MockSession backend for deterministic testing
mock = []

## All features
full = ["sync", "async-tokio", "ssh", "screen", "tracing", "metrics"]

[dependencies]
rust-pty = { workspace = true }
rust-expect-macros = { workspace = true }
tokio = { workspace = true }
regex = { workspace = true }
thiserror = { workspace = true }
crossterm = { workspace = true }

# Optional dependencies
tracing = { workspace = true, optional = true }
vte = { workspace = true, optional = true }
russh = { workspace = true, optional = true }
russh-keys = { workspace = true, optional = true }

[dev-dependencies]
tokio = { workspace = true, features = ["test-util", "macros"] }
proptest = { workspace = true }
```

### 11.3 Feature Flag Effects

| Feature | Effect |
|---------|--------|
| `sync` | Enables `SyncSession` wrapper with internal tokio runtime |
| `async-tokio` | Full tokio features for standalone async usage |
| `ssh` | Enables `SshSession` and SSH-related APIs |
| `screen` | Enables ANSI parsing, `Screen` buffer, screen-based matching |
| `tracing` | Enables structured logging throughout the library |
| `metrics` | Prometheus/OpenTelemetry metrics export (session counts, latency histograms, error rates) |
| `pii-redaction` | Auto-detect and redact sensitive patterns (credit cards, SSNs, API keys) in transcripts |
| `mock` | Enables `MockSession` backend for deterministic testing without real processes |
| `full` | All features enabled (except `mock` and `pii-redaction` which are opt-in) |

---

## 12. Dependencies

### 12.1 Core Dependencies

| Crate | Version | Purpose | License |
|-------|---------|---------|---------|
| `tokio` | 1.43+ | Async runtime | MIT |
| `regex` | 1.12+ | Pattern matching | MIT/Apache-2.0 |
| `thiserror` | 2.0+ | Error derive macros | MIT/Apache-2.0 |
| `crossterm` | 0.29+ | Terminal manipulation | MIT |

### 12.2 Platform-Specific Dependencies

| Crate | Platform | Purpose | License |
|-------|----------|---------|---------|
| `rustix` | Unix | Safe syscall bindings | Apache-2.0/MIT |
| `signal-hook` | Unix | Signal handling | MIT/Apache-2.0 |
| `signal-hook-tokio` | Unix | Async signal handling | MIT/Apache-2.0 |
| `windows-sys` | Windows | Win32 API bindings | MIT/Apache-2.0 |

### 12.3 Optional Dependencies

| Crate | Feature | Purpose | License |
|-------|---------|---------|---------|
| `tracing` | `tracing` | Structured logging | MIT |
| `vte` | `screen` | ANSI parser | MIT/Apache-2.0 |
| `russh` | `ssh` | SSH client | Apache-2.0 |
| `russh-keys` | `ssh` | SSH key handling | Apache-2.0 |

### 12.4 Development Dependencies

| Crate | Purpose |
|-------|---------|
| `proptest` | Property-based testing |
| `criterion` | Benchmarking |
| `tokio-test` | Async test utilities |

---

## 13. Data Flow

### 13.1 Spawn Flow

```
User Code                    rust-expect                      rust-pty                    OS
    │                            │                                │                        │
    │ Session::builder()         │                                │                        │
    │   .command("bash")         │                                │                        │
    │   .spawn()                 │                                │                        │
    │ ──────────────────────────►│                                │                        │
    │                            │ PtyConfig                      │                        │
    │                            │ ──────────────────────────────►│                        │
    │                            │                                │ allocate_pty()         │
    │                            │                                │ ──────────────────────►│
    │                            │                                │◄─────────────────────── │
    │                            │                                │ (master_fd, slave_path)│
    │                            │                                │                        │
    │                            │                                │ fork() / CreateProcess │
    │                            │                                │ ──────────────────────►│
    │                            │                                │◄─────────────────────── │
    │                            │                                │ (child_pid)            │
    │                            │ (PtyMaster, PtyChild)          │                        │
    │                            │◄─────────────────────────────── │                        │
    │ Session                    │                                │                        │
    │◄────────────────────────── │                                │                        │
```

### 13.2 Expect Flow

```
User Code                    Session                      Pattern Matcher              PTY
    │                            │                                │                        │
    │ session.expect("$")        │                                │                        │
    │ ──────────────────────────►│                                │                        │
    │                            │ try_match(buffer)              │                        │
    │                            │ ──────────────────────────────►│                        │
    │                            │◄─────────────────────────────── │                        │
    │                            │ None (no match yet)            │                        │
    │                            │                                │                        │
    │                            │ read()                         │                        │
    │                            │ ────────────────────────────────────────────────────────►
    │                            │◄──────────────────────────────────────────────────────── │
    │                            │ bytes                          │                        │
    │                            │                                │                        │
    │                            │ buffer.extend(bytes)           │                        │
    │                            │ try_match(buffer)              │                        │
    │                            │ ──────────────────────────────►│                        │
    │                            │◄─────────────────────────────── │                        │
    │                            │ Some(MatchResult)              │                        │
    │                            │                                │                        │
    │                            │ buffer.consume(match.end)      │                        │
    │ MatchResult                │                                │                        │
    │◄────────────────────────── │                                │                        │
```

### 13.3 Interact Flow

```
User Terminal               Session::interact()              PTY Backend
      │                            │                                │
      │ (raw mode enabled)         │                                │
      │                            │                                │
      │ keystroke                  │                                │
      │ ──────────────────────────►│                                │
      │                            │ (apply input hooks)            │
      │                            │ write(keystroke)               │
      │                            │ ──────────────────────────────►│
      │                            │                                │
      │                            │◄─────────────────────────────── │
      │                            │ read() → output                │
      │                            │ (apply output hooks)           │
      │◄────────────────────────── │                                │
      │ output to stdout           │                                │
      │                            │                                │
      │ escape char (Ctrl+])       │                                │
      │ ──────────────────────────►│                                │
      │                            │ (exit interact)                │
      │ (raw mode disabled)        │                                │
      │ InteractResult::Escaped    │                                │
      │◄────────────────────────── │                                │
```

---

## 14. Testing Architecture

### 14.1 Test Categories

```
tests/
├── unit/                    # Fast, isolated unit tests
│   ├── pattern_tests.rs     # Pattern matching logic
│   ├── buffer_tests.rs      # Buffer management
│   └── config_tests.rs      # Configuration parsing
│
├── integration/             # Full session lifecycle tests
│   ├── spawn_tests.rs       # Process spawning
│   ├── expect_tests.rs      # Pattern matching with real processes
│   ├── send_tests.rs        # Sending data
│   ├── interact_tests.rs    # Interactive mode
│   └── multi_tests.rs       # Multi-session
│
├── platform/                # Platform-specific tests
│   ├── unix_tests.rs        # Unix-specific behavior
│   └── windows_tests.rs     # Windows-specific behavior
│
├── stress/                  # Stress and performance tests
│   ├── large_output.rs      # 100MB, 1GB output handling
│   ├── many_sessions.rs     # 100+ concurrent sessions
│   └── rapid_io.rs          # High-frequency I/O
│
└── property/                # Property-based tests
    ├── pattern_props.rs     # Arbitrary patterns
    └── buffer_props.rs      # Arbitrary byte sequences
```

### 14.2 Test Fixture Binaries

Purpose-built deterministic test binaries in `test-utils/`:

```rust
// test-utils/test-echo/src/main.rs
//
// Echoes input with configurable delay and formatting

use std::io::{self, BufRead, Write};
use std::time::Duration;
use std::thread;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let delay_ms: u64 = args.get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line.expect("failed to read line");

        if delay_ms > 0 {
            thread::sleep(Duration::from_millis(delay_ms));
        }

        writeln!(stdout, "{line}").expect("failed to write");
        stdout.flush().expect("failed to flush");
    }
}
```

```rust
// test-utils/test-prompt/src/main.rs
//
// Simulates login prompts for testing expect patterns

use std::io::{self, Write};

fn main() {
    let mut stdout = io::stdout();
    let stdin = io::stdin();
    let mut input = String::new();

    // Username prompt
    write!(stdout, "login: ").unwrap();
    stdout.flush().unwrap();
    stdin.read_line(&mut input).unwrap();
    let username = input.trim().to_string();
    input.clear();

    // Password prompt (no echo in real scenario)
    write!(stdout, "Password: ").unwrap();
    stdout.flush().unwrap();
    stdin.read_line(&mut input).unwrap();
    let password = input.trim().to_string();

    // Simulate authentication
    if username == "testuser" && password == "testpass" {
        writeln!(stdout, "Welcome, {username}!").unwrap();
        writeln!(stdout, "$ ").unwrap();
    } else {
        writeln!(stdout, "Login incorrect").unwrap();
        std::process::exit(1);
    }

    stdout.flush().unwrap();
}
```

### 14.3 Test Utilities

```rust
// tests/common/mod.rs

use rust_expect::{Session, Result};
use std::time::Duration;

/// Standard timeout for tests (generous to avoid CI flakiness)
pub const TEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Spawn a test session with standard configuration
pub async fn test_session(command: &str) -> Result<Session> {
    Session::builder()
        .command(command)
        .timeout(TEST_TIMEOUT)
        .dimensions(80, 24)
        .env("TERM", "dumb")  // Disable ANSI in tests where not needed
        .spawn()
        .await
}

/// Path to test fixture binary
pub fn test_binary(name: &str) -> String {
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // Remove test binary name
    path.pop(); // Remove 'deps' directory
    path.push(name);

    #[cfg(windows)]
    path.set_extension("exe");

    path.to_string_lossy().to_string()
}

/// Assert that a session matches a pattern within timeout
#[macro_export]
macro_rules! assert_expect {
    ($session:expr, $pattern:expr) => {
        $session.expect($pattern).await.expect(concat!(
            "expected pattern '", stringify!($pattern), "' not found"
        ))
    };
}
```

### 14.4 CI Matrix

```yaml
# .github/workflows/ci.yml

name: CI

on: [push, pull_request]

jobs:
  test:
    strategy:
      matrix:
        include:
          # Linux x86_64
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu

          # Linux ARM64
          - os: ubuntu-24.04-arm
            target: aarch64-unknown-linux-gnu

          # macOS Intel
          - os: macos-13
            target: x86_64-apple-darwin

          # macOS Apple Silicon
          - os: macos-latest
            target: aarch64-apple-darwin

          # Windows
          - os: windows-latest
            target: x86_64-pc-windows-msvc

    runs-on: ${{ matrix.os }}

    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Build
        run: cargo build --all-features --target ${{ matrix.target }}

      - name: Test
        run: cargo test --all-features --target ${{ matrix.target }}

      - name: Test (minimal features)
        run: cargo test --no-default-features --target ${{ matrix.target }}

  msrv:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.85
      - run: cargo check --all-features

  clippy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - run: cargo clippy --all-features -- -D warnings

  doc:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo doc --all-features --no-deps
        env:
          RUSTDOCFLAGS: -D warnings
```

---

## 15. Security Considerations

### 15.1 Threat Model

| Threat | Mitigation |
|--------|------------|
| Command injection via spawn | Use argument arrays, not shell strings; validate input |
| Credential exposure in logs | Credentials not logged by default; redaction support |
| Buffer overflow | Rust memory safety; bounded buffers with configurable limits |
| Resource exhaustion | Configurable limits; automatic cleanup on drop |
| Zombie processes | Proper wait() on all child processes; Job Objects on Windows |
| Unsafe code bugs | Minimal unsafe (only for FFI); documented safety invariants |

### 15.2 Security Best Practices

```rust
// Credential redaction in logs

impl Session<B> {
    pub fn set_log_redaction(&mut self, patterns: &[&str]) {
        for pattern in patterns {
            self.transcript.add_redaction(pattern);
        }
    }
}

// Usage
session.set_log_redaction(&["password", "secret", "token"]);
```

### 15.3 Unsafe Code Policy

- **rust-pty**: Minimal unsafe for FFI calls to OS APIs
  - Unix: `fork()`, `ioctl()` via rustix (mostly safe wrappers)
  - Windows: Win32 API calls via windows-sys
  - All unsafe blocks documented with safety invariants

- **rust-expect**: Zero unsafe code
  - All platform-specific code delegated to rust-pty
  - Pattern matching uses safe regex crate

- **Audit**: Critical unsafe blocks tagged for periodic review

```rust
// Example documented unsafe block

/// # Safety
///
/// This is safe because:
/// - `master_fd` is a valid, open file descriptor
/// - We check the return value and handle errors
/// - The ioctl request TIOCSWINSZ is appropriate for PTY master
unsafe fn set_window_size(master_fd: RawFd, cols: u16, rows: u16) -> io::Result<()> {
    let ws = libc::winsize {
        ws_col: cols,
        ws_row: rows,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };

    let result = libc::ioctl(master_fd, libc::TIOCSWINSZ, &ws);

    if result == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
```

---

## 16. Performance Baselines

This section establishes performance targets and measurement methodology for rust-expect. These baselines inform optimization decisions and provide regression detection thresholds.

### 16.1 Latency Targets

| Operation | Target | Stretch Goal | Notes |
|-----------|--------|--------------|-------|
| PTY spawn (Unix) | < 50ms | < 5ms | Stretch requires warm fork pool, pre-allocated PTY |
| PTY spawn (Windows) | < 50ms | < 10ms | ConPTY creation is heavier |
| Pattern match (literal) | < 1µs | < 100ns | Stretch via `memchr` SIMD acceleration |
| Pattern match (regex) | < 50µs | < 10µs | Depends on pattern complexity |
| Regex compilation | < 1ms | < 500µs | Cached with LRU eviction after first use |
| PTY read (small) | < 100µs | < 50µs | < 4KB, includes syscall overhead |
| PTY write | < 50µs | < 20µs | Includes kernel buffer copy |
| Session close | < 10ms | < 5ms | Includes process termination |
| SSH connect | < 500ms | < 200ms | Network-dependent |
| SSH auth (key) | < 100ms | < 50ms | Crypto operation dependent |

**Reference:** Linux pipe round-trip latency is approximately 500µs including context switch overhead ([source](https://manpages.ubuntu.com/manpages/xenial/lat_pipe.8.html)).

### 16.2 Throughput Targets

| Scenario | Target | Stretch Goal | Notes |
|----------|--------|--------------|-------|
| PTY read throughput | > 100 MB/s | > 500 MB/s | Large continuous output |
| Pattern scan rate (literal) | > 100 MB/s | > 1 GB/s | Stretch via `memchr` SIMD; zero-copy buffer views |
| Pattern scan rate (regex) | > 50 MB/s | > 100 MB/s | Regex caching; DFA compilation |
| SSH channel throughput | > 50 MB/s | > 100 MB/s | Encrypted, network-bound |
| Concurrent sessions | > 1,000 | > 10,000 | Stretch requires shared thread pool; no per-session threads |

**Reference:** Linux pipes can achieve 17 GB/s throughput ([source](https://mazzo.li/posts/fast-pipes.html)), though real-world PTY throughput is lower due to terminal emulation overhead.

### 16.2.1 Performance Implementation Notes

**Achieving Stretch Goals:**

| Goal | Implementation Strategy |
|------|------------------------|
| **< 5ms spawn latency** | Warm fork pool with pre-forked worker processes; pre-allocated PTY pairs |
| **> 1 GB/s literal matching** | Use `memchr` crate for SIMD-accelerated byte scanning; zero-copy buffer views |
| **> 100 MB/s regex** | `RegexCache` with LRU eviction (Section 6.3.2); avoid recompilation |
| **> 10,000 sessions** | Shared thread pool for Windows ConPTY; avoid per-session thread overhead |
| **< 64 KB per session** | Lazy buffer allocation; small-buffer optimization for typical workloads |

**Dependency Notes:**
- `memchr` crate provides cross-platform SIMD acceleration (SSE2, AVX2, NEON)
- Ring buffer implementation uses `VecDeque` for < 10MB, `MmapBuffer` for larger outputs (Section 6.4.1)

### 16.3 Memory Targets

| Component | Target | Maximum | Notes |
|-----------|--------|---------|-------|
| Session overhead | < 8 KB | < 32 KB | Excluding output buffer |
| Default output buffer | 1 MB | Configurable | Ring buffer |
| Regex cache entry | < 100 KB | < 1 MB | Per compiled pattern |
| Screen buffer (80x24) | < 8 KB | < 16 KB | With attributes |
| SSH session overhead | < 64 KB | < 256 KB | Crypto state |

### 16.4 Benchmarking Methodology

#### 16.4.1 Benchmark Suite Structure

```
benches/
├── spawn/
│   ├── unix_spawn.rs       # PTY allocation + fork
│   ├── windows_spawn.rs    # ConPTY creation
│   └── ssh_connect.rs      # SSH connection establishment
├── pattern/
│   ├── literal_match.rs    # Literal string matching
│   ├── regex_match.rs      # Regex pattern matching
│   └── streaming.rs        # Incremental buffer matching
├── throughput/
│   ├── read_large.rs       # Large output handling
│   ├── write_burst.rs      # Burst write patterns
│   └── concurrent.rs       # Many simultaneous sessions
└── memory/
    ├── session_overhead.rs # Per-session memory
    └── buffer_growth.rs    # Buffer scaling behavior
```

#### 16.4.2 Benchmark Implementation

```rust
// benches/pattern/literal_match.rs

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use rust_expect::pattern::{Pattern, Matcher};

fn bench_literal_match(c: &mut Criterion) {
    let mut group = c.benchmark_group("literal_match");

    // Various buffer sizes
    for size in [1_000, 10_000, 100_000, 1_000_000] {
        group.throughput(Throughput::Bytes(size as u64));

        let buffer = generate_buffer(size);
        let pattern = Pattern::literal("FOUND");
        let matcher = Matcher::new(&pattern);

        group.bench_with_input(
            format!("size_{size}"),
            &buffer,
            |b, buffer| {
                b.iter(|| {
                    black_box(matcher.find(buffer))
                });
            },
        );
    }

    group.finish();
}

fn bench_regex_match(c: &mut Criterion) {
    let mut group = c.benchmark_group("regex_match");

    let patterns = [
        ("simple", r"\d+"),
        ("medium", r"error:\s+\w+"),
        ("complex", r"(?i)failed.*connection.*timeout"),
    ];

    for (name, regex) in patterns {
        let buffer = generate_buffer(10_000);
        let pattern = Pattern::regex(regex).unwrap();
        let matcher = Matcher::new(&pattern);

        group.bench_function(name, |b| {
            b.iter(|| {
                black_box(matcher.find(&buffer))
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_literal_match, bench_regex_match);
criterion_main!(benches);
```

#### 16.4.3 Latency Measurement

```rust
// benches/spawn/unix_spawn.rs

use criterion::{criterion_group, criterion_main, Criterion};
use rust_pty::{PtySystem, PtyConfig};
use std::time::Instant;

fn bench_spawn_latency(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("pty_spawn", |b| {
        b.iter_custom(|iters| {
            rt.block_on(async {
                let mut total = std::time::Duration::ZERO;

                for _ in 0..iters {
                    let start = Instant::now();

                    let config = PtyConfig::builder()
                        .command("true")  // Minimal command
                        .build();

                    let (master, child) = PtySystem::spawn(config).await.unwrap();
                    total += start.elapsed();

                    // Clean up
                    drop(master);
                    child.wait().await.unwrap();
                }

                total
            })
        });
    });
}

criterion_group!(benches, bench_spawn_latency);
criterion_main!(benches);
```

#### 16.4.4 Continuous Benchmarking

```yaml
# .github/workflows/bench.yml

name: Benchmarks

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable

      - name: Run benchmarks
        run: cargo bench --bench '*' -- --save-baseline pr

      - name: Compare with main
        if: github.event_name == 'pull_request'
        run: |
          git fetch origin main
          git checkout origin/main
          cargo bench --bench '*' -- --save-baseline main
          git checkout -
          cargo bench --bench '*' -- --baseline main --load-baseline pr

      - name: Upload benchmark results
        uses: actions/upload-artifact@v4
        with:
          name: benchmark-results
          path: target/criterion
```

### 16.5 Performance Anti-Patterns

| Anti-Pattern | Problem | Solution |
|--------------|---------|----------|
| Compiling regex in loops | ~1ms per compilation | Pre-compile and cache |
| Unbounded buffer growth | Memory exhaustion | Use ring buffer with max size |
| Synchronous SSH in async context | Blocks executor | Use russh async APIs |
| Large pattern on every byte | O(n*m) per read | Use search windows |
| Spawning shell for simple commands | ~5-10ms overhead | Direct exec when possible |

### 16.6 Profiling Integration

```rust
// Optional profiling support via feature flag

#[cfg(feature = "profiling")]
use tracing::{instrument, Span};

#[cfg(feature = "profiling")]
impl<B: Backend> Session<B> {
    #[instrument(skip(self), fields(pattern = %pattern))]
    pub async fn expect(&mut self, pattern: impl Into<Pattern>) -> Result<MatchResult, Error> {
        let pattern = pattern.into();
        let span = Span::current();

        // Record pattern complexity
        span.record("regex", pattern.is_regex());
        span.record("length", pattern.source_len());

        self.expect_inner(pattern).await
    }
}

// Integration with profiling tools
#[cfg(feature = "profiling")]
pub fn enable_profiling() {
    // Support for:
    // - Tracy (tracy-client)
    // - perf (via tracing-subscriber)
    // - flamegraph generation
}
```

---

## 17. Encoding and Character Handling

### 17.1 Encoding Strategy

rust-expect operates on bytes by default, with optional UTF-8 interpretation for pattern matching and output display.

```rust
/// Encoding mode for session output
#[derive(Clone, Copy, Debug, Default)]
pub enum EncodingMode {
    /// Raw bytes, no interpretation
    #[default]
    Binary,
    /// UTF-8 with replacement for invalid sequences
    Utf8Lossy,
    /// UTF-8 with strict validation
    Utf8Strict,
    /// Latin-1 / ISO-8859-1
    Latin1,
    /// Custom encoding via callback
    Custom(fn(&[u8]) -> Cow<'_, str>),
}

impl SessionBuilder {
    /// Set the encoding mode for this session
    pub fn encoding(mut self, mode: EncodingMode) -> Self {
        self.encoding = mode;
        self
    }
}
```

### 17.2 Pattern Matching with Encodings

```rust
// crates/rust-expect/src/encoding.rs

use std::borrow::Cow;

/// Decoder for session output
pub struct Decoder {
    mode: EncodingMode,
    /// Incomplete UTF-8 sequence from previous chunk
    pending: Vec<u8>,
}

impl Decoder {
    pub fn new(mode: EncodingMode) -> Self {
        Self {
            mode,
            pending: Vec::new(),
        }
    }

    /// Decode bytes to string, handling partial sequences
    pub fn decode<'a>(&mut self, input: &'a [u8]) -> DecodedChunk<'a> {
        match self.mode {
            EncodingMode::Binary => {
                DecodedChunk {
                    text: None,
                    bytes: input,
                    incomplete: false,
                }
            }
            EncodingMode::Utf8Lossy => {
                let combined = self.combine_pending(input);
                let text = String::from_utf8_lossy(&combined);
                DecodedChunk {
                    text: Some(text),
                    bytes: input,
                    incomplete: false,
                }
            }
            EncodingMode::Utf8Strict => {
                let combined = self.combine_pending(input);
                match std::str::from_utf8(&combined) {
                    Ok(s) => DecodedChunk {
                        text: Some(Cow::Borrowed(s)),
                        bytes: input,
                        incomplete: false,
                    },
                    Err(e) => {
                        // Check if error is due to incomplete sequence at end
                        let valid_up_to = e.valid_up_to();
                        if e.error_len().is_none() && valid_up_to > 0 {
                            // Incomplete sequence at end
                            self.pending = combined[valid_up_to..].to_vec();
                            let valid = &combined[..valid_up_to];
                            DecodedChunk {
                                text: Some(Cow::Owned(
                                    std::str::from_utf8(valid).unwrap().to_string()
                                )),
                                bytes: &input[..input.len() - self.pending.len()],
                                incomplete: true,
                            }
                        } else {
                            // Invalid sequence
                            DecodedChunk {
                                text: None,
                                bytes: input,
                                incomplete: false,
                            }
                        }
                    }
                }
            }
            EncodingMode::Latin1 => {
                // Latin-1 is a direct byte-to-codepoint mapping
                let text: String = input.iter().map(|&b| b as char).collect();
                DecodedChunk {
                    text: Some(Cow::Owned(text)),
                    bytes: input,
                    incomplete: false,
                }
            }
            EncodingMode::Custom(decoder) => {
                DecodedChunk {
                    text: Some(decoder(input)),
                    bytes: input,
                    incomplete: false,
                }
            }
        }
    }

    fn combine_pending(&mut self, input: &[u8]) -> Vec<u8> {
        if self.pending.is_empty() {
            input.to_vec()
        } else {
            let mut combined = std::mem::take(&mut self.pending);
            combined.extend_from_slice(input);
            combined
        }
    }
}

/// Result of decoding a chunk of bytes
pub struct DecodedChunk<'a> {
    /// Decoded text (if applicable)
    pub text: Option<Cow<'a, str>>,
    /// Original bytes
    pub bytes: &'a [u8],
    /// Whether there's an incomplete sequence pending
    pub incomplete: bool,
}
```

### 17.3 Terminal Control Sequences

```rust
// Handling of ANSI control sequences in pattern matching

/// Options for control sequence handling
#[derive(Clone, Copy, Debug, Default)]
pub enum ControlSequenceHandling {
    /// Keep all control sequences in output
    #[default]
    Preserve,
    /// Strip ANSI escape sequences before matching
    Strip,
    /// Strip and track cursor position
    Interpret,
}

impl SessionBuilder {
    /// Configure control sequence handling
    pub fn control_sequences(mut self, handling: ControlSequenceHandling) -> Self {
        self.control_handling = handling;
        self
    }
}

/// Strip ANSI escape sequences from text
pub fn strip_ansi(input: &str) -> Cow<'_, str> {
    // Regex for ANSI escape sequences:
    // ESC [ ... letter  (CSI sequences)
    // ESC ] ... BEL/ST  (OSC sequences)
    // ESC ( letter      (Character set)
    // etc.

    lazy_static::lazy_static! {
        static ref ANSI_RE: regex::Regex = regex::Regex::new(
            r"\x1b\[[0-9;]*[A-Za-z]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b[()][AB012]"
        ).unwrap();
    }

    ANSI_RE.replace_all(input, "")
}
```

### 17.4 Unicode Normalization

```rust
// Optional Unicode normalization for pattern matching

#[cfg(feature = "unicode-normalization")]
use unicode_normalization::UnicodeNormalization;

/// Unicode normalization forms
#[derive(Clone, Copy, Debug)]
pub enum NormalizationForm {
    /// No normalization
    None,
    /// Canonical decomposition (NFD)
    Nfd,
    /// Canonical composition (NFC)
    Nfc,
    /// Compatibility decomposition (NFKD)
    Nfkd,
    /// Compatibility composition (NFKC)
    Nfkc,
}

#[cfg(feature = "unicode-normalization")]
pub fn normalize(text: &str, form: NormalizationForm) -> Cow<'_, str> {
    match form {
        NormalizationForm::None => Cow::Borrowed(text),
        NormalizationForm::Nfd => Cow::Owned(text.nfd().collect()),
        NormalizationForm::Nfc => Cow::Owned(text.nfc().collect()),
        NormalizationForm::Nfkd => Cow::Owned(text.nfkd().collect()),
        NormalizationForm::Nfkc => Cow::Owned(text.nfkc().collect()),
    }
}
```

---

## 18. Observability and Metrics

### 18.1 Metrics Architecture

rust-expect provides optional metrics collection via the `metrics` crate facade, allowing integration with any metrics backend (Prometheus, StatsD, etc.).

```rust
// crates/rust-expect/src/metrics.rs

use std::time::Instant;

#[cfg(feature = "metrics")]
use metrics::{counter, gauge, histogram};

/// Metrics for session operations
pub struct SessionMetrics {
    session_id: String,
}

impl SessionMetrics {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
        }

        #[cfg(feature = "metrics")]
        {
            gauge!("rust_expect_active_sessions").increment(1.0);
        }
    }

    pub fn record_spawn(&self, duration: std::time::Duration, success: bool) {
        #[cfg(feature = "metrics")]
        {
            histogram!("rust_expect_spawn_duration_seconds")
                .record(duration.as_secs_f64());

            if success {
                counter!("rust_expect_spawns_total", "status" => "success").increment(1);
            } else {
                counter!("rust_expect_spawns_total", "status" => "failure").increment(1);
            }
        }
    }

    pub fn record_expect(&self, pattern: &str, duration: std::time::Duration, matched: bool) {
        #[cfg(feature = "metrics")]
        {
            histogram!("rust_expect_expect_duration_seconds")
                .record(duration.as_secs_f64());

            let status = if matched { "matched" } else { "timeout" };
            counter!("rust_expect_expects_total", "status" => status).increment(1);
        }
    }

    pub fn record_send(&self, bytes: usize) {
        #[cfg(feature = "metrics")]
        {
            counter!("rust_expect_bytes_sent_total").increment(bytes as u64);
        }
    }

    pub fn record_receive(&self, bytes: usize) {
        #[cfg(feature = "metrics")]
        {
            counter!("rust_expect_bytes_received_total").increment(bytes as u64);
        }
    }

    pub fn record_buffer_size(&self, size: usize) {
        #[cfg(feature = "metrics")]
        {
            gauge!("rust_expect_buffer_bytes", "session" => self.session_id.clone())
                .set(size as f64);
        }
    }
}

impl Drop for SessionMetrics {
    fn drop(&mut self) {
        #[cfg(feature = "metrics")]
        {
            gauge!("rust_expect_active_sessions").decrement(1.0);
        }
    }
}
```

### 18.2 Available Metrics

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `rust_expect_active_sessions` | Gauge | - | Currently active sessions |
| `rust_expect_spawns_total` | Counter | `status` | Total spawn attempts |
| `rust_expect_spawn_duration_seconds` | Histogram | - | Spawn latency |
| `rust_expect_expects_total` | Counter | `status` | Total expect operations |
| `rust_expect_expect_duration_seconds` | Histogram | - | Expect latency |
| `rust_expect_bytes_sent_total` | Counter | - | Bytes sent to processes |
| `rust_expect_bytes_received_total` | Counter | - | Bytes received from processes |
| `rust_expect_buffer_bytes` | Gauge | `session` | Current buffer size per session |
| `rust_expect_pattern_match_duration_seconds` | Histogram | `type` | Pattern matching latency |
| `rust_expect_ssh_connections_total` | Counter | `status` | SSH connection attempts |
| `rust_expect_ssh_auth_duration_seconds` | Histogram | `method` | SSH auth latency |

### 18.3 Tracing Integration

```rust
// Extended tracing spans beyond basic logging

use tracing::{info_span, Instrument};

impl<B: Backend> Session<B> {
    pub async fn expect_with_tracing(
        &mut self,
        pattern: impl Into<Pattern>,
    ) -> Result<MatchResult, Error> {
        let pattern = pattern.into();

        let span = info_span!(
            "expect",
            pattern = %pattern,
            pattern_type = ?pattern.pattern_type(),
            otel.kind = "client",
            otel.status_code = tracing::field::Empty,
        );

        async {
            let start = Instant::now();
            let result = self.expect_inner(pattern).await;
            let duration = start.elapsed();

            match &result {
                Ok(m) => {
                    tracing::Span::current().record("otel.status_code", "OK");
                    tracing::info!(
                        matched_at = m.end,
                        duration_ms = duration.as_millis() as u64,
                        "pattern matched"
                    );
                }
                Err(e) => {
                    tracing::Span::current().record("otel.status_code", "ERROR");
                    tracing::warn!(
                        error = %e,
                        duration_ms = duration.as_millis() as u64,
                        "expect failed"
                    );
                }
            }

            result
        }
        .instrument(span)
        .await
    }
}
```

### 18.4 Health Check Endpoint

For applications exposing rust-expect sessions as services:

```rust
// crates/rust-expect/src/health.rs

use std::time::{Duration, Instant};

/// Health status of a session
#[derive(Clone, Debug)]
pub struct HealthStatus {
    pub healthy: bool,
    pub last_activity: Instant,
    pub idle_duration: Duration,
    pub buffer_size: usize,
    pub process_alive: bool,
    pub error_count: u64,
    pub details: Option<String>,
}

impl<B: Backend> Session<B> {
    /// Check session health
    pub async fn health_check(&mut self) -> HealthStatus {
        let now = Instant::now();
        let idle = now.duration_since(self.last_activity);
        let process_alive = self.is_alive().await;

        HealthStatus {
            healthy: process_alive && idle < Duration::from_secs(300),
            last_activity: self.last_activity,
            idle_duration: idle,
            buffer_size: self.buffer.len(),
            process_alive,
            error_count: self.error_count,
            details: None,
        }
    }

    /// Check if the underlying process is still running
    pub async fn is_alive(&mut self) -> bool {
        self.backend.is_alive().await
    }
}

/// Health aggregator for multiple sessions
pub struct HealthAggregator<B: Backend> {
    sessions: Vec<(String, Session<B>)>,
}

impl<B: Backend> HealthAggregator<B> {
    pub async fn check_all(&mut self) -> Vec<(String, HealthStatus)> {
        let mut results = Vec::with_capacity(self.sessions.len());

        for (name, session) in &mut self.sessions {
            let status = session.health_check().await;
            results.push((name.clone(), status));
        }

        results
    }

    pub async fn is_healthy(&mut self) -> bool {
        self.check_all().await.iter().all(|(_, s)| s.healthy)
    }
}
```

---

## 19. Configuration File Support

### 19.1 Configuration Schema

rust-expect supports declarative session configuration via TOML or YAML files.

```toml
# rust-expect.toml

[defaults]
timeout = "30s"
encoding = "utf8-lossy"
dimensions = { cols = 80, rows = 24 }
buffer_size = "1MB"

[sessions.db-server]
command = "ssh admin@db.example.com"
timeout = "60s"
env = { TERM = "xterm-256color" }

[sessions.db-server.expect]
login = "password:"
prompt = "\\$"

[sessions.web-server]
type = "ssh"
host = "web.example.com"
port = 22
username = "deploy"
auth = { type = "key", path = "~/.ssh/id_ed25519" }

[sessions.local-shell]
command = "bash"
dimensions = { cols = 120, rows = 40 }
```

### 19.2 Configuration Types

```rust
// crates/rust-expect/src/config.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Root configuration structure
#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub defaults: DefaultConfig,
    #[serde(default)]
    pub sessions: HashMap<String, SessionConfig>,
}

/// Default settings applied to all sessions
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct DefaultConfig {
    #[serde(default, with = "humantime_serde")]
    pub timeout: Option<Duration>,
    #[serde(default)]
    pub encoding: Option<String>,
    #[serde(default)]
    pub dimensions: Option<Dimensions>,
    #[serde(default, with = "bytesize_serde")]
    pub buffer_size: Option<u64>,
}

/// Session-specific configuration
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SessionConfig {
    /// Local command via PTY
    Command {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
        #[serde(default)]
        dimensions: Option<Dimensions>,
        #[serde(default, with = "humantime_serde")]
        timeout: Option<Duration>,
        #[serde(default)]
        expect: HashMap<String, String>,
    },
    /// SSH connection
    Ssh {
        host: String,
        #[serde(default = "default_ssh_port")]
        port: u16,
        username: String,
        auth: SshAuthConfig,
        #[serde(default)]
        dimensions: Option<Dimensions>,
        #[serde(default, with = "humantime_serde")]
        timeout: Option<Duration>,
    },
}

fn default_ssh_port() -> u16 {
    22
}

/// SSH authentication configuration
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SshAuthConfig {
    Password { password: String },
    Key {
        path: PathBuf,
        #[serde(default)]
        passphrase: Option<String>,
    },
    Agent,
}

/// Terminal dimensions
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Dimensions {
    pub cols: u16,
    pub rows: u16,
}

impl Default for Dimensions {
    fn default() -> Self {
        Self { cols: 80, rows: 24 }
    }
}
```

### 19.3 Configuration Loading

```rust
// crates/rust-expect/src/config/loader.rs

use std::path::Path;

impl Config {
    /// Load configuration from file (auto-detects format)
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)?;

        match path.extension().and_then(|e| e.to_str()) {
            Some("toml") => Self::from_toml(&content),
            Some("yaml") | Some("yml") => Self::from_yaml(&content),
            Some("json") => Self::from_json(&content),
            _ => {
                // Try formats in order
                Self::from_toml(&content)
                    .or_else(|_| Self::from_yaml(&content))
                    .or_else(|_| Self::from_json(&content))
            }
        }
    }

    pub fn from_toml(content: &str) -> Result<Self, ConfigError> {
        toml::from_str(content).map_err(ConfigError::Toml)
    }

    #[cfg(feature = "yaml")]
    pub fn from_yaml(content: &str) -> Result<Self, ConfigError> {
        serde_yaml::from_str(content).map_err(ConfigError::Yaml)
    }

    pub fn from_json(content: &str) -> Result<Self, ConfigError> {
        serde_json::from_str(content).map_err(ConfigError::Json)
    }

    /// Create a session from named configuration
    pub async fn spawn_session(&self, name: &str) -> Result<Session<impl Backend>, Error> {
        let session_config = self.sessions.get(name)
            .ok_or_else(|| Error::Config(format!("session '{}' not found", name)))?;

        match session_config {
            SessionConfig::Command { command, args, env, dimensions, timeout, .. } => {
                let mut builder = Session::builder().command(command);

                for arg in args {
                    builder = builder.arg(arg);
                }

                for (k, v) in env {
                    builder = builder.env(k, v);
                }

                if let Some(dim) = dimensions.or(self.defaults.dimensions) {
                    builder = builder.dimensions(dim.cols, dim.rows);
                }

                if let Some(t) = timeout.or(self.defaults.timeout) {
                    builder = builder.timeout(t);
                }

                builder.spawn().await
            }
            SessionConfig::Ssh { host, port, username, auth, dimensions, timeout } => {
                #[cfg(feature = "ssh")]
                {
                    let mut builder = SshSessionBuilder::new(host)
                        .port(*port)
                        .username(username);

                    builder = match auth {
                        SshAuthConfig::Password { password } => builder.password(password),
                        SshAuthConfig::Key { path, passphrase } => {
                            if let Some(pass) = passphrase {
                                builder.private_key_with_passphrase(path, pass)
                            } else {
                                builder.private_key(path)
                            }
                        }
                        SshAuthConfig::Agent => builder.agent(),
                    };

                    if let Some(dim) = dimensions.or(self.defaults.dimensions) {
                        builder = builder.dimensions(dim.cols, dim.rows);
                    }

                    if let Some(t) = timeout.or(self.defaults.timeout) {
                        builder = builder.timeout(t);
                    }

                    builder.spawn().await
                }
                #[cfg(not(feature = "ssh"))]
                {
                    Err(Error::Config("SSH support not enabled".into()))
                }
            }
        }
    }
}
```

### 19.4 Environment Variable Interpolation

```rust
// Support for environment variable substitution in config

use regex::Regex;
use std::env;

/// Expand environment variables in configuration strings
pub fn expand_env(input: &str) -> String {
    lazy_static::lazy_static! {
        static ref ENV_RE: Regex = Regex::new(r"\$\{([^}]+)\}|\$([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    }

    ENV_RE.replace_all(input, |caps: &regex::Captures| {
        let var_name = caps.get(1).or_else(|| caps.get(2)).unwrap().as_str();

        // Support default values: ${VAR:-default}
        if let Some((name, default)) = var_name.split_once(":-") {
            env::var(name).unwrap_or_else(|_| default.to_string())
        } else {
            env::var(var_name).unwrap_or_default()
        }
    }).into_owned()
}

// Example usage in config:
// command = "${SHELL:-/bin/bash}"
// host = "${DB_HOST}"
```

---

## 20. Transcript Logging

### 20.1 Transcript Format Specification

rust-expect supports multiple transcript formats for session recording, debugging, and audit trails.

#### 20.1.1 Native Format (NDJSON)

The native format uses newline-delimited JSON, compatible with standard log aggregation tools.

```json
{"v":1,"session":"a1b2c3","start":"2025-12-26T10:30:00Z","command":"bash","dimensions":[80,24]}
{"t":0.000,"e":"spawn","pid":12345}
{"t":0.050,"e":"recv","d":"$ "}
{"t":0.100,"e":"send","d":"ls -la\n"}
{"t":0.150,"e":"recv","d":"total 48\ndrwxr-xr-x  5 user user 4096 Dec 26 10:30 .\n"}
{"t":0.200,"e":"match","p":"\\$","at":45}
{"t":0.250,"e":"send","d":"exit\n"}
{"t":0.300,"e":"exit","code":0}
```

#### 20.1.2 Format Schema

```rust
// crates/rust-expect/src/transcript/format.rs

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Transcript file header (first line)
#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptHeader {
    /// Format version
    pub v: u32,
    /// Session identifier
    pub session: String,
    /// Session start timestamp (ISO 8601)
    pub start: String,
    /// Command that was executed
    pub command: String,
    /// Terminal dimensions [cols, rows]
    pub dimensions: [u16; 2],
    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Transcript event (subsequent lines)
#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptEvent {
    /// Time offset from session start (seconds)
    pub t: f64,
    /// Event type
    pub e: EventType,
    /// Event-specific data
    #[serde(flatten)]
    pub data: EventData,
}

/// Event types
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventType {
    /// Process spawned
    Spawn,
    /// Data received from process
    Recv,
    /// Data sent to process
    Send,
    /// Pattern matched
    Match,
    /// Timeout occurred
    Timeout,
    /// Window size changed
    Resize,
    /// Process exited
    Exit,
    /// Error occurred
    Error,
    /// Custom annotation
    Note,
}

/// Event-specific data
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EventData {
    Spawn { pid: u32 },
    Data { d: String },
    Match { p: String, at: usize },
    Resize { cols: u16, rows: u16 },
    Exit { code: i32 },
    Error { msg: String },
    Note { text: String },
    Empty {},
}
```

#### 20.1.3 Asciicast v2 Compatibility

For compatibility with [asciinema](https://docs.asciinema.org/manual/asciicast/v2/), rust-expect can export to asciicast v2 format:

```rust
// crates/rust-expect/src/transcript/asciicast.rs

use serde::Serialize;

/// Asciicast v2 header
#[derive(Debug, Serialize)]
pub struct AsciicastHeader {
    pub version: u32,  // Always 2
    pub width: u16,
    pub height: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<AsciicastEnv>,
}

#[derive(Debug, Serialize)]
pub struct AsciicastEnv {
    #[serde(rename = "SHELL")]
    pub shell: Option<String>,
    #[serde(rename = "TERM")]
    pub term: Option<String>,
}

/// Asciicast v2 event: [time, event_type, data]
/// For output: [1.234, "o", "output text"]
/// For input:  [1.234, "i", "input text"]
pub type AsciicastEvent = (f64, String, String);

/// Convert native transcript to asciicast format
pub fn to_asciicast(transcript: &Transcript) -> Vec<String> {
    let mut lines = Vec::new();

    // Header
    let header = AsciicastHeader {
        version: 2,
        width: transcript.dimensions.0,
        height: transcript.dimensions.1,
        timestamp: Some(transcript.start_time.timestamp()),
        duration: Some(transcript.duration().as_secs_f64()),
        command: Some(transcript.command.clone()),
        title: None,
        env: Some(AsciicastEnv {
            shell: std::env::var("SHELL").ok(),
            term: Some("xterm-256color".to_string()),
        }),
    };
    lines.push(serde_json::to_string(&header).unwrap());

    // Events
    for event in &transcript.events {
        let asciicast_event: Option<AsciicastEvent> = match &event.data {
            EventData::Data { d } if event.e == EventType::Recv => {
                Some((event.t, "o".to_string(), d.clone()))
            }
            EventData::Data { d } if event.e == EventType::Send => {
                Some((event.t, "i".to_string(), d.clone()))
            }
            _ => None,
        };

        if let Some(e) = asciicast_event {
            lines.push(serde_json::to_string(&e).unwrap());
        }
    }

    lines
}
```

### 20.2 Transcript Recording

```rust
// crates/rust-expect/src/transcript/recorder.rs

use std::io::Write;
use std::fs::File;
use std::time::Instant;

/// Transcript recorder attached to a session
pub struct TranscriptRecorder {
    /// Output writer
    writer: Box<dyn Write + Send>,
    /// Session start time
    start: Instant,
    /// Format to use
    format: TranscriptFormat,
    /// Redaction patterns
    redactions: Vec<regex::Regex>,
    /// Whether to include input (send) events
    record_input: bool,
}

/// Available transcript formats
#[derive(Clone, Copy, Debug, Default)]
pub enum TranscriptFormat {
    /// Native NDJSON format
    #[default]
    Native,
    /// Asciicast v2 format (.cast)
    Asciicast,
    /// Raw bytes (script-compatible)
    Raw,
}

impl TranscriptRecorder {
    /// Create a new recorder writing to a file
    pub fn to_file(path: impl AsRef<std::path::Path>) -> std::io::Result<Self> {
        let file = File::create(path)?;
        Ok(Self {
            writer: Box::new(std::io::BufWriter::new(file)),
            start: Instant::now(),
            format: TranscriptFormat::Native,
            redactions: Vec::new(),
            record_input: true,
        })
    }

    /// Create a recorder writing to a buffer
    pub fn to_buffer() -> (Self, std::sync::Arc<std::sync::Mutex<Vec<u8>>>) {
        let buffer = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let writer = BufferWriter(buffer.clone());
        (
            Self {
                writer: Box::new(writer),
                start: Instant::now(),
                format: TranscriptFormat::Native,
                redactions: Vec::new(),
                record_input: true,
            },
            buffer,
        )
    }

    /// Set the output format
    pub fn format(mut self, format: TranscriptFormat) -> Self {
        self.format = format;
        self
    }

    /// Add a redaction pattern
    pub fn redact(mut self, pattern: &str) -> Self {
        if let Ok(re) = regex::Regex::new(pattern) {
            self.redactions.push(re);
        }
        self
    }

    /// Disable input recording (for security)
    pub fn no_input(mut self) -> Self {
        self.record_input = false;
        self
    }

    /// Write the header
    pub fn write_header(&mut self, header: &TranscriptHeader) -> std::io::Result<()> {
        let line = serde_json::to_string(header)?;
        writeln!(self.writer, "{}", line)?;
        self.writer.flush()
    }

    /// Record an event
    pub fn record(&mut self, event: TranscriptEvent) -> std::io::Result<()> {
        // Skip input if disabled
        if !self.record_input && event.e == EventType::Send {
            return Ok(());
        }

        // Apply redactions
        let event = self.apply_redactions(event);

        let line = serde_json::to_string(&event)?;
        writeln!(self.writer, "{}", line)?;
        Ok(())
    }

    fn apply_redactions(&self, mut event: TranscriptEvent) -> TranscriptEvent {
        if let EventData::Data { ref mut d } = event.data {
            for re in &self.redactions {
                *d = re.replace_all(d, "[REDACTED]").into_owned();
            }
        }
        event
    }

    /// Get elapsed time since start
    pub fn elapsed(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }
}

struct BufferWriter(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl Write for BufferWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
```

### 20.3 Session Integration

```rust
// Integration with Session

impl SessionBuilder {
    /// Enable transcript recording to file
    pub fn transcript(mut self, path: impl AsRef<std::path::Path>) -> Self {
        self.transcript = Some(TranscriptRecorder::to_file(path).ok());
        self
    }

    /// Enable transcript with custom recorder
    pub fn transcript_recorder(mut self, recorder: TranscriptRecorder) -> Self {
        self.transcript = Some(Some(recorder));
        self
    }
}

impl<B: Backend> Session<B> {
    /// Get access to transcript recorder
    pub fn transcript(&self) -> Option<&TranscriptRecorder> {
        self.transcript.as_ref()
    }

    /// Add an annotation to the transcript
    pub fn annotate(&mut self, text: impl Into<String>) {
        if let Some(ref mut recorder) = self.transcript {
            let _ = recorder.record(TranscriptEvent {
                t: recorder.elapsed(),
                e: EventType::Note,
                data: EventData::Note { text: text.into() },
            });
        }
    }
}
```

### 20.4 Transcript Playback

```rust
// crates/rust-expect/src/transcript/player.rs

use std::io::BufRead;
use std::time::Duration;
use tokio::time::sleep;

/// Transcript player for replaying recorded sessions
pub struct TranscriptPlayer {
    events: Vec<TranscriptEvent>,
    speed: f64,
}

impl TranscriptPlayer {
    /// Load a transcript from file
    pub fn load(path: impl AsRef<std::path::Path>) -> std::io::Result<Self> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let mut lines = reader.lines();

        // Skip header
        let _header: TranscriptHeader = lines.next()
            .ok_or(std::io::ErrorKind::UnexpectedEof)??
            .parse()
            .map_err(|_| std::io::ErrorKind::InvalidData)?;

        let events: Vec<TranscriptEvent> = lines
            .filter_map(|line| line.ok())
            .filter_map(|line| serde_json::from_str(&line).ok())
            .collect();

        Ok(Self { events, speed: 1.0 })
    }

    /// Set playback speed (1.0 = realtime, 2.0 = 2x speed)
    pub fn speed(mut self, speed: f64) -> Self {
        self.speed = speed;
        self
    }

    /// Play back the transcript, calling the callback for each output event
    pub async fn play<F>(&self, mut on_output: F)
    where
        F: FnMut(&str),
    {
        let mut last_time = 0.0;

        for event in &self.events {
            // Wait for appropriate delay
            let delay = (event.t - last_time) / self.speed;
            if delay > 0.0 {
                sleep(Duration::from_secs_f64(delay)).await;
            }
            last_time = event.t;

            // Handle output events
            if let EventData::Data { d } = &event.data {
                if event.e == EventType::Recv {
                    on_output(d);
                }
            }
        }
    }
}
```

### 20.5 PII Auto-Redaction

The library provides optional automatic detection and redaction of personally identifiable information (PII) in transcripts. This feature implements FR-3.7.7.

#### 20.5.1 Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            PII Redaction Pipeline                            │
├─────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                      Pattern Detectors                               │    │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌────────────┐  │    │
│  │  │ Credit Card │  │    SSN      │  │  API Keys   │  │  Custom    │  │    │
│  │  │   (Luhn)    │  │  (Format)   │  │  (Patterns) │  │  Patterns  │  │    │
│  │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └─────┬──────┘  │    │
│  └─────────┼────────────────┼────────────────┼───────────────┼─────────┘    │
│            │                │                │               │              │
│            ▼                ▼                ▼               ▼              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                        Redaction Engine                              │    │
│  │         Input: "Card: 4111111111111111"                              │    │
│  │         Output: "Card: [REDACTED:CREDIT_CARD]"                       │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### 20.5.2 PII Pattern Types

```rust
/// PII pattern types with detection strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiiType {
    /// Credit card numbers (validated with Luhn algorithm)
    CreditCard,
    /// US Social Security Numbers (XXX-XX-XXXX format)
    Ssn,
    /// API keys (various vendor patterns)
    ApiKey,
    /// Email addresses
    Email,
    /// Phone numbers (US format)
    PhoneNumber,
    /// IPv4 addresses
    IpAddress,
    /// Custom user-defined pattern
    Custom(&'static str),
}

impl PiiType {
    /// Redaction placeholder for each PII type
    pub fn placeholder(&self) -> &'static str {
        match self {
            PiiType::CreditCard => "[REDACTED:CREDIT_CARD]",
            PiiType::Ssn => "[REDACTED:SSN]",
            PiiType::ApiKey => "[REDACTED:API_KEY]",
            PiiType::Email => "[REDACTED:EMAIL]",
            PiiType::PhoneNumber => "[REDACTED:PHONE]",
            PiiType::IpAddress => "[REDACTED:IP]",
            PiiType::Custom(name) => name,
        }
    }
}
```

#### 20.5.3 Credit Card Detection with Luhn Validation

```rust
/// Detect and validate credit card numbers using Luhn algorithm
pub struct CreditCardDetector {
    /// Regex for potential credit card patterns
    pattern: Regex,
}

impl CreditCardDetector {
    pub fn new() -> Self {
        Self {
            // Match 13-19 digit sequences (with optional separators)
            pattern: Regex::new(r"\b(?:\d[ -]*?){13,19}\b").unwrap(),
        }
    }

    /// Check if a number passes the Luhn algorithm
    fn luhn_check(digits: &[u8]) -> bool {
        let mut sum = 0;
        let mut alternate = false;

        for &digit in digits.iter().rev() {
            let mut d = digit - b'0';
            if alternate {
                d *= 2;
                if d > 9 {
                    d -= 9;
                }
            }
            sum += d as u32;
            alternate = !alternate;
        }

        sum % 10 == 0
    }

    /// Detect credit card numbers in text
    pub fn detect(&self, text: &str) -> Vec<PiiMatch> {
        let mut matches = Vec::new();

        for m in self.pattern.find_iter(text) {
            // Extract digits only
            let digits: Vec<u8> = m.as_str()
                .bytes()
                .filter(|b| b.is_ascii_digit())
                .collect();

            // Validate length and Luhn check
            if digits.len() >= 13 && digits.len() <= 19 && Self::luhn_check(&digits) {
                matches.push(PiiMatch {
                    pii_type: PiiType::CreditCard,
                    start: m.start(),
                    end: m.end(),
                    matched_text: m.as_str().to_string(),
                });
            }
        }

        matches
    }
}
```

#### 20.5.4 API Key Detection Patterns

```rust
/// Common API key patterns by vendor
pub struct ApiKeyDetector {
    patterns: Vec<(Regex, &'static str)>,
}

impl ApiKeyDetector {
    pub fn new() -> Self {
        Self {
            patterns: vec![
                // AWS Access Key
                (Regex::new(r"AKIA[0-9A-Z]{16}").unwrap(), "AWS"),
                // GitHub Token
                (Regex::new(r"ghp_[a-zA-Z0-9]{36}").unwrap(), "GitHub"),
                (Regex::new(r"github_pat_[a-zA-Z0-9]{22}_[a-zA-Z0-9]{59}").unwrap(), "GitHub"),
                // Stripe Key
                (Regex::new(r"sk_live_[a-zA-Z0-9]{24,}").unwrap(), "Stripe"),
                (Regex::new(r"pk_live_[a-zA-Z0-9]{24,}").unwrap(), "Stripe"),
                // Slack Token
                (Regex::new(r"xox[baprs]-[0-9a-zA-Z-]+").unwrap(), "Slack"),
                // Generic high-entropy strings (32+ hex chars)
                (Regex::new(r"\b[a-fA-F0-9]{32,}\b").unwrap(), "Generic"),
            ],
        }
    }

    pub fn detect(&self, text: &str) -> Vec<PiiMatch> {
        let mut matches = Vec::new();

        for (pattern, _vendor) in &self.patterns {
            for m in pattern.find_iter(text) {
                matches.push(PiiMatch {
                    pii_type: PiiType::ApiKey,
                    start: m.start(),
                    end: m.end(),
                    matched_text: m.as_str().to_string(),
                });
            }
        }

        matches
    }
}
```

#### 20.5.5 SSN and Other Pattern Detection

```rust
/// Social Security Number detector
pub struct SsnDetector {
    pattern: Regex,
}

impl SsnDetector {
    pub fn new() -> Self {
        Self {
            // XXX-XX-XXXX format (with validation for valid area numbers)
            pattern: Regex::new(
                r"\b(?!000|666|9\d{2})\d{3}-(?!00)\d{2}-(?!0000)\d{4}\b"
            ).unwrap(),
        }
    }

    pub fn detect(&self, text: &str) -> Vec<PiiMatch> {
        self.pattern.find_iter(text)
            .map(|m| PiiMatch {
                pii_type: PiiType::Ssn,
                start: m.start(),
                end: m.end(),
                matched_text: m.as_str().to_string(),
            })
            .collect()
    }
}
```

#### 20.5.6 Redaction Engine

```rust
/// Configuration for PII redaction
#[derive(Debug, Clone)]
pub struct RedactionConfig {
    /// Enable credit card detection
    pub detect_credit_cards: bool,
    /// Enable SSN detection
    pub detect_ssn: bool,
    /// Enable API key detection
    pub detect_api_keys: bool,
    /// Enable email detection
    pub detect_email: bool,
    /// Custom patterns to detect
    pub custom_patterns: Vec<(Regex, String)>,
}

impl Default for RedactionConfig {
    fn default() -> Self {
        Self {
            detect_credit_cards: true,
            detect_ssn: true,
            detect_api_keys: true,
            detect_email: false,  // Off by default (often needed in transcripts)
            custom_patterns: Vec::new(),
        }
    }
}

/// PII redaction engine
#[cfg(feature = "pii-redaction")]
pub struct Redactor {
    config: RedactionConfig,
    credit_card: CreditCardDetector,
    ssn: SsnDetector,
    api_key: ApiKeyDetector,
}

impl Redactor {
    pub fn new(config: RedactionConfig) -> Self {
        Self {
            config,
            credit_card: CreditCardDetector::new(),
            ssn: SsnDetector::new(),
            api_key: ApiKeyDetector::new(),
        }
    }

    /// Redact all PII from text
    pub fn redact(&self, text: &str) -> String {
        let mut all_matches = Vec::new();

        if self.config.detect_credit_cards {
            all_matches.extend(self.credit_card.detect(text));
        }
        if self.config.detect_ssn {
            all_matches.extend(self.ssn.detect(text));
        }
        if self.config.detect_api_keys {
            all_matches.extend(self.api_key.detect(text));
        }

        // Sort by position (reverse) to replace from end to start
        all_matches.sort_by(|a, b| b.start.cmp(&a.start));

        let mut result = text.to_string();
        for m in all_matches {
            result.replace_range(m.start..m.end, m.pii_type.placeholder());
        }

        result
    }
}
```

#### 20.5.7 Transcript Integration

```rust
/// Transcript recorder with PII redaction
pub struct RedactingTranscriptRecorder {
    inner: TranscriptRecorder,
    redactor: Redactor,
}

impl RedactingTranscriptRecorder {
    pub fn new(path: impl AsRef<Path>, redactor: Redactor) -> io::Result<Self> {
        Ok(Self {
            inner: TranscriptRecorder::new(path)?,
            redactor,
        })
    }

    /// Record output with automatic PII redaction
    pub fn record_output(&mut self, data: &str) -> io::Result<()> {
        let redacted = self.redactor.redact(data);
        self.inner.record_output(&redacted)
    }

    /// Record input with automatic PII redaction
    pub fn record_input(&mut self, data: &str) -> io::Result<()> {
        let redacted = self.redactor.redact(data);
        self.inner.record_input(&redacted)
    }
}

// Usage example
let redactor = Redactor::new(RedactionConfig::default());
let recorder = RedactingTranscriptRecorder::new("session.log", redactor)?;

session.set_recorder(recorder);

// Now any PII in input/output is automatically redacted before logging
session.send_line("4111111111111111").await?;
// Logged as: [REDACTED:CREDIT_CARD]
```

#### 20.5.8 Performance Considerations

| Optimization | Technique |
|--------------|-----------|
| **Lazy initialization** | Pattern detectors compiled once on first use |
| **Early termination** | Skip detection if no potential matches (no digits for credit cards) |
| **Streaming mode** | Process line-by-line for large outputs |
| **Compiled regexes** | All patterns pre-compiled at startup |
| **Zero-copy where possible** | Return byte ranges, not copied strings |

---

## 21. Zero-Config Mode

Zero-Config Mode enables automatic detection and configuration to minimize boilerplate for common use cases. This section documents the architecture for FR-1.6.

### 21.1 Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            Session::auto(command)                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                              AutoConfig Engine                               │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │    Shell    │  │ Line Ending │  │   Prompt    │  │      Encoding       │ │
│  │  Detector   │  │  Detector   │  │  Detector   │  │      Detector       │ │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘ │
│         │                │                │                     │           │
│         ▼                ▼                ▼                     ▼           │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                         SessionConfig                                 │   │
│  │  shell: ShellType, line_ending: LineEnding, prompts: Vec<Pattern>,   │   │
│  │  encoding: Encoding, terminal_size: (u16, u16)                       │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 21.2 Shell Type Detection

The library auto-detects shell type from environment and command inspection.

#### 21.2.1 Detection Sources (Priority Order)

```rust
pub enum ShellType {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Cmd,
    Unknown(String),
}

impl ShellType {
    /// Detect shell type from command and environment
    pub fn detect(command: &str) -> Self {
        // 1. Parse command name from path
        let cmd_name = Path::new(command)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(command)
            .to_lowercase();

        match cmd_name.as_str() {
            "bash" => ShellType::Bash,
            "zsh" => ShellType::Zsh,
            "fish" => ShellType::Fish,
            "pwsh" | "powershell" => ShellType::PowerShell,
            "cmd" => ShellType::Cmd,
            _ => {
                // 2. Fallback to $SHELL environment variable
                if let Ok(shell) = std::env::var("SHELL") {
                    return ShellType::detect(&shell);
                }
                ShellType::Unknown(cmd_name)
            }
        }
    }
}
```

#### 21.2.2 Shell-Specific Configurations

| Shell | Line Ending | Common Prompts | Exit Command |
|-------|-------------|----------------|--------------|
| Bash | `\n` | `$ `, `# `, `bash-*$ ` | `exit` |
| Zsh | `\n` | `% `, `➜ `, `❯ ` | `exit` |
| Fish | `\n` | `> `, `❯ ` | `exit` |
| PowerShell | `\r\n` | `PS>`, `>>> ` | `exit` |
| Cmd | `\r\n` | `>`, `C:\>` | `exit` |

### 21.3 Line Ending Detection

```rust
pub enum LineEnding {
    Lf,     // Unix: \n
    CrLf,   // Windows: \r\n
    Auto,   // Detect from shell type and platform
}

impl LineEnding {
    pub fn detect(shell: &ShellType) -> Self {
        match shell {
            ShellType::PowerShell | ShellType::Cmd => LineEnding::CrLf,
            ShellType::Bash | ShellType::Zsh | ShellType::Fish => LineEnding::Lf,
            ShellType::Unknown(_) => {
                // Platform-based fallback
                if cfg!(windows) {
                    LineEnding::CrLf
                } else {
                    LineEnding::Lf
                }
            }
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            LineEnding::Lf => "\n",
            LineEnding::CrLf => "\r\n",
            LineEnding::Auto => if cfg!(windows) { "\r\n" } else { "\n" },
        }
    }
}
```

### 21.4 Prompt Pattern Detection

The library provides built-in patterns for common shell prompts.

```rust
/// Common prompt patterns by shell type
pub fn default_prompts(shell: &ShellType) -> Vec<Pattern> {
    match shell {
        ShellType::Bash => vec![
            Pattern::regex(r"\$ $"),           // Standard user prompt
            Pattern::regex(r"# $"),            // Root prompt
            Pattern::regex(r"bash-\d+\.\d+\$ $"), // Bash version prompt
            Pattern::regex(r"\w+@\w+[:\$#] $"), // user@host prompt
        ],
        ShellType::Zsh => vec![
            Pattern::regex(r"% $"),
            Pattern::regex(r"➜ "),
            Pattern::regex(r"❯ $"),
        ],
        ShellType::Fish => vec![
            Pattern::regex(r"> $"),
            Pattern::regex(r"❯ $"),
        ],
        ShellType::PowerShell => vec![
            Pattern::regex(r"PS [^>]+> $"),
            Pattern::regex(r">>> $"),
        ],
        ShellType::Cmd => vec![
            Pattern::regex(r"[A-Z]:\\[^>]*>$"),
            Pattern::regex(r">$"),
        ],
        ShellType::Unknown(_) => vec![
            // Fallback: common prompt endings
            Pattern::regex(r"[\$#>%] $"),
        ],
    }
}
```

### 21.5 Encoding Detection

Encoding is auto-detected from locale environment variables.

```rust
pub fn detect_encoding() -> Encoding {
    // Check locale environment variables in priority order
    for var in &["LC_ALL", "LC_CTYPE", "LANG"] {
        if let Ok(locale) = std::env::var(var) {
            let locale_lower = locale.to_lowercase();

            // Parse encoding from locale string (e.g., "en_US.UTF-8")
            if locale_lower.contains("utf-8") || locale_lower.contains("utf8") {
                return Encoding::Utf8;
            }
            if locale_lower.contains("iso-8859") || locale_lower.contains("latin") {
                return Encoding::Latin1;
            }
            if locale_lower.contains("cp1252") || locale_lower.contains("windows-1252") {
                return Encoding::Windows1252;
            }
        }
    }

    // Platform-based default
    if cfg!(windows) {
        Encoding::Windows1252
    } else {
        Encoding::Utf8
    }
}
```

### 21.6 Terminal Size Detection

```rust
pub fn detect_terminal_size() -> (u16, u16) {
    // Try to inherit from parent terminal
    if let Some((cols, rows)) = terminal_size::terminal_size() {
        return (cols.0, rows.0);
    }

    // Check environment variables
    if let (Ok(cols), Ok(rows)) = (
        std::env::var("COLUMNS").and_then(|s| s.parse().map_err(|_| std::env::VarError::NotPresent)),
        std::env::var("LINES").and_then(|s| s.parse().map_err(|_| std::env::VarError::NotPresent)),
    ) {
        return (cols, rows);
    }

    // Sensible default
    (80, 24)
}
```

### 21.7 API Usage

```rust
// Zero-config mode: auto-detects everything
let session = Session::auto("bash").spawn().await?;

// Equivalent to:
let session = Session::builder()
    .command("bash")
    .shell_type(ShellType::Bash)
    .line_ending(LineEnding::Lf)
    .default_prompts()
    .encoding(Encoding::Utf8)
    .terminal_size(80, 24)
    .spawn()
    .await?;

// Override specific settings while keeping auto-detection for others
let session = Session::auto("bash")
    .terminal_size(120, 40)  // Override terminal size
    .spawn()
    .await?;
```

---

## 22. Mock Session Backend

The Mock Session Backend provides deterministic testing without spawning real processes. This section documents the architecture for Section 9.1a requirements.

### 22.1 Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              MockSession                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────┐  ┌─────────────────────────────────────────┐│
│  │      Scenario Engine        │  │           Response Queue                ││
│  │  ┌───────────────────────┐  │  │  ┌─────────────────────────────────┐   ││
│  │  │   Pattern Triggers    │  │  │  │    Scripted Outputs             │   ││
│  │  │   Input → Response    │  │  │  │    Timing Controls              │   ││
│  │  └───────────────────────┘  │  │  │    Delay Simulation             │   ││
│  └─────────────────────────────┘  │  └─────────────────────────────────┘   ││
├─────────────────────────────────────────────────────────────────────────────┤
│                         Implements Backend Trait                             │
│  read() → scripted output    send() → triggers patterns    close() → EOF    │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 22.2 Core Types

```rust
/// Mock session for deterministic testing
#[cfg(feature = "mock")]
pub struct MockSession {
    /// Scripted events to replay
    events: VecDeque<MockEvent>,
    /// Pattern-triggered responses
    triggers: HashMap<Pattern, Vec<u8>>,
    /// Input capture for assertions
    sent_data: Vec<u8>,
    /// Current time for timing simulation
    virtual_time: Instant,
    /// Configuration
    config: MockConfig,
}

/// A single mock event in a scenario
#[derive(Debug, Clone)]
pub enum MockEvent {
    /// Output data to the "process" (received by expect)
    Output { data: Vec<u8>, delay: Duration },
    /// Expect input from the client
    ExpectInput { pattern: Pattern, timeout: Duration },
    /// Simulate process exit
    Exit { code: i32 },
    /// Pause for timing tests
    Delay(Duration),
}

/// Configuration for mock behavior
#[derive(Debug, Clone)]
pub struct MockConfig {
    /// Whether to use real-time delays or skip them
    pub real_time: bool,
    /// Default timeout for expect operations
    pub default_timeout: Duration,
    /// Whether to fail on unexpected input
    pub strict_mode: bool,
}
```

### 22.3 Transcript Replay

MockSession supports loading scenarios from NDJSON transcript files.

#### 22.3.1 NDJSON Scenario Format

```json
{"t": 0.0, "e": "o", "d": "login: "}
{"t": 0.1, "e": "i", "d": "admin\r\n"}
{"t": 0.15, "e": "o", "d": "Password: "}
{"t": 0.2, "e": "i", "d": "secret123\r\n"}
{"t": 0.5, "e": "o", "d": "Welcome to the system!\r\n$ "}
{"t": 5.0, "e": "x", "code": 0}
```

| Field | Description |
|-------|-------------|
| `t` | Timestamp in seconds from session start |
| `e` | Event type: `o` (output), `i` (input), `x` (exit) |
| `d` | Data for output/input events |
| `code` | Exit code for exit events |

#### 22.3.2 Loading Transcripts

```rust
impl MockSession {
    /// Load scenario from NDJSON transcript file
    pub fn from_transcript<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut events = VecDeque::new();
        let mut last_time = 0.0;

        for line in reader.lines() {
            let line = line?;
            let event: TranscriptEvent = serde_json::from_str(&line)?;

            // Calculate delay from previous event
            let delay = Duration::from_secs_f64(event.t - last_time);
            last_time = event.t;

            match event.e.as_str() {
                "o" => events.push_back(MockEvent::Output {
                    data: event.d.unwrap_or_default().into_bytes(),
                    delay,
                }),
                "i" => events.push_back(MockEvent::ExpectInput {
                    pattern: Pattern::exact(&event.d.unwrap_or_default()),
                    timeout: Duration::from_secs(30),
                }),
                "x" => events.push_back(MockEvent::Exit {
                    code: event.code.unwrap_or(0),
                }),
                _ => return Err(Error::InvalidTranscript),
            }
        }

        Ok(Self::from_events(events))
    }
}
```

### 22.4 Pattern-Triggered Responses

For interactive testing, MockSession supports pattern-triggered responses.

```rust
impl MockSession {
    /// Add a pattern trigger that responds when specific input is sent
    pub fn on_input<P: Into<Pattern>>(mut self, pattern: P, response: &str) -> Self {
        self.triggers.insert(pattern.into(), response.as_bytes().to_vec());
        self
    }
}

// Usage example
let mock = MockSession::new()
    .on_input("username:", "admin\r\n")
    .on_input("password:", "secret\r\n")
    .on_input(Pattern::regex(r"\$ $"), "exit\r\n");
```

### 22.5 Built-in Scenarios

Common testing scenarios are provided as convenience methods.

```rust
impl MockSession {
    /// SSH login scenario
    pub fn ssh_login(username: &str, password: &str, success: bool) -> Self {
        let mut events = vec![
            MockEvent::Output {
                data: format!("{username}@host's password: ").into_bytes(),
                delay: Duration::from_millis(100),
            },
            MockEvent::ExpectInput {
                pattern: Pattern::exact(&format!("{password}\r\n")),
                timeout: Duration::from_secs(30),
            },
        ];

        if success {
            events.push(MockEvent::Output {
                data: b"Welcome to Ubuntu 22.04\r\n$ ".to_vec(),
                delay: Duration::from_millis(200),
            });
        } else {
            events.push(MockEvent::Output {
                data: b"Permission denied, please try again.\r\n".to_vec(),
                delay: Duration::from_millis(100),
            });
        }

        Self::from_events(events.into())
    }

    /// Sudo password prompt scenario
    pub fn sudo_prompt(password: &str, success: bool) -> Self {
        // Similar pattern...
    }

    /// Simple shell prompt scenario
    pub fn shell_prompt(prompt: &str) -> Self {
        Self::from_events(vec![
            MockEvent::Output {
                data: prompt.as_bytes().to_vec(),
                delay: Duration::from_millis(50),
            },
        ].into())
    }
}
```

### 22.6 Timing Control

```rust
impl MockSession {
    /// Skip timing delays (fast mode for unit tests)
    pub fn instant(mut self) -> Self {
        self.config.real_time = false;
        self
    }

    /// Use real-time delays (for integration/timeout tests)
    pub fn real_time(mut self) -> Self {
        self.config.real_time = true;
        self
    }

    /// Add configurable delay for timeout testing
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.events.push_front(MockEvent::Delay(delay));
        self
    }
}
```

### 22.7 Test Integration Example

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_login_dialog() {
        let mock = MockSession::from_transcript("fixtures/ssh_login.json")
            .instant();  // Skip delays for fast tests

        let mut session = Session::from_backend(mock);

        session.expect("login: ").await?;
        session.send_line("admin").await?;
        session.expect("Password: ").await?;
        session.send_line("secret123").await?;
        session.expect("$ ").await?;

        assert!(session.sent_data().contains(b"admin"));
    }

    #[tokio::test]
    async fn test_timeout_behavior() {
        let mock = MockSession::new()
            .with_delay(Duration::from_secs(5))
            .then_output("delayed response");

        let mut session = Session::from_backend(mock)
            .real_time();  // Use real delays

        let result = session.expect_timeout("response", Duration::from_secs(1)).await;
        assert!(matches!(result, Err(Error::Timeout { .. })));
    }
}
```

---

## 23. Supply Chain Security

This section documents the architecture for supply chain security requirements (NFR-5.7), ensuring verifiable builds and auditable dependencies.

### 23.1 Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           CI/CD Pipeline                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                        Build Stage                                   │    │
│  │  cargo build → deterministic → reproducible outputs                 │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    │                                         │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                       Security Checks                                │    │
│  │  cargo-audit │ cargo-deny │ cargo-vet                                │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    │                                         │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                      Artifact Generation                             │    │
│  │  SLSA Provenance │ Sigstore Signing │ SBOM (SPDX + CycloneDX)       │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 23.2 Reproducible Builds

Reproducible builds ensure that the same source code always produces identical binaries.

#### 23.2.1 Build Environment Controls

```yaml
# .github/workflows/release.yml
env:
  # Lock timestamps for reproducibility
  SOURCE_DATE_EPOCH: ${{ github.event.repository.updated_at }}
  # Disable debug info that includes paths
  CARGO_PROFILE_RELEASE_DEBUG: 0
  # Use consistent Rust toolchain
  RUSTUP_TOOLCHAIN: "1.85.0"
```

#### 23.2.2 Cargo Configuration

```toml
# .cargo/config.toml
[build]
# Consistent target directory
target-dir = "target"

[profile.release]
# Reproducibility settings
debug = 0
strip = "symbols"
lto = "thin"
codegen-units = 1
```

### 23.3 SLSA Level 3 Provenance

SLSA (Supply-chain Levels for Software Artifacts) Level 3 provides verifiable build provenance.

#### 23.3.1 Provenance Generation

```yaml
# .github/workflows/release.yml
jobs:
  build:
    permissions:
      id-token: write  # For OIDC token
      contents: read
      attestations: write

    steps:
      - uses: actions/checkout@v4

      - name: Build release artifacts
        run: cargo build --release

      - name: Generate SLSA provenance
        uses: slsa-framework/slsa-github-generator/.github/workflows/builder_go_slsa3.yml@v2
        with:
          artifacts: target/release/librust_expect*
```

#### 23.3.2 Provenance Attestation Format

```json
{
  "_type": "https://in-toto.io/Statement/v1",
  "subject": [
    {
      "name": "librust_expect.rlib",
      "digest": {
        "sha256": "abc123..."
      }
    }
  ],
  "predicateType": "https://slsa.dev/provenance/v1",
  "predicate": {
    "buildDefinition": {
      "buildType": "https://slsa-framework.github.io/github-actions-buildtypes/workflow/v1",
      "externalParameters": {
        "workflow": ".github/workflows/release.yml"
      }
    },
    "runDetails": {
      "builder": {
        "id": "https://github.com/slsa-framework/slsa-github-generator"
      }
    }
  }
}
```

### 23.4 Sigstore Keyless Signing

Sigstore provides keyless signing tied to GitHub Actions workflow identity.

```yaml
# .github/workflows/release.yml
- name: Sign with Sigstore
  uses: sigstore/cosign-installer@v3

- name: Sign release artifacts
  run: |
    cosign sign-blob \
      --yes \
      --oidc-issuer https://token.actions.githubusercontent.com \
      --output-signature target/release/librust_expect.sig \
      --output-certificate target/release/librust_expect.crt \
      target/release/librust_expect.rlib
```

#### 23.4.1 Verification

Users can verify signatures with:

```bash
cosign verify-blob \
  --certificate rust-expect.crt \
  --signature rust-expect.sig \
  --certificate-identity-regexp "github.com/rust-expect/rust-expect" \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  librust_expect.rlib
```

### 23.5 SBOM Generation

Software Bill of Materials (SBOM) enables dependency auditing.

#### 23.5.1 SPDX Format

```yaml
# .github/workflows/release.yml
- name: Generate SPDX SBOM
  run: |
    cargo sbom --format spdx > sbom.spdx.json
```

#### 23.5.2 CycloneDX Format

```yaml
- name: Generate CycloneDX SBOM
  run: |
    cargo cyclonedx --format json > sbom.cdx.json
```

#### 23.5.3 SBOM Contents

| Component | Included Data |
|-----------|---------------|
| Direct dependencies | Name, version, license, PURL |
| Transitive dependencies | Full dependency tree |
| Build tools | Rust version, cargo version |
| Checksums | SHA-256 of all components |

### 23.6 Dependency Auditing

#### 23.6.1 cargo-audit (Vulnerability Checking)

```yaml
# .github/workflows/ci.yml
- name: Security audit
  run: |
    cargo install cargo-audit
    cargo audit --deny warnings
```

**Policy:** CI fails on any known vulnerability (RUSTSEC advisory).

#### 23.6.2 cargo-deny (License and Duplicate Checking)

```toml
# deny.toml
[licenses]
unlicensed = "deny"
allow = ["MIT", "Apache-2.0", "BSD-3-Clause", "ISC", "Zlib"]
copyleft = "deny"

[bans]
multiple-versions = "warn"
wildcards = "deny"
deny = [
  # Known problematic crates
]

[advisories]
vulnerability = "deny"
unmaintained = "warn"
```

#### 23.6.3 cargo-vet (Attestation)

```toml
# supply-chain/config.toml
[policy.rust-expect]
criteria = "safe-to-deploy"

# supply-chain/audits.toml
[[audits.tokio]]
who = "rust-expect maintainers"
criteria = "safe-to-deploy"
version = "1.40.0"
notes = "Audited for memory safety and async correctness"
```

### 23.7 CI Security Pipeline

```yaml
# .github/workflows/security.yml
name: Security

on:
  push:
    branches: [main]
  pull_request:
  schedule:
    - cron: '0 0 * * *'  # Daily vulnerability check

jobs:
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: cargo-audit
        run: cargo audit --deny warnings

      - name: cargo-deny
        run: cargo deny check

      - name: cargo-vet
        run: cargo vet --locked
```

### 23.8 Release Artifact Manifest

Each release includes:

| Artifact | Description |
|----------|-------------|
| `librust_expect-*.rlib` | Compiled library |
| `*.sig` | Sigstore signature |
| `*.crt` | Signing certificate |
| `sbom.spdx.json` | SPDX SBOM |
| `sbom.cdx.json` | CycloneDX SBOM |
| `provenance.intoto.jsonl` | SLSA provenance |
| `SHA256SUMS` | Checksums for all artifacts |

---

## Appendix A: Glossary

| Term | Definition |
|------|------------|
| AsyncFd | Tokio type for registering file descriptors for async I/O |
| Backend | Abstract interface for session I/O (PTY, SSH, etc.) |
| Buffer | Ring buffer holding process output for pattern matching |
| ConPTY | Windows Console Pseudo Terminal API (Windows 10 1809+) |
| CycloneDX | OASIS standard for Software Bill of Materials (SBOM) format |
| exp_continue | Continue matching within same expect call after action |
| expect_before | Persistent patterns checked before main patterns |
| expect_after | Persistent patterns checked after main patterns |
| Job Object | Windows mechanism for process group management |
| Luhn Algorithm | Checksum formula for validating credit card numbers |
| MockSession | Test backend that replays scripted scenarios without real processes |
| MSRV | Minimum Supported Rust Version |
| NDJSON | Newline-delimited JSON format for streaming/transcript logs |
| Overlapped I/O | Windows async I/O mechanism |
| PII | Personally Identifiable Information (credit cards, SSNs, etc.) |
| PTY | Pseudo-terminal; virtual terminal device |
| PtyMaster | The controlling side of a PTY (where we read/write) |
| PtySlave | The process side of a PTY (terminal to child) |
| SBOM | Software Bill of Materials; inventory of software components |
| Sigstore | Keyless signing infrastructure for software artifacts |
| SIGWINCH | Unix signal for terminal window size change |
| SLSA | Supply-chain Levels for Software Artifacts; build provenance framework |
| Session | Handle to a spawned process with expect capabilities |
| SPDX | Software Package Data Exchange; SBOM format standard |
| Zero-Config Mode | Auto-detection of shell type, prompts, and encoding for simplified usage |

---

## Appendix B: References

1. [Original Expect Manpage](https://www.tcl-lang.org/man/expect5.31/expect.1.html)
2. [pexpect Documentation](https://pexpect.readthedocs.io/)
3. [Alacritty PTY Implementation](https://github.com/alacritty/alacritty/tree/master/alacritty_terminal/src/tty)
4. [WezTerm portable-pty](https://github.com/wez/wezterm/tree/main/pty)
5. [Windows ConPTY Documentation](https://docs.microsoft.com/en-us/windows/console/creating-a-pseudoconsole-session)
6. [ConPTY Overlapped I/O PR](https://github.com/microsoft/terminal/pull/17510)
7. [rustix Documentation](https://docs.rs/rustix)
8. [windows-sys Documentation](https://docs.rs/windows-sys)
9. [tokio AsyncFd](https://docs.rs/tokio/latest/tokio/io/unix/struct.AsyncFd.html)
10. [crossterm Documentation](https://docs.rs/crossterm)
11. [expectrl Documentation](https://docs.rs/expectrl)
12. [rexpect Documentation](https://docs.rs/rexpect)
13. [asciinema Recording Format](https://docs.asciinema.org/manual/asciicast/v2/)
14. [SLSA Specification](https://slsa.dev/spec/v1.0/)
15. [Sigstore Documentation](https://docs.sigstore.dev/)
16. [SPDX Specification](https://spdx.dev/specifications/)
17. [CycloneDX Specification](https://cyclonedx.org/specification/overview/)
18. [cargo-audit](https://github.com/rustsec/rustsec/tree/main/cargo-audit)
19. [cargo-deny](https://github.com/EmbarkStudios/cargo-deny)
20. [cargo-vet](https://mozilla.github.io/cargo-vet/)
21. [Luhn Algorithm (Wikipedia)](https://en.wikipedia.org/wiki/Luhn_algorithm)

---

## Appendix C: Migration Guide

This appendix provides migration guidance for users coming from the two primary Rust expect libraries: **rexpect** and **expectrl**.

### C.1 Migration from rexpect

[rexpect](https://github.com/rust-cli/rexpect) is a Rust port of pexpect with a focus on simplicity. rust-expect provides a superset of rexpect's functionality with better async support, cross-platform capabilities, and error handling.

#### C.1.1 Spawning Sessions

```rust
// rexpect: Spawn with optional timeout
use rexpect::spawn;
let mut p = spawn("cat", Some(30_000))?;  // 30 second timeout

// rust-expect: Builder pattern with configuration
use rust_expect::Session;
let mut session = Session::builder("cat")
    .timeout(Duration::from_secs(30))
    .spawn()
    .await?;
```

**Bash helpers:**

```rust
// rexpect: Built-in bash helpers
use rexpect::spawn_bash;
let mut p = spawn_bash(Some(10_000))?;
p.wait_for_prompt()?;

// rust-expect: Use session with shell detection
use rust_expect::Session;
let mut session = Session::shell()
    .timeout(Duration::from_secs(10))
    .spawn()
    .await?;
session.expect_prompt().await?;  // Detects PS1/PS2 automatically
```

#### C.1.2 Pattern Matching

| rexpect | rust-expect | Notes |
|---------|-------------|-------|
| `p.exp_string("text")` | `session.expect("text").await` | Literal string matching |
| `p.exp_regex(r"pattern")` | `session.expect(Regex::new(r"pattern")?).await` | Regex matching |
| `p.exp_eof()` | `session.expect(Eof).await` | Wait for process exit |
| `p.exp_any(vec![...])` | `session.expect(Any::new([...])).await` | Multiple patterns |
| N/A | `session.expect(NBytes(100)).await` | Byte count matching |

**Example migration:**

```rust
// rexpect
p.exp_string("Password:")?;
p.send_line("secret")?;
p.exp_regex(r"[$#] ")?;

// rust-expect
session.expect("Password:").await?;
session.send_line("secret").await?;
session.expect(Regex::new(r"[$#] ")?).await?;
```

#### C.1.3 Sending Input

| rexpect | rust-expect | Notes |
|---------|-------------|-------|
| `p.send("text")` | `session.send("text").await` | Send raw text |
| `p.send_line("text")` | `session.send_line("text").await` | Send with newline |
| `p.send_control('c')` | `session.send_control(ControlCode::EndOfText).await` | Control characters |
| N/A | `session.send_slow("text", delay).await` | Human-like typing |

#### C.1.4 Error Handling

```rust
// rexpect: Uses rexpect::errors::Error
use rexpect::errors::Error;
match p.exp_string("text") {
    Ok(_) => {},
    Err(Error::Timeout { .. }) => println!("Timed out"),
    Err(e) => return Err(e.into()),
}

// rust-expect: Rich error types with context
use rust_expect::{Error, MatchError};
match session.expect("text").await {
    Ok(m) => {},
    Err(Error::Match(MatchError::Timeout { elapsed, buffer, .. })) => {
        println!("Timed out after {:?}", elapsed);
        println!("Buffer contents: {:?}", buffer);
    },
    Err(e) => return Err(e.into()),
}
```

#### C.1.5 Async vs Sync

rexpect is synchronous only. rust-expect is async-first with a sync wrapper:

```rust
// rexpect: Always synchronous
let output = p.exp_string("$")?;

// rust-expect: Async by default
let output = session.expect("$").await?;

// rust-expect: Sync wrapper for compatibility
use rust_expect::blocking::Session;
let mut session = Session::builder("cat").spawn()?;  // No .await
let output = session.expect("$")?;                   // No .await
```

### C.2 Migration from expectrl

[expectrl](https://github.com/zhiburt/expectrl) is a more feature-rich library that inspired several rust-expect APIs. Migration is generally straightforward with some naming differences.

#### C.2.1 Spawning Sessions

```rust
// expectrl: Simple spawn function
use expectrl::{spawn, Session};
let mut session = spawn("cat")?;

// rust-expect: Builder pattern for consistency
use rust_expect::Session;
let mut session = Session::builder("cat")
    .spawn()
    .await?;

// expectrl with timeout
session.set_expect_timeout(Some(Duration::from_secs(30)));

// rust-expect with timeout
let mut session = Session::builder("cat")
    .timeout(Duration::from_secs(30))
    .spawn()
    .await?;
```

#### C.2.2 Pattern Matching

The pattern matching API is similar, but rust-expect uses await for all operations:

```rust
// expectrl (sync)
use expectrl::{Regex, Eof, NBytes};
session.expect("text")?;
session.expect(Regex("pattern"))?;
session.expect(Eof)?;
session.expect(NBytes(100))?;

// rust-expect (async)
use rust_expect::{Regex, Eof, NBytes};
session.expect("text").await?;
session.expect(Regex::new("pattern")?).await?;
session.expect(Eof).await?;
session.expect(NBytes(100)).await?;
```

#### C.2.3 Control Codes

Both libraries provide similar control code abstractions:

```rust
// expectrl
use expectrl::ControlCode;
session.send(ControlCode::EndOfTransmission)?;  // Ctrl+D

// rust-expect (identical API)
use rust_expect::ControlCode;
session.send_control(ControlCode::EndOfTransmission).await?;  // Ctrl+D
```

**Control code mapping:**

| ASCII | expectrl | rust-expect |
|-------|----------|-------------|
| Ctrl+A | `ControlCode::StartOfHeading` | `ControlCode::StartOfHeading` |
| Ctrl+C | `ControlCode::EndOfText` | `ControlCode::EndOfText` |
| Ctrl+D | `ControlCode::EndOfTransmission` | `ControlCode::EndOfTransmission` |
| Ctrl+Z | `ControlCode::Substitute` | `ControlCode::Substitute` |
| Ctrl+\\ | `ControlCode::FileSeparator` | `ControlCode::FileSeparator` |

#### C.2.4 Interactive Sessions

```rust
// expectrl: InteractSession for bidirectional interaction
use expectrl::interact::InteractSession;
let mut interact = InteractSession::new(&mut session, std::io::stdin(), std::io::stdout());
interact.spawn()?;

// rust-expect: Interact mode with hooks
session.interact()
    .on_output(|data| {
        std::io::stdout().write_all(data)?;
        std::io::stdout().flush()?;
        Ok(())
    })
    .on_input(|data| {
        session.send(data).await?;
        Ok(())
    })
    .run()
    .await?;
```

#### C.2.5 Async Feature Flags

```rust
// expectrl: Opt-in async with feature flag
// Cargo.toml: expectrl = { version = "0.8", features = ["async"] }

// rust-expect: Async by default, sync opt-in
// Cargo.toml: rust-expect = { version = "0.1", features = ["blocking"] }
```

### C.3 Feature Comparison Matrix

| Feature | rexpect | expectrl | rust-expect |
|---------|---------|----------|-------------|
| Async support | ❌ | ✅ (feature) | ✅ (default) |
| Windows support | ❌ | ⚠️ (limited) | ✅ (full ConPTY) |
| SSH backend | ❌ | ❌ | ✅ (russh) |
| Screen buffer | ❌ | ❌ | ✅ (vte) |
| Dialog system | ❌ | ❌ | ✅ |
| Multi-session | ❌ | ❌ | ✅ |
| Transcript logging | ❌ | ✅ | ✅ (enhanced) |
| exp_continue | ❌ | ❌ | ✅ |
| expect_before/after | ❌ | ❌ | ✅ |
| Human typing simulation | ❌ | ❌ | ✅ |
| Regex patterns | ✅ | ✅ | ✅ |
| Literal patterns | ✅ | ✅ | ✅ |
| EOF detection | ✅ | ✅ | ✅ |
| NBytes matching | ❌ | ✅ | ✅ |
| Control codes | ✅ | ✅ | ✅ |
| Timeout per operation | ⚠️ (global) | ✅ | ✅ |
| Rich error context | ❌ | ⚠️ | ✅ |
| Streaming patterns | ❌ | ❌ | ✅ |

### C.4 Common Migration Patterns

#### C.4.1 Converting Synchronous Code to Async

```rust
// Before (rexpect)
fn automate_server() -> Result<(), Box<dyn std::error::Error>> {
    let mut p = rexpect::spawn("ssh server", Some(30_000))?;
    p.exp_string("password:")?;
    p.send_line("secret")?;
    p.exp_string("$")?;
    p.send_line("uptime")?;
    let output = p.exp_regex(r"load average.*")?;
    println!("{}", output.1);
    Ok(())
}

// After (rust-expect)
async fn automate_server() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = rust_expect::Session::builder("ssh")
        .args(["server"])
        .timeout(Duration::from_secs(30))
        .spawn()
        .await?;

    session.expect("password:").await?;
    session.send_line("secret").await?;
    session.expect("$").await?;
    session.send_line("uptime").await?;

    let output = session.expect(Regex::new(r"load average.*")?).await?;
    println!("{}", output.matched());

    Ok(())
}
```

#### C.4.2 Using the Blocking API

If you need to maintain synchronous code:

```rust
// Add feature to Cargo.toml
// rust-expect = { version = "0.1", features = ["blocking"] }

use rust_expect::blocking::Session;

fn automate_server() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = Session::builder("ssh")
        .args(["server"])
        .timeout(Duration::from_secs(30))
        .spawn()?;  // No .await

    session.expect("password:")?;  // No .await
    session.send_line("secret")?;
    session.expect("$")?;
    session.send_line("uptime")?;

    let output = session.expect(Regex::new(r"load average.*")?)?;
    println!("{}", output.matched());

    Ok(())
}
```

#### C.4.3 Leveraging New Features

After migration, take advantage of rust-expect's enhanced capabilities:

```rust
use rust_expect::{Session, Dialog, patterns};

async fn enhanced_automation() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Use dialog system for common patterns
    let mut session = Session::builder("ssh")
        .args(["server"])
        .spawn()
        .await?;

    // Built-in login dialog handles password prompts
    Dialog::login("secret").execute(&mut session).await?;

    // 2. Use persistent patterns with expect_before
    session.expect_before(patterns![
        "Connection closed" => |_| Err(Error::Disconnected),
        "Permission denied" => |_| Err(Error::PermissionDenied),
    ]);

    // 3. Use exp_continue for multi-step matches
    session.expect(patterns![
        "Continue? [y/n]" => |s| { s.send_line("y").await?; ExpectAction::Continue },
        "$" => |_| ExpectAction::Return,
    ]).await?;

    // 4. Use screen buffer for complex output
    session.send_line("htop").await?;
    tokio::time::sleep(Duration::from_secs(1)).await;
    let screen = session.screen();
    let cpu_line = screen.row(0);  // Get first row of screen

    // 5. Use transcript logging for debugging
    session.enable_transcript("/tmp/session.log").await?;

    Ok(())
}
```

### C.5 Troubleshooting Migration Issues

#### C.5.1 "Method not found" Errors

Most methods now require `.await`:

```rust
// Error: no method named `expect` found
session.expect("$")?;

// Fix: Add .await
session.expect("$").await?;
```

#### C.5.2 Timeout Differences

```rust
// rexpect: Timeout in spawn call
let p = spawn("cmd", Some(30_000))?;  // milliseconds

// rust-expect: Timeout in builder, uses Duration
let session = Session::builder("cmd")
    .timeout(Duration::from_secs(30))  // Duration type
    .spawn()
    .await?;
```

#### C.5.3 Pattern Type Changes

```rust
// expectrl: Regex as tuple struct
session.expect(Regex("pattern"))?;

// rust-expect: Regex::new for validation
session.expect(Regex::new("pattern")?).await?;
```

#### C.5.4 Error Type Changes

```rust
// rexpect error handling
use rexpect::errors::Error;

// rust-expect error handling
use rust_expect::Error;
// Errors provide more context (buffer contents, elapsed time, etc.)
```

### C.6 Performance Comparison

| Operation | rexpect | expectrl | rust-expect |
|-----------|---------|----------|-------------|
| Spawn | ~50ms | ~45ms | ~40ms |
| Literal match | ~5µs | ~4µs | ~3µs |
| Regex match | ~15µs | ~12µs | ~7µs |
| Large buffer (1MB) | Varies | ~50ms | ~35ms |

rust-expect's performance improvements come from:
- Zero-copy pattern matching where possible
- Optimized ring buffer implementation
- Lazy regex compilation with caching
- Streaming pattern matching (no full buffer scans)

---

*This document is the authoritative source of technical architecture for rust-expect. All implementation work should adhere to the patterns and structures defined herein.*
