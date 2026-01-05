# rust-expect Justfile
# https://just.systems/man/en/
#
# Usage: just [recipe] [arguments...]
# Run `just help` for available recipes

# ============================================================================
# PROJECT CONFIGURATION
# ============================================================================

# Project metadata (from Cargo.toml)
project_name := "rust-expect"
version := "0.1.0"
msrv := "1.85"
edition := "2024"

# Main crate for default operations
main_crate := "rust-expect"

# Parallel job count (use all available cores)
jobs := `nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4`

# ============================================================================
# FEATURE CONFIGURATION
# ============================================================================

# Control feature flags via environment variable:
#   just test                     # all features (default, comprehensive)
#   FEATURES=none just test       # default features only (fast)
#   FEATURES=ssh,screen just test # specific features
features := env_var_or_default("FEATURES", "all")
_ff := if features == "all" { "--all-features" } else if features == "none" { "" } else { "--features " + features }

# ============================================================================
# PLATFORM DETECTION
# ============================================================================

# Detect platform for platform-specific commands
platform := if os() == "macos" { "macos" } else if os() == "windows" { "windows" } else { "linux" }

# Open command varies by platform
open_cmd := if os() == "macos" { "open" } else if os() == "windows" { "start" } else { "xdg-open" }

# Docker command (podman compatibility)
docker := if `command -v podman 2>/dev/null || echo ""` != "" { "podman" } else { "docker" }

# Cargo command (allow override for cross-compilation)
cargo := env_var_or_default("CARGO", "cargo")

# ============================================================================
# TERMINAL COLORS
# ============================================================================

# ANSI color codes for pretty output
reset := '\033[0m'
bold := '\033[1m'
red := '\033[31m'
green := '\033[32m'
yellow := '\033[33m'
blue := '\033[34m'
cyan := '\033[36m'

# ============================================================================
# DEFAULT RECIPE
# ============================================================================

# Default recipe: show help
@_default:
    just help

# ============================================================================
# SETUP RECIPES
# ============================================================================

[group('setup')]
[doc("Full development environment bootstrap (system packages + tools + hooks)")]
bootstrap: setup-system setup-tools setup-hooks
    @printf '{{green}}[OK]{{reset}}   Development environment bootstrapped\n'

[group('setup')]
[doc("Install system packages (platform-specific)")]
setup-system:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Installing system packages for {{platform}}...\n'

    case "{{platform}}" in
        linux)
            if command -v apt-get &> /dev/null; then
                printf '{{cyan}}[INFO]{{reset}} Debian/Ubuntu detected\n'
                # SSH feature requires OpenSSL dev headers
                sudo apt-get update
                sudo apt-get install -y build-essential pkg-config libssl-dev
            elif command -v dnf &> /dev/null; then
                printf '{{cyan}}[INFO]{{reset}} Fedora/RHEL detected\n'
                sudo dnf install -y gcc openssl-devel
            elif command -v pacman &> /dev/null; then
                printf '{{cyan}}[INFO]{{reset}} Arch Linux detected\n'
                sudo pacman -S --noconfirm base-devel openssl
            else
                printf '{{yellow}}[WARN]{{reset}} Unknown Linux distribution, skipping system packages\n'
            fi
            ;;
        macos)
            printf '{{cyan}}[INFO]{{reset}} macOS detected\n'
            if ! command -v brew &> /dev/null; then
                printf '{{yellow}}[WARN]{{reset}} Homebrew not installed, skipping system packages\n'
            else
                brew install openssl pkg-config || true
            fi
            ;;
        windows)
            printf '{{cyan}}[INFO]{{reset}} Windows detected\n'
            printf '{{yellow}}[WARN]{{reset}} Install Visual Studio Build Tools manually if needed\n'
            ;;
    esac
    printf '{{green}}[OK]{{reset}}   System packages ready\n'

[group('setup')]
[doc("Install Rust development tools")]
setup-tools:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Installing Rust development tools...\n'

    # Core tools (nightly rustfmt for unstable options like imports_granularity)
    rustup component add clippy
    rustup toolchain install nightly --component rustfmt --profile minimal
    printf '{{cyan}}[INFO]{{reset}} Using nightly rustfmt for import grouping support\n'

    # Optional but recommended tools
    TOOLS=(
        "cargo-nextest"      # Faster test runner
        "cargo-watch"        # File watcher
        "cargo-audit"        # Security audit
        "cargo-deny"         # License/advisory checks
        "cargo-outdated"     # Dependency updates
        "cargo-machete"      # Unused dependency detection
        "cargo-semver-checks" # Semver verification
        "typos-cli"          # Spell checker
        "git-cliff"          # Changelog generator
    )

    for tool in "${TOOLS[@]}"; do
        if ! command -v "${tool//-/_}" &> /dev/null && ! cargo install --list | grep -q "^$tool "; then
            printf '{{cyan}}[INFO]{{reset}} Installing %s...\n' "$tool"
            cargo install "$tool" --locked 2>/dev/null || printf '{{yellow}}[WARN]{{reset}} Failed to install %s (optional)\n' "$tool"
        fi
    done

    printf '{{green}}[OK]{{reset}}   Development tools installed\n'

[group('setup')]
[doc("Install Git hooks for pre-commit checks")]
setup-hooks:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Installing Git hooks...\n'

    # Create hooks directory if needed
    mkdir -p .git/hooks

    # Pre-commit hook
    printf '#!/bin/bash\nset -e\njust pre-commit\n' > .git/hooks/pre-commit
    chmod +x .git/hooks/pre-commit

    # Pre-push hook
    printf '#!/bin/bash\nset -e\njust pre-push\n' > .git/hooks/pre-push
    chmod +x .git/hooks/pre-push

    printf '{{green}}[OK]{{reset}}   Git hooks installed\n'

