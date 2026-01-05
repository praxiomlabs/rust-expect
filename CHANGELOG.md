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

### Added

- Convenience pattern methods: `shell_prompt()`, `password_prompt()`, `login_prompt()`, `ipv4()`, `email()`, `error_indicator()`, `success_indicator()`
- Session helper methods for common operations
- Comprehensive pattern matching and error handling tests
- New examples demonstrating convenience patterns

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
