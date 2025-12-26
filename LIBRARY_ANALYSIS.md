# Expect-Style Library Deep-Dive Analysis

**Date:** 2025-12-25
**Scope:** expectrl (Rust), rexpect (Rust), pexpect (Python)

---

## Executive Summary

| Aspect | expectrl | rexpect | pexpect |
|--------|----------|---------|---------|
| **Language** | Rust | Rust | Python |
| **Stars** | 198 | 370 | 2,800 |
| **Downloads/month** | ~9,655 | ~54,538 | Millions |
| **Latest Version** | 0.8.0 (Sep 2025) | 0.6.2 (Jun 2025) | 4.9.0 (Nov 2023) |
| **Windows Support** | Partial (conpty) | None | Limited |
| **Async Support** | Yes | No | Partial |
| **Open Issues** | 14 | 28 | 150 |
| **Maturity** | Moderate | Moderate | Very High |

*Note: Rust crate statistics sourced from [lib.rs](https://lib.rs). GitHub stars and issues from respective repositories. Download metrics represent monthly averages and may vary by snapshot date.*

---

## 1. expectrl (zhiburt/expectrl)

### Overview
A Rust library for controlling interactive programs in a pseudo-terminal, inspired by both rexpect and pexpect. Positioned as a more feature-rich alternative to rexpect.

**Repository:** https://github.com/zhiburt/expectrl
**Crates.io:** https://crates.io/crates/expectrl
**Docs:** https://docs.rs/expectrl

### Strengths

1. **Async/Await Support**: First-class async support via optional feature flag
2. **Cross-Platform Ambition**: Attempts Windows support via conpty
3. **Logging Integration**: Built-in logging capabilities for debugging
4. **Interactive Mode**: Supports `interact()` for human-in-the-loop sessions
5. **Active Development**: Updated as recently as Sep 2025
6. **Clean API Design**: Well-documented (92% coverage) with sensible abstractions
7. **Feature Flags**: Modular design with optional features (`async`, `polling`)

### Weaknesses

1. **Version Lag on crates.io**: Published version often lacks changes from master branch
   - Source: [Issue #13](https://github.com/zhiburt/expectrl/issues/13)

2. **Windows Issues**:
   - Command arguments not respected ([Issue #63](https://github.com/zhiburt/expectrl/issues/63))
   - GitHub Actions failures ([Issue #52](https://github.com/zhiburt/expectrl/issues/52))
   - conpty 0.3 limitations cause argument handling issues

3. **Interactive Mode Bugs**:
   - Character duplication after `interact()` ([Issue #50](https://github.com/zhiburt/expectrl/issues/50))
   - Mac typing not echoed ([Issue #48](https://github.com/zhiburt/expectrl/issues/48))
   - Input echo issues with bash ([Issue #10](https://github.com/zhiburt/expectrl/issues/10))

4. **Blocking Interactive Mode**: No non-blocking reads for `interact()` ([Issue #13](https://github.com/zhiburt/expectrl/issues/13))

5. **Missing Features**:
   - Cannot spawn with modified environment variables ([Issue #69](https://github.com/zhiburt/expectrl/issues/69))
   - No SSH/pxssh integration
   - No method to extract remaining buffer on timeout ([Issue #75](https://github.com/zhiburt/expectrl/issues/75))

6. **Small Contributor Base**: Only 4 contributors

### Key Issues Summary

| Issue | Description | Status | Impact |
|-------|-------------|--------|--------|
| #75 | No buffer extraction on timeout | Open | Medium |
| #69 | Cannot spawn with env vars | Open | High |
| #63 | Windows command args ignored | Open | High |
| #52 | Windows GitHub Actions broken | Open | Medium |
| #50 | Character duplication in interact | Open | Medium |
| #48 | Mac interact echo issues | Open | Medium |
| #13 | Blocking interactive mode | Open | High |

### Architecture Note

expectrl uses the [ptyprocess](https://crates.io/crates/ptyprocess) crate as its PTY backend on Unix systems. ptyprocess was developed by the same author (Maxim Zhiburt) specifically as the foundation for expectrl. For Windows, it uses [conpty](https://crates.io/crates/conpty).

### Trade-offs & Design Decisions

- **Chose conpty for Windows**: Provides Windows support but with limitations
- **Optional async**: Keeps sync API simple, async opt-in
- **PTY-based via ptyprocess**: Provides authentic terminal behavior but OS-dependent
- **Separate PTY abstraction**: ptyprocess can be used directly for lower-level control

### User Feedback Themes

- Users appreciate the async support and logging
- Windows support is problematic and unreliable
- Interactive mode has shell-specific quirks
- Documentation is generally good

---

## 2. rexpect (rust-cli/rexpect)

### Overview
A Rust port of Python's pexpect, focused on process automation and testing. Part of the rust-cli organization with broader community support.

**Repository:** https://github.com/rust-cli/rexpect
**Crates.io:** https://crates.io/crates/rexpect
**Docs:** https://docs.rs/rexpect

### Strengths

1. **Community Backing**: Part of rust-cli organization, 23+ contributors
2. **Higher Adoption**: ~54K downloads/month, used by 56 crates (per lib.rs) / 1,176 repositories (per GitHub)
3. **Simpler API**: Straightforward pexpect-style interface
4. **Stable Design**: Well-understood patterns from pexpect
5. **Better Documented Use Cases**: Clear examples for FTP, bash, job control
6. **Dual License**: MIT + Apache-2.0 flexibility

### Weaknesses

1. **No Windows Support**: Unix-only, will not compile on Windows
   - [Issue #11](https://github.com/rust-cli/rexpect/issues/11) open since Feb 2020
   - Requires WSL or VM for Windows users

2. **No Async Support**: Synchronous only, blocking operations

3. **GitHub Release Lag**: GitHub shows only v0.5.0 (Oct 2022) as a tagged release, though crates.io has v0.6.2 (Jun 2025)
   - This discrepancy causes confusion about maintenance status
   - [Issue #114](https://github.com/rust-cli/rexpect/issues/114) initially requested release (now resolved on crates.io)

4. **Timeout Handling Issues**:
   - Sleeps not relative to timeout size ([Issue #144](https://github.com/rust-cli/rexpect/issues/144))
   - Resume after timeout fails ([Issue #125](https://github.com/rust-cli/rexpect/issues/125))

5. **Unicode Problems**: Text integrity issues through read/write cycles ([Issue #105](https://github.com/rust-cli/rexpect/issues/105))

6. **Flaky CI**: Tests fail sporadically ([Issue #104](https://github.com/rust-cli/rexpect/issues/104))

7. **Missing Features**:
   - Cannot set terminal dimensions ([Issue #119](https://github.com/rust-cli/rexpect/issues/119))
   - No environment variable passing ([Issue #121](https://github.com/rust-cli/rexpect/issues/121))
   - No debugging capabilities ([Issue #123](https://github.com/rust-cli/rexpect/issues/123))
   - Bracketed paste mode issues ([Issue #143](https://github.com/rust-cli/rexpect/issues/143))

8. **28 Open Issues**: Significant backlog

### Key Issues Summary

| Issue | Description | Status | Impact |
|-------|-------------|--------|--------|
| #144 | Sleep intervals not optimized | Open | Medium |
| #143 | Bracketed paste mode problems | Open | Low |
| #135 | Child won't respond to quit | Open | High |
| #125 | Resume after timeout fails | Open | High |
| #121 | No env var support | Open | High |
| #119 | No terminal dimension control | Open | Medium |
| #114 | Need new release | Open | High |
| #105 | Unicode integrity issues | Open | Medium |
| #11 | No Windows support | Open | Critical |

### Trade-offs & Design Decisions

- **Unix-only**: Simplifies implementation, sacrifices portability
- **Sync-only**: Simpler mental model, limits use cases
- **pexpect-faithful**: Familiar API, inherits pexpect limitations

### User Feedback Themes

- Widely used for CLI testing
- Windows users frustrated by lack of support
- Requests for new releases go unaddressed
- Generally stable for Unix use cases

---

## 3. pexpect (pexpect/pexpect)

### Overview
The original Python expect-style library, mature and widely adopted. The reference implementation that inspired the Rust alternatives.

**Repository:** https://github.com/pexpect/pexpect
**PyPI:** https://pypi.org/project/pexpect/
**Docs:** https://pexpect.readthedocs.io/

### Strengths

1. **Extreme Maturity**: 20+ years of development (since 2003)
2. **Massive Ecosystem**: 500K+ dependents, millions of downloads
3. **Comprehensive Documentation**: Extensive guides, examples, API docs
4. **Feature Rich**:
   - `pxssh` for SSH automation
   - `fdpexpect` for file descriptor control
   - `popen_spawn` for subprocess integration
   - `replwrap` for REPL interactions
5. **95 Contributors**: Large community support
6. **ISC License**: Permissive, GPL-compatible
7. **Known Workarounds**: Most issues have documented solutions

### Weaknesses

1. **Windows Support Limitations**:
   - Main features require `pty` module (Unix-only)
   - `pexpect.spawn` doesn't work on Windows ([Issue #439](https://github.com/pexpect/pexpect/issues/439))
   - Workarounds exist but are incomplete

2. **Performance Issues**:
   - Very poor performance with large output streams ([Issue #438](https://github.com/pexpect/pexpect/issues/438))
   - Anything >50MB takes 30+ minutes
   - Slow spawning in Docker containers ([Issue #633](https://github.com/pexpect/pexpect/issues/633))

3. **Maintenance Concerns**:
   - No new releases in 2+ years (last: Nov 2023)
   - Missing security policy
   - 150 open issues backlog

4. **Python-Specific Issues**:
   - Python 3.12 `os.fork()` warnings ([Issue #817](https://github.com/pexpect/pexpect/issues/817))
   - Test failures with parallel execution ([Issue #809](https://github.com/pexpect/pexpect/issues/809))
   - Async behavior inconsistencies ([Issue #789](https://github.com/pexpect/pexpect/issues/789))

5. **Timeout Bugs**: `use_poll=True` breaks timeout behavior ([Issue #491](https://github.com/pexpect/pexpect/issues/491))

6. **Output Truncation**: Some text cut with ellipsis ([Issue #811](https://github.com/pexpect/pexpect/issues/811))

7. **Threading Limitations**: Must spawn and interact in same thread

### Key Issues Summary

| Issue | Description | Status | Impact |
|-------|-------------|--------|--------|
| #823 | Async test failures | Open | Medium |
| #817 | Python 3.12 fork warnings | Open | Medium |
| #811 | Output truncation | Open | High |
| #809 | Parallel test failures | Open | Medium |
| #491 | Timeout with use_poll broken | Open | High |
| #439 | Windows spawn broken | Open | Critical |
| #438 | Poor large output performance | Open | High |

### Common Problems & Solutions

| Problem | Solution |
|---------|----------|
| Password echo in output | Tune `delaybeforesend` attribute |
| Large output slow | Increase `maxread`, set `searchwindowsize` |
| Docker slowness | Set `--ulimit nofile=1024` |
| Threading issues | Spawn and interact in same thread |
| Windows | Use `wexpect` or `PopenSpawn` |

### Trade-offs & Design Decisions

- **Pure Python**: Portability over performance
- **PTY-based**: Authentic terminal, Unix-centric
- **Backward compatibility**: Supports Python 2.7+, limits modernization

### User Feedback Themes

- De facto standard for Python terminal automation
- Performance issues with large outputs are well-known
- Windows support is a constant pain point
- Mature but showing age, especially with async

---

## Comparative Analysis

### Feature Comparison

| Feature | expectrl | rexpect | pexpect |
|---------|----------|---------|---------|
| Regex matching | ✅ | ✅ | ✅ |
| String matching | ✅ | ✅ | ✅ |
| EOF detection | ✅ | ✅ | ✅ |
| Timeout handling | ✅ | ✅ | ✅ |
| Async support | ✅ | ❌ | Partial |
| Windows support | Partial | ❌ | Limited |
| SSH integration | ❌ | ❌ | ✅ (pxssh) |
| REPL wrapper | ❌ | ❌ | ✅ |
| Logging built-in | ✅ | ❌ | ❌ |
| Interactive mode | ✅ | ❌ | ✅ |
| Env var passing | ❌ | ❌ | ✅ |
| Terminal dimensions | ❌ | ❌ | ✅ |

### Platform Support

| Platform | expectrl | rexpect | pexpect |
|----------|----------|---------|---------|
| Linux | ✅ | ✅ | ✅ |
| macOS | ⚠️ (interact issues) | ✅ | ✅ |
| Windows | ⚠️ (conpty issues) | ❌ | ⚠️ (limited) |
| WSL | ✅ | ✅ | ✅ |

### When to Use Each

**Use expectrl when:**
- You need async/await in Rust
- Windows support (with caveats) is required
- Logging integration is valuable
- You want the most actively developed Rust option

**Use rexpect when:**
- You're on Unix-only environments
- You want simpler, pexpect-like API
- Sync-only operation is acceptable
- You need broader community adoption/support

**Use pexpect when:**
- You're working in Python
- You need SSH integration (pxssh)
- Maximum ecosystem compatibility matters
- Extensive documentation is critical

### Risk Assessment

| Library | Risk Level | Primary Concerns |
|---------|------------|------------------|
| expectrl | Medium | Small team (4 contributors), Windows bugs, master-to-crates.io lag |
| rexpect | Medium | No Windows support, 28 open issues, GitHub release tagging lag |
| pexpect | Low-Medium | Maintenance slowdown (no release since Nov 2023), performance issues |

---

## Recommendations

### For New Rust Projects

**Primary recommendation: expectrl**
- More features (async, logging, Windows attempt)
- More recent updates
- Better positioned for cross-platform needs

**Alternative: rexpect**
- If Unix-only and prefer simpler API
- Higher adoption provides more battle-testing

### For Python Projects

**Continue using pexpect**
- Unmatched maturity and ecosystem
- Well-documented workarounds for known issues
- Consider `wexpect` for Windows-specific needs
- Consider `Paramiko` or `RedExpect` for pure SSH work

### For Cross-Platform Needs

**No great option exists**
- expectrl is closest but has significant Windows issues
- Consider architecture with platform-specific backends
- Or use process-level abstraction that falls back gracefully

---

## Sources

### expectrl
- [GitHub Repository](https://github.com/zhiburt/expectrl)
- [Issues](https://github.com/zhiburt/expectrl/issues)
- [Docs.rs](https://docs.rs/expectrl)
- [Lib.rs Analysis](https://lib.rs/crates/expectrl)

### rexpect
- [GitHub Repository](https://github.com/rust-cli/rexpect)
- [Issues](https://github.com/rust-cli/rexpect/issues)
- [Docs.rs](https://docs.rs/rexpect)
- [Lib.rs Analysis](https://lib.rs/crates/rexpect)
- [Rust Adventure Tutorial](https://www.rustadventure.dev/building-a-digital-garden-cli/clap-v4/testing-interactive-clis-with-rexpect)

### pexpect
- [GitHub Repository](https://github.com/pexpect/pexpect)
- [Issues](https://github.com/pexpect/pexpect/issues)
- [Official Documentation](https://pexpect.readthedocs.io/)
- [PyPI](https://pypi.org/project/pexpect/)
- [Common Issues Guide](https://pexpect.readthedocs.io/en/stable/commonissues.html)
- [Performance Issue #438](https://github.com/pexpect/pexpect/issues/438)

### Related Crates
- [ptyprocess](https://crates.io/crates/ptyprocess) - PTY backend for expectrl (same author)
- [conpty](https://crates.io/crates/conpty) - Windows pseudo-console used by expectrl
- [portable-pty](https://crates.io/crates/portable-pty) - Cross-platform PTY abstraction (part of wezterm)
- [anticipate](https://crates.io/crates/anticipate) - Fork of expectrl focused on asciinema automation (~455 downloads/month)

### Comparisons & Alternatives
- [Paramiko vs Pexpect vs Fabric](https://piptrends.com/compare/paramiko-vs-pexpect-vs-fabric)
- [Rust PTY Libraries](https://crates.io/keywords/pty)
- [wexpect (Windows alternative)](https://pypi.org/project/wexpect/)

---

# Part II: Design Specification for a Superior Implementation

**Objective:** Create a Rust expect-style library that exceeds ALL existing implementations in every dimension - features, performance, cross-platform support, API ergonomics, and reliability.

---

## Competitive Gap Analysis

### Features NO Existing Library Has

Based on analysis of the original [Expect manpage](https://www.tcl-lang.org/man/expect5.31/expect.1.html) and user requests across all projects, these capabilities exist in the original Tcl Expect but are **missing from ALL modern implementations**:

| Feature | Original Expect | expectrl | rexpect | pexpect |
|---------|-----------------|----------|---------|---------|
| `expect_background` (non-blocking) | ✅ | ❌ | ❌ | ❌ |
| `expect_before`/`expect_after` | ✅ | ❌ | ❌ | ❌ |
| `exp_continue` (continue matching) | ✅ | ❌ | ❌ | ❌ |
| Advanced `interact` with patterns | ✅ | ❌ | ❌ | ❌ |
| Multi-spawn management (`-i` flag) | ✅ | ❌ | ❌ | ❌ |
| Indirect spawn IDs | ✅ | ❌ | ❌ | ❌ |
| `send_slow`/`send_human` | ✅ | ❌ | ❌ | ❌ |
| `fork` (process cloning) | ✅ | ❌ | ❌ | ❌ |
| `disconnect` (background mode) | ✅ | ❌ | ❌ | ❌ |
| Spawn with `-open` (files/pipes) | ✅ | ❌ | ❌ | ❌ |
| Signal trapping (`trap`) | ✅ | ❌ | ❌ | ❌ |
| Dialogs concept | ✅ | ❌ | ❌ | ❌ |

### Critical Gaps Across All Implementations

| Gap | expectrl | rexpect | pexpect | Impact |
|-----|----------|---------|---------|--------|
| **Robust Windows support** | Broken | None | Limited | Critical |
| **True async/non-blocking** | Partial | None | Partial | High |
| **Multi-process orchestration** | None | None | Limited | High |
| **Environment variable passing** | Missing | Missing | Works | High |
| **Terminal dimension control** | Missing | Missing | Works | Medium |
| **Buffer extraction on timeout** | Missing | Missing | Works | Medium |
| **Performance with large output** | Unknown | Unknown | Very Poor | High |
| **Thread safety** | Unknown | N/A | Broken | High |
| **SSH integration** | None | None | pxssh | Medium |
| **ANSI parsing/screen state** | None | None | Deprecated | Medium |

### User-Requested Features (Unmet Across Ecosystem)

From GitHub issues across all three projects:

1. **Multithread/multiprocess support** - pexpect [#369](https://github.com/pexpect/pexpect/issues/369)
2. **Wait for multiple processes** - pexpect [#50](https://github.com/pexpect/pexpect/issues/50)
3. **Configurable timeout behavior** - rexpect [#142](https://github.com/rust-cli/rexpect/issues/142), [#144](https://github.com/rust-cli/rexpect/issues/144)
4. **Debugging/tracing capabilities** - rexpect [#123](https://github.com/rust-cli/rexpect/issues/123)
5. **Discard output mode** - pexpect [#54](https://github.com/pexpect/pexpect/issues/54)
6. **Standard logging integration** - pexpect [#133](https://github.com/pexpect/pexpect/issues/133)
7. **Better timeout error messages** - pexpect [#130](https://github.com/pexpect/pexpect/issues/130)
8. **Bracketed paste mode control** - rexpect [#143](https://github.com/rust-cli/rexpect/issues/143)

---

## Design Requirements

### Tier 1: Must Have (Minimum Viable Superiority)

These features are **required** to exceed all competitors:

#### 1.1 True Cross-Platform PTY
- Linux: Native PTY via `nix` crate
- macOS: Native PTY with proper SIGWINCH handling
- Windows: ConPTY with correct argument handling
- **Backend abstraction**: Runtime-selectable implementation (like [portable-pty](https://lib.rs/crates/portable-pty))

#### 1.2 Async-First Architecture
- Native `async`/`await` with Tokio runtime
- Sync API as thin wrapper over async (not vice versa)
- Non-blocking pattern matching (like Expect's `expect_background`)
- `tokio::select!` compatible for multi-session management

#### 1.3 Complete Pattern Matching
- Regex via `regex` crate
- Glob patterns
- Exact string matching
- EOF detection
- Timeout with configurable behavior
- **`exp_continue`** - continue matching after action without re-entering expect
- **`expect_before`/`expect_after`** - persistent patterns across all expect calls

#### 1.4 Environment & Terminal Control
- Full environment variable passing on spawn
- Terminal dimension setting and dynamic resize (SIGWINCH)
- Bracketed paste mode control
- Raw mode toggling

#### 1.5 Robust Error Handling
- Detailed timeout errors with duration and context
- Buffer state accessible on timeout/error
- Proper cleanup on panic (no zombie processes)
- Thread-safe session handles

### Tier 2: Should Have (Clear Competitive Advantage)

#### 2.1 Multi-Session Orchestration
- Spawn and manage multiple processes concurrently
- `tokio::select!` across multiple sessions
- Session groups with shared patterns
- Indirect session references (dynamic session lists)

#### 2.2 Advanced Interactive Mode
- Pattern detection DURING interaction (like original Expect)
- Hookable input/output streams
- Non-blocking interaction with timeout
- Session handoff between multiple spawned processes

#### 2.3 Human-Like Typing
- `send_slow` - configurable character delay
- `send_human` - variable timing to simulate human typing
- Configurable `delaybeforesend` (pexpect's solution to timing bugs)

#### 2.4 Logging & Debugging
- Structured logging via `tracing` crate
- Session transcript recording
- Pattern match attempt tracing
- Debug mode with internal state exposure

#### 2.5 SSH Integration
- Native SSH via [russh](https://github.com/Eugeny/russh) (actively maintained)
- `pxssh`-equivalent convenience wrapper
- Key-based and password authentication
- Jump host / bastion support

### Tier 3: Nice to Have (Excellence Beyond Expectations)

#### 3.1 ANSI/Terminal Emulation
- State machine parser via [vtparse](https://docs.rs/vtparse) or [anstyle-parse](https://docs.rs/anstyle-parse)
- Virtual screen buffer (like [memterm](https://docs.rs/memterm) / Python's pyte)
- Screen scraping capabilities
- Cursor position tracking

#### 3.2 Dialog System
- Reusable dialog definitions (pattern → action mappings)
- Dialog composition and chaining
- State machine-based conversation flows
- Error recovery dialogs

#### 3.3 Process Control
- `fork` equivalent (spawn exact copy)
- `disconnect` (background/daemonize)
- Signal forwarding and trapping
- Spawn from file descriptors/pipes (Expect's `-open` flag)

#### 3.4 Performance Optimizations
- Zero-copy buffer handling where possible
- Configurable `searchwindowsize` (pexpect's solution for large outputs)
- Streaming pattern matching (avoid buffering entire output)
- Memory-mapped I/O for large transcripts

---

## Recommended Technology Stack

### Core Dependencies

| Component | Recommended Crate | Rationale |
|-----------|-------------------|-----------|
| **Async Runtime** | `tokio` | Industry standard, excellent process support |
| **PTY (Unix)** | `nix` + custom | Direct control, avoid abstraction overhead |
| **PTY (Windows)** | `windows` crate + ConPTY | Official Windows crate, latest API |
| **Cross-Platform PTY** | `portable-pty` (reference) | Proven in WezTerm, 213K downloads/month |
| **Regex** | `regex` | Fast, safe, well-maintained |
| **SSH** | `russh` | Active development, modern async API |
| **Logging** | `tracing` | Structured, async-compatible |
| **Error Handling** | `thiserror` | Ergonomic, zero-cost |
| **ANSI Parsing** | `vtparse` or `anstyle-parse` | State machine based, proven |
| **Terminal Screen** | `memterm` (or custom) | Rust pyte equivalent |
| **Signal Handling** | `signal-hook` | Cross-platform, Tokio compatible |

### Feature Flags

```toml
[features]
default = ["sync"]
sync = []                    # Synchronous API (thin async wrapper)
async = ["tokio"]            # Full async support
ssh = ["russh"]              # SSH integration
screen = ["vtparse"]         # ANSI parsing and screen buffer
full = ["async", "ssh", "screen"]
```

### Licensing Compatibility

All recommended dependencies are compatible with permissive licensing:

| Crate | License | Compatibility |
|-------|---------|---------------|
| tokio | MIT | ✅ Permissive |
| nix | MIT | ✅ Permissive |
| windows | MIT/Apache-2.0 | ✅ Permissive |
| portable-pty | MIT | ✅ Permissive |
| regex | MIT/Apache-2.0 | ✅ Permissive |
| russh | Apache-2.0 | ✅ Permissive |
| tracing | MIT | ✅ Permissive |
| thiserror | MIT/Apache-2.0 | ✅ Permissive |
| vtparse | MIT | ✅ Permissive |
| signal-hook | MIT/Apache-2.0 | ✅ Permissive |

**Recommended License:** Dual MIT/Apache-2.0 (standard for Rust ecosystem, maximum compatibility)

---

## Architecture Recommendations

### Layered Design

```
┌─────────────────────────────────────────────────────────────┐
│                      User API Layer                          │
│   Session, Expect, Send, Interact, SSH, Dialog              │
├─────────────────────────────────────────────────────────────┤
│                    Pattern Matching Engine                   │
│   Regex, Glob, Exact, EOF, Timeout, Composite               │
├─────────────────────────────────────────────────────────────┤
│                     Stream Abstraction                       │
│   AsyncRead/AsyncWrite, Buffering, Logging                  │
├─────────────────────────────────────────────────────────────┤
│                    Process Abstraction                       │
│   Spawn, Environment, Signals, Cleanup                      │
├─────────────────────────────────────────────────────────────┤
│                     PTY Backend Layer                        │
│   ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐       │
│   │  Linux  │  │  macOS  │  │ Windows │  │   SSH   │       │
│   │   PTY   │  │   PTY   │  │ ConPTY  │  │ Channel │       │
│   └─────────┘  └─────────┘  └─────────┘  └─────────┘       │
└─────────────────────────────────────────────────────────────┘
```

### Key Design Principles

1. **Async-first, sync-compatible**: Core is async; sync API uses `block_on`
2. **Backend-agnostic sessions**: Same `Session` type works with PTY or SSH
3. **Zero-cost abstractions**: Feature flags eliminate unused code paths
4. **Fail-fast with context**: Rich errors, never silent failures
5. **Thread-safe by default**: `Send + Sync` session handles
6. **Cancelation-safe**: Proper cleanup on `Drop` and async cancelation

### Session Lifecycle

```rust
// Conceptual API
let session = Session::spawn("bash")
    .env("TERM", "xterm-256color")
    .dimensions(80, 24)
    .timeout(Duration::from_secs(30))
    .build()
    .await?;

// Pattern matching with exp_continue equivalent
session.expect(patterns![
    regex!(r"password:") => |s| { s.send_line("secret")?; Continue },
    regex!(r"\$\s*$") => |s| { Break(s.matched()) },
    Timeout => |s| { Err(TimeoutError::with_buffer(s.buffer())) },
])?;

// Multi-session orchestration
tokio::select! {
    result = session1.expect("ready") => { /* ... */ },
    result = session2.expect("ready") => { /* ... */ },
}

// Advanced interact with pattern hooks
session.interact()
    .on_output(regex!(r"error"), |ctx| log::warn!("{}", ctx.matched()))
    .on_input("quit", |ctx| ctx.send_to_process("exit\n"))
    .run()
    .await?;
```

---

## Implementation Roadmap

### Phase 1: Foundation (Core Superiority)
1. Cross-platform PTY abstraction with proper Windows support
2. Async spawn with environment and dimension control
3. Basic pattern matching (regex, string, EOF, timeout)
4. Buffer management with extraction on timeout
5. Comprehensive test suite across platforms

### Phase 2: Power Features
1. `exp_continue` and `expect_before`/`expect_after`
2. Multi-session management with `select!`
3. Advanced interact with pattern hooks
4. `send_slow`/`send_human` typing simulation
5. Structured logging and debugging

### Phase 3: Integration
1. SSH backend via russh
2. ANSI parsing and screen buffer
3. Dialog system for reusable patterns
4. Signal handling and process control
5. Performance optimization for large outputs

### Phase 4: Ecosystem
1. Comprehensive documentation with examples
2. Migration guides from pexpect/rexpect/expectrl
3. CLI tool for interactive development
4. Integration with test frameworks (cargo test, nextest)

---

## Testing Infrastructure

### CI Matrix

Cross-platform validation requires comprehensive CI coverage:

| Platform | Runner | PTY Backend | Priority |
|----------|--------|-------------|----------|
| Linux x86_64 | ubuntu-latest | Native PTY | P0 |
| Linux ARM64 | ubuntu-24.04-arm | Native PTY | P1 |
| macOS x86_64 | macos-13 | Native PTY | P0 |
| macOS ARM64 | macos-latest | Native PTY | P0 |
| Windows x86_64 | windows-latest | ConPTY | P0 |

### Testing Strategy

1. **Unit Tests**: Pattern matching engine, buffer management, timeout logic
2. **Integration Tests**: Full session lifecycle with real processes (bash, python, ssh)
3. **Property-Based Tests**: Use `proptest` for:
   - Arbitrary byte sequences through PTY
   - Random pattern/input combinations
   - Timeout edge cases
4. **Stress Tests**: Large output handling (10MB, 100MB, 1GB)
5. **Flakiness Prevention**:
   - Deterministic timeouts with generous margins
   - Retry logic for inherently racy PTY operations
   - Isolated test processes (no shared state)

### Test Utilities

```rust
// Test helper for cross-platform shell commands
fn test_shell() -> &'static str {
    if cfg!(windows) { "powershell" } else { "bash" }
}

// Macro for platform-conditional tests
#[cfg(unix)]
#[test]
fn test_unix_specific() { /* ... */ }
```

---

## Success Metrics

A successful implementation will:

| Metric | Target |
|--------|--------|
| **Platform Support** | Linux ✅, macOS ✅, Windows ✅ (no caveats) |
| **API Completeness** | All original Expect features + modern additions |
| **Performance** | Handle 100MB+ output without degradation |
| **Reliability** | Zero flaky tests, no zombie processes |
| **Ergonomics** | Simpler than pexpect for common cases |
| **Documentation** | 100% public API coverage |
| **Adoption** | Become the default choice for Rust terminal automation |

---

## Competitive Positioning

| Dimension | Our Target | vs expectrl | vs rexpect | vs pexpect |
|-----------|------------|-------------|------------|------------|
| Windows | First-class | Far better | N/A | Far better |
| Async | Native | Equal | Far better | Far better |
| Features | Original Expect++ | Far better | Far better | Better |
| Performance | Excellent | Better | Better | Far better |
| API Ergonomics | Best-in-class | Better | Better | Equal |
| Documentation | Comprehensive | Better | Better | Equal |
| Maintenance | Active | Better | Better | Better |