[group('setup')]
[doc("Check development environment and installed tools")]
setup:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Development Environment ══════{{reset}}\n\n'

    # Required tools
    printf '{{bold}}Required:{{reset}}\n'
    printf '  Rust:    %s\n' "$(rustc --version 2>/dev/null || echo '❌ not found')"
    printf '  Cargo:   %s\n' "$(cargo --version 2>/dev/null || echo '❌ not found')"
    printf '  Rustfmt: %s\n' "$(rustfmt --version 2>/dev/null || echo '❌ not found')"
    printf '  Clippy:  %s\n' "$(cargo clippy --version 2>/dev/null || echo '❌ not found')"

    # Optional tools
    printf '\n{{bold}}Optional:{{reset}}\n'
    for tool in cargo-nextest cargo-watch cargo-audit cargo-deny cargo-outdated cargo-machete cargo-semver-checks typos git-cliff; do
        if command -v "$tool" &> /dev/null || cargo install --list | grep -q "^$tool "; then
            printf '  ✅ %s\n' "$tool"
        else
            printf '  ⬚  %s (install with: cargo install %s)\n' "$tool" "$tool"
        fi
    done

    printf '\n{{green}}[OK]{{reset}}   Environment check complete\n'

# ============================================================================
# BUILD RECIPES
# ============================================================================

[group('build')]
[doc("Build workspace (use FEATURES=none for fast builds)")]
build:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Building workspace (features={{features}})...\n'
    {{cargo}} build --workspace {{_ff}} -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   Build complete\n'

[group('build')]
[doc("Check workspace compiles (use FEATURES=none for fast checks)")]
check:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Checking workspace (features={{features}})...\n'
    {{cargo}} check --workspace {{_ff}} -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   Check complete\n'

[group('build')]
[doc("Build in release mode")]
build-release:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Building workspace (release, features={{features}})...\n'
    {{cargo}} build --workspace --release {{_ff}} -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   Release build complete\n'

[group('build')]
[doc("Check MSRV compliance")]
msrv-check:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Checking MSRV ({{msrv}}) compliance (features={{features}})...\n'

    # Check if MSRV toolchain is installed
    if ! rustup run {{msrv}} cargo --version &> /dev/null; then
        printf '{{yellow}}[WARN]{{reset}} Installing Rust {{msrv}}...\n'
        rustup install {{msrv}}
    fi

    rustup run {{msrv}} cargo check --workspace {{_ff}}
    printf '{{green}}[OK]{{reset}}   MSRV check passed ({{msrv}})\n'

# ============================================================================
# TEST RECIPES
# ============================================================================

[group('test')]
[doc("Run tests")]
test:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running tests (features={{features}})...\n'
    {{cargo}} test --workspace {{_ff}} -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   Tests passed\n'

[group('test')]
[doc("Run tests with nextest (faster, better output)")]
nextest:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running tests with nextest (features={{features}})...\n'
    {{cargo}} nextest run --workspace {{_ff}} -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   Tests passed\n'

[group('test')]
[doc("Run tests with nextest using locked deps (CI mode)")]
nextest-locked:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running tests with nextest (locked, features={{features}})...\n'
    {{cargo}} nextest run --workspace {{_ff}} --locked -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   Tests passed\n'

[group('test')]
[doc("Run a specific test by name")]
test-one name:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running test: {{name}}\n'
    {{cargo}} test --workspace {{_ff}} -- "{{name}}" --nocapture
    printf '{{green}}[OK]{{reset}}   Test passed\n'

[group('test')]
[doc("Run tests for specific feature")]
test-feature feature:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running tests for feature: {{feature}}\n'
    {{cargo}} test -p {{main_crate}} --features "{{feature}}" -j {{jobs}}
    printf '{{green}}[OK]{{reset}}   Tests passed\n'

[group('test')]
[doc("Test all feature combinations")]
test-features:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Testing feature combinations...\n'

    # Individual features
    for feature in ssh mock screen pii-redaction metrics test-utils; do
        printf '{{cyan}}[INFO]{{reset}} Testing feature: %s\n' "$feature"
        {{cargo}} test -p {{main_crate}} --features "$feature" --no-fail-fast || exit 1
    done

    # Combined features
    printf '{{cyan}}[INFO]{{reset}} Testing: ssh + screen\n'
    {{cargo}} test -p {{main_crate}} --features "ssh screen" --no-fail-fast

    printf '{{cyan}}[INFO]{{reset}} Testing: full\n'
    {{cargo}} test -p {{main_crate}} --features full --no-fail-fast

    printf '{{green}}[OK]{{reset}}   All feature combinations passed\n'

[group('test')]
[doc("Check feature flag configuration")]
check-feature-flags:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Checking feature flags...\n'

    # Check each feature compiles independently
    for feature in ssh mock screen pii-redaction metrics test-utils legacy-encoding; do
        printf '{{cyan}}[INFO]{{reset}} Checking feature: %s\n' "$feature"
        {{cargo}} check -p {{main_crate}} --features "$feature" || exit 1
    done

    # Check full feature set
    printf '{{cyan}}[INFO]{{reset}} Checking feature: full\n'
    {{cargo}} check -p {{main_crate}} --features full

    # Check dangerous feature (insecure-skip-verify)
    printf '{{cyan}}[INFO]{{reset}} Checking feature: ssh + insecure-skip-verify\n'
    {{cargo}} check -p {{main_crate}} --features "ssh insecure-skip-verify"

    printf '{{green}}[OK]{{reset}}   All feature flags valid\n'

# ============================================================================
# LINT RECIPES
# ============================================================================

[group('lint')]
[doc("Format code with rustfmt (uses nightly for import grouping)")]
fmt:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Formatting code (nightly rustfmt)...\n'
    cargo +nightly fmt --all
    printf '{{green}}[OK]{{reset}}   Code formatted\n'

