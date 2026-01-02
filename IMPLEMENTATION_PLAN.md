# rust-expect Implementation Plan

**Version:** 1.0.0
**Created:** 2025-12-30
**Status:** Active
**Total Stub Files:** 184

---

## Executive Summary

This document defines the phased implementation strategy for the rust-expect workspace.
The plan ensures correct dependency ordering, enables incremental `cargo check` validation,
and maximizes parallelization opportunities.

### Key Metrics

| Category | Count | Parallelizable |
|----------|-------|----------------|
| Cargo Configuration | 11 | No |
| Tooling Config | 7 | Yes |
| Core Types (rust-pty) | 5 | No |
| Platform Code (rust-pty) | 9 | Partial |
| Proc-Macro Crate | 5 | No |
| Core Types (rust-expect) | 5 | No |
| Core Modules | 8 | No |
| Feature Modules | 48 | Partial |
| Test Utilities | 13 | Yes |
| Integration Tests | 21 | Yes |
| Examples | 13 | Yes |
| Benchmarks | 6 | Yes |
| CI/CD & GitHub | 13 | Yes |
| Documentation | 8 | Yes |
| Fixtures | 8 | Yes |
| Licenses | 2 | Yes |
| **Total** | **184** | |

---

## Dependency Graph

```
                    ┌─────────────────┐
                    │  rust-toolchain │
                    │    Cargo.toml   │
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
              ▼              ▼              ▼
        ┌──────────┐  ┌──────────────┐  ┌──────────┐
        │ rust-pty │  │rust-expect-  │  │test-utils│
        │          │  │   macros     │  │          │
        └────┬─────┘  └──────┬───────┘  └────┬─────┘
             │               │               │
             │               │               │
             └───────┬───────┘               │
                     ▼                       │
              ┌──────────────┐               │
              │ rust-expect  │◄──────────────┘
              │ (main crate) │
              └──────┬───────┘
                     │
        ┌────────────┼────────────┐
        ▼            ▼            ▼
    ┌───────┐   ┌────────┐   ┌──────────┐
    │ tests │   │examples│   │benchmarks│
    └───────┘   └────────┘   └──────────┘
```

---

## Phase 1: Cargo & Toolchain Setup

**Goal:** Enable `cargo check` to pass (even with empty modules)
**Blocking:** All subsequent phases
**Parallelizable:** No

### 1.1 Root Workspace Configuration

| File | Purpose | Priority |
|------|---------|----------|
| `rust-toolchain.toml` | Pin Rust 1.85, Edition 2024 | 1 |
| `Cargo.toml` | Workspace manifest, resolver v3 | 1 |
| `.cargo/config.toml` | MSRV resolver, build settings | 1 |

### 1.2 Crate Cargo.toml Files

| File | Dependencies | Priority |
|------|--------------|----------|
| `crates/rust-pty/Cargo.toml` | None | 2 |
| `crates/rust-expect-macros/Cargo.toml` | None (proc-macro) | 2 |
| `crates/rust-expect/Cargo.toml` | rust-pty, rust-expect-macros | 2 |
| `test-utils/Cargo.toml` | Virtual workspace | 3 |
| `test-utils/test-echo/Cargo.toml` | None | 3 |
| `test-utils/test-prompt/Cargo.toml` | None | 3 |
| `test-utils/test-output/Cargo.toml` | None | 3 |
| `test-utils/test-signals/Cargo.toml` | None | 3 |
| `test-utils/test-timing/Cargo.toml` | None | 3 |
| `test-utils/test-hang/Cargo.toml` | None | 3 |

### 1.3 Tooling Configuration

| File | Purpose | Parallelizable |
|------|---------|----------------|
| `rustfmt.toml` | Code formatting rules | Yes |
| `clippy.toml` | Lint configuration | Yes |
| `deny.toml` | Dependency audit rules | Yes |
| `cliff.toml` | Changelog generation | Yes |
| `supply-chain/config.toml` | cargo-vet config | Yes |
| `supply-chain/audits.toml` | Audit attestations | Yes |

**Exit Criteria:** `cargo check --workspace` compiles (with warnings for empty files)

---

## Phase 2: rust-pty Crate (Foundation)

**Goal:** Implement cross-platform PTY abstraction
**Blocking:** rust-expect core
**Parallelizable:** Unix/Windows can be parallel after traits

