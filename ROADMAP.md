# rust-expect Roadmap

**Target Version:** 0.1.0 (initial crates.io release) → 1.0.0
**Last Updated:** 2026-05-20
**Status:** Feature-complete; release-prep polish landed; cutting v0.1.0

---

## Overview

This roadmap outlines the development priorities for rust-expect, organized by milestone.
Each milestone builds on the previous, with clear exit criteria and deliverables.

### Current Status

The project has completed implementation of all planned features (Milestones 1-4) and a
round of release-hygiene polish (May 2026). Key achievements:

- Cross-platform PTY support (Unix via rustix, Windows via ConPTY)
- Async-first Session API with pattern matching
- Multi-session management with `expect_any()` and `expect_all()`
- Interactive mode with pattern hooks
- Screen buffer with full VT100 emulation and visual diff (feature-gated)
- Mock backend for testing (feature-gated)
- Complete SSH backend with connection pooling, resilient sessions, and retry policies
- PII detection and redaction with custom patterns
- Prometheus and OpenTelemetry metrics export
- Zero-copy I/O and mmap-backed large buffers
- TUI-driving primitives: output taps, screen-aware expects, bracketed paste, back-tab
- Comprehensive CI with cross-platform testing (Linux/macOS/Windows × stable/beta/MSRV 1.88)
- 864 passing tests, clippy clean (`all + pedantic + nursery`), all doctests compile-checked

### Benchmark Baselines (Captured 2025-01-02)

| Benchmark | Result | Notes |
|-----------|--------|-------|
| literal_pattern_match | ~46 ns | memchr-based, fast |
| regex_pattern_match | ~522 ns | Compiled regex |
| pattern_set (4 patterns) | ~46-410 ns | Depends on match position |
| ring_buffer append (1K) | ~2.4 µs | Sequential writes |
| ring_buffer search | ~13.4 µs | Needle in 4KB buffer |
| screen_buffer find | ~5.3 µs | VTE-based |
| ANSI parser | ~47-200 ns/byte | Streaming parse |

---

## Milestone 1: v0.2.0 - API Completeness ✅ COMPLETE

**Goal:** Complete the public API with missing convenience methods and async execution.

### High Priority

| Task | Status | Description |
|------|--------|-------------|
| Dialog async execution | 🟢 Done | Added `Session::run_dialog()` and async `DialogExecutor::execute()` |
| `expect_eof()` convenience | 🟢 Done | Added `Session::expect_eof()` and `expect_eof_timeout()` methods |
| Error context enrichment | 🟢 Done | Errors include buffer snippets, line counts, and actionable tips |

### Medium Priority

| Task | Status | Description |
|------|--------|-------------|
| API documentation review | 🟢 Done | Added examples to Pattern, Dialog, Session modules |
| Windows parity validation | 🟡 Pending | CI configured; awaiting validation run |
| QuickSession improvements | 🟢 Done | Added comprehensive spawn helpers |

### Exit Criteria

- [x] All core types have rustdoc examples
- [x] Dialog execution works async
- [x] Error messages include actionable buffer context
- [x] Windows CI passes all tests

---

## Milestone 2: v0.3.0 - SSH Backend ✅ COMPLETE

**Goal:** Production-ready SSH support for remote automation.

### High Priority

| Task | Status | Description |
|------|--------|-------------|
| SSH session builder | 🟢 Done | Complete `SshSessionBuilder` with all auth methods |
| Connection pooling | 🟢 Done | `ConnectionPool` with configurable limits |
| Keepalive management | 🟢 Done | `KeepaliveManager` with automatic ping handling |
| Host key verification | 🟢 Done | `HostKeyVerification` with known_hosts support |

### Medium Priority

| Task | Status | Description |
|------|--------|-------------|
| Retry policies | 🟢 Done | `RetryPolicy` with configurable backoff strategies |
| Resilient sessions | 🟢 Done | `ResilientSession` with auto-reconnect |
| SSH agent support | 🟢 Done | `SSH_AUTH_SOCK` integration via `AgentAuth` |
| Encrypted key support | 🟢 Done | Password-protected private key handling |
| Keyboard-interactive auth | 🟢 Done | PAM and 2FA support |

### Exit Criteria

- [x] Can automate remote servers via SSH
- [x] Connection pooling reduces overhead for multiple sessions
- [x] Graceful handling of network interruptions
- [x] Full documentation with examples

---

## Milestone 3: v0.4.0 - Advanced Features ✅ COMPLETE

**Goal:** Complete feature-gated modules for specialized use cases.

### Screen Buffer (`feature = "screen"`)

| Task | Status | Description |
|------|--------|-------------|
| Full VT100 emulation | 🟢 Done | Complete cursor movement, scrolling, attributes |
| Screen queries | 🟢 Done | `screen.find_text()`, `screen.get_region()`, `screen.query()` |
| Visual diff | 🟢 Done | Compare screen states with `ScreenDiff` |

### PII Redaction (`feature = "pii-redaction"`)

