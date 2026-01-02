# Contributing to rust-expect

Thank you for your interest in contributing to rust-expect! This document provides guidelines and information for contributors.

## Code of Conduct

This project adheres to a [Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code.

## How to Contribute

### Reporting Issues

- Use the GitHub issue tracker
- Search existing issues before creating a new one
- Provide detailed reproduction steps
- Include Rust version and OS information

### Pull Requests

1. Fork the repository
2. Create a feature branch from `main`
3. Make your changes
4. Add or update tests as needed
5. Run the full test suite
6. Submit a pull request

### Development Setup

```bash
# Clone your fork
git clone https://github.com/YOUR_USERNAME/rust-expect.git
cd rust-expect

# Build the project
cargo build --workspace --all-features

# Run tests
cargo test --workspace --all-features

# Run clippy
cargo clippy --workspace --all-features

# Format code
cargo fmt --all
```

## Code Style

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `rustfmt` for formatting
- Address all `clippy` warnings
- Write documentation for public APIs
- Include examples in documentation where helpful

## Testing

- Write unit tests for new functionality
- Add integration tests for complex features
- Test edge cases and error conditions
- Ensure tests pass on Linux and macOS

## Documentation

- Update README if adding new features
- Add doc comments to public items
- Include examples in doc comments
- Update CHANGELOG for notable changes

## Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` New features
- `fix:` Bug fixes
- `docs:` Documentation changes
- `test:` Test additions/changes
- `refactor:` Code refactoring
- `chore:` Maintenance tasks

## License

By contributing, you agree that your contributions will be licensed under the same terms as the project (MIT OR Apache-2.0).