### 2.1 Core Types (Sequential)

| File | Purpose | Depends On |
|------|---------|------------|
| `crates/rust-pty/src/error.rs` | PtyError type | None |
| `crates/rust-pty/src/config.rs` | PtyConfig, PtySignal | error.rs |
| `crates/rust-pty/src/traits.rs` | PtyMaster, PtyChild, PtySystem | error.rs, config.rs |
| `crates/rust-pty/src/lib.rs` | Public API, re-exports | All above |

### 2.2 Unix Implementation (cfg(unix))

| File | Purpose | Depends On |
|------|---------|------------|
| `crates/rust-pty/src/unix/pty.rs` | PTY allocation via rustix | traits.rs |
| `crates/rust-pty/src/unix/child.rs` | Child process management | pty.rs |
| `crates/rust-pty/src/unix/buffer.rs` | Zero-copy buffer | None |
| `crates/rust-pty/src/unix/signals.rs` | SIGWINCH, SIGCHLD | child.rs |
| `crates/rust-pty/src/unix.rs` | Module root | All unix/* |

### 2.3 Windows Implementation (cfg(windows))

| File | Purpose | Depends On |
|------|---------|------------|
| `crates/rust-pty/src/windows/conpty.rs` | ConPTY management | traits.rs |
| `crates/rust-pty/src/windows/pipes.rs` | Pipe I/O handling | None |
| `crates/rust-pty/src/windows/child.rs` | Process + Job Object | conpty.rs |
| `crates/rust-pty/src/windows/async_adapter.rs` | Async I/O bridge | pipes.rs |
| `crates/rust-pty/src/windows.rs` | Module root | All windows/* |

**Exit Criteria:** `cargo check -p rust-pty` passes on target platform

---

## Phase 3: rust-expect-macros Crate

**Goal:** Implement procedural macros
**Blocking:** rust-expect (macros used at compile time)
**Parallelizable:** Each macro can be parallel after lib.rs skeleton

### 3.1 Macro Implementation

| File | Purpose | Depends On |
|------|---------|------------|
| `crates/rust-expect-macros/src/lib.rs` | Macro exports | None |
| `crates/rust-expect-macros/src/patterns.rs` | patterns! { } macro | lib.rs |
| `crates/rust-expect-macros/src/regex.rs` | regex! validation | lib.rs |
| `crates/rust-expect-macros/src/dialog.rs` | dialog! { } macro | lib.rs |
| `crates/rust-expect-macros/src/timeout.rs` | timeout! macro | lib.rs |

**Exit Criteria:** `cargo check -p rust-expect-macros` passes

---

## Phase 4: rust-expect Core Types

**Goal:** Establish foundational types for main crate
**Blocking:** All rust-expect modules
**Parallelizable:** No

### 4.1 Foundation Layer (Strict Order)

| Order | File | Purpose | Depends On |
|-------|------|---------|------------|
| 1 | `src/error.rs` | ExpectError, ErrorContext | None |
| 2 | `src/types.rs` | Timeout, MatchResult, shared types | error.rs |
| 3 | `src/encoding.rs` | Encoding detection & conversion | error.rs |
| 4 | `src/prelude.rs` | Common imports | error.rs, types.rs |
| 5 | `src/lib.rs` | Public API, feature gates | All above |

**Exit Criteria:** `cargo check -p rust-expect --no-default-features` passes

---

## Phase 5: rust-expect Core Modules

**Goal:** Implement essential functionality (non-feature-gated)
**Blocking:** Feature modules
**Parallelizable:** Partially (after session module)

### 5.1 Session Module (Core)

| File | Purpose | Depends On |
|------|---------|------------|
| `src/session/handle.rs` | Session handle & state | types.rs, error.rs |
| `src/session/builder.rs` | SessionBuilder (fluent API) | handle.rs |
| `src/session/lifecycle.rs` | Spawn, wait, kill, close | handle.rs |
| `src/session/screen.rs` | Screen-enabled wrapper | handle.rs (+ screen feature) |
| `src/session.rs` | Module root | All session/* |

### 5.2 Pattern Matching (Expect)

| File | Purpose | Depends On |
|------|---------|------------|
| `src/expect/pattern.rs` | Pattern enum | types.rs |
| `src/expect/buffer.rs` | Ring buffer | None |
| `src/expect/large_buffer.rs` | MmapBuffer >10MB | buffer.rs |
| `src/expect/cache.rs` | RegexCache LRU | None |
| `src/expect/matcher.rs` | Matching engine | pattern.rs, buffer.rs |
| `src/expect/before_after.rs` | expect_before/after | matcher.rs |
| `src/expect.rs` | Module root | All expect/* |

### 5.3 Send Operations

| File | Purpose | Depends On |
|------|---------|------------|
| `src/send/basic.rs` | send, send_line, send_control | session |
| `src/send/human.rs` | send_slow, send_human | basic.rs |
| `src/send.rs` | Module root | All send/* |

### 5.4 Backend Abstraction

| File | Purpose | Depends On |
|------|---------|------------|
| `src/backend/pty.rs` | PTY backend (wraps rust-pty) | rust-pty crate |
| `src/backend.rs` | Backend trait | pty.rs |

### 5.5 Utilities

| File | Purpose | Depends On |
|------|---------|------------|
| `src/util/timeout.rs` | Timeout wrappers | types.rs |
| `src/util/bytes.rs` | Byte helpers | None |
| `src/util/backpressure.rs` | Backpressure handling | None |

### 5.6 Sync Wrapper

| File | Purpose | Depends On |
|------|---------|------------|
| `src/sync.rs` | Blocking API | session, expect |

**Exit Criteria:** `cargo check -p rust-expect` passes with default features

---

## Phase 6: Feature Modules

**Goal:** Implement feature-gated functionality
**Parallelizable:** Yes (each feature is independent)

### 6.1 SSH Backend (feature = "ssh")

| File | Purpose |
|------|---------|
| `src/backend/ssh/session.rs` | SshSession |
| `src/backend/ssh/builder.rs` | SshSessionBuilder |
| `src/backend/ssh/auth.rs` | Authentication methods |
| `src/backend/ssh/channel.rs` | Channel management |
| `src/backend/ssh/pool.rs` | Connection pooling |
| `src/backend/ssh/retry.rs` | Retry policies |
| `src/backend/ssh/resilient.rs` | Auto-reconnect |
| `src/backend/ssh/keepalive.rs` | Keepalive management |
| `src/backend/ssh.rs` | Module root |

### 6.2 Mock Backend (feature = "mock")

| File | Purpose |
|------|---------|
| `src/mock/session.rs` | MockSession type |
| `src/mock/event.rs` | MockEvent enum |
| `src/mock/scenario.rs` | NDJSON scenario loading |
| `src/mock/builtin.rs` | Built-in scenarios |
| `src/mock.rs` | Module root |

### 6.3 Screen Buffer (feature = "screen")

| File | Purpose |
|------|---------|
| `src/screen/parser.rs` | ANSI parsing (vte) |
| `src/screen/buffer.rs` | Virtual screen buffer |
| `src/screen/query.rs` | Screen queries |
| `src/screen.rs` | Module root |

### 6.4 PII Redaction (feature = "pii-redaction")

| File | Purpose |
|------|---------|
| `src/pii/detector.rs` | PiiDetector trait |
| `src/pii/credit_card.rs` | Luhn validation |
| `src/pii/ssn.rs` | SSN patterns |
| `src/pii/api_key.rs` | API key patterns |
| `src/pii/redactor.rs` | Redaction engine |
| `src/pii.rs` | Module root |

### 6.5 Metrics (feature = "metrics")

| File | Purpose |
|------|---------|
| `src/metrics.rs` | Prometheus/OpenTelemetry |
| `src/health.rs` | Health checks |

### 6.6 Interactive Mode

| File | Purpose |
|------|---------|
| `src/interact/mode.rs` | Interact loop |
| `src/interact/hooks.rs` | Pattern/input hooks |
| `src/interact/terminal.rs` | Raw mode (crossterm) |
| `src/interact.rs` | Module root |

### 6.7 Multi-Session

| File | Purpose |
|------|---------|
| `src/multi/group.rs` | SessionGroup |
| `src/multi/select.rs` | select_expect! |
| `src/multi.rs` | Module root |

### 6.8 Dialog System

| File | Purpose |
|------|---------|
| `src/dialog/definition.rs` | Dialog DSL |
| `src/dialog/executor.rs` | Execution engine |
| `src/dialog/common.rs` | Built-in dialogs |
| `src/dialog.rs` | Module root |

### 6.9 Transcript

| File | Purpose |
|------|---------|
| `src/transcript/format.rs` | NDJSON types |
| `src/transcript/asciicast.rs` | Asciicast v2 |
| `src/transcript/recorder.rs` | Recording |
| `src/transcript/player.rs` | Playback |
| `src/transcript.rs` | Module root |

### 6.10 Auto-Config (Zero-Config Mode)

| File | Purpose |
|------|---------|
| `src/auto_config/shell.rs` | Shell detection |
| `src/auto_config/line_ending.rs` | Line ending detection |
| `src/auto_config/prompt.rs` | Prompt detection |
| `src/auto_config/locale.rs` | Locale detection |
| `src/auto_config.rs` | Module root |

### 6.11 Configuration

| File | Purpose |
|------|---------|
| `src/config/file.rs` | TOML parsing |
| `src/config/env.rs` | Environment overrides |
| `src/config.rs` | Module root |

**Exit Criteria:** `cargo check -p rust-expect --all-features` passes

---

## Phase 7: Test Utilities

**Goal:** Build test fixture binaries
**Parallelizable:** Yes (independent binaries)

| Binary | Purpose |
|--------|---------|
| `test-utils/test-echo/src/main.rs` | Simple echo |
| `test-utils/test-prompt/src/main.rs` | Configurable prompt |
| `test-utils/test-output/src/main.rs` | Large output generation |
| `test-utils/test-signals/src/main.rs` | Signal handling |
| `test-utils/test-timing/src/main.rs` | Timing tests |
| `test-utils/test-hang/src/main.rs` | Timeout simulation |

**Exit Criteria:** All test-utils binaries compile

---

## Phase 8: Tests, Examples, Benchmarks

**Goal:** Verification layer
**Parallelizable:** Yes

### 8.1 Integration Tests

| File | Tests |
|------|-------|
| `tests/common.rs` | Shared utilities |
| `tests/common/fixtures.rs` | Fixture helpers |
| `tests/common/assertions.rs` | Custom assertions |
| `tests/spawn_tests.rs` | Session spawning |
| `tests/expect_tests.rs` | Pattern matching |
| `tests/send_tests.rs` | Send operations |
| `tests/interact_tests.rs` | Interactive mode |
| `tests/multi_session_tests.rs` | Multi-session |
| `tests/dialog_tests.rs` | Dialog system |
| `tests/sync_tests.rs` | Sync API |
| `tests/ssh_tests.rs` | SSH (feature) |
| `tests/mock_tests.rs` | Mock (feature) |
| `tests/screen_tests.rs` | Screen (feature) |
| `tests/pii_tests.rs` | PII (feature) |
| `tests/encoding_tests.rs` | Encoding |
| `tests/transcript_tests.rs` | Transcript |
| `tests/auto_config_tests.rs` | Auto-config |
| `tests/config_tests.rs` | Configuration |
| `tests/health_tests.rs` | Health checks |
| `tests/platform/unix.rs` | Unix-specific |
| `tests/platform/windows.rs` | Windows-specific |

### 8.2 Examples

| File | Demonstrates |
|------|--------------|
| `examples/basic.rs` | Simple spawn/expect |
| `examples/ssh.rs` | SSH sessions |
| `examples/interactive.rs` | Interactive mode |
| `examples/multi_session.rs` | Multiple sessions |
| `examples/large_output.rs` | Large output |
| `examples/dialog.rs` | Dialog system |
| `examples/transcript.rs` | Recording/playback |
| `examples/zero_config.rs` | Auto-config |
| `examples/mock_testing.rs` | Mock sessions |
| `examples/screen_buffer.rs` | Screen buffer |
| `examples/pii_redaction.rs` | PII redaction |
| `examples/metrics.rs` | Observability |
| `examples/sync_api.rs` | Sync wrapper |

### 8.3 Benchmarks

| File | Measures |
|------|----------|
| `benches/main.rs` | Harness |
| `benches/spawn.rs` | Spawn latency |
| `benches/pattern.rs` | Pattern matching |
| `benches/buffer.rs` | Ring buffer |
| `benches/regex_cache.rs` | Cache hit/miss |
| `benches/throughput.rs` | Data throughput |

**Exit Criteria:** `cargo test --workspace` and `cargo bench --workspace` pass

---

## Phase 9: CI/CD & Documentation

**Goal:** Automation and community files
**Parallelizable:** Yes (all independent)

### 9.1 GitHub Workflows

| File | Purpose |
|------|---------|
| `.github/workflows/ci.yml` | Main CI pipeline |
| `.github/workflows/bench.yml` | Benchmarks |
| `.github/workflows/release.yml` | Release automation |
| `.github/workflows/security.yml` | Security scans |

### 9.2 GitHub Community Files

| File | Purpose |
|------|---------|
| `.github/CODEOWNERS` | Code ownership |
| `.github/FUNDING.yml` | Sponsorship |
| `.github/dependabot.yml` | Dependency updates |
| `.github/PULL_REQUEST_TEMPLATE.md` | PR template |
| `.github/ISSUE_TEMPLATE/config.yml` | Template chooser |
| `.github/ISSUE_TEMPLATE/bug_report.md` | Bug template |
| `.github/ISSUE_TEMPLATE/feature_request.md` | Feature template |
| `.github/ISSUE_TEMPLATE/security_vulnerability.md` | Security template |

### 9.3 Documentation

| File | Purpose |
|------|---------|
| `README.md` | Project overview |
| `CONTRIBUTING.md` | Contribution guide |
| `SECURITY.md` | Security policy |
| `CODE_OF_CONDUCT.md` | Contributor Covenant |
| `CHANGELOG.md` | Version history |
| `crates/rust-pty/README.md` | Crate docs |
| `crates/rust-expect/README.md` | Crate docs |
| `crates/rust-expect-macros/README.md` | Crate docs |

### 9.4 Licenses

| File | License |
|------|---------|
| `LICENSE-MIT` | MIT License |
| `LICENSE-APACHE` | Apache 2.0 |

**Exit Criteria:** All CI workflows pass, documentation complete

---

## Phase 10: Fixtures

**Goal:** Test data files
**Parallelizable:** Yes

| File | Purpose |
|------|---------|
| `fixtures/transcripts/ssh_login.ndjson` | SSH test transcript |
| `fixtures/transcripts/sudo_prompt.ndjson` | Sudo test transcript |
| `fixtures/transcripts/shell_session.ndjson` | Shell test transcript |
| `fixtures/keys/test_ed25519` | Test private key |
| `fixtures/keys/test_ed25519.pub` | Test public key |
| `fixtures/configs/default.toml` | Default config |
| `fixtures/configs/custom.toml` | Custom config |

**Exit Criteria:** Fixtures available for tests

---

## Parallel Workstreams

These workstreams can proceed independently:

```
Workstream A (Rust Implementation):
  Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7 → Phase 8

Workstream B (Documentation):
  Can start immediately, no Rust dependencies:
  - README.md
  - CONTRIBUTING.md
  - SECURITY.md
  - CODE_OF_CONDUCT.md
  - CHANGELOG.md
  - Crate READMEs

Workstream C (CI/CD):
  Can start after Phase 1 (Cargo.toml files exist):
  - .github/workflows/*.yml
  - .github/ISSUE_TEMPLATE/*
  - .github/*.md

Workstream D (Fixtures):
  Can start immediately:
  - fixtures/transcripts/*.ndjson
  - fixtures/configs/*.toml
  - fixtures/keys/*

Workstream E (Licenses):
  Can start immediately:
  - LICENSE-MIT
  - LICENSE-APACHE
```

---

## Implementation Checklist

### Phase 1: Cargo Setup (11 files)
- [ ] `rust-toolchain.toml`
- [ ] `Cargo.toml` (workspace)
- [ ] `.cargo/config.toml`
- [ ] `crates/rust-pty/Cargo.toml`
- [ ] `crates/rust-expect-macros/Cargo.toml`
- [ ] `crates/rust-expect/Cargo.toml`
- [ ] `test-utils/Cargo.toml`
- [ ] `test-utils/test-echo/Cargo.toml`
- [ ] `test-utils/test-prompt/Cargo.toml`
- [ ] `test-utils/test-output/Cargo.toml`
- [ ] `test-utils/test-signals/Cargo.toml`
- [ ] `test-utils/test-timing/Cargo.toml`
- [ ] `test-utils/test-hang/Cargo.toml`

### Phase 2: rust-pty (14 files)
- [ ] `crates/rust-pty/src/error.rs`
- [ ] `crates/rust-pty/src/config.rs`
- [ ] `crates/rust-pty/src/traits.rs`
- [ ] `crates/rust-pty/src/lib.rs`
- [ ] `crates/rust-pty/src/unix/pty.rs`
- [ ] `crates/rust-pty/src/unix/child.rs`
- [ ] `crates/rust-pty/src/unix/buffer.rs`
- [ ] `crates/rust-pty/src/unix/signals.rs`
- [ ] `crates/rust-pty/src/unix.rs`
- [ ] `crates/rust-pty/src/windows/conpty.rs`
- [ ] `crates/rust-pty/src/windows/pipes.rs`
- [ ] `crates/rust-pty/src/windows/child.rs`
- [ ] `crates/rust-pty/src/windows/async_adapter.rs`
- [ ] `crates/rust-pty/src/windows.rs`

### Phase 3: rust-expect-macros (5 files)
- [ ] `crates/rust-expect-macros/src/lib.rs`
- [ ] `crates/rust-expect-macros/src/patterns.rs`
- [ ] `crates/rust-expect-macros/src/regex.rs`
- [ ] `crates/rust-expect-macros/src/dialog.rs`
- [ ] `crates/rust-expect-macros/src/timeout.rs`

### Phase 4: rust-expect Core Types (5 files)
- [ ] `crates/rust-expect/src/error.rs`
- [ ] `crates/rust-expect/src/types.rs`
- [ ] `crates/rust-expect/src/encoding.rs`
- [ ] `crates/rust-expect/src/prelude.rs`
- [ ] `crates/rust-expect/src/lib.rs`

### Phase 5: rust-expect Core Modules (17 files)
- [ ] `crates/rust-expect/src/session/handle.rs`
- [ ] `crates/rust-expect/src/session/builder.rs`
- [ ] `crates/rust-expect/src/session/lifecycle.rs`
- [ ] `crates/rust-expect/src/session/screen.rs`
- [ ] `crates/rust-expect/src/session.rs`
- [ ] `crates/rust-expect/src/expect/pattern.rs`
- [ ] `crates/rust-expect/src/expect/buffer.rs`
- [ ] `crates/rust-expect/src/expect/large_buffer.rs`
- [ ] `crates/rust-expect/src/expect/cache.rs`
- [ ] `crates/rust-expect/src/expect/matcher.rs`
- [ ] `crates/rust-expect/src/expect/before_after.rs`
- [ ] `crates/rust-expect/src/expect.rs`
- [ ] `crates/rust-expect/src/send/basic.rs`
- [ ] `crates/rust-expect/src/send/human.rs`
- [ ] `crates/rust-expect/src/send.rs`
- [ ] `crates/rust-expect/src/backend/pty.rs`
- [ ] `crates/rust-expect/src/backend.rs`
- [ ] `crates/rust-expect/src/util/timeout.rs`
- [ ] `crates/rust-expect/src/util/bytes.rs`
- [ ] `crates/rust-expect/src/util/backpressure.rs`
- [ ] `crates/rust-expect/src/sync.rs`

### Phase 6: Feature Modules (48 files)
#### SSH (9 files)
- [ ] `crates/rust-expect/src/backend/ssh/session.rs`
- [ ] `crates/rust-expect/src/backend/ssh/builder.rs`
- [ ] `crates/rust-expect/src/backend/ssh/auth.rs`
- [ ] `crates/rust-expect/src/backend/ssh/channel.rs`
- [ ] `crates/rust-expect/src/backend/ssh/pool.rs`
- [ ] `crates/rust-expect/src/backend/ssh/retry.rs`
- [ ] `crates/rust-expect/src/backend/ssh/resilient.rs`
- [ ] `crates/rust-expect/src/backend/ssh/keepalive.rs`
- [ ] `crates/rust-expect/src/backend/ssh.rs`

#### Mock (5 files)
- [ ] `crates/rust-expect/src/mock/session.rs`
- [ ] `crates/rust-expect/src/mock/event.rs`
- [ ] `crates/rust-expect/src/mock/scenario.rs`
- [ ] `crates/rust-expect/src/mock/builtin.rs`
- [ ] `crates/rust-expect/src/mock.rs`

#### Screen (4 files)
- [ ] `crates/rust-expect/src/screen/parser.rs`
- [ ] `crates/rust-expect/src/screen/buffer.rs`
- [ ] `crates/rust-expect/src/screen/query.rs`
- [ ] `crates/rust-expect/src/screen.rs`

#### PII (6 files)
- [ ] `crates/rust-expect/src/pii/detector.rs`
- [ ] `crates/rust-expect/src/pii/credit_card.rs`
- [ ] `crates/rust-expect/src/pii/ssn.rs`
- [ ] `crates/rust-expect/src/pii/api_key.rs`
- [ ] `crates/rust-expect/src/pii/redactor.rs`
- [ ] `crates/rust-expect/src/pii.rs`

#### Metrics (2 files)
- [ ] `crates/rust-expect/src/metrics.rs`
- [ ] `crates/rust-expect/src/health.rs`

#### Interact (4 files)
- [ ] `crates/rust-expect/src/interact/mode.rs`
- [ ] `crates/rust-expect/src/interact/hooks.rs`
- [ ] `crates/rust-expect/src/interact/terminal.rs`
- [ ] `crates/rust-expect/src/interact.rs`

#### Multi (3 files)
- [ ] `crates/rust-expect/src/multi/group.rs`
- [ ] `crates/rust-expect/src/multi/select.rs`
- [ ] `crates/rust-expect/src/multi.rs`

#### Dialog (4 files)
- [ ] `crates/rust-expect/src/dialog/definition.rs`
- [ ] `crates/rust-expect/src/dialog/executor.rs`
- [ ] `crates/rust-expect/src/dialog/common.rs`
- [ ] `crates/rust-expect/src/dialog.rs`

#### Transcript (5 files)
- [ ] `crates/rust-expect/src/transcript/format.rs`
- [ ] `crates/rust-expect/src/transcript/asciicast.rs`
- [ ] `crates/rust-expect/src/transcript/recorder.rs`
- [ ] `crates/rust-expect/src/transcript/player.rs`
- [ ] `crates/rust-expect/src/transcript.rs`

#### Auto-Config (5 files)
- [ ] `crates/rust-expect/src/auto_config/shell.rs`
- [ ] `crates/rust-expect/src/auto_config/line_ending.rs`
- [ ] `crates/rust-expect/src/auto_config/prompt.rs`
- [ ] `crates/rust-expect/src/auto_config/locale.rs`
- [ ] `crates/rust-expect/src/auto_config.rs`

#### Config (3 files)
- [ ] `crates/rust-expect/src/config/file.rs`
- [ ] `crates/rust-expect/src/config/env.rs`
- [ ] `crates/rust-expect/src/config.rs`

### Phase 7: Test Utilities (6 files)
- [ ] `test-utils/test-echo/src/main.rs`
- [ ] `test-utils/test-prompt/src/main.rs`
- [ ] `test-utils/test-output/src/main.rs`
- [ ] `test-utils/test-signals/src/main.rs`
- [ ] `test-utils/test-timing/src/main.rs`
- [ ] `test-utils/test-hang/src/main.rs`

### Phase 8: Tests & Examples (40 files)
#### Tests (21 files)
- [ ] `tests/common.rs`
- [ ] `tests/common/fixtures.rs`
- [ ] `tests/common/assertions.rs`
- [ ] `tests/spawn_tests.rs`
- [ ] `tests/expect_tests.rs`
- [ ] `tests/send_tests.rs`
- [ ] `tests/interact_tests.rs`
- [ ] `tests/multi_session_tests.rs`
- [ ] `tests/dialog_tests.rs`
- [ ] `tests/sync_tests.rs`
- [ ] `tests/ssh_tests.rs`
- [ ] `tests/mock_tests.rs`
- [ ] `tests/screen_tests.rs`
- [ ] `tests/pii_tests.rs`
- [ ] `tests/encoding_tests.rs`
- [ ] `tests/transcript_tests.rs`
- [ ] `tests/auto_config_tests.rs`
- [ ] `tests/config_tests.rs`
- [ ] `tests/health_tests.rs`
- [ ] `tests/platform/unix.rs`
- [ ] `tests/platform/windows.rs`

#### Examples (13 files)
- [ ] `examples/basic.rs`
- [ ] `examples/ssh.rs`
- [ ] `examples/interactive.rs`
- [ ] `examples/multi_session.rs`
- [ ] `examples/large_output.rs`
- [ ] `examples/dialog.rs`
- [ ] `examples/transcript.rs`
- [ ] `examples/zero_config.rs`
- [ ] `examples/mock_testing.rs`
- [ ] `examples/screen_buffer.rs`
- [ ] `examples/pii_redaction.rs`
- [ ] `examples/metrics.rs`
- [ ] `examples/sync_api.rs`

#### Benchmarks (6 files)
- [ ] `benches/main.rs`
- [ ] `benches/spawn.rs`
- [ ] `benches/pattern.rs`
- [ ] `benches/buffer.rs`
- [ ] `benches/regex_cache.rs`
- [ ] `benches/throughput.rs`

### Phase 9: CI/CD & Docs (21 files)
#### Workflows (4 files)
- [ ] `.github/workflows/ci.yml`
- [ ] `.github/workflows/bench.yml`
- [ ] `.github/workflows/release.yml`
- [ ] `.github/workflows/security.yml`

#### GitHub Files (9 files)
- [ ] `.github/CODEOWNERS`
- [ ] `.github/FUNDING.yml`
- [ ] `.github/dependabot.yml`
- [ ] `.github/PULL_REQUEST_TEMPLATE.md`
- [ ] `.github/ISSUE_TEMPLATE/config.yml`
- [ ] `.github/ISSUE_TEMPLATE/bug_report.md`
- [ ] `.github/ISSUE_TEMPLATE/feature_request.md`
- [ ] `.github/ISSUE_TEMPLATE/security_vulnerability.md`

#### Documentation (8 files)
- [ ] `README.md`
- [ ] `CONTRIBUTING.md`
- [ ] `SECURITY.md`
- [ ] `CODE_OF_CONDUCT.md`
- [ ] `CHANGELOG.md`
- [ ] `crates/rust-pty/README.md`
- [ ] `crates/rust-expect/README.md`
- [ ] `crates/rust-expect-macros/README.md`

### Phase 10: Fixtures & Licenses (10 files)
- [ ] `fixtures/transcripts/ssh_login.ndjson`
- [ ] `fixtures/transcripts/sudo_prompt.ndjson`
- [ ] `fixtures/transcripts/shell_session.ndjson`
- [ ] `fixtures/keys/test_ed25519`
- [ ] `fixtures/keys/test_ed25519.pub`
- [ ] `fixtures/configs/default.toml`
- [ ] `fixtures/configs/custom.toml`
- [ ] `LICENSE-MIT`
- [ ] `LICENSE-APACHE`

### Tooling Config (6 files)
- [ ] `rustfmt.toml`
- [ ] `clippy.toml`
- [ ] `deny.toml`
- [ ] `cliff.toml`
- [ ] `supply-chain/config.toml`
- [ ] `supply-chain/audits.toml`

---

## Success Metrics

| Milestone | Verification Command | Expected Result |
|-----------|---------------------|-----------------|
| Phase 1 Complete | `cargo check --workspace` | Compiles (empty modules ok) |
| Phase 2 Complete | `cargo check -p rust-pty` | No errors |
| Phase 3 Complete | `cargo check -p rust-expect-macros` | No errors |
| Phase 4 Complete | `cargo check -p rust-expect --no-default-features` | No errors |
| Phase 5 Complete | `cargo check -p rust-expect` | No errors |
| Phase 6 Complete | `cargo check -p rust-expect --all-features` | No errors |
| Phase 7 Complete | `cargo build -p test-echo` (etc.) | Binaries build |
| Phase 8 Complete | `cargo test --workspace` | Tests pass |
| Full Implementation | `cargo clippy --workspace -- -D warnings` | No warnings |

---

## Notes

1. **Cargo.lock**: Will be auto-generated after Phase 1; do not manually create
2. **Feature flags**: Ensure proper `#[cfg(feature = "...")]` guards
3. **Platform code**: Use `#[cfg(unix)]` and `#[cfg(windows)]` appropriately
4. **Documentation**: Run `cargo doc --workspace --all-features` to verify
5. **MSRV**: Test with `cargo +1.85 check` to verify Edition 2024 compatibility