| Task | Status | Description |
|------|--------|-------------|
| Credit card detection | 🟢 Done | Luhn validation with issuer identification |
| SSN detection | 🟢 Done | Pattern matching with format normalization |
| API key detection | 🟢 Done | Common patterns (AWS, GitHub, Stripe, etc.) |
| Custom patterns | 🟢 Done | `PatternRegistry` for user-defined PII rules |
| Email detection | 🟢 Done | RFC-compliant email pattern matching |

### Transcript Recording

| Task | Status | Description |
|------|--------|-------------|
| NDJSON recording | 🟢 Done | Event-based recording with metadata |
| Asciicast v2 export | 🟢 Done | Full compatibility with asciinema |
| Playback | 🟢 Done | Replay recorded sessions with speed control |

### Exit Criteria

- [x] Screen buffer handles complex TUI applications
- [x] PII redaction is configurable and extensible
- [x] Transcripts can be shared via asciinema

---

## Milestone 4: v0.5.0 - Performance & Observability ✅ COMPLETE

**Goal:** Production hardening with metrics and optimization.

### Performance

| Task | Status | Description |
|------|--------|-------------|
| Large buffer optimization | 🟢 Done | Mmap-backed buffers for >10MB via `AdaptiveBuffer` |
| Regex cache tuning | 🟢 Done | LRU cache with configurable limits |
| Zero-copy I/O | 🟢 Done | `ZeroCopyReader`/`ZeroCopyWriter` utilities |

### Observability (`feature = "metrics"`)

| Task | Status | Description |
|------|--------|-------------|
| Prometheus metrics | 🟢 Done | Full metrics export via `prometheus_export` module |
| OpenTelemetry spans | 🟢 Done | Trace session operations via `otel` module |
| Health checks | 🟢 Done | Session health monitoring with status reporting |
| Core metrics | 🟢 Done | Counter, Gauge, Histogram, Timer implementations |

### Benchmarks

| Task | Status | Description |
|------|--------|-------------|
| Pattern matching suite | 🟢 Done | `benches/pattern_matching.rs` |
| Screen buffer benchmarks | 🟢 Done | `benches/screen_buffer.rs` |
| Comparative benchmarks | 🟢 Done | `benches/comparative.rs` vs expectrl |

### Exit Criteria

- [x] Large buffer handling optimized
- [x] Metrics exportable to Prometheus/OTLP
- [x] Benchmark suite established

---

## Milestone 5: v1.0.0 - Production Release 🟡 IN PROGRESS

**Goal:** Stable 1.0 release with API stability guarantees.
**Target:** Q1 2025

### Stability

| Task | Status | Description |
|------|--------|-------------|
| API review | 🟢 Done | API audit completed |
| MSRV policy | 🟢 Done | Rust 1.88+ (Edition 2024) |
| Semver compliance | 🟢 Done | Breaking change audit complete |

### Documentation

| Task | Status | Description |
|------|--------|-------------|
| User guide | 🟢 Done | Comprehensive getting started guide (see docs/GUIDE.md) |
| Migration guide | 🟢 Done | From pexpect/expectrl (see MIGRATION.md) |
| API reference | 🟢 Done | Generated rustdoc with examples |

### Ecosystem

| Task | Status | Description |
|------|--------|-------------|
| crates.io publish | 🟡 Pending | Ready for release |
| CHANGELOG | 🟢 Done | Maintained changelog |
| Security policy | 🟢 Done | SECURITY.md with disclosure policy |

### Exit Criteria

- [x] All public APIs documented with examples
- [x] Migration guide from pexpect/expectrl
- [ ] Published to crates.io
- [x] No known critical bugs

---

## Competitive Position

### Current Advantages

1. **Async-first design** - Native Tokio integration, no blocking
2. **Type safety** - Strong Rust types, no stringly-typed APIs
3. **Cross-platform** - Single API for Unix and Windows
4. **Modern tooling** - Edition 2024, workspace structure
5. **Complete SSH backend** - Connection pooling, resilient sessions, retry policies
6. **PII protection** - Built-in sensitive data redaction
7. **Observability** - Prometheus and OpenTelemetry integration

### Differentiation from expectrl

| Feature | rust-expect | expectrl |
|---------|-------------|----------|
| Async-first | ✅ Native | ⚠️ Added later |
| Windows ConPTY | ✅ Full support | ⚠️ Limited |
| SSH resilience | ✅ Auto-reconnect | ❌ Manual |
| PII redaction | ✅ Built-in | ❌ Not available |
| Screen emulation | ✅ VT100 + visual diff | ⚠️ Basic |
| Metrics | ✅ Prometheus/OTLP | ❌ Not available |

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development workflow.

### Priority for Contributors

1. **High impact, low effort**: Documentation improvements, example code
2. **High impact, medium effort**: Migration guides, tutorials
3. **Medium impact, low effort**: Test coverage expansion

### Getting Started

```bash
# Clone and build
git clone https://github.com/praxiomlabs/rust-expect
cd rust-expect
cargo build --workspace --all-features

# Run tests
cargo test --workspace --all-features

# Run benchmarks
cargo bench -p rust-expect --features full
```

---

## Version History

| Version | Date | Highlights |
|---------|------|------------|
| 0.1.0 | 2025-01-03 | Initial implementation - feature complete |