[group('lint')]
[doc("Check formatting without modifying files (uses nightly for import grouping)")]
fmt-check:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Checking code formatting (nightly rustfmt)...\n'
    cargo +nightly fmt --all -- --check
    printf '{{green}}[OK]{{reset}}   Formatting check passed\n'

[group('lint')]
[doc("Run clippy lints")]
clippy:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running clippy (features={{features}})...\n'
    {{cargo}} clippy --workspace --all-targets {{_ff}} -- -D warnings
    printf '{{green}}[OK]{{reset}}   Clippy passed\n'

[group('lint')]
[doc("Run clippy and automatically fix issues")]
clippy-fix:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running clippy with auto-fix (features={{features}})...\n'
    {{cargo}} clippy --workspace --all-targets {{_ff}} --fix --allow-dirty --allow-staged
    printf '{{green}}[OK]{{reset}}   Clippy fixes applied\n'

[group('lint')]
[doc("Run security audit on dependencies")]
audit:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running security audit...\n'
    {{cargo}} audit
    printf '{{green}}[OK]{{reset}}   Security audit passed\n'

[group('lint')]
[doc("Run cargo-deny checks (licenses, advisories, sources)")]
deny:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running cargo-deny checks...\n'
    {{cargo}} deny check
    printf '{{green}}[OK]{{reset}}   Cargo-deny passed\n'

[group('lint')]
[doc("Detect unused dependencies")]
machete:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Checking for unused dependencies...\n'
    {{cargo}} machete
    printf '{{green}}[OK]{{reset}}   No unused dependencies found\n'

[group('lint')]
[doc("Run semver compatibility check")]
semver:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Checking semver compatibility...\n'
    {{cargo}} semver-checks check-release
    printf '{{green}}[OK]{{reset}}   Semver check passed\n'

[group('lint')]
[doc("Run all lints (fmt + clippy)")]
lint: fmt-check clippy
    @printf '{{green}}[OK]{{reset}}   All lints passed\n'

# ============================================================================
# DOCUMENTATION RECIPES
# ============================================================================

[group('docs')]
[doc("Build documentation")]
doc:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Building documentation (features={{features}})...\n'
    {{cargo}} doc --workspace --no-deps {{_ff}}
    printf '{{green}}[OK]{{reset}}   Documentation built: target/doc/{{main_crate}}/index.html\n'

[group('docs')]
[doc("Build documentation and check for warnings")]
doc-check:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Checking documentation (features={{features}})...\n'
    RUSTDOCFLAGS="-D warnings" {{cargo}} doc --workspace --no-deps {{_ff}}
    printf '{{green}}[OK]{{reset}}   Documentation check passed\n'

[group('docs')]
[doc("Build and open documentation in browser")]
doc-open: doc
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Opening documentation in browser...\n'
    {{open_cmd}} target/doc/{{main_crate}}/index.html

# ============================================================================
# COVERAGE RECIPES
# ============================================================================

[group('coverage')]
[doc("Generate test coverage report (HTML)")]
coverage:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Generating coverage report (features={{features}})...\n'
    {{cargo}} llvm-cov --workspace {{_ff}} --html
    printf '{{green}}[OK]{{reset}}   Coverage report: target/llvm-cov/html/index.html\n'

[group('coverage')]
[doc("Generate coverage in LCOV format (for CI)")]
coverage-lcov:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Generating coverage report (LCOV, features={{features}})...\n'
    {{cargo}} llvm-cov --workspace {{_ff}} --lcov --output-path lcov.info
    printf '{{green}}[OK]{{reset}}   Coverage report: lcov.info\n'

[group('coverage')]
[doc("Open coverage report in browser")]
coverage-open: coverage
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Opening coverage report in browser...\n'
    {{open_cmd}} target/llvm-cov/html/index.html

# ============================================================================
# BENCHMARK RECIPES
# ============================================================================

[group('bench')]
[doc("Run all benchmarks")]
bench:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running benchmarks...\n'
    {{cargo}} bench -p {{main_crate}} --features full
    printf '{{green}}[OK]{{reset}}   Benchmarks complete\n'

[group('bench')]
[doc("Run pattern matching benchmarks")]
bench-patterns:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running pattern matching benchmarks...\n'
    {{cargo}} bench -p {{main_crate}} --bench pattern_matching
    printf '{{green}}[OK]{{reset}}   Pattern benchmarks complete\n'

[group('bench')]
[doc("Run screen buffer benchmarks")]
bench-screen:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running screen buffer benchmarks...\n'
    {{cargo}} bench -p {{main_crate}} --features screen --bench screen_buffer
    printf '{{green}}[OK]{{reset}}   Screen benchmarks complete\n'

[group('bench')]
[doc("Run comparative benchmarks (vs expectrl)")]
bench-compare:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running comparative benchmarks...\n'
    {{cargo}} bench -p {{main_crate}} --bench comparative
    printf '{{green}}[OK]{{reset}}   Comparative benchmarks complete\n'

# ============================================================================
# EXAMPLES
# ============================================================================

[group('examples')]
[doc("Run a specific example")]
example name:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running example: {{name}} (features={{features}})\n'
    {{cargo}} run -p {{main_crate}} --example "{{name}}" {{_ff}}
    printf '{{green}}[OK]{{reset}}   Example complete\n'

[group('examples')]
[doc("Build all examples")]
examples:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Building examples (features={{features}})...\n'
    {{cargo}} build -p {{main_crate}} --examples {{_ff}}
    printf '{{green}}[OK]{{reset}}   Examples built\n'

