# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Updated development toolchain to Rust 1.92
- MSRV remains at 1.88 for Edition 2024 and let chains support

### Fixed

- macOS PTY compatibility for window size operations
- Windows SSH pageant authentication handling
- Resolved all clippy lints including `io_other_error` and `collapsible_if`
- Fixed broken intra-doc link in session handle documentation
- CI fixes for cross-platform testing (Windows SSH, macOS PTY)
- **UTF-8 corruption in screen parser**: `Screen::process` previously cast each
  input byte to `char` directly, splitting multi-byte UTF-8 sequences into
  Latin-1 garbage. The parser now accumulates continuation bytes and emits a
  single `Print(char)` for the complete Unicode scalar value. Box-drawing
  chrome, arrows, and emoji now round-trip cleanly through `screen.text()`.
  Malformed sequences emit `U+FFFD`. Survives bytes delivered one at a time.
- **`SessionBuilder::env()` was silently dropped on Unix**: the env map was
  not plumbed from `SessionConfig` through `PtyConfig` into the spawn path,
  so child processes always inherited the parent environment verbatim.
  Windows had the symmetric defect. `PtyConfig` now carries an `env` map;
  Unix applies it via `setenv` between fork and exec (with `clearenv` /
  manual environ walk for `EnvMode::Clear`); Windows passes it through to
  ConPTY's env field.

### Added

- Convenience pattern methods: `shell_prompt()`, `password_prompt()`, `login_prompt()`, `ipv4()`, `email()`, `error_indicator()`, `success_indicator()`
- Session helper methods for common operations
- Comprehensive pattern matching and error handling tests
- New examples demonstrating convenience patterns

#### TUI-driving primitives

The following additions make `Session` suitable for driving full-screen TUI
applications (vim, lazygit, k9s, fzf, Claude Code, …) where the byte stream
is dominated by ANSI escape sequences and literal substring matching on the
raw buffer is impractical. See `tests/output_tap_and_screen.rs` for
integration coverage.

- `Session::add_output_tap(F)` — registers a synchronous callback that
  observes every chunk of bytes the matcher buffer sees, in registration
  order. Foundation for screen emulation, transcript recording, and any
  feature needing visibility into the raw stream as it arrives.
- `Session::output_taps()` — slice accessor for the registered taps.
- `Session::attach_screen()` / `Session::attach_screen_with_dims(rows, cols)`
  (feature `screen`) — creates a virtual `Screen` sized to the PTY and wires
  it via an output tap so it stays up to date during `expect_*`, `wait`,
  and `wait_screen_stable`.
- `Session::screen()` — accessor for the attached `Arc<Mutex<Screen>>`.
- `Session::expect_screen_contains(needle, timeout)` (feature `screen`) —
  polls the rendered screen for a substring while driving reads in short
  increments. The screen-aware counterpart to `expect`.
- `Session::wait_screen_stable(quiet, max_wait)` (feature `screen`) —
  returns once the rendered screen has been unchanged for `quiet`, or
  errors at `max_wait`. EOF counts as stable.
- `Session::send_paste(text)` — wraps `text` in bracketed-paste markers
  (`\x1b[200~ … \x1b[201~`). Lets drivers submit input to a TUI that has
  enabled DECSET 2004 without triggering autocomplete, slash-command popups,
  or per-character interpretation of multi-line content.

#### Public API additions

- `backend::PtyConfig` now exposes an `env: HashMap<String, String>` field
  used to plumb `SessionBuilder::env()` values through to the spawned child.

## [0.1.0] - 2025-01-03

### Added

- Initial release of rust-expect
- Core session management with async/await support
- Pattern matching with literal, regex, and glob patterns
- PTY (pseudo-terminal) support for Unix and Windows (ConPTY)
- Dialog system for scripted interactions
- Human-like typing simulation

### Feature Modules

- `ssh` - SSH session support via russh
- `mock` - Mock sessions for testing
- `screen` - Virtual terminal emulation with ANSI support
- `pii-redaction` - Automatic PII masking in logs
- `test-utils` - Testing utilities and fixtures
- `metrics` - Performance monitoring

### Crates

- `rust-expect` - Main library
- `rust-expect-macros` - Procedural macros
- `rust-pty` - Low-level PTY abstraction
