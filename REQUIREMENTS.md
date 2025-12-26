# rust-expect: Functional Requirements Specification

**Version:** 1.1.0
**Date:** 2025-12-26
**Status:** Authoritative

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Project Vision](#2-project-vision)
3. [Scope & Boundaries](#3-scope--boundaries)
4. [Functional Requirements](#4-functional-requirements)
   - [FR-1: Process Spawning & Management](#fr-1-process-spawning--management)
   - [FR-2: Pattern Matching & Expectation](#fr-2-pattern-matching--expectation)
   - [FR-3: Input/Output Operations](#fr-3-inputoutput-operations)
   - [FR-4: Interactive Mode](#fr-4-interactive-mode)
   - [FR-5: Multi-Session Management](#fr-5-multi-session-management)
   - [FR-6: SSH Integration](#fr-6-ssh-integration)
   - [FR-7: Terminal Emulation](#fr-7-terminal-emulation)
   - [FR-8: Dialog System](#fr-8-dialog-system)
   - [FR-9: PTY Backend](#fr-9-pty-backend-rust-pty-crate)
5. [Non-Functional Requirements](#5-non-functional-requirements)
   - [NFR-1: Performance](#nfr-1-performance)
   - [NFR-2: Reliability](#nfr-2-reliability)
   - [NFR-3: Portability](#nfr-3-portability)
   - [NFR-4: Usability](#nfr-4-usability)
   - [NFR-5: Security](#nfr-5-security)
6. [Platform Requirements](#6-platform-requirements)
7. [API Requirements](#7-api-requirements)
8. [Integration Requirements](#8-integration-requirements)
9. [Testing Requirements](#9-testing-requirements)
10. [Documentation Requirements](#10-documentation-requirements)
11. [Success Criteria](#11-success-criteria)
12. [Appendices](#12-appendices)
    - [Appendix A: Glossary](#appendix-a-glossary)
    - [Appendix B: Reference Documents](#appendix-b-reference-documents)
    - [Appendix C: Decision Log](#appendix-c-decision-log)
    - [Appendix D: Resolved Questions](#appendix-d-resolved-questions)
    - [Appendix E: Versioning & Compatibility](#appendix-e-versioning--compatibility)

---

## 1. Executive Summary

**rust-expect** is a next-generation terminal automation library for Rust that will exceed all existing implementations (expectrl, rexpect, pexpect) in every dimension: features, performance, cross-platform support, API ergonomics, and reliability.

### Key Differentiators

| Capability | rust-expect | Best Competitor |
|------------|-------------|-----------------|
| Windows Support | First-class, fully tested | Broken (expectrl) or None (rexpect) |
| Async Architecture | Native async-first | Bolted-on (expectrl) or None (rexpect) |
| Original Expect Parity | All 12 key features listed below | Partial coverage |
| Multi-Session | Native with `select!` | None |
| SSH Integration | Built-in via russh | None (Rust) / pxssh (Python) |
| Performance (100MB output) | < 1 second (target) | 30+ minutes (pexpect, documented) |

### Competitive Gaps Addressed

This library will implement **12 features from original Tcl Expect** that NO modern library provides:

1. `expect_background` - Non-blocking pattern matching
2. `expect_before` / `expect_after` - Persistent patterns
3. `exp_continue` - Continue matching after action
4. Advanced `interact` with pattern hooks
5. Multi-spawn management
6. Indirect spawn IDs
7. `send_slow` / `send_human` - Human-like typing
8. `fork` - Process cloning
9. `disconnect` - Background/daemonize
10. Spawn with `-open` (files/pipes as sessions)
11. Signal trapping (`trap`)
12. Dialog system for reusable conversations

---

## 2. Project Vision

### 2.1 Mission Statement

To create the definitive terminal automation library for the Rust ecosystem—one that developers choose by default because it is simultaneously the most powerful, most reliable, and easiest to use option available.

### 2.2 Design Principles

| Principle | Description |
|-----------|-------------|
| **Async-First** | Core implementation is async; sync API is a thin wrapper |
| **Cross-Platform by Design** | Windows is not an afterthought; it's a first-class target |
| **Zero Surprises** | Behavior matches documentation; edge cases are handled explicitly |
| **Fail-Fast with Context** | Errors are informative; debugging is straightforward |
| **Performance Without Compromise** | Handle gigabytes of output without degradation |
| **Batteries Included** | SSH, logging, screen buffer—all optional but available |

### 2.3 Target Users

1. **CLI Tool Developers** - Testing interactive command-line applications
2. **DevOps Engineers** - Automating system administration tasks
3. **Security Researchers** - Interacting with network services and protocols
4. **Test Automation Engineers** - End-to-end testing of terminal applications
5. **Embedded Systems Developers** - Communicating with serial devices

---

## 3. Scope & Boundaries

### 3.1 In Scope

- Process spawning with PTY/ConPTY
- Pattern matching (regex, glob, exact, EOF, timeout)
- Bidirectional I/O with spawned processes
- Interactive mode with user handoff
- Multi-session orchestration
- SSH session management
- ANSI parsing and virtual screen buffer
- Cross-platform support (Linux, macOS, Windows)
- Async and sync API surfaces
- Comprehensive logging and debugging

### 3.2 Out of Scope

- GUI automation (use AccessKit, windows-rs, etc.)
- Web browser automation (use chromiumoxide, fantoccini, etc.)
- Serial port communication (use serialport crate; could be added later)
- Telnet protocol handling (use telnet crate; could be added later)
- Full terminal emulator UI (use crossterm, ratatui, etc.)
- WebAssembly (WASM) support (PTY operations require native OS APIs)

### 3.3 Future Considerations

- WebSocket-based remote sessions
- Kubernetes pod exec integration
- Docker container exec integration
- Recording/playback for test generation

---

## 4. Functional Requirements

Requirements are prioritized using MoSCoW:
- **M** = Must Have (required for 1.0)
- **S** = Should Have (highly desired for 1.0)
- **C** = Could Have (nice for 1.0, acceptable for 1.x)
- **W** = Won't Have (out of scope for 1.x)

---

### FR-1: Process Spawning & Management

#### FR-1.1: Basic Process Spawning [M]

The library MUST support spawning processes with PTY attachment.

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1.1.1 | Spawn a process by command string (e.g., `"bash -l"`) | M |
| FR-1.1.2 | Spawn a process by command + arguments vector | M |
| FR-1.1.3 | Spawn using `std::process::Command` as input | M |
| FR-1.1.4 | Spawn using `tokio::process::Command` as input | M |
| FR-1.1.5 | Return a `Session` handle for further interaction | M |

#### FR-1.2: Environment Control [M]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1.2.1 | Set environment variables for spawned process | M |
| FR-1.2.2 | Clear/inherit parent environment selectively | M |
| FR-1.2.3 | Set working directory for spawned process | M |
| FR-1.2.4 | Set `TERM` environment variable (default: `xterm-256color`) | M |

#### FR-1.3: Terminal Configuration [M]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1.3.1 | Set initial terminal dimensions (columns × rows) | M |
| FR-1.3.2 | Resize terminal dynamically after spawn | M |
| FR-1.3.3 | Send SIGWINCH on resize (Unix) | M |
| FR-1.3.4 | Handle ConPTY resize (Windows) | M |
| FR-1.3.5 | Query current terminal dimensions | S |

#### FR-1.4: Process Lifecycle [M]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1.4.1 | Check if process is still running | M |
| FR-1.4.2 | Wait for process termination | M |
| FR-1.4.3 | Get process exit status | M |
| FR-1.4.4 | Kill process (SIGKILL/TerminateProcess) | M |
| FR-1.4.5 | Send arbitrary signal (Unix) | S |
| FR-1.4.6 | Graceful termination with timeout (SIGTERM → SIGKILL) | S |
| FR-1.4.7 | Automatic cleanup on `Session` drop | M |
| FR-1.4.8 | No zombie processes after session ends | M |

#### FR-1.5: Advanced Spawning [C]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1.5.1 | Spawn from file descriptor (Expect's `-open`) | C |
| FR-1.5.2 | Spawn from pipe/socket | C |
| FR-1.5.3 | Fork/clone current session (Expect's `fork`) | C |
| FR-1.5.4 | Disconnect/daemonize session (Expect's `disconnect`) | C |

---

### FR-2: Pattern Matching & Expectation

#### FR-2.1: Pattern Types [M]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-2.1.1 | Match exact string | M |
| FR-2.1.2 | Match regex pattern (via `regex` crate) | M |
| FR-2.1.3 | Match glob pattern | S |
| FR-2.1.4 | Match EOF (process terminated) | M |
| FR-2.1.5 | Match timeout (configurable duration) | M |
| FR-2.1.6 | Match N bytes received | S |
| FR-2.1.7 | Match any of multiple patterns (first wins) | M |
| FR-2.1.8 | Match all of multiple patterns (any order) | C |

#### FR-2.2: Core Expect Operation [M]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-2.2.1 | `expect(pattern)` - Block until pattern matches or timeout | M |
| FR-2.2.2 | Return matched text and capture groups | M |
| FR-2.2.3 | Return buffer contents before match | M |
| FR-2.2.4 | Configurable timeout per-call and per-session | M |
| FR-2.2.5 | Timeout error includes buffer contents for debugging | M |

#### FR-2.3: Advanced Expect Operations [S]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-2.3.1 | `exp_continue` - Continue matching after action without re-entering expect | S |
| FR-2.3.2 | `expect_before` - Add patterns checked before every expect | S |
| FR-2.3.3 | `expect_after` - Add patterns checked after every expect | S |
| FR-2.3.4 | `expect_background` - Non-blocking pattern matching | S |
| FR-2.3.5 | Clear `expect_before`/`expect_after` patterns | S |
| FR-2.3.6 | Pattern priority/ordering control | S |

#### FR-2.4: Async Expect [M]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-2.4.1 | `expect()` returns `Future` | M |
| FR-2.4.2 | Compatible with `tokio::select!` | M |
| FR-2.4.3 | Cancellation-safe (no data loss on cancel) | M |
| FR-2.4.4 | `try_expect()` - Non-blocking check | S |

#### FR-2.5: Buffer Management [M]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-2.5.1 | Access raw buffer contents at any time | M |
| FR-2.5.2 | Clear buffer manually | M |
| FR-2.5.3 | Configurable max buffer size | M |
| FR-2.5.4 | Configurable search window size (for performance) | S |
| FR-2.5.5 | Buffer overflow handling (oldest data discarded) | M |
| FR-2.5.6 | Extract remaining buffer on timeout/error | M |

---

### FR-3: Input/Output Operations

#### FR-3.1: Sending Data [M]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-3.1.1 | `send(data)` - Send raw bytes | M |
| FR-3.1.2 | `send_line(text)` - Send text with newline | M |
| FR-3.1.3 | `send_control(char)` - Send control character (Ctrl+C = `send_control('c')`) | M |
| FR-3.1.4 | Configurable line ending (LF, CRLF, CR) | S |
| FR-3.1.5 | Flush after send (configurable) | M |

#### FR-3.2: Human-Like Typing [S]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-3.2.1 | `send_slow(text, delay)` - Character-by-character with fixed delay | S |
| FR-3.2.2 | `send_human(text, config)` - Variable delay simulating human typing | S |
| FR-3.2.3 | Configurable inter-character delay range | S |
| FR-3.2.4 | Configurable inter-word delay | C |
| FR-3.2.5 | Configurable "typo" simulation | W |

#### FR-3.3: Pre-Send Delay [S]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-3.3.1 | Configurable delay before send (pexpect's `delaybeforesend`) | S |
| FR-3.3.2 | Per-session and per-call configuration | S |
| FR-3.3.3 | Default to small delay (50ms) to avoid timing issues | S |

#### FR-3.4: Reading Data [M]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-3.4.1 | Read raw bytes from process output | M |
| FR-3.4.2 | Read until specific pattern | M |
| FR-3.4.3 | Read with timeout | M |
| FR-3.4.4 | Non-blocking read (return immediately with available data) | S |
| FR-3.4.5 | Read lines (iterator/stream) | S |

#### FR-3.5: Output Control [S]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-3.5.1 | Discard output mode (don't buffer, for large outputs) | S |
| FR-3.5.2 | Pause/resume output buffering | S |
| FR-3.5.3 | Tee output to external writer (file, logger) | S |

#### FR-3.6: Encoding & Unicode Handling [M]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-3.6.1 | Default to UTF-8 encoding for all text operations | M |
| FR-3.6.2 | Handle invalid UTF-8 gracefully (replacement char or raw bytes mode) | M |
| FR-3.6.3 | Raw bytes mode for binary protocols | M |
| FR-3.6.4 | Configurable encoding per-session | S |
| FR-3.6.5 | Line ending normalization (LF, CRLF, CR) configurable | S |
| FR-3.6.6 | Grapheme cluster awareness for `send_human` (emoji, combining chars) | C |
| FR-3.6.7 | Legacy encoding support (ISO-8859-1, Windows-1252) via feature flag | C |

#### FR-3.7: Session Logging & Transcript [M]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-3.7.1 | `log_file(path)` - Record all session I/O to file | M |
| FR-3.7.2 | `log_user(bool)` - Enable/disable echoing to stdout | M |
| FR-3.7.3 | Configurable log format (raw, timestamped, JSON) | S |
| FR-3.7.4 | Separate logging of sent vs received data | S |
| FR-3.7.5 | Log rotation and size limits | C |
| FR-3.7.6 | Redaction of sensitive patterns in logs | S |

---

### FR-4: Interactive Mode

#### FR-4.1: Basic Interact [M]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-4.1.1 | `interact()` - Transfer control to user (stdin↔process, process↔stdout) | M |
| FR-4.1.2 | Configurable escape character to exit interact | M |
| FR-4.1.3 | Return to programmatic control after interact | M |
| FR-4.1.4 | Async `interact()` returning `Future` (compatible with `tokio::select!`) | M |
| FR-4.1.5 | Raw mode terminal handling during interact (via crossterm) | M |
| FR-4.1.6 | Graceful terminal state restoration on exit/panic | M |

**Implementation Note:** Async interact requires bidirectional stream multiplexing between user terminal and PTY. The implementation will use crossterm's `event-stream` feature for async terminal input on Unix, with platform-specific handling for Windows console input. Terminal raw mode must be properly managed to avoid leaving the user's terminal in a broken state.

#### FR-4.2: Advanced Interact [S]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-4.2.1 | Pattern hooks during interact (detect patterns in output) | S |
| FR-4.2.2 | Input hooks during interact (detect user keystrokes) | S |
| FR-4.2.3 | Execute callback on pattern match during interact | S |
| FR-4.2.4 | Modify/filter output before displaying to user | C |
| FR-4.2.5 | Modify/filter input before sending to process | C |
| FR-4.2.6 | Timeout during interact (return to program after idle) | S |

#### FR-4.3: Multi-Process Interact [C]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-4.3.1 | Connect user to multiple processes simultaneously | C |
| FR-4.3.2 | Pipe output from one process to input of another | C |
| FR-4.3.3 | Session handoff (transfer user between processes) | C |

---

### FR-5: Multi-Session Management

#### FR-5.1: Concurrent Sessions [S]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-5.1.1 | Spawn and manage multiple sessions concurrently | S |
| FR-5.1.2 | Each session operates independently | S |
| FR-5.1.3 | Sessions are `Send + Sync` (usable across threads) | M |
| FR-5.1.4 | No global state; all state in session handles | M |

#### FR-5.2: Session Selection [S]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-5.2.1 | Wait on multiple sessions with `tokio::select!` | S |
| FR-5.2.2 | `select_expect(sessions, pattern)` - First session to match wins | S |
| FR-5.2.3 | `expect_all(sessions, pattern)` - Wait for all sessions to match | C |
| FR-5.2.4 | Session groups (named collections of sessions) | C |

#### FR-5.3: Indirect References [C]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-5.3.1 | Dynamic session lists (add/remove sessions from group) | C |
| FR-5.3.2 | Reference sessions by name/ID | C |
| FR-5.3.3 | Broadcast send to all sessions in group | C |

---

### FR-6: SSH Integration

#### FR-6.1: SSH Session Creation [S]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-6.1.1 | Connect via SSH with password authentication | S |
| FR-6.1.2 | Connect via SSH with key-based authentication | S |
| FR-6.1.3 | Connect via SSH with agent authentication | S |
| FR-6.1.4 | SSH session returns same `Session` interface as PTY | S |
| FR-6.1.5 | Configurable connection timeout | S |
| FR-6.1.6 | Configurable host key verification (strict, accept new, none) | S |

#### FR-6.2: SSH Features [S]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-6.2.1 | Execute single command over SSH | S |
| FR-6.2.2 | Start interactive shell over SSH | S |
| FR-6.2.3 | PTY allocation for SSH shell | S |
| FR-6.2.4 | Terminal resize over SSH | S |
| FR-6.2.5 | Environment variable passing over SSH | C |

#### FR-6.3: SSH Advanced [C]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-6.3.1 | Jump host / bastion host support | C |
| FR-6.3.2 | Port forwarding (local and remote) | C |
| FR-6.3.3 | SFTP file transfer | C |
| FR-6.3.4 | Connection pooling / reuse | C |

---

### FR-7: Terminal Emulation

#### FR-7.1: ANSI Parsing [C]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-7.1.1 | Parse ANSI escape sequences from output | C |
| FR-7.1.2 | Identify sequence types (SGR, cursor, etc.) | C |
| FR-7.1.3 | Strip ANSI sequences (plain text extraction) | S |
| FR-7.1.4 | Preserve ANSI sequences (pass-through mode) | M |

#### FR-7.2: Virtual Screen Buffer [C]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-7.2.1 | Maintain virtual screen state (like pyte/memterm) | C |
| FR-7.2.2 | Query screen contents at coordinates | C |
| FR-7.2.3 | Query cursor position | C |
| FR-7.2.4 | Detect screen changes | C |
| FR-7.2.5 | Screen diffing (what changed between states) | C |

#### FR-7.3: Screen-Based Matching [C]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-7.3.1 | Match pattern at specific screen coordinates | C |
| FR-7.3.2 | Wait for screen region to contain text | C |
| FR-7.3.3 | Snapshot screen state | C |

---

### FR-8: Dialog System

#### FR-8.1: Dialog Definition [C]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-8.1.1 | Define reusable pattern→action mappings | C |
| FR-8.1.2 | Dialogs are composable (combine multiple dialogs) | C |
| FR-8.1.3 | Dialogs can include sub-dialogs | C |
| FR-8.1.4 | Error/exception dialogs (global error handlers) | C |

#### FR-8.2: Dialog Execution [C]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-8.2.1 | Execute dialog on session | C |
| FR-8.2.2 | Dialog returns structured result | C |
| FR-8.2.3 | Dialog timeout (overall and per-step) | C |
| FR-8.2.4 | Dialog retry logic | C |

#### FR-8.3: Common Dialogs [C]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-8.3.1 | Login dialog (username + password prompts) | C |
| FR-8.3.2 | Sudo dialog (password prompt with retry) | C |
| FR-8.3.3 | Confirmation dialog (yes/no prompts) | C |

---

### FR-9: PTY Backend (`rust-pty` Crate)

**Rationale:** No existing Rust crate provides async + cross-platform PTY support. This is a gap in the ecosystem that rust-expect will fill by developing a purpose-built PTY crate that can also benefit other projects.

#### FR-9.1: Core PTY Operations [M]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-9.1.1 | Open/allocate PTY pair (master + slave) | M |
| FR-9.1.2 | Spawn process attached to PTY slave | M |
| FR-9.1.3 | Async read from PTY master (`AsyncRead` trait) | M |
| FR-9.1.4 | Async write to PTY master (`AsyncWrite` trait) | M |
| FR-9.1.5 | Resize PTY dimensions (columns × rows) | M |
| FR-9.1.6 | Close PTY and wait for child process | M |
| FR-9.1.7 | Get child process exit status | M |

#### FR-9.2: Unix PTY Backend [M]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-9.2.1 | Use `rustix` crate for PTY syscalls (modern, safe) | M |
| FR-9.2.2 | Native async via tokio `AsyncFd` registration | M |
| FR-9.2.3 | SIGWINCH signal handling for resize | M |
| FR-9.2.4 | SIGCHLD handling for child death notification | S |
| FR-9.2.5 | Proper session leader and controlling terminal setup | M |
| FR-9.2.6 | Support for `login_tty` semantics | S |

#### FR-9.3: Windows ConPTY Backend [M]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-9.3.1 | Use `windows-sys` crate for Win32 API | M |
| FR-9.3.2 | `CreatePseudoConsole` for PTY allocation | M |
| FR-9.3.3 | `ResizePseudoConsole` for dimension changes | M |
| FR-9.3.4 | `ClosePseudoConsole` for cleanup | M |
| FR-9.3.5 | Async adapter for synchronous ConPTY pipes | M |
| FR-9.3.6 | Forward-compatible with Windows overlapped I/O (26H2+) | M |
| FR-9.3.7 | Graceful fallback for older Windows versions | M |
| FR-9.3.8 | UTF-8 codepage (65001) configuration | M |
| FR-9.3.9 | `GenerateConsoleCtrlEvent` for Ctrl+C/Ctrl+Break | M |
| FR-9.3.10 | Process tree termination via Job Objects | M |

**Windows Overlapped I/O Note:** As of Windows 11 24H2 (build 26100), ConPTY only supports synchronous I/O. Overlapped I/O support has been developed ([microsoft/terminal PR #17510](https://github.com/microsoft/terminal/pull/17510)) and will ship in Windows 26H2. The implementation MUST:
1. Detect Windows version at runtime
2. Use overlapped I/O when available (26H2+)
3. Fall back to thread-per-pipe pattern on older versions
4. Present unified async interface regardless of underlying mechanism

#### FR-9.4: Backend Abstraction [M]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-9.4.1 | Unified `Pty` trait abstracting platform differences | M |
| FR-9.4.2 | `PtyMaster` type implementing `AsyncRead + AsyncWrite` | M |
| FR-9.4.3 | `PtyChild` type for process lifecycle management | M |
| FR-9.4.4 | Runtime backend selection (for testing/flexibility) | S |
| FR-9.4.5 | Backend-agnostic resize API | M |
| FR-9.4.6 | Backend-agnostic signal/control API | M |

#### FR-9.5: PTY Configuration [M]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-9.5.1 | Initial dimensions (columns × rows) | M |
| FR-9.5.2 | Environment variables for child process | M |
| FR-9.5.3 | Working directory for child process | M |
| FR-9.5.4 | TERM environment variable (default: `xterm-256color`) | M |
| FR-9.5.5 | termios settings on Unix (raw mode, echo, etc.) | S |
| FR-9.5.6 | Console mode settings on Windows | S |

#### FR-9.6: Error Handling [M]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-9.6.1 | Distinct error types for spawn, read, write, resize failures | M |
| FR-9.6.2 | Platform-specific error details preserved | M |
| FR-9.6.3 | Graceful handling of unexpected child death | M |
| FR-9.6.4 | No panics from PTY operations (all errors via Result) | M |
| FR-9.6.5 | Timeout support for all blocking operations | M |

#### FR-9.7: Resource Management [M]

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-9.7.1 | Automatic cleanup on `Pty` drop | M |
| FR-9.7.2 | No zombie processes after PTY close | M |
| FR-9.7.3 | No leaked file descriptors/handles | M |
| FR-9.7.4 | Cancellation-safe async operations | M |
| FR-9.7.5 | Thread-safe (`Send + Sync`) PTY handles | M |

---

## 5. Non-Functional Requirements

### NFR-1: Performance

| ID | Requirement | Target | Priority |
|----|-------------|--------|----------|
| NFR-1.1 | Process spawn latency | < 50ms | M |
| NFR-1.2 | Pattern match throughput | > 100 MB/s | M |
| NFR-1.3 | Memory usage for 100MB buffer | < 150 MB | M |
| NFR-1.4 | Handle 1GB output without crash | < 10s, < 2GB RAM | S |
| NFR-1.5 | Concurrent session overhead | < 1 MB per session | S |
| NFR-1.6 | Async task spawn overhead | < 1 μs | S |

### NFR-2: Reliability

| ID | Requirement | Priority |
|----|-------------|----------|
| NFR-2.1 | No zombie processes after session cleanup | M |
| NFR-2.2 | No resource leaks (file descriptors, memory) | M |
| NFR-2.3 | Graceful handling of process crashes | M |
| NFR-2.4 | Cancellation-safe async operations | M |
| NFR-2.5 | Panic-safe (cleanup on panic via Drop) | M |
| NFR-2.6 | Zero flaky tests in CI | M |
| NFR-2.7 | Deterministic timeout behavior (monotonic time, millisecond resolution minimum) | M |
| NFR-2.8 | Windows process tree termination (kill child processes) | M |

#### NFR-2.9: Cancellation Semantics [M]

Precise behavior when async operations are cancelled (via `select!`, timeout, or drop):

| Scenario | Behavior | Priority |
|----------|----------|----------|
| `expect()` cancelled mid-match | Buffer preserved, partial match discarded, session remains usable | M |
| `send()` cancelled mid-write | Partial write may occur, session remains usable | M |
| `interact()` cancelled | User I/O stops, session remains usable | M |
| Session dropped during operation | Graceful cleanup, process terminated | M |
| Timeout during expect | Buffer contents returned in error, session usable | M |

### NFR-2.10: Resource Limits [M]

| ID | Requirement | Default | Priority |
|----|-------------|---------|----------|
| NFR-2.10.1 | Maximum buffer size per session | 100 MB | M |
| NFR-2.10.2 | Maximum concurrent sessions | Unlimited (OS limits apply) | M |
| NFR-2.10.3 | Behavior at buffer limit | Oldest data discarded (ring buffer) | M |
| NFR-2.10.4 | File descriptor limit documentation | Document OS limits | S |
| NFR-2.10.5 | Configurable limits via builder | All limits configurable | S |

### NFR-3: Portability

| ID | Requirement | Priority |
|----|-------------|----------|
| NFR-3.1 | Full functionality on Linux x86_64 | M |
| NFR-3.2 | Full functionality on Linux ARM64 | M |
| NFR-3.3 | Full functionality on macOS x86_64 | M |
| NFR-3.4 | Full functionality on macOS ARM64 | M |
| NFR-3.5 | Full functionality on Windows x86_64 | M |
| NFR-3.6 | Compile on stable Rust (no nightly required) | M |
| NFR-3.7 | MSRV (Minimum Supported Rust Version) documented | M |
| NFR-3.8 | Cross-compilation support | S |

### NFR-4: Usability

| ID | Requirement | Priority |
|----|-------------|----------|
| NFR-4.1 | Intuitive API (discoverable via IDE autocomplete) | M |
| NFR-4.2 | Informative error messages with context | M |
| NFR-4.3 | Comprehensive documentation with examples | M |
| NFR-4.4 | Common patterns require minimal boilerplate | M |
| NFR-4.5 | Migration guide from pexpect/rexpect/expectrl | S |
| NFR-4.6 | Consistent naming conventions | M |

### NFR-5: Security

| ID | Requirement | Priority |
|----|-------------|----------|
| NFR-5.1 | No unsafe code in public API | M |
| NFR-5.2 | Minimal unsafe code internally (audited) | M |
| NFR-5.3 | Credentials not logged by default | M |
| NFR-5.4 | Secure SSH host key handling | S |
| NFR-5.5 | No arbitrary code execution from patterns | M |
| NFR-5.6 | Memory cleared for sensitive data | S |

---

## 6. Platform Requirements

### 6.1 Linux

| Requirement | Details |
|-------------|---------|
| PTY Backend | Native via `rustix` crate (modern, safe PTY syscalls) |
| Async Integration | `tokio::io::unix::AsyncFd` for non-blocking I/O |
| Signal Handling | SIGWINCH for resize, SIGCHLD for child death (via `signal-hook`) |
| Minimum Kernel | 3.10+ (for full PTY support) |
| Tested Distributions | Ubuntu 20.04+, Debian 11+, Fedora 38+, Alpine 3.18+ |
| libc Requirement | glibc 2.17+ or musl 1.1+ |

### 6.2 macOS

| Requirement | Details |
|-------------|---------|
| PTY Backend | Native via `rustix` crate (BSD-style PTY) |
| Async Integration | `tokio::io::unix::AsyncFd` for non-blocking I/O |
| Signal Handling | SIGWINCH for resize (via `signal-hook`) |
| Minimum Version | macOS 11 (Big Sur)+ |
| Tested Architectures | x86_64 (Intel), ARM64 (Apple Silicon) |

### 6.3 Windows

| Requirement | Details |
|-------------|---------|
| PTY Backend | ConPTY via `windows-sys` crate |
| Minimum Version | Windows 10 1809+ (ConPTY introduction) |
| Recommended Version | Windows 11 26H2+ (overlapped I/O support) |
| Console Handling | Proper argument escaping, ANSI mode enabled via `ENABLE_VIRTUAL_TERMINAL_PROCESSING` |
| Signal Equivalent | `TerminateProcess` for kill, `GenerateConsoleCtrlEvent` for Ctrl+C/Ctrl+Break |
| Process Management | Job Objects for process tree termination |
| Encoding | Set codepage to UTF-8 (65001) for consistent encoding; see note below |

**ConPTY Encoding Note:** ConPTY does not default to UTF-8. The library must explicitly set the console output codepage to 65001 (UTF-8) for consistent cross-platform behavior. On Windows versions prior to 1903, users may need to enable "Use Unicode UTF-8 for worldwide language support" in system settings for full UTF-8 support.

**ConPTY Async I/O Strategy:**

| Windows Version | I/O Strategy | Details |
|-----------------|--------------|---------|
| 10 1809 - 11 24H2 | Thread-per-pipe | Dedicated threads for read/write, async channel to tokio |
| 11 26H2+ | Native overlapped I/O | Direct async via `CreateOverlappedPipe` |

The implementation MUST detect Windows version at runtime and select the appropriate strategy automatically.

### 6.4 PTY Backend Abstraction

The PTY backend provides an async-first abstraction over platform-specific PTY implementations:

```rust
use std::future::Future;
use std::io::Result;
use std::process::ExitStatus;
use tokio::io::{AsyncRead, AsyncWrite};

/// Configuration for spawning a PTY
pub struct PtyConfig {
    pub command: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub working_dir: Option<PathBuf>,
    pub dimensions: (u16, u16),  // (cols, rows)
}

/// Handle to the master side of a PTY
pub trait PtyMaster: AsyncRead + AsyncWrite + Send + Sync + Unpin {
    /// Resize the PTY dimensions
    fn resize(&self, cols: u16, rows: u16) -> impl Future<Output = Result<()>> + Send;

    /// Get current dimensions
    fn dimensions(&self) -> (u16, u16);
}

/// Handle to the child process spawned in the PTY
pub trait PtyChild: Send + Sync {
    /// Check if the child process is still running
    fn is_running(&self) -> bool;

    /// Wait for the child process to exit
    fn wait(&mut self) -> impl Future<Output = Result<ExitStatus>> + Send;

    /// Send a signal to the child (Unix) or equivalent (Windows)
    fn signal(&self, signal: PtySignal) -> Result<()>;

    /// Kill the child process immediately
    fn kill(&mut self) -> Result<()>;

    /// Get the child's process ID
    fn pid(&self) -> u32;
}

/// Signals that can be sent to a PTY child
pub enum PtySignal {
    Interrupt,      // SIGINT / Ctrl+C
    Terminate,      // SIGTERM / graceful shutdown
    Kill,           // SIGKILL / TerminateProcess
    Hangup,         // SIGHUP (Unix only)
    WindowChange,   // SIGWINCH (handled internally on resize)
}

/// Factory for creating PTY instances
pub trait PtySystem: Send + Sync {
    type Master: PtyMaster;
    type Child: PtyChild;

    /// Spawn a new process in a PTY
    fn spawn(&self, config: PtyConfig) -> impl Future<Output = Result<(Self::Master, Self::Child)>> + Send;
}
```

**Design Rationale:**
- Async traits use `impl Future` return types for zero-cost abstraction
- `PtyMaster` implements `AsyncRead + AsyncWrite` for seamless tokio integration
- `PtyChild` is separate from `PtyMaster` for independent lifecycle management
- `PtySystem` factory enables runtime backend selection and testing
- All traits require `Send + Sync` for use across async task boundaries

---

## 7. API Requirements

### 7.1 Async-First Design

| Requirement | Details |
|-------------|---------|
| Core API is async | All I/O operations return `Future` |
| Sync wrapper provided | Thin wrapper using `block_on` |
| Tokio runtime | Primary runtime support |
| Runtime-agnostic core | Core logic usable with other runtimes via traits |

### 7.2 Builder Pattern for Configuration

```rust
let session = Session::builder()
    .command("bash")
    .args(&["-l"])
    .env("TERM", "xterm-256color")
    .dimensions(80, 24)
    .timeout(Duration::from_secs(30))
    .working_directory("/home/user")
    .spawn()
    .await?;
```

### 7.3 Fluent Expect API

```rust
// Simple expect
let matched = session.expect("password:").await?;

// Multiple patterns
let result = session.expect_any(&[
    Pattern::regex(r"password:"),
    Pattern::exact("Permission denied"),
    Pattern::eof(),
    Pattern::timeout(Duration::from_secs(10)),
]).await?;

match result {
    Match::Pattern(0, captures) => { /* password prompt */ },
    Match::Pattern(1, _) => { /* permission denied */ },
    Match::Eof => { /* process ended */ },
    Match::Timeout(buffer) => { /* timed out, here's what we got */ },
}
```

### 7.4 Macro Support

```rust
// Pattern matching macro
session.expect(patterns![
    regex!(r"password:") => |s| s.send_line("secret"),
    exact!("$") => |s| Break(Ok(())),
    timeout!(10s) => |s| Err(Error::Timeout),
])?;
```

### 7.5 Error Handling

```rust
pub enum Error {
    Spawn(SpawnError),
    Io(std::io::Error),
    Timeout {
        duration: Duration,
        buffer: String,
        pattern: String,
    },
    PatternNotFound {
        pattern: String,
        buffer: String,
    },
    ProcessExited {
        exit_status: ExitStatus,
        buffer: String,
    },
    Ssh(SshError),
    // ...
}
```

### 7.6 Feature Flags

```toml
[features]
default = ["sync", "tracing"]
sync = []                     # Synchronous API wrapper (thin async wrapper)
async-tokio = ["tokio"]       # Tokio async runtime (primary)
ssh = ["russh"]               # SSH integration (uses aws-lc-rs crypto by default)
screen = ["vte"]              # ANSI parsing and virtual screen buffer
tracing = ["dep:tracing"]     # Structured logging via tracing crate
full = ["async-tokio", "ssh", "screen", "tracing"]
```

### 7.7 Crate Structure

The project is organized as a Cargo workspace with the following crates:

| Crate | Purpose | Published |
|-------|---------|-----------|
| `rust-pty` | Cross-platform async PTY backend | Yes (standalone value) |
| `rust-expect` | Main expect-style automation library | Yes |
| `rust-expect-macros` | Procedural macros for pattern DSL | Yes |

```toml
# Workspace Cargo.toml
[workspace]
members = ["crates/rust-pty", "crates/rust-expect", "crates/rust-expect-macros"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.85"
license = "MIT OR Apache-2.0"
repository = "https://github.com/..."
```

**Rationale:**
- `rust-pty` as standalone crate provides ecosystem value beyond rust-expect
- Workspace enables shared dependencies and coordinated releases
- Macros in separate crate due to Rust's proc-macro crate requirements
- Edition 2024 for async closures and modern features (MSRV 1.85)

---

## 8. Integration Requirements

### 8.1 Logging Integration

| Requirement | Details |
|-------------|---------|
| Framework | `tracing` crate |
| Levels | ERROR, WARN, INFO, DEBUG, TRACE |
| Spans | Per-session, per-operation spans |
| Fields | session_id, command, pattern, duration |
| Opt-out | Compile-time via feature flag |

### 8.2 Test Framework Integration

| Requirement | Details |
|-------------|---------|
| cargo test | Works out of the box |
| nextest | Full compatibility |
| proptest | Property-based testing utilities |
| Test utilities | Helpers for common test patterns |

### 8.3 Ecosystem Compatibility

| Crate | Integration |
|-------|-------------|
| tokio | Native async support |
| tracing | Structured logging |
| regex | Pattern matching |
| russh | SSH backend |
| serde | Serializable configuration (optional) |

---

## 9. Testing Requirements

### 9.1 Test Categories

| Category | Description | Coverage Target |
|----------|-------------|-----------------|
| Unit Tests | Individual functions and modules | 90%+ |
| Integration Tests | Full session lifecycle | All platforms |
| Property Tests | Randomized input testing | Pattern matching, buffers |
| Stress Tests | Large output, many sessions | 100MB, 1GB, 100 sessions |
| Platform Tests | Platform-specific behavior | CI matrix |

### 9.2 CI Matrix

| Platform | Runner | Priority |
|----------|--------|----------|
| Linux x86_64 | ubuntu-latest | P0 |
| Linux ARM64 | ubuntu-24.04-arm | P1 |
| macOS x86_64 | macos-13 | P0 |
| macOS ARM64 | macos-latest | P0 |
| Windows x86_64 | windows-latest | P0 |

### 9.3 Test Reliability

| Requirement | Details |
|-------------|---------|
| No flaky tests | All tests deterministic |
| Timeout margins | Generous timeouts (10x expected duration) to avoid CI flakiness |
| Isolation | Each test spawns own processes; no shared state |
| Cleanup | All resources released after test; verified via Drop assertions |
| Deterministic commands | Use predictable programs, not interactive shells |

**Deterministic Test Command Guidelines:**

Tests SHOULD use deterministic programs rather than interactive shells where timing varies:

| Preferred | Avoid | Rationale |
|-----------|-------|-----------|
| `echo "hello"` | `bash -c "..."` | Echo is deterministic; shell startup varies |
| `cat` with piped input | Interactive `python` | Cat is simple; REPL timing varies |
| Custom test binaries | `ssh localhost` | Full control over behavior |
| `printf` for formatting | Shell string interpolation | Portable, predictable output |

**Test Fixture Binaries:**

The test suite SHOULD include purpose-built test binaries for complex scenarios:

| Binary | Purpose |
|--------|---------|
| `test-echo` | Echoes input with configurable delay |
| `test-prompt` | Simulates login prompts |
| `test-output` | Generates predictable output patterns |
| `test-signals` | Responds to signals predictably |
| `test-hang` | Simulates unresponsive processes |

These binaries ensure consistent behavior across platforms and CI environments.

### 9.4 Benchmark Suite

| Benchmark | Metric |
|-----------|--------|
| Spawn latency | Time to spawn and get first output |
| Pattern match | Throughput (MB/s) for large buffers |
| Memory usage | Peak memory for various output sizes |
| Concurrent sessions | Overhead per session |

---

## 10. Documentation Requirements

### 10.1 API Documentation

| Requirement | Details |
|-------------|---------|
| 100% coverage | All public items documented |
| Examples | Every function has at least one example |
| Panics | All panic conditions documented |
| Errors | All error conditions documented |
| Safety | Unsafe code has safety documentation |

### 10.2 Guide Documentation

| Document | Content |
|----------|---------|
| Getting Started | Installation, first example |
| Tutorial | Step-by-step common use cases |
| Patterns Guide | Pattern matching deep dive |
| Async Guide | Async usage patterns |
| SSH Guide | SSH session setup and usage |
| Migration Guide | From pexpect/rexpect/expectrl |
| Platform Notes | Platform-specific behavior |

### 10.3 Examples

| Example | Description |
|---------|-------------|
| basic.rs | Spawn, expect, send |
| ssh.rs | SSH session management |
| interactive.rs | Interactive mode |
| multi_session.rs | Multiple concurrent sessions |
| large_output.rs | Handling large output |
| dialog.rs | Dialog system usage |
| logging.rs | Tracing integration |

---

## 11. Success Criteria

### 11.1 Functional Success

| Criterion | Measurement |
|-----------|-------------|
| Feature completeness | All "Must Have" requirements implemented |
| Platform support | All 5 target platforms pass CI |
| API stability | No breaking changes after 1.0 |

### 11.2 Performance Success

| Criterion | Target |
|-----------|--------|
| Spawn latency | < 50ms on all platforms |
| 100MB output | < 1 second to process |
| Memory efficiency | < 1.5x output size for buffering |

### 11.3 Quality Success

| Criterion | Target |
|-----------|--------|
| Test coverage | > 80% line coverage |
| Documentation coverage | 100% public API |
| Zero known bugs | No open P0/P1 bugs at release |

### 11.4 Adoption Success

| Criterion | Target (6 months post-1.0) |
|-----------|----------------------------|
| GitHub stars | > 500 |
| Downloads/month | > 10,000 |
| Dependent crates | > 50 |
| Open issues | < 30 |

---

## 12. Appendices

### Appendix A: Glossary

| Term | Definition |
|------|------------|
| PTY | Pseudo-terminal; virtual terminal device for process I/O |
| ConPTY | Windows Console Pseudo Terminal API (Windows 10 1809+) |
| Overlapped I/O | Windows async I/O mechanism; ConPTY support in Windows 26H2+ |
| AsyncFd | Tokio type for registering file descriptors for async I/O |
| Session | Handle to a spawned process with PTY |
| Expect | Operation that waits for pattern match in output |
| Interact | Mode where user directly controls the process |
| Dialog | Reusable sequence of expect/send operations |
| PtyMaster | The controlling side of a PTY pair (where we read/write) |
| PtySlave | The process side of a PTY pair (appears as terminal to child) |
| Raw Mode | Terminal mode where input is not line-buffered |
| SIGWINCH | Unix signal sent when terminal window size changes |
| Job Object | Windows mechanism for managing groups of processes |
| VTE | Virtual Terminal Emulator; also the name of Alacritty's parser crate |
| Cancellation-safe | Async operation that preserves state correctly when cancelled |
| exp_continue | Original Expect feature to continue matching without re-entering expect |

### Appendix B: Reference Documents

| Document | Purpose |
|----------|---------|
| [Original Expect Manpage](https://www.tcl-lang.org/man/expect5.31/expect.1.html) | Feature reference |
| [pexpect Documentation](https://pexpect.readthedocs.io/) | Python implementation reference |
| [Alacritty tty module](https://docs.rs/alacritty_terminal/latest/alacritty_terminal/tty/) | Production PTY implementation reference |
| [portable-pty](https://lib.rs/crates/portable-pty) | Cross-platform PTY reference (WezTerm) |
| [pty-process](https://lib.rs/crates/pty-process) | Async PTY reference (Unix) |
| [crossterm](https://docs.rs/crossterm/latest/crossterm/) | Terminal manipulation, async input |
| [rustix](https://docs.rs/rustix/latest/rustix/) | Safe Unix syscall bindings |
| [windows-sys](https://docs.rs/windows-sys/latest/windows_sys/) | Windows API bindings |
| [russh](https://github.com/Eugeny/russh) | SSH library reference |
| [vte](https://docs.rs/vte/latest/vte/) | ANSI parser (Alacritty project) |
| [microsoft/terminal PR #17510](https://github.com/microsoft/terminal/pull/17510) | ConPTY overlapped I/O implementation |
| [LIBRARY_ANALYSIS.md](./LIBRARY_ANALYSIS.md) | Competitive analysis |

### Appendix C: Decision Log

| Decision | Rationale | Date |
|----------|-----------|------|
| Async-first architecture | Modern Rust idiom; enables multi-session | 2025-12-25 |
| Tokio as primary runtime | Industry standard; best process support | 2025-12-25 |
| Feature flags for optional deps | Minimize compile time for basic usage | 2025-12-25 |
| Dual MIT/Apache-2.0 license | Maximum ecosystem compatibility | 2025-12-25 |
| Builder pattern for spawn | Flexible configuration without overloads | 2025-12-25 |
| vte for ANSI parsing | 15x more adoption than vtparse (2.15M vs 143K/month), Alacritty backing | 2025-12-26 |
| MSRV 1.85 with Edition 2024 | Async closures, modern patterns; accept slightly reduced adoption for better code | 2025-12-26 |
| Explicit encoding handling | Critical for cross-platform; UTF-8 default with fallbacks | 2025-12-26 |
| Build custom PTY crate (`rust-pty`) | No existing crate provides async + cross-platform; ecosystem gap worth filling | 2025-12-26 |
| `rustix` over `nix` for Unix PTY | Modern, maintained, better API design; used by pty-process (296K/mo) | 2025-12-26 |
| `windows-sys` over `winapi` for ConPTY | Official Microsoft crate, actively maintained, better type safety | 2025-12-26 |
| Async `interact()` in 1.0 | Do it right the first time; sync-only would require breaking API change later | 2025-12-26 |
| crossterm for terminal input | Cross-platform, async support via `event-stream`, widely adopted (4.8M/mo) | 2025-12-26 |
| Runtime Windows version detection | Forward-compatible with overlapped I/O (26H2+); graceful fallback for older | 2025-12-26 |
| Thread-per-pipe for pre-26H2 Windows | Proven pattern from Alacritty/WezTerm; unavoidable given ConPTY sync-only I/O | 2025-12-26 |
| Job Objects for Windows process trees | Reliable child process termination; prevents orphaned processes | 2025-12-26 |
| signal-hook for Unix signals | Cross-platform (macOS + Linux), tokio-compatible, well-maintained | 2025-12-26 |

### Appendix D: Resolved Questions

All questions have been resolved. No open questions remain for 1.0 scope.

| Question | Status | Resolution |
|----------|--------|------------|
| Screen buffer parser | **Resolved** | Use `vte` (2.15M downloads/month, Alacritty project, v0.15.0). Alternative: `vtparse` for dynamic OSC buffers. |
| MSRV policy | **Resolved** | **1.85** with Edition 2024. Required for async closures and modern patterns. |
| russh crypto backend | **Resolved** | Document that russh defaults to `aws-lc-rs` with `ring` fallback; has cross-platform build implications. |
| Should `interact` be sync-only? | **Resolved** | **No.** Async `interact()` is required for 1.0 (FR-4.1.4). Implementation uses crossterm's `event-stream` feature for async terminal input. Terminal raw mode management handled via crossterm. |
| PTY backend selection | **Resolved** | **Build custom `rust-pty` crate.** No existing crate provides async + cross-platform. Unix: `rustix` + tokio `AsyncFd`. Windows: `windows-sys` + ConPTY with thread-per-pipe (sync) or overlapped I/O (26H2+). See FR-9. |
| Windows async strategy | **Resolved** | Runtime Windows version detection. Use overlapped I/O on 26H2+, fall back to thread-per-pipe pattern on older versions. Unified async interface regardless of underlying mechanism. |
| WebSocket session backend | **Deferred** | Post-1.0 consideration. Architecture should not preclude future addition. |
| Container exec (Docker/K8s) | **Deferred** | Post-1.0 consideration. Architecture should not preclude future addition. |
| Serial port support | **Deferred** | Post-1.0 consideration. Use `serialport` crate; architecture should allow future `SerialSession` type. |

**Deferred Items Rationale:** WebSocket, container exec, and serial port support are explicitly out of scope for 1.0 but the architecture (trait-based session abstraction) is designed to accommodate these as future backends without breaking changes.

### Appendix E: Versioning & Compatibility

| Policy | Commitment |
|--------|------------|
| Semantic Versioning | Strict SemVer for all releases |
| Breaking Changes | None after 1.0 without major version bump |
| Deprecation Period | Minimum 2 minor versions before removal |
| MSRV Bumps | Considered breaking; requires minor version bump |
| Feature Flag Stability | Feature flags stable after 1.0 |

---

*This document is the authoritative source of functional requirements for rust-expect. All implementation work should trace back to requirements defined herein.*