[group('examples')]
[doc("List available examples")]
examples-list:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{bold}}Available Examples:{{reset}}\n\n'
    printf '{{cyan}}Default Features:{{reset}}\n'
    printf '  basic          - Core spawn/expect workflow\n'
    printf '  dialog         - Dialog-based automation\n'
    printf '  patterns       - Pattern matching capabilities\n'
    printf '  transcript     - Recording and playback\n'
    printf '  interactive    - Interactive terminal mode\n'
    printf '  multi_session  - Managing multiple sessions\n'
    printf '  sync_api       - Synchronous API usage\n'
    printf '\n{{cyan}}Feature-Gated:{{reset}}\n'
    printf '  screen_buffer  - VT100 emulation (screen)\n'
    printf '  pii_redaction  - Sensitive data masking (pii-redaction)\n'
    printf '  ssh            - SSH session concepts (ssh)\n'
    printf '  mock_testing   - Mock backend for testing (mock)\n'
    printf '  metrics        - Prometheus/OpenTelemetry (metrics)\n'
    printf '\n{{cyan}}Usage:{{reset}} just example <name>\n'

# ============================================================================
# DEVELOPMENT WORKFLOW RECIPES
# ============================================================================

[group('dev')]
[doc("Full development check (build + test + lint)")]
dev: build test lint
    @printf '{{green}}[OK]{{reset}}   Development checks passed\n'

[group('dev')]
[no-exit-message]
[doc("Watch mode: re-run tests on file changes")]
watch:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Watching for changes (tests, features={{features}})...\n'
    {{cargo}} watch -x "test --workspace {{_ff}}"

[group('dev')]
[no-exit-message]
[doc("Watch mode: re-run check on file changes")]
watch-check:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Watching for changes (check, features={{features}})...\n'
    {{cargo}} watch -x "check --workspace {{_ff}}"

[group('dev')]
[no-exit-message]
[doc("Watch mode: re-run clippy on file changes")]
watch-clippy:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Watching for changes (clippy, features={{features}})...\n'
    {{cargo}} watch -x "clippy --workspace --all-targets {{_ff}}"

# ============================================================================
# CI/CD RECIPES
# ============================================================================

[group('ci')]
[doc("Check documentation versions match Cargo.toml")]
version-sync:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Checking version sync...\n'
    VERSION=$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "{{main_crate}}") | .version')
    MAJOR_MINOR=$(echo "$VERSION" | cut -d. -f1,2)

    # Check README.md
    if ! grep -q "rust-expect = \"$MAJOR_MINOR\"" README.md 2>/dev/null; then
        printf '{{yellow}}[WARN]{{reset}} README.md may need version update (expected %s)\n' "$MAJOR_MINOR"
    fi

    printf '{{green}}[OK]{{reset}}   Version sync check complete (v%s)\n' "$VERSION"

[group('ci')]
[doc("Standard CI pipeline (fmt + clippy + test + doc + examples)")]
ci: fmt-check clippy nextest-locked doc-check examples
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ CI Pipeline Complete (features={{features}}) ══════{{reset}}\n\n'
    printf '{{green}}[OK]{{reset}}   All CI checks passed\n'

[group('ci')]
[doc("Quick verification: fmt + clippy + check (no tests, fastest feedback)")]
quick: fmt-check clippy check
    @printf '{{green}}[OK]{{reset}}   Quick checks passed\n'

[group('ci')]
[doc("Fast CI checks (no tests)")]
ci-fast: fmt-check clippy check
    @printf '{{green}}[OK]{{reset}}   Fast CI checks passed\n'

[group('ci')]
[doc("Full CI with coverage and security audit")]
ci-full: ci coverage-lcov audit deny
    @printf '{{green}}[OK]{{reset}}   Full CI pipeline passed\n'

[group('ci')]
[doc("Complete CI with all checks (for releases)")]
ci-release: ci-full semver msrv-check test-features
    @printf '{{green}}[OK]{{reset}}   Release CI pipeline passed\n'

[group('ci')]
[doc("Check if CI passed on the main branch (use before tagging)")]
ci-status:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Checking CI status on main branch...\n'

    if ! command -v gh &> /dev/null; then
        printf '{{red}}[ERR]{{reset}}  GitHub CLI (gh) is required for this check\n'
        printf '{{cyan}}[INFO]{{reset}} Install: https://cli.github.com/\n'
        exit 1
    fi

    STATUS=$(gh run list --branch main --limit 1 --json status,conclusion,headSha,displayTitle --jq '.[0]')

    if [ -z "$STATUS" ]; then
        printf '{{red}}[ERR]{{reset}}  No workflow runs found on main branch\n'
        exit 1
    fi

    RUN_STATUS=$(echo "$STATUS" | jq -r '.status')
    CONCLUSION=$(echo "$STATUS" | jq -r '.conclusion')
    SHA=$(echo "$STATUS" | jq -r '.headSha' | cut -c1-7)
    TITLE=$(echo "$STATUS" | jq -r '.displayTitle')

    printf '{{cyan}}[INFO]{{reset}} Latest run: %s (%s)\n' "$TITLE" "$SHA"

    if [ "$RUN_STATUS" != "completed" ]; then
        printf '{{yellow}}[WARN]{{reset}} CI is still running (status: %s)\n' "$RUN_STATUS"
        printf '{{cyan}}[INFO]{{reset}} Wait for CI to complete: gh run watch\n'
        exit 1
    fi

    if [ "$CONCLUSION" != "success" ]; then
        printf '{{red}}[ERR]{{reset}}  CI failed (conclusion: %s)\n' "$CONCLUSION"
        printf '{{cyan}}[INFO]{{reset}} Check the workflow: gh run view\n'
        exit 1
    fi

    HEAD_SHA=$(git rev-parse HEAD | cut -c1-7)
    if [ "$SHA" != "$HEAD_SHA" ]; then
        printf '{{yellow}}[WARN]{{reset}} Latest CI run (%s) does not match HEAD (%s)\n' "$SHA" "$HEAD_SHA"
        printf '{{cyan}}[INFO]{{reset}} Push your commits and wait for CI to pass\n'
        exit 1
    fi

    printf '{{green}}[OK]{{reset}}   CI passed on main (commit %s)\n' "$SHA"

