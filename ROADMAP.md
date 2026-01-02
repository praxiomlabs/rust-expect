# rust-expect Roadmap

**Version:** 1.0.0
**Last Updated:** 2026-01-02
**Status:** Active Development

---

## Overview

This roadmap outlines the development priorities for rust-expect, organized by milestone.
Each milestone builds on the previous, with clear exit criteria and deliverables.

### Current Status

The project has completed initial implementation of core modules (Phases 1-6 of the
Implementation Plan). Key achievements:

- Cross-platform PTY support (Unix via rustix, Windows via ConPTY)
- Async-first Session API with pattern matching
- Multi-session management with `expect_any()` and `expect_all()`
- Interactive mode with pattern hooks
- Screen buffer with ANSI parsing (feature-gated)
- Mock backend for testing (feature-gated)
- Comprehensive CI with cross-platform testing

### Benchmark Baselines (Captured 2026-01-02)

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

## Milestone 1: v0.2.0 - API Completeness

**Goal:** Complete the public API with missing convenience methods and async execution.
**Target:** Q1 2026

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
| Windows parity validation | 🟡 Partial | CI configured; needs validation run; `tests/platform/windows.rs` empty |
| QuickSession improvements | 🔴 Pending | Add more spawn helpers |

### Exit Criteria

- [x] All core types have rustdoc examples
- [x] Dialog execution works async
- [x] Error messages include actionable buffer context
- [ ] Windows CI passes all tests (needs validation)

---

## Milestone 2: v0.3.0 - SSH Backend

**Goal:** Production-ready SSH support for remote automation.
**Target:** Q2 2026

### High Priority

| Task | Status | Description |
|------|--------|-------------|
| SSH session builder | 🟡 Partial | Complete `SshSessionBuilder` with all auth methods |
| Connection pooling | 🔴 Pending | Implement connection reuse |
| Keepalive management | 🔴 Pending | Automatic keepalive handling |
| Host key verification | 🔴 Pending | Proper known_hosts support |

### Medium Priority

| Task | Status | Description |
|------|--------|-------------|
| Retry policies | 🔴 Pending | Configurable retry with backoff |
| Resilient sessions | 🔴 Pending | Auto-reconnect on disconnect |
| SSH agent support | 🔴 Pending | SSH_AUTH_SOCK integration |

### Exit Criteria

- [ ] Can automate remote servers via SSH
- [ ] Connection pooling reduces overhead for multiple sessions
- [ ] Graceful handling of network interruptions
- [ ] Full documentation with examples

---

## Milestone 3: v0.4.0 - Advanced Features

**Goal:** Complete feature-gated modules for specialized use cases.
**Target:** Q3 2026

### Screen Buffer (`feature = "screen"`)

| Task | Status | Description |
|------|--------|-------------|
| Full VT100 emulation | 🟡 Partial | Complete cursor movement, scrolling |
| Screen queries | 🟡 Partial | Add `screen.find_text()`, `screen.get_region()` |
| Visual diff | 🔴 Pending | Compare screen states |

### PII Redaction (`feature = "pii-redaction"`)

| Task | Status | Description |
|------|--------|-------------|
| Credit card detection | 🟢 Done | Luhn validation |
| SSN detection | 🟢 Done | Pattern matching |
| API key detection | 🟡 Partial | Common patterns |
| Custom patterns | 🔴 Pending | User-defined PII rules |

### Transcript Recording

| Task | Status | Description |
|------|--------|-------------|
| NDJSON recording | 🟢 Done | Event-based recording |
| Asciicast v2 export | 🟡 Partial | Compatibility mode |
| Playback | 🟢 Done | Replay recorded sessions |

### Exit Criteria

- [ ] Screen buffer handles complex TUI applications
- [ ] PII redaction is configurable and extensible
- [ ] Transcripts can be shared via asciinema

---

## Milestone 4: v0.5.0 - Performance & Observability

**Goal:** Production hardening with metrics and optimization.
**Target:** Q4 2026

### Performance

| Task | Status | Description |
|------|--------|-------------|
| Large buffer optimization | 🔴 Pending | Mmap-backed buffers for >10MB |
| Regex cache tuning | 🟡 Partial | LRU cache sizing |
| Zero-copy I/O | 🔴 Pending | Reduce allocations in hot path |

### Observability (`feature = "metrics"`)

| Task | Status | Description |
|------|--------|-------------|
| Prometheus metrics | 🟡 Partial | Basic counters |
| OpenTelemetry spans | 🔴 Pending | Trace session operations |
| Health checks | 🟢 Done | Session health monitoring |

### Benchmarks

| Task | Status | Description |
|------|--------|-------------|
| Spawn latency suite | 🔴 Pending | Target <5ms spawn time |
| Throughput benchmarks | 🔴 Pending | Target 100 MB/s streaming |
| Concurrent session tests | 🔴 Pending | 1000+ simultaneous sessions |

### Exit Criteria

- [ ] Spawn latency <5ms on Unix, <50ms on Windows
- [ ] Streaming throughput >100 MB/s
- [ ] Metrics exportable to Prometheus/OTLP

---

## Milestone 5: v1.0.0 - Production Release

**Goal:** Stable 1.0 release with API stability guarantees.
**Target:** Q1 2027

### Stability

| Task | Status | Description |
|------|--------|-------------|
| API review | 🔴 Pending | Final API audit for stability |
| MSRV policy | 🟢 Done | Rust 1.85+ (Edition 2024) |
| Semver compliance | 🔴 Pending | Breaking change audit |

### Documentation

| Task | Status | Description |
|------|--------|-------------|
| User guide | 🔴 Pending | Comprehensive getting started |
| Migration guide | 🔴 Pending | From pexpect/expectrl |
| API reference | 🟡 Partial | Generated docs |

### Ecosystem

| Task | Status | Description |
|------|--------|-------------|
| crates.io publish | 🔴 Pending | Initial release |
| CHANGELOG | 🔴 Pending | git-cliff integration |
| Security policy | 🔴 Pending | Responsible disclosure |

### Exit Criteria

- [ ] All public APIs documented with examples
- [ ] Full test coverage on Linux, macOS, Windows
- [ ] Published to crates.io
- [ ] No known critical bugs

---

## Competitive Position

### Current Advantages

1. **Async-first design** - Native Tokio integration, no blocking
2. **Type safety** - Strong Rust types, no stringly-typed APIs
3. **Cross-platform** - Single API for Unix and Windows
4. **Modern tooling** - Edition 2024, workspace structure

### Areas for Improvement

1. **SSH maturity** - expectrl has production SSH; we need to catch up
2. **Documentation** - Need comprehensive user guide
3. **Ecosystem** - Need cookbook, examples repository

### Differentiation Strategy

Focus on developer experience:
- Clear error messages with buffer context
- Fluent builder APIs
- Comprehensive test utilities
- First-class mock backend for testing

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development workflow.

### Priority for Contributors

1. **High impact, low effort**: Documentation improvements, example code
2. **High impact, medium effort**: Dialog async execution, error messages
3. **High impact, high effort**: SSH backend completion

### Getting Started

```bash
# Clone and build
git clone https://github.com/jkindrix/rust-expect
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
| 0.1.0 | 2025-12-30 | Initial implementation |
| 0.2.0 | TBD | API completeness |
| 0.3.0 | TBD | SSH backend |
| 0.4.0 | TBD | Advanced features |
| 0.5.0 | TBD | Performance & observability |
| 1.0.0 | TBD | Stable release |
