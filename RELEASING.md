# Releasing rust-expect

**Version:** 0.1.0
**Last Updated:** 2025-01-03
**Workspace Crates:** 3 (rust-pty, rust-expect-macros, rust-expect)
**Status:** Pre-release

---

## Table of Contents

1. [Cardinal Rules](#cardinal-rules)
2. [Git Hygiene Protocol](#git-hygiene-protocol)
3. [Version Numbering](#version-numbering)
4. [Crate Dependency Graph](#crate-dependency-graph)
5. [Pre-Release Checklist](#pre-release-checklist)
6. [Release Workflow](#release-workflow)
7. [Feature-Specific Testing](#feature-specific-testing)
8. [CI Automation Coverage](#ci-automation-coverage)
9. [Troubleshooting](#troubleshooting)
10. [Security Incident Response](#security-incident-response)
11. [Post-Release Verification](#post-release-verification)

---

## Cardinal Rules

### Rule 1: Never Manual Publish

**Always use the automated GitHub Actions workflow for publishing.**

```bash
# ✅ CORRECT: Tag triggers automated publish
just release-check
just ci-status-all
just tag
git push origin v0.1.0

# ❌ WRONG: Manual cargo publish
cargo publish -p rust-pty  # NEVER DO THIS
```

Why? Manual publishing:
- Bypasses CI verification
- Risks version mismatches
- Cannot be undone (crates.io publishes are permanent)
- Creates inconsistent release artifacts

### Rule 2: Verify ALL CI Workflows

Before tagging, **all three workflows** must pass on the exact HEAD commit:

| Workflow | Purpose | Must Pass |
|----------|---------|-----------|
| CI | Tests, clippy, formatting | ✅ Required |
| Security Audit | Dependency vulnerabilities | ✅ Required |

```bash
# Check all workflows passed on HEAD
just ci-status-all

# If any failed, fix issues and push again
# NEVER tag until ALL workflows show green on HEAD
```

### Rule 3: Use --all-features for Testing

The `full` feature enables most optional features. Always test with it:

```bash
# Local verification must use all features
just ci-all           # Includes --all-features
just test-all         # Tests with all features
just clippy-all       # Lints with all features

# This catches issues like:
# - Missing feature gates
# - Incompatible feature combinations
# - Conditional compilation errors
```

### Rule 4: Publish in Dependency Order

Crates must be published in topological order with delays between tiers:

```
Tier 0: rust-pty           (no internal deps)
        ↓
Tier 1: rust-expect-macros (no internal deps, proc-macro)
        ↓
Tier 2: rust-expect        (depends on rust-pty, rust-expect-macros)
```

Each tier needs a 30-second delay for crates.io index propagation.

---

## Git Hygiene Protocol

### Release Branch Strategy

For significant releases, use a release branch to batch changes:

```bash
# Create release branch
git checkout -b release/v0.2.0

# Make changes (CHANGELOG, version bumps, doc updates)
# Commit frequently for reviewability

# When ready, create PR to main
gh pr create --title "Release v0.2.0" --body "Release preparation"

# After PR approved and merged, tag from main
git checkout main
git pull
just tag
git push origin v0.2.0
```

### Commit Message Standards

Use conventional commits for automatic changelog generation:

```bash
# Feature additions
feat(screen): add visual diff comparison

# Bug fixes
fix(ssh): handle connection timeout correctly

# Breaking changes (note the !)
feat(session)!: rename spawn() to new()

# Scope by crate when relevant
fix(rust-pty): handle SIGWINCH on FreeBSD
```

### Squash vs Merge

- **Squash merge** release branches (cleaner history)
- **Regular merge** for feature branches with valuable commit history
- **Never force push** to main

---

## Version Numbering

### Semantic Versioning

```
MAJOR.MINOR.PATCH

0.x.y  = Pre-1.0, breaking changes allowed in minor versions
1.0.0  = Stable API commitment begins
1.x.0  = New features, backwards compatible
1.x.y  = Bug fixes only
```

### Current Status

| Crate | Version | Stability |
|-------|---------|-----------|
| rust-pty | 0.1.0 | Pre-stable |
| rust-expect-macros | 0.1.0 | Pre-stable |
| rust-expect | 0.1.0 | Pre-stable |

### Version Synchronization

All crates in the workspace share the same version via `workspace.package.version`.
When releasing, all crates are published together at the same version.

---

## Crate Dependency Graph

### Tier Structure

```
┌──────────────────────────────────────────────────────────────┐
│                    TIER 2: Main Library                      │
│  ┌────────────────────────────────────────────────────────┐  │
│  │  rust-expect                                            │  │
│  │  - Session management, pattern matching, dialogs        │  │
│  │  - Depends on: rust-pty, rust-expect-macros            │  │
│  │  - Features: ssh, mock, screen, pii-redaction, metrics │  │
│  └────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
                              │
                    ┌─────────┴─────────┐
                    ▼                   ▼
┌──────────────────────────┐  ┌──────────────────────────────┐
│     TIER 1: Proc-Macro   │  │       TIER 0: PTY Layer      │
│  ┌────────────────────┐  │  │  ┌────────────────────────┐  │
│  │ rust-expect-macros │  │  │  │       rust-pty         │  │
│  │ - Pattern macros   │  │  │  │ - Cross-platform PTY   │  │
│  │ - Compile-time     │  │  │  │ - Unix: rustix         │  │
│  │   validation       │  │  │  │ - Windows: ConPTY      │  │
│  └────────────────────┘  │  │  └────────────────────────┘  │
└──────────────────────────┘  └──────────────────────────────┘
```

### Publishing Order

```bash
# Tier 0 (independent)
cargo publish -p rust-pty
sleep 30

# Tier 1 (proc-macro, independent)
cargo publish -p rust-expect-macros
sleep 30

# Tier 2 (depends on Tier 0 and 1)
cargo publish -p rust-expect
```

### Feature Dependencies

| Feature | External Dependencies |
|---------|----------------------|
| `ssh` | russh, russh-keys |
| `mock` | (none) |
| `screen` | vte, bitflags |
| `pii-redaction` | (none) |
| `metrics` | opentelemetry, prometheus |
| `test-utils` | (none) |
| `full` | All above except insecure-skip-verify |

---

## Pre-Release Checklist

### Automated Checks (via `just release-check`)

```bash
# Run the full release validation
just release-check

# This executes:
# 1. ci-all          - Full CI with all features
# 2. wip-check       - No TODO/FIXME/unimplemented! in src/
# 3. panic-audit     - Review .unwrap() and .expect() usage
# 4. version-sync    - README version matches Cargo.toml
# 5. metadata-check  - Cargo.toml has required fields
# 6. audit           - No known vulnerabilities
# 7. deny            - License and advisory compliance
```

### Manual Checks

Before running `just tag`:

- [ ] CHANGELOG.md has entry for this version
- [ ] README.md version references are current
- [ ] All examples compile and run
- [ ] Breaking changes documented in MIGRATION.md
- [ ] ROADMAP.md updated if milestones completed
- [ ] No uncommitted changes (`git status`)
- [ ] On main branch, up to date with origin

### Documentation Review

```bash
# Build and review docs locally
just doc
just doc-open

# Check for:
# - Missing documentation on public items
# - Broken intra-doc links
# - Outdated examples
```

---

## Release Workflow

### Standard Release

```bash
# 1. Ensure you're on main and up to date
git checkout main
git pull origin main

# 2. Run full release validation
just release-check

# 3. Verify all CI workflows passed on HEAD
just ci-status-all

# 4. Create and verify the tag
just tag
# Review the tag message

# 5. Push tag to trigger automated release
git push origin v0.1.0

# 6. Monitor the release workflow
gh run watch

# 7. Verify on crates.io
just crates-io rust-expect
just docs-rs rust-expect
```

### Hotfix Release

For urgent fixes to a released version:

```bash
# 1. Create hotfix branch from the release tag
git checkout -b hotfix/v0.1.1 v0.1.0

# 2. Apply minimal fix
# ... make changes ...

# 3. Bump patch version in Cargo.toml
# 4. Update CHANGELOG.md

# 5. Merge to main via PR
git push origin hotfix/v0.1.1
gh pr create --base main

# 6. After merge, tag and release as normal
git checkout main
git pull
just tag
git push origin v0.1.1
```

---

## Feature-Specific Testing

### SSH Feature

```bash
# Test SSH compilation
cargo build -p rust-expect --features ssh

# Run SSH-specific tests (requires SSH server)
cargo test -p rust-expect --features ssh ssh_

# Check SSH examples compile
cargo build --example ssh --features ssh
```

### Screen Feature

```bash
# Test screen buffer
cargo test -p rust-expect --features screen screen_

# Run screen benchmarks
cargo bench -p rust-expect --features screen -- screen
```

### PII Redaction Feature

```bash
# Test PII detection
cargo test -p rust-expect --features pii-redaction pii_

# Verify credit card detection
cargo test -p rust-expect --features pii-redaction credit_card
```

### Metrics Feature

```bash
# Test metrics export
cargo test -p rust-expect --features metrics metrics_

# Check OpenTelemetry integration
cargo build -p rust-expect --features metrics
```

### All Features Combined

```bash
# The full feature set (excludes insecure-skip-verify)
cargo test -p rust-expect --features full

# Verify feature combinations work together
cargo check -p rust-expect --features "ssh screen pii-redaction"
cargo check -p rust-expect --features "mock test-utils"
```

---

## CI Automation Coverage

### What CI Checks

| Check | Default Features | All Features |
|-------|------------------|--------------|
| `cargo fmt --check` | ✅ | ✅ |
| `cargo clippy` | ✅ | ✅ |
| `cargo test` | ✅ | ✅ |
| `cargo doc` | ✅ | ✅ |
| `cargo audit` | ✅ | ✅ |
| `cargo deny check` | ✅ | ✅ |

### Platform Matrix

| Platform | Tests | Notes |
|----------|-------|-------|
| Linux x86_64 | ✅ Full | Primary platform |
| macOS x86_64 | ✅ Full | Includes PTY tests |
| macOS ARM64 | ✅ Full | Apple Silicon |
| Windows x86_64 | ✅ Full | ConPTY tests |

### What CI Does NOT Check

The following require manual verification:

- [ ] SSH tests against real servers
- [ ] Performance regression (review benchmarks)
- [ ] Documentation accuracy
- [ ] Example code correctness
- [ ] Cross-compilation targets

---

## Troubleshooting

### crates.io Index Propagation

**Symptom:** `cargo publish` fails with "dependency not found"

```
error: failed to verify package tarball
Caused by: no matching package named `rust-pty` found
```

**Solution:** Wait for index propagation (30-60 seconds between tiers)

```bash
# The automated workflow handles this with delays
# If manual publishing (emergency only), add delays:
cargo publish -p rust-pty
sleep 60  # Wait longer if needed
cargo publish -p rust-expect-macros
sleep 60
cargo publish -p rust-expect
```

### Version Already Exists

**Symptom:** "crate version already exists"

**Solution:** You cannot republish the same version. Bump the version:

```bash
# In Cargo.toml, increment patch version
# 0.1.0 → 0.1.1

# Update CHANGELOG
# Re-run release checks
just release-check
just tag
```

### CI Status Mismatch

**Symptom:** `just ci-status-all` shows workflows passed on different commit

**Solution:** Your local commits aren't pushed or CI hasn't run yet

```bash
# Push latest commits
git push origin main

# Wait for CI to complete
gh run watch

# Verify again
just ci-status-all
```

### Feature Compilation Errors

**Symptom:** Build fails with `--all-features`

**Common causes:**
1. Missing feature gate on conditional code
2. Incompatible dependency versions between features
3. Platform-specific code not gated

**Debug:**

```bash
# Test individual features
cargo check -p rust-expect --features ssh
cargo check -p rust-expect --features screen
cargo check -p rust-expect --features metrics

# Test feature combinations
cargo check -p rust-expect --features "ssh screen"
```

---

## Security Incident Response

### Discovering a Vulnerability

1. **Do NOT** disclose publicly
2. File a security advisory: https://github.com/praxiomlabs/rust-expect/security/advisories/new
3. Assess severity and affected versions

### Yanking a Release

If a release contains a security vulnerability:

```bash
# Yank the affected version (does NOT delete, just warns users)
cargo yank --version 0.1.0 rust-expect
cargo yank --version 0.1.0 rust-expect-macros
cargo yank --version 0.1.0 rust-pty

# Publish patched version immediately
# Follow standard release process with fix
```

### Known Vulnerability: RUSTSEC-2023-0071

The `ssh` feature depends on `russh` which uses the `rsa` crate with a known timing vulnerability. See SECURITY.md for mitigation guidance (use Ed25519 keys instead of RSA).

---

## Post-Release Verification

After the release workflow completes:

### Verify on crates.io

```bash
# Open crates.io pages
just crates-io rust-expect
just crates-io rust-pty
just crates-io rust-expect-macros

# Verify:
# - Version number is correct
# - README renders properly
# - Feature flags are listed
# - License is shown
```

### Verify on docs.rs

```bash
# Open docs.rs pages
just docs-rs rust-expect

# Verify:
# - Documentation built successfully
# - Feature-gated items are documented
# - Examples render correctly
# - All public items have docs
```

### Verify Installation

```bash
# In a new directory, test installation
cargo new test-install
cd test-install

# Add dependency
echo 'rust-expect = "0.1"' >> Cargo.toml

# Verify it builds
cargo check

# Test with features
echo 'rust-expect = { version = "0.1", features = ["ssh", "screen"] }' >> Cargo.toml
cargo check
```

### Announce the Release

After verification:

1. Create GitHub Release from the tag
2. Include CHANGELOG excerpt in release notes
3. Announce on relevant channels if significant

---

## Quick Reference

### Common Commands

```bash
# Pre-release validation
just release-check      # Full validation with all features
just ci-status-all      # Verify all workflows passed

# Create release
just tag                # Create annotated tag
git push origin vX.Y.Z  # Trigger release workflow

# Verify release
just crates-io rust-expect
just docs-rs rust-expect

# Emergency procedures
cargo yank --version X.Y.Z rust-expect  # Security incidents only
```

### Release Checklist Summary

1. [ ] `just release-check` passes
2. [ ] `just ci-status-all` shows all green on HEAD
3. [ ] CHANGELOG.md updated
4. [ ] No uncommitted changes
5. [ ] `just tag` creates version tag
6. [ ] `git push origin vX.Y.Z` triggers release
7. [ ] Verify on crates.io and docs.rs
8. [ ] Create GitHub Release

---

## Appendix: First-Time Setup

### crates.io Token

To publish (automated workflow uses repository secrets):

```bash
# Login to crates.io
cargo login

# Token is stored in ~/.cargo/credentials.toml
# For CI, set CARGO_REGISTRY_TOKEN secret in GitHub
```

### GitHub CLI

Required for `just ci-status` and `just ci-status-all`:

```bash
# Install
brew install gh  # macOS
sudo apt install gh  # Ubuntu

# Authenticate
gh auth login
```

### Just Command Runner

```bash
# Install
cargo install just

# Verify
just --version
```