[group('ci')]
[doc("Check if ALL CI workflows passed on main (REQUIRED before tagging)")]
ci-status-all:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Checking ALL workflow statuses on main branch...\n'

    if ! command -v gh &> /dev/null; then
        printf '{{red}}[ERR]{{reset}}  GitHub CLI (gh) is required for this check\n'
        exit 1
    fi

    HEAD_SHA=$(git rev-parse HEAD | cut -c1-7)
    FAILED=0

    for WORKFLOW in "CI" "Security Audit"; do
        STATUS=$(gh run list --branch main --workflow="${WORKFLOW}" --limit 1 --json status,conclusion,headSha --jq '.[0]' 2>/dev/null || echo "{}")

        if [ -z "$STATUS" ] || [ "$STATUS" = "{}" ]; then
            printf '{{yellow}}[WARN]{{reset}} No runs found for workflow: %s\n' "$WORKFLOW"
            continue
        fi

        RUN_STATUS=$(echo "$STATUS" | jq -r '.status // "unknown"')
        CONCLUSION=$(echo "$STATUS" | jq -r '.conclusion // "unknown"')
        SHA=$(echo "$STATUS" | jq -r '.headSha // "unknown"' | cut -c1-7)

        if [ "$RUN_STATUS" != "completed" ]; then
            printf '{{red}}[ERR]{{reset}}  %s: still running\n' "$WORKFLOW"
            FAILED=1
        elif [ "$CONCLUSION" != "success" ]; then
            printf '{{red}}[ERR]{{reset}}  %s: %s (commit %s)\n' "$WORKFLOW" "$CONCLUSION" "$SHA"
            FAILED=1
        elif [ "$SHA" != "$HEAD_SHA" ]; then
            printf '{{yellow}}[WARN]{{reset}} %s: passed but on different commit (%s vs HEAD %s)\n' "$WORKFLOW" "$SHA" "$HEAD_SHA"
            FAILED=1
        else
            printf '{{green}}[OK]{{reset}}   %s: passed (commit %s)\n' "$WORKFLOW" "$SHA"
        fi
    done

    if [ $FAILED -ne 0 ]; then
        printf '\n{{red}}[ERR]{{reset}}  Not all workflows passed. Fix issues before tagging.\n'
        printf '{{cyan}}[INFO]{{reset}} Run: gh run list --branch main\n'
        exit 1
    fi

    printf '\n{{green}}[OK]{{reset}}   All workflows passed on HEAD (%s)\n' "$HEAD_SHA"

[group('ci')]
[doc("Pre-commit hook checks")]
pre-commit: fmt-check clippy check
    @printf '{{green}}[OK]{{reset}}   Pre-commit checks passed\n'

[group('ci')]
[doc("Pre-push hook checks")]
pre-push: ci
    @printf '{{green}}[OK]{{reset}}   Pre-push checks passed\n'

# ============================================================================
# DEPENDENCY MANAGEMENT
# ============================================================================

[group('deps')]
[doc("Check for outdated dependencies")]
outdated:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Checking for outdated dependencies...\n'
    {{cargo}} outdated -R

[group('deps')]
[doc("Update Cargo.lock to latest compatible versions")]
update:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Updating dependencies...\n'
    {{cargo}} update
    printf '{{green}}[OK]{{reset}}   Dependencies updated\n'

[group('deps')]
[doc("Show dependency tree")]
tree:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Dependency tree:\n'
    {{cargo}} tree --workspace

[group('deps')]
[doc("Show duplicate dependencies")]
tree-duplicates:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Duplicate dependencies:\n'
    {{cargo}} tree --workspace --duplicates

# ============================================================================
# RELEASE CHECKLIST RECIPES
# ============================================================================

