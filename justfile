# rust-expect justfile
# https://github.com/casey/just

# Default recipe - show available commands
default:
    @just --list

# Build all workspace crates
build:
    cargo build --workspace

# Build in release mode
build-release:
    cargo build --workspace --release

# Run all tests
test:
    cargo test --workspace

# Run tests with output
test-verbose:
    cargo test --workspace -- --nocapture

# Run a specific test
test-one NAME:
    cargo test --workspace {{NAME}} -- --nocapture

# Check code without building
check:
    cargo check --workspace --all-targets

# Run clippy lints
clippy:
    cargo clippy --workspace --all-targets

# Format code
fmt:
    cargo fmt --all

# Check formatting
fmt-check:
    cargo fmt --all -- --check

# Run all CI checks (format, clippy, test)
ci: fmt-check clippy test

# Generate documentation
doc:
    cargo doc --workspace --no-deps

# Open documentation in browser
doc-open:
    cargo doc --workspace --no-deps --open

# Clean build artifacts
clean:
    cargo clean

# Install rust-expect locally
install:
    cargo install --path crates/rust-expect

# Install with all features
install-all:
    cargo install --path crates/rust-expect --all-features

# Uninstall rust-expect
uninstall:
    cargo uninstall rust-expect

# Run an example
example NAME:
    cargo run --package rust-expect --example {{NAME}}

# List available examples
examples:
    @echo "Available examples:"
    @ls -1 crates/rust-expect/examples/*.rs | xargs -I{} basename {} .rs

# Run benchmarks
bench:
    cargo bench --workspace

# Update dependencies
update:
    cargo update

# Show outdated dependencies
outdated:
    cargo outdated --workspace

# Audit dependencies for security vulnerabilities
audit:
    cargo audit

# Generate and view test coverage (requires cargo-llvm-cov)
coverage:
    cargo llvm-cov --workspace --html
    @echo "Coverage report: target/llvm-cov/html/index.html"

# Watch for changes and run tests
watch:
    cargo watch -x 'test --workspace'

# Watch for changes and run clippy
watch-clippy:
    cargo watch -x 'clippy --workspace --all-targets'

# Publish crates (dry run)
publish-dry:
    cargo publish --package rust-pty --dry-run
    cargo publish --package rust-expect-macros --dry-run
    cargo publish --package rust-expect --dry-run

# Show dependency tree
tree:
    cargo tree --workspace

# Check minimum supported Rust version
msrv:
    cargo msrv --workspace