[group('release')]
[doc("Check for WIP markers (TODO, FIXME, XXX, HACK, todo!, unimplemented!)")]
wip-check:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Checking for WIP markers...\n'

    COMMENTS=$(grep -rn "TODO\|FIXME\|XXX\|HACK" --include="*.rs" crates/ 2>/dev/null || true)
    if [ -n "$COMMENTS" ]; then
        printf '{{yellow}}[WARN]{{reset}} Found WIP comments:\n'
        echo "$COMMENTS" | head -20
        COMMENT_COUNT=$(echo "$COMMENTS" | wc -l)
        if [ "$COMMENT_COUNT" -gt 20 ]; then
            printf '{{yellow}}[WARN]{{reset}} ... and %d more\n' "$((COMMENT_COUNT - 20))"
        fi
    fi

    MACROS=$(grep -rn "todo!\|unimplemented!" --include="*.rs" crates/*/src/ 2>/dev/null || true)
    if [ -n "$MACROS" ]; then
        printf '{{red}}[ERR]{{reset}}  Found incomplete macros in production code:\n'
        echo "$MACROS"
        exit 1
    fi

    printf '{{green}}[OK]{{reset}}   WIP check passed (no blocking issues)\n'

[group('release')]
[doc("Audit panic paths (.unwrap(), .expect()) in production code")]
panic-audit:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Auditing panic paths in production code...\n'

    UNWRAPS=$(grep -rn "\.unwrap()" crates/*/src/ --include="*.rs" 2>/dev/null || true)
    EXPECTS=$(grep -rn "\.expect(" crates/*/src/ --include="*.rs" 2>/dev/null || true)

    if [ -n "$UNWRAPS" ] || [ -n "$EXPECTS" ]; then
        printf '{{yellow}}[WARN]{{reset}} Found potential panic paths:\n'
        if [ -n "$UNWRAPS" ]; then
            echo "$UNWRAPS" | head -15
            UNWRAP_COUNT=$(echo "$UNWRAPS" | wc -l)
            printf '{{cyan}}[INFO]{{reset}} Total .unwrap() calls: %d\n' "$UNWRAP_COUNT"
        fi
        if [ -n "$EXPECTS" ]; then
            echo "$EXPECTS" | head -10
            EXPECT_COUNT=$(echo "$EXPECTS" | wc -l)
            printf '{{cyan}}[INFO]{{reset}} Total .expect() calls: %d\n' "$EXPECT_COUNT"
        fi
        printf '{{yellow}}[NOTE]{{reset}} Review each for production safety.\n'
    else
        printf '{{green}}[OK]{{reset}}   No panic paths found in production code\n'
    fi

[group('release')]
[doc("Verify Cargo.toml metadata for crates.io publishing")]
metadata-check:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Checking Cargo.toml metadata...\n'

    METADATA=$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "{{main_crate}}")')

    DESC=$(echo "$METADATA" | jq -r '.description // empty')
    LICENSE=$(echo "$METADATA" | jq -r '.license // empty')
    REPO=$(echo "$METADATA" | jq -r '.repository // empty')

    MISSING=""
    [ -z "$DESC" ] && MISSING="$MISSING description"
    [ -z "$LICENSE" ] && MISSING="$MISSING license"
    [ -z "$REPO" ] && MISSING="$MISSING repository"

    if [ -n "$MISSING" ]; then
        printf '{{red}}[ERR]{{reset}}  Missing required fields:%s\n' "$MISSING"
        exit 1
    fi

    KEYWORDS=$(echo "$METADATA" | jq -r '.keywords // [] | length')
    CATEGORIES=$(echo "$METADATA" | jq -r '.categories // [] | length')

    [ "$KEYWORDS" -eq 0 ] && printf '{{yellow}}[WARN]{{reset}} No keywords defined\n'
    [ "$CATEGORIES" -eq 0 ] && printf '{{yellow}}[WARN]{{reset}} No categories defined\n'

    printf '{{cyan}}[INFO]{{reset}} Package metadata:\n'
    printf '  description: %s\n' "$DESC"
    printf '  license:     %s\n' "$LICENSE"
    printf '  repository:  %s\n' "$REPO"
    printf '  keywords:    %d defined\n' "$KEYWORDS"
    printf '  categories:  %d defined\n' "$CATEGORIES"

    printf '{{green}}[OK]{{reset}}   Metadata check passed\n'

[group('release')]
[doc("Run typos spell checker")]
typos:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Running typos spell checker...\n'
    if ! command -v typos &> /dev/null; then
        printf '{{yellow}}[WARN]{{reset}} typos not installed (cargo install typos-cli)\n'
        exit 0
    fi
    typos crates/ README.md CHANGELOG.md RELEASING.md ROADMAP.md
    printf '{{green}}[OK]{{reset}}   Typos check passed\n'

[group('release')]
[doc("Prepare for release (runs full CI with all features)")]
release-check: ci-release check-feature-flags wip-check panic-audit version-sync typos machete metadata-check url-check
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Release Validation ══════{{reset}}\n\n'
    printf '{{cyan}}[INFO]{{reset}} Checking for uncommitted changes...\n'
    if ! git diff-index --quiet HEAD --; then
        printf '{{red}}[ERR]{{reset}}  Uncommitted changes detected\n'
        exit 1
    fi
    printf '{{cyan}}[INFO]{{reset}} Checking for unpushed commits...\n'
    if [ -n "$(git log @{u}.. 2>/dev/null)" ]; then
        printf '{{yellow}}[WARN]{{reset}} Unpushed commits detected\n'
    fi
    printf '{{green}}[OK]{{reset}}   Ready for release\n'
    printf '\n{{cyan}}[NEXT]{{reset}} Run: just ci-status-all && just tag\n'

[group('release')]
[doc("Publish all crates to crates.io (dry run)")]
publish-dry:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Publishing (dry run) in dependency order...\n'
    # Tier 0: Independent PTY crate
    {{cargo}} publish --dry-run -p rust-pty
    # Tier 1: Proc-macro crate
    {{cargo}} publish --dry-run -p rust-expect-macros
    # Tier 2: Main library
    {{cargo}} publish --dry-run -p rust-expect
    printf '{{green}}[OK]{{reset}}   Dry run complete\n'

[group('release')]
[confirm("⚠️ MANUAL PUBLISHING IS A LAST RESORT! Use the automated GitHub Actions workflow instead. Type 'yes' to acknowledge this is IRREVERSIBLE:")]
[doc("Publish all crates to crates.io (LAST RESORT - prefer automated release)")]
publish:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{red}}════════════════════════════════════════════════════════════════{{reset}}\n'
    printf '{{bold}}{{red}}  ⚠️  WARNING: MANUAL PUBLISHING IS A LAST RESORT!              {{reset}}\n'
    printf '{{bold}}{{red}}════════════════════════════════════════════════════════════════{{reset}}\n\n'
    printf '{{yellow}}You should almost NEVER use this command.{{reset}}\n\n'
    printf 'The correct release workflow is:\n'
    printf '  1. just release-check\n'
    printf '  2. just ci-status-all\n'
    printf '  3. just tag\n'
    printf '  4. git push origin vX.Y.Z  (triggers automated publish)\n\n'
    printf '{{cyan}}[INFO]{{reset}} Publishing to crates.io in dependency order...\n'
    printf '{{cyan}}[INFO]{{reset}} Note: 30s delays between tiers for index propagation\n\n'

    # Tier 0: Independent PTY crate
    printf '{{bold}}Tier 0: PTY Layer{{reset}}\n'
    {{cargo}} publish -p rust-pty
    printf '{{cyan}}[INFO]{{reset}} Waiting 30s for index propagation...\n'
    sleep 30

    # Tier 1: Proc-macro crate
    printf '{{bold}}Tier 1: Proc-macro{{reset}}\n'
    {{cargo}} publish -p rust-expect-macros
    printf '{{cyan}}[INFO]{{reset}} Waiting 30s for index propagation...\n'
    sleep 30

    # Tier 2: Main library
    printf '{{bold}}Tier 2: Main Library{{reset}}\n'
    {{cargo}} publish -p rust-expect

    printf '\n{{green}}[OK]{{reset}}   All crates published successfully\n'

[group('release')]
[doc("Validate dependency graph for publishing")]
dep-graph:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Dependency graph for publishing:\n\n'
    printf '{{bold}}Tier 0 (Independent):{{reset}}\n'
    printf '  rust-pty           Cross-platform PTY abstraction\n\n'
    printf '{{bold}}Tier 1 (Proc-macro):{{reset}}\n'
    printf '  rust-expect-macros Compile-time pattern validation\n\n'
    printf '{{bold}}Tier 2 (Main library):{{reset}}\n'
    printf '  rust-expect        → rust-pty, rust-expect-macros\n\n'
    printf '{{yellow}}[NOTE]{{reset}} Publish in tier order with 30s delays between tiers\n'

[group('release')]
[doc("Check repository URLs are correct")]
url-check:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Checking repository URLs...\n'
    REPO=$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "{{main_crate}}") | .repository')
    if [[ "$REPO" != *"praxiomlabs/rust-expect"* ]]; then
        printf '{{red}}[ERR]{{reset}}  Repository URL incorrect: %s\n' "$REPO"
        printf '{{red}}[ERR]{{reset}}  Expected: https://github.com/praxiomlabs/rust-expect\n'
        exit 1
    fi
    printf '{{green}}[OK]{{reset}}   Repository URL correct: %s\n' "$REPO"

[group('release')]
[doc("Create git tag for release (verifies ALL CI workflows passed first)")]
tag:
    #!/usr/bin/env bash
    set -euo pipefail

    printf '{{cyan}}[INFO]{{reset}} Verifying ALL workflows passed before tagging...\n'

    if ! command -v gh &> /dev/null; then
        printf '{{red}}[ERR]{{reset}}  GitHub CLI (gh) is required.\n'
        printf '{{red}}[ERR]{{reset}}  Cannot create tag without verifying CI status.\n'
        printf '{{cyan}}[INFO]{{reset}} Install: https://cli.github.com/\n'
        exit 1
    fi

    HEAD_SHA=$(git rev-parse HEAD | cut -c1-7)
    FAILED=0

    for WORKFLOW in "CI" "Security Audit"; do
        STATUS=$(gh run list --branch main --workflow="${WORKFLOW}" --limit 1 --json status,conclusion,headSha --jq '.[0]' 2>/dev/null || echo "{}")

        if [ -z "$STATUS" ] || [ "$STATUS" = "{}" ]; then
            printf '{{yellow}}[WARN]{{reset}} No runs found for workflow: %s\n' "$WORKFLOW"
            continue
        fi

        RUN_STATUS=$(echo "$STATUS" | jq -r '.status // "unknown"')
        CONCLUSION=$(echo "$STATUS" | jq -r '.conclusion // "unknown"')
        SHA=$(echo "$STATUS" | jq -r '.headSha // "unknown"' | cut -c1-7)

        if [ "$RUN_STATUS" != "completed" ]; then
            printf '{{red}}[ERR]{{reset}}  %s: still running. Wait for completion.\n' "$WORKFLOW"
            FAILED=1
        elif [ "$CONCLUSION" != "success" ]; then
            printf '{{red}}[ERR]{{reset}}  %s: failed (%s). Fix before tagging.\n' "$WORKFLOW" "$CONCLUSION"
            FAILED=1
        elif [ "$SHA" != "$HEAD_SHA" ]; then
            printf '{{red}}[ERR]{{reset}}  %s: passed on %s but HEAD is %s. Push and wait for CI.\n' "$WORKFLOW" "$SHA" "$HEAD_SHA"
            FAILED=1
        else
            printf '{{green}}[OK]{{reset}}   %s: passed (commit %s)\n' "$WORKFLOW" "$SHA"
        fi
    done

    if [ $FAILED -ne 0 ]; then
        printf '\n{{red}}[ERR]{{reset}}  Cannot create tag. Fix ALL workflow failures first.\n'
        printf '{{cyan}}[INFO]{{reset}} Run: gh run list --branch main\n'
        exit 1
    fi

    printf '\n{{cyan}}[INFO]{{reset}} Creating tag v{{version}}...\n'
    git tag -a "v{{version}}" -m "Release v{{version}}"
    printf '{{green}}[OK]{{reset}}   Tag created: v{{version}}\n'
    printf '\n{{bold}}{{yellow}}⚠️  FINAL CHECK before pushing tag:{{reset}}\n'
    printf '    1. Confirm version {{version}} is correct\n'
    printf '    2. Confirm CHANGELOG has entry for v{{version}}\n'
    printf '\n{{cyan}}[NEXT]{{reset}} Push tag to trigger release: git push origin v{{version}}\n'
    printf '{{red}}[WARN]{{reset}} Once pushed, the release workflow will publish to crates.io.\n'
    printf '{{red}}[WARN]{{reset}} DO NOT cancel the workflow mid-flight!\n'

# ============================================================================
# UTILITIES
# ============================================================================

[group('util')]
[doc("Open crate on crates.io")]
crates-io crate="rust-expect":
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Opening {{crate}} on crates.io...\n'
    {{open_cmd}} "https://crates.io/crates/{{crate}}"

[group('util')]
[doc("Open crate documentation on docs.rs")]
docs-rs crate="rust-expect":
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Opening {{crate}} on docs.rs...\n'
    {{open_cmd}} "https://docs.rs/{{crate}}"

[group('util')]
[doc("Count lines of code")]
loc:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Lines of code:\n'
    if command -v tokei &> /dev/null; then
        tokei . --exclude target --exclude node_modules
    else
        find crates -name '*.rs' | xargs wc -l | tail -1
    fi

[group('util')]
[doc("Analyze binary size bloat")]
bloat crate="rust-expect":
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Binary size analysis for {{crate}}...\n'
    {{cargo}} bloat --release -p {{crate}} --crates

[group('security')]
[doc("Generate Software Bill of Materials (SBOM) in CycloneDX format")]
sbom output="sbom.cyclonedx.json":
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Generating SBOM...\n'
    if ! command -v cargo-sbom &> /dev/null; then
        printf '{{yellow}}[WARN]{{reset}} cargo-sbom not installed\n'
        printf '{{cyan}}[INFO]{{reset}} Install with: cargo install cargo-sbom\n'
        exit 1
    fi
    {{cargo}} sbom --output-format cyclonedx-json > {{output}}
    printf '{{green}}[OK]{{reset}}   SBOM generated: {{output}}\n'

[group('security')]
[doc("Check for unsafe code usage")]
geiger:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Scanning for unsafe code...\n'
    for crate in crates/*/; do
        name=$(basename "$crate")
        printf '{{cyan}}[INFO]{{reset}} Scanning %s...\n' "$name"
        {{cargo}} geiger -p "$name" --all-targets 2>/dev/null || true
    done
    printf '{{green}}[OK]{{reset}}   Unsafe code scan complete\n'

[group('util')]
[doc("Show expanded macros")]
expand crate:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Expanding macros in {{crate}}...\n'
    {{cargo}} expand -p {{crate}}

[group('util')]
[doc("Generate and display project statistics")]
stats: loc
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{blue}}══════ Project Statistics ══════{{reset}}\n\n'
    printf '{{cyan}}Crates:{{reset}}\n'
    find crates -maxdepth 1 -type d | tail -n +2 | while read dir; do
        name=$(basename "$dir")
        printf '  - %s\n' "$name"
    done
    printf '\n{{cyan}}Dependencies:{{reset}}\n'
    printf '  Direct: %s\n' "$({{cargo}} tree --workspace --depth 1 | grep -c '├\|└')"
    printf '  Total:  %s\n' "$({{cargo}} tree --workspace | wc -l)"
    printf '\n'

[group('util')]
[doc("Clean build artifacts")]
clean:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Cleaning build artifacts...\n'
    {{cargo}} clean
    printf '{{green}}[OK]{{reset}}   Clean complete\n'

[group('util')]
[doc("Generate CHANGELOG using git-cliff")]
changelog:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '{{cyan}}[INFO]{{reset}} Generating CHANGELOG...\n'
    if ! command -v git-cliff &> /dev/null; then
        printf '{{yellow}}[WARN]{{reset}} git-cliff not installed\n'
        printf '{{cyan}}[INFO]{{reset}} Install with: cargo install git-cliff\n'
        exit 1
    fi
    git-cliff --output CHANGELOG.md
    printf '{{green}}[OK]{{reset}}   CHANGELOG.md generated\n'

# ============================================================================
# HELP & DOCUMENTATION
# ============================================================================

[group('help')]
[doc("Show version and environment info")]
info:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{project_name}} v{{version}}{{reset}}\n'
    printf '═══════════════════════════════════════\n'
    printf '{{cyan}}MSRV:{{reset}}      {{msrv}}\n'
    printf '{{cyan}}Edition:{{reset}}   {{edition}}\n'
    printf '{{cyan}}Platform:{{reset}}  {{platform}}\n'
    printf '{{cyan}}Jobs:{{reset}}      {{jobs}}\n'
    printf '\n{{cyan}}Rust:{{reset}}      %s\n' "$(rustc --version)"
    printf '{{cyan}}Cargo:{{reset}}     %s\n' "$(cargo --version)"
    printf '{{cyan}}Just:{{reset}}      %s\n' "$(just --version)"
    printf '\n'

[group('help')]
[doc("Check which development tools are installed")]
check-tools: setup

[group('help')]
[doc("Show all available recipes grouped by category")]
help:
    #!/usr/bin/env bash
    set -euo pipefail
    printf '\n{{bold}}{{project_name}} v{{version}}{{reset}} — Terminal Automation Library\n'
    printf 'MSRV: {{msrv}} | Edition: {{edition}} | Platform: {{platform}}\n\n'
    printf '{{bold}}Usage:{{reset}} just [recipe] [arguments...]\n\n'
    printf '{{bold}}Feature Control:{{reset}}\n'
    printf '  Recipes default to ALL features (comprehensive testing).\n'
    printf '  Override with FEATURES environment variable:\n\n'
    printf '    just test                      # all features (default)\n'
    printf '    FEATURES=none just test        # default features only (fast)\n'
    printf '    FEATURES=ssh,screen just test  # specific features\n\n'
    printf '{{bold}}Quick Start:{{reset}}\n'
    printf '  just bootstrap   Full setup (system pkgs + tools + hooks)\n'
    printf '  just setup       Check development environment\n'
    printf '  just quick       Fast feedback (fmt + clippy + check)\n'
    printf '  just ci          Run CI pipeline\n'
    printf '  just ci-release  Full release validation\n\n'
    printf '{{bold}}Available Features:{{reset}}\n'
    printf '  ssh             SSH backend (russh)\n'
    printf '  mock            Mock sessions for testing\n'
    printf '  screen          VT100 terminal emulation\n'
    printf '  pii-redaction   Sensitive data masking\n'
    printf '  metrics         Prometheus/OpenTelemetry\n'
    printf '  test-utils      Testing utilities\n'
    printf '  full            All above features\n\n'
    just --list --unsorted
