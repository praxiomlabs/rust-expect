# Project Scaffolding Prompt Chain

A structured prompt chain for scaffolding Rust projects using `tree2repo`.

**Version:** 1.3.0
**Target:** Rust workspaces with multiple crates
**Complexity:** Medium-to-large projects (10+ source files)
**Last Updated:** 2025-12-30

---

## Table of Contents

1. [Overview](#overview)
2. [Prerequisites](#prerequisites)
3. [Quick Reference](#quick-reference)
4. [Phase 1: Structure Definition](#phase-1-structure-definition) (Prompts 1-4)
5. [Phase 2: Scaffolding](#phase-2-scaffolding) (Prompt 5)
6. [Phase 3: Implementation](#phase-3-implementation) (Prompts 6-8)
7. [Phase 4: Verification](#phase-4-verification) (Prompts 9-11)
8. [Parallel Workstreams](#parallel-workstreams)
9. [Special Cases](#special-cases)
10. [Troubleshooting](#troubleshooting)
11. [Success Criteria](#success-criteria)
12. [References](#references)
13. [Changelog](#changelog)

---

## Overview

This prompt chain guides the systematic scaffolding and implementation of Rust workspace projects. It follows a structured approach:

<!-- Width: 73 chars -->
```
┌─────────────────────────────────────────────────────────────────────┐
│  Phase 1: Structure Definition                                      │
│  ┌─────────┐   ┌──────────┐   ┌──────────┐   ┌────────────┐         │
│  │Prompt 1 │ → │Prompt 2  │ → │Prompt 3  │ → │ Prompt 4   │         │
│  │Generate │   │Validate  │   │Sync Docs │   │Final Gate  │         │
│  │tree.txt │   │Structure │   │          │   │            │         │
│  └─────────┘   └──────────┘   └──────────┘   └────────────┘         │
└─────────────────────────────────────────────────────────────────────┘
                                   ↓
┌─────────────────────────────────────────────────────────────────────┐
│  Phase 2: Scaffolding                                               │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ Prompt 5: Run tree2repo → Create stub files → Commit        │    │
│  └─────────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────┘
                                   ↓
┌─────────────────────────────────────────────────────────────────────┐
│  Phase 3: Implementation (repeat for each phase)                    │
│  ┌─────────────┐   ┌─────────────┐   ┌─────────────┐                │
│  │ Prompt 6    │ → │ Prompt 7    │ → │ Prompt 8    │ ──┐            │
│  │ Plan Phases │   │ Implement   │   │ Validate    │   │            │
│  └─────────────┘   └─────────────┘   └─────────────┘   │            │
│                          ↑                             │            │
│                          └─────────────────────────────┘            │
└─────────────────────────────────────────────────────────────────────┘
                                   ↓
┌─────────────────────────────────────────────────────────────────────┐
│  Phase 4: Verification                                              │
│  ┌───────────┐   ┌───────────┐   ┌────────────┐                     │
│  │ Prompt 9  │ → │ Prompt 10 │ → │ Prompt 11  │                     │
│  │Integration│   │ Features  │   │Final Commit│                     │
│  └───────────┘   └───────────┘   └────────────┘                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Key Principles

1. **tree.txt is authoritative** — Documentation follows structure, not vice versa
2. **Validate before execution** — Always preview with `--dry-run` first
3. **Incremental verification** — Check each phase before proceeding
4. **No accumulated debt** — Fix issues immediately, don't defer
5. **Workspace inheritance** — Use `workspace.dependencies`, `workspace.package`, `workspace.lints`

---

## Prerequisites

Before starting this chain, ensure:

| Requirement | Verification | Notes |
|-------------|--------------|-------|
| `tree2repo` installed | `which tree2repo` | Parses tree structure into files |
| Git repository initialized | `git status` | Clean working tree recommended |
| ARCHITECTURE.md exists | Contains technical design | Source for structure decisions |
| REQUIREMENTS.md exists | Contains functional specs | Source for feature completeness |
| Rust toolchain installed | `rustc --version` | 1.85+ required for Edition 2024 |
| Cargo installed | `cargo --version` | Same version as rustc |

### When to Use This Chain

**Use this chain for:**
- Multi-crate Rust workspaces
- Projects with 10+ source files
- When you need reproducible, documented project structure
- Production-grade projects requiring CI/CD, security tooling

**Do NOT use this chain for:**
- Single-file utilities (just write the file)
- Quick prototypes (scaffold directly)
- Projects with <10 files (overkill)
- Learning exercises (adds unnecessary complexity)

---

## Quick Reference

### tree2repo Commands

```bash
# Preview (ALWAYS run first)
tree2repo --dry-run --list --validate <path>/tree.txt

# Execute (from PARENT of tree root directory)
tree2repo <path>/tree.txt

# Force overwrite existing files (use with CAUTION)
tree2repo --force <path>/tree.txt
```

### Critical: Directory Context

```
⚠️  CRITICAL: If tree.txt starts with a root directory name (e.g., `my-project/`),
    you MUST run tree2repo from the PARENT of that directory.

    Example filesystem:
    /home/user/projects/           ← Run tree2repo FROM HERE
    └── my-project/
        ├── tree.txt               ← tree.txt is HERE
        ├── ARCHITECTURE.md        ← (existing, will be preserved)
        └── ...

    Command:
    cd /home/user/projects && tree2repo my-project/tree.txt
```

### Key Behaviors

| Behavior | Description |
|----------|-------------|
| **Stubs** | 0-byte empty files ready for implementation |
| **Existing files** | Preserved by default (use `--force` to overwrite) |
| **Directory context** | Run from PARENT of tree root directory |
| **Validation** | `--validate` catches structural issues before execution |
| **Dry run** | `--dry-run --list` shows exactly what will be created |

### What Gets Preserved vs Created

| File Type | Behavior | Example |
|-----------|----------|---------|
| Pre-existing files | **Preserved** (not overwritten) | ARCHITECTURE.md, .gitignore |
| New files in tree.txt | **Created as 0-byte stubs** | src/lib.rs, Cargo.toml |
| Directories | **Created if missing** | crates/, tests/, .github/ |

---

## Phase 1: Structure Definition

### Prompt 1: Establish Context and Generate tree.txt

~~~
**Role:** You are a Rust systems architect designing a production-grade project structure.

**Context:** I want to scaffold a Rust project using `tree2repo`, which creates empty stub files from a tree structure. **tree.txt will be the AUTHORITATIVE source of truth** for project structure.

---

### About tree2repo

`tree2repo` parses a tree structure file and scaffolds the entire directory/file structure as empty stubs.

**Key behaviors:**
- Stubs are 0-byte empty files ready for implementation
- Existing files are preserved (use `--force` to overwrite)
- **CRITICAL:** Run from the PARENT of the tree root directory
  - If tree.txt defines `my-project/` as root, run from the directory *containing* `my-project/`
- Preview: `tree2repo --dry-run --list --validate tree.txt`
- Execute: `tree2repo tree.txt`

---

### Task

Based on ARCHITECTURE.md and REQUIREMENTS.md, generate a complete tree.txt.

**Structure Requirements:**
- [ ] Modern Rust 2018+ module conventions (`module.rs` + `module/`, NOT `mod.rs`)
- [ ] Virtual manifest at workspace root (no `src/` in root, only `[workspace]`)
- [ ] Crates in `crates/` directory with kebab-case naming
- [ ] All crates, modules, tests, examples, benches, fixtures included
- [ ] Platform-specific code in platform-specific modules (`unix/`, `windows/`)

**Cargo Workspace Requirements (Edition 2024):**
- [ ] `resolver = "3"` in workspace Cargo.toml (or implied by `edition = "2024"`)
- [ ] `workspace.dependencies` for shared dependencies across crates
- [ ] `workspace.package` for shared metadata (version, edition, authors, license, repository)
- [ ] `workspace.lints` for shared Clippy/rustc lint configuration
- [ ] Member crates use inheritance: `version.workspace = true`, `edition.workspace = true`
- [ ] Cargo.lock committed (required for applications and reproducibility)

**Ecosystem Requirements:**
- [ ] CI/CD workflows (`.github/workflows/`) with Swatinem/rust-cache for faster builds
- [ ] Community health files (CONTRIBUTING.md, CODE_OF_CONDUCT.md, SECURITY.md)
- [ ] Supply chain security (cargo-vet, cargo-deny, cargo-audit, cargo-auditable)
- [ ] Changelog automation (git-cliff with cliff.toml)
- [ ] Dependabot or Renovate configuration

**Completeness Requirements:**
- [ ] Feature parity: every feature flag has modules, tests, AND examples
- [ ] Every public module has corresponding integration tests in `tests/`
- [ ] Configuration files for all tooling (rustfmt.toml, clippy.toml, deny.toml)
- [ ] Crate-specific README.md files for docs.rs

**Format:** Use Unicode tree characters (├──, └──, │) with inline comments describing each file's purpose.

**Example format:**
```
my-project/
├── Cargo.toml                    # Workspace manifest (Edition 2024, resolver v3)
├── Cargo.lock                    # Locked dependencies (committed for reproducibility)
├── crates/
│   └── my-crate/
│       ├── Cargo.toml            # Crate manifest (inherits workspace settings)
│       └── src/
│           ├── lib.rs            # Public API, re-exports
│           └── error.rs          # Error types (thiserror)
```

Search online if needed to verify current Rust ecosystem best practices (Edition 2024, resolver v3, workspace inheritance, etc.).
~~~

**Expected Output:** Complete tree.txt file with 50-300 entries depending on project scope.

**Exit Criteria:** tree.txt generated with all requirements addressed.

---

### Prompt 2: Validate Before Lock-in

~~~
**Role:** You are a critical code reviewer examining a project structure for gaps.

**Task:** Review tree.txt for completeness against these criteria:

---

### Structural Validation

- [ ] Every source module in `src/` has corresponding test in `tests/`
- [ ] Every feature flag has corresponding example in `examples/`
- [ ] Every crate has its own README.md for docs.rs
- [ ] Platform-specific code (`unix/`, `windows/`) has platform-specific tests
- [ ] No `mod.rs` files (using modern `module.rs` + `module/` pattern)

### Workspace Validation

- [ ] Root Cargo.toml is virtual manifest (`[workspace]` only, no `[package]`)
- [ ] All crates listed in `workspace.members` (use glob patterns: `crates/*`)
- [ ] Shared dependencies in `workspace.dependencies`
- [ ] Shared metadata in `workspace.package`
- [ ] Shared lints in `workspace.lints`
- [ ] Cargo.lock present (for applications)
- [ ] rust-toolchain.toml pins toolchain version

### Ecosystem Validation

- [ ] GitHub community health files complete:
  - SECURITY.md, CONTRIBUTING.md, CODE_OF_CONDUCT.md
  - .github/CODEOWNERS
  - .github/FUNDING.yml (if applicable)
- [ ] Issue templates and PR templates present:
  - .github/ISSUE_TEMPLATE/bug_report.md
  - .github/ISSUE_TEMPLATE/feature_request.md
  - .github/PULL_REQUEST_TEMPLATE.md
- [ ] CI workflows cover: build, test, lint, security, release
- [ ] Dependabot configuration present (.github/dependabot.yml)

### Consistency Validation

- [ ] Naming follows kebab-case convention throughout
- [ ] Comments describe purpose for all non-obvious files
- [ ] No orphaned directories (every directory has files or subdirectories)

---

**Output:**
1. List any gaps found with severity (critical/important/minor)
2. Update tree.txt to address ALL gaps
3. Confirm zero remaining issues

**Optional but recommended:** Request external/peer review of tree.txt before proceeding. Peer review catches:
- Outdated conventions (e.g., resolver version, module patterns)
- Missing ecosystem standards (e.g., SLSA, Sigstore)
- Structural gaps (e.g., missing tests for features)
~~~

**Exit Criteria:** All checklist items verified, zero known gaps.

---

### Prompt 3: Sync Documentation

~~~
**Task:** Update ARCHITECTURE.md Section 3.1 (Directory Layout) to match tree.txt exactly.

**Rules:**
- tree.txt is authoritative — documentation follows structure, not vice versa
- Include the full tree structure in ARCHITECTURE.md
- Update any version references:
  - `resolver = "3"` for Edition 2024
  - Rust version 1.85+
  - Any dependency versions mentioned
- Ensure ARCHITECTURE.md and tree.txt are byte-for-byte identical in the tree section

**Verification:**
```bash
# Extract tree sections and compare
grep -A 1000 "^rust-expect/" tree.txt > /tmp/tree1.txt
grep -A 1000 "^rust-expect/" ARCHITECTURE.md | head -n $(wc -l < /tmp/tree1.txt) > /tmp/tree2.txt
diff /tmp/tree1.txt /tmp/tree2.txt
# Should show no differences
```
~~~

**Exit Criteria:** ARCHITECTURE.md Section 3.1 matches tree.txt exactly.

---

### Prompt 4: Final Gate

~~~
**⚠️ POINT OF NO RETURN**

After this prompt, tree.txt becomes the scaffold for real files. All structural decisions are locked.

---

### Final Review Checklist

| Category | Verification |
|----------|--------------|
| **Modules** | Every module has corresponding tests |
| **Features** | Every feature has corresponding examples |
| **CI/CD** | Workflows cover build, test, lint, security, release |
| **Security** | Supply chain tooling configured (cargo-vet, deny, audit) |
| **Community** | Health files complete (CONTRIBUTING, CODE_OF_CONDUCT, SECURITY) |
| **Conventions** | Modern patterns throughout (no mod.rs, resolver v3) |
| **Documentation** | ARCHITECTURE.md synced with tree.txt |

---

### Questions to Answer (Honestly)

1. **Uncertainty:** Are there any files you're uncertain about including?
2. **Completeness:** Are there modules implied by REQUIREMENTS.md that are missing?
3. **Naming:** Are there any naming inconsistencies or violations of kebab-case?
4. **Dependencies:** Are inter-crate dependencies correctly represented?
5. **Features:** Do all feature flags have complete coverage (module + test + example)?

---

**Decision:**
- If ANY concerns exist → Address them now, return to Prompt 2
- If NO concerns exist → Proceed to Phase 2: Scaffolding
~~~

**Exit Criteria:** All questions answered satisfactorily, explicit "proceed" decision made.

---

## Phase 2: Scaffolding

### Prompt 5: Scaffold and Commit

~~~
**Task:** Create the project scaffold from tree.txt using tree2repo.

---

### Step 1: Preview (REQUIRED)

From the **PARENT directory** of the tree root:

```bash
# If tree.txt is at /path/to/my-project/tree.txt
# and tree.txt starts with "my-project/", run from /path/to/
cd /path/to
tree2repo --dry-run --list --validate my-project/tree.txt
```

**Verify output shows:**
- Expected number of directories
- Expected number of files
- No validation errors

---

### Step 2: Execute

```bash
tree2repo my-project/tree.txt
```

**Expected output format:**
```
✓ Success!
  46 directories created
  187 files created
```

**Note:** The file count may be less than tree.txt entries if existing files were preserved.

---

### Step 3: Verify

```bash
# Count new untracked files
git status --short | wc -l

# Verify stub files are 0-byte (ready for implementation)
find . -name "*.rs" -empty | wc -l

# Verify existing files were preserved (should have content)
wc -c ARCHITECTURE.md  # Should show bytes, not 0
wc -c REQUIREMENTS.md  # Should show bytes, not 0

# List what was preserved vs created
git status --short | head -20
```

---

### Step 4: Commit Scaffold

```bash
git add -A
git commit -m "chore: scaffold project structure from tree.txt

- X directories created
- Y stub files created (0-byte, ready for implementation)
- Existing files preserved: ARCHITECTURE.md, REQUIREMENTS.md, etc."
```

---

### Important Notes

- **Preserved files:** ARCHITECTURE.md, REQUIREMENTS.md, .gitignore, tree.txt, .git/
- **Stub files:** All new files are 0-byte and need implementation
- **If tree2repo fails:** Fix tree.txt syntax and re-run (validation should catch most issues)
~~~

**Exit Criteria:** All directories and stub files created, committed to git, working tree clean.

---

## Phase 3: Implementation

### Prompt 6: Implementation Strategy

~~~
**Context:** We have N empty stub files from tree2repo. Before implementing, we need a strategic plan.

**Task:** Analyze the stub files and create a phased implementation plan.

---

### Step 1: Dependency Analysis

Identify what must be implemented first:

| Dependency Type | Example | Why First |
|-----------------|---------|-----------|
| Crate dependencies | `rust-pty` before `rust-expect` | Can't compile dependent crate |
| Proc-macro crates | `rust-expect-macros` before `rust-expect` | Macros must exist at compile time |
| Module dependencies | `error.rs`, `types.rs` | Other modules import these |
| Trait definitions | `traits.rs` | Implementations require traits |
| Cargo.toml files | All `Cargo.toml` | Enables `cargo check` |

---

### Step 2: Categorize Files

| Category | Files | Parallelizable | Priority |
|----------|-------|----------------|----------|
| **Cargo Configuration** | `Cargo.toml`, `Cargo.lock` | No | 1 (first) |
| **Tooling Config** | `rustfmt.toml`, `clippy.toml`, `deny.toml` | Yes | 2 |
| **Core Types** | `error.rs`, `types.rs`, `traits.rs` | No | 3 |
| **Proc-Macro Crates** | `*-macros/src/*.rs` | No | 4 |
| **Core Implementation** | `lib.rs`, main modules | No | 5 |
| **Feature Modules** | Feature-gated code | Partially | 6 |
| **Test Utilities** | `test-utils/**/main.rs` | Yes | 7 |
| **Tests** | `tests/**/*.rs` | Yes | 8 |
| **Examples** | `examples/*.rs` | Yes | 8 |
| **CI/CD** | `.github/workflows/*.yml` | Yes | 9 |
| **Documentation** | `README.md`, `CONTRIBUTING.md` | Yes | 9 |
| **Fixtures** | `fixtures/**/*` | Yes | 10 (last) |

---

### Step 3: Define Implementation Phases

**Recommended phase structure:**

| Phase | Name | Files | Enables |
|-------|------|-------|---------|
| 1 | **Cargo Setup** | All `Cargo.toml` files, `rust-toolchain.toml` | `cargo check` works |
| 2 | **Tooling** | `*.toml` config files (rustfmt, clippy, deny) | `cargo fmt`, `cargo clippy` work |
| 3 | **Foundation** | `error.rs`, `types.rs`, `traits.rs`, `prelude.rs` | Core types available |
| 4 | **Proc-Macros** | `*-macros/src/*.rs` | Macros usable by main crate |
| 5 | **Core Library** | `lib.rs`, primary modules | Basic functionality |
| 6 | **Features** | Feature-gated modules | Full functionality |
| 7 | **Test Utilities** | `test-utils/*` binaries | Test fixtures available |
| 8 | **Testing** | `tests/`, `examples/` | Verification |
| 9 | **CI/CD & Docs** | `.github/`, `*.md` | Automation |

---

### Step 4: Identify Parallelization Opportunities

**Can run in parallel with Rust implementation:**
- CI/CD workflows (`.github/workflows/*.yml`)
- Documentation (README.md, CONTRIBUTING.md, SECURITY.md)
- Fixtures (`fixtures/**/*`)
- GitHub templates (`.github/ISSUE_TEMPLATE/`, `.github/PULL_REQUEST_TEMPLATE.md`)

**Must be sequential:**
- Cargo.toml → error.rs → types.rs → traits.rs → lib.rs → feature modules
- Proc-macro crate → crates that use the macros

See [Parallel Workstreams](#parallel-workstreams) for detailed guidance.

---

**Output:** Ordered list of phases with:
- Files in each phase
- Dependencies on previous phases
- Which files can be parallelized
~~~

**Exit Criteria:** Implementation plan documented with clear phase boundaries.

---

### Prompt 7: Implement Phase [N]

~~~
**Task:** Implement Phase [N]: [Phase Name]

**Files to implement:**
- [ ] `path/to/file1.rs`
- [ ] `path/to/file2.rs`
- [ ] `path/to/file3.rs`

---

### Requirements

**Code Quality:**
- Production-quality code, not placeholders or TODOs
- Follow patterns established in ARCHITECTURE.md
- Include module-level documentation (`//!` comments at top of file)
- Use `// SAFETY:` comments for any unsafe code

**Feature Gating:**
- Gate optional functionality with `#[cfg(feature = "...")]`
- Gate optional dependencies in Cargo.toml: `dep = { version = "1.0", optional = true }`
- Ensure features are additive (enabling a feature never breaks compilation)

**Workspace Inheritance:**
```toml
# In crate Cargo.toml, inherit from workspace:
[package]
name = "my-crate"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
tokio.workspace = true
serde.workspace = true

[lints]
workspace = true

# NOT:
# tokio = "1.0"  # Don't duplicate versions
# edition = "2024"  # Don't duplicate metadata
```

---

### Constraints

- Do NOT modify files outside this phase's scope
- Do NOT add dependencies not specified in ARCHITECTURE.md without noting it
- Do NOT change public API signatures established in previous phases
- Do NOT leave TODO/FIXME comments (implement fully or note as future work)

---

### After Implementation

```bash
# Must pass before phase is complete
cargo check --all-features
cargo fmt --check
```

**If check fails:** Fix ALL errors before proceeding. Do not accumulate technical debt.
~~~

**Exit Criteria:** All files in phase implemented, `cargo check --all-features` passes.

---

### Prompt 8: Checkpoint Validation

~~~
**Task:** Validate Phase [N] before proceeding to next phase.

---

### Validation Commands (All Must Pass)

```bash
# 1. Formatting check
cargo fmt --check

# 2. Compilation check
cargo check --all-features

# 3. Lint check (treat warnings as errors)
cargo clippy --all-features -- -D warnings

# 4. Documentation builds
cargo doc --all-features --no-deps
```

---

### Optional: Faster Testing with cargo-nextest

For larger workspaces, [cargo-nextest](https://nexte.st/) provides faster test execution:

```bash
# Install nextest
cargo install cargo-nextest

# Run tests with nextest (faster for large projects)
cargo nextest run --all-features

# Still need separate doctest run (nextest limitation)
cargo test --doc --all-features
```

**When to use nextest:**
- Workspaces with many crates and test binaries
- Projects with long-pole tests that benefit from parallelization
- CI pipelines where test speed matters

**Caveats:**
- Doctests not supported — run `cargo test --doc` separately
- May be slower for projects with fast unit tests and slow integration tests
- Process-per-test model uses more resources

---

### If Issues Found

1. **List each issue** with file and line number
2. **Categorize:** error vs warning vs style
3. **Fix all issues** (do not defer)
4. **Re-run validation** until all pass
5. **Only proceed** when ALL checks pass

---

### On Success: Commit

```bash
git add -A
git commit -m "feat(<scope>): implement <phase description>

- Implemented: file1.rs, file2.rs, file3.rs
- All checks pass: fmt, check, clippy, doc"
```
~~~

**Repeat Prompts 7-8 for each phase until all phases complete.**

**Exit Criteria:** Phase committed with passing validation.

---

## Phase 4: Verification

### Prompt 9: Integration Verification

~~~
**Task:** All implementation phases complete. Run full integration verification.

---

### Build Verification

```bash
# Debug build (all features)
cargo build --all-features

# Release build (catches different issues)
cargo build --release --all-features

# Minimal build (no features)
cargo build --no-default-features
```

---

### Test Verification

```bash
# Full test suite
cargo test --all-features

# Release tests (catch release-only bugs)
cargo test --all-features --release

# Doc tests specifically
cargo test --all-features --doc
```

---

### Documentation Verification

```bash
# Build docs
cargo doc --all-features --no-deps

# Open and manually verify navigation works
# target/doc/<crate>/index.html

# Check for broken intra-doc links
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
```

---

### Examples Verification

```bash
# Build all examples
cargo build --examples --all-features

# Run key examples
cargo run --example basic
cargo run --example <other-example> --features <required-feature>
```

---

**If any verification fails:** Fix before proceeding. Do not skip.
~~~

**Exit Criteria:** All integration verifications pass.

---

### Prompt 10: Feature Flag Verification

~~~
**Task:** Verify feature flags work correctly in isolation and combination.

---

### Individual Feature Testing

```bash
# Default features only
cargo check

# No features (minimal build)
cargo check --no-default-features

# All features
cargo check --all-features

# Each feature individually
cargo check --no-default-features --features ssh
cargo check --no-default-features --features mock
cargo check --no-default-features --features screen
cargo check --no-default-features --features pii-redaction
cargo check --no-default-features --features metrics
# ... for each feature in Cargo.toml
```

---

### Feature Combination Testing

```bash
# Test common combinations
cargo check --no-default-features --features "ssh,mock"
cargo check --no-default-features --features "screen,pii-redaction"
```

---

### Exhaustive Feature Testing with cargo-hack

For comprehensive feature testing, use [cargo-hack](https://github.com/taiki-e/cargo-hack) to test all feature combinations automatically:

```bash
# Install cargo-hack
cargo install cargo-hack

# Test feature powerset (all combinations)
# --depth 2 limits to pairs to avoid combinatorial explosion
# --no-dev-deps avoids cargo#4866 issues
cargo hack check --feature-powerset --depth 2 --no-dev-deps

# Test each feature individually
cargo hack check --each-feature --no-dev-deps

# For CI: exclude certain feature combinations if needed
cargo hack check --feature-powerset --depth 2 \
    --exclude-features "unstable" \
    --no-dev-deps
```

**cargo-hack advantages:**
- Deduplicates equivalent feature combinations automatically
- `--depth N` controls maximum features per combination
- `--mutually-exclusive-features` handles conflicting features
- `--at-least-one-of` ensures required feature coverage

---

### What to Look For

| Issue | Symptom | Fix |
|-------|---------|-----|
| Missing `#[cfg(feature)]` guards | "unresolved import" when feature disabled | Add cfg guard to import and code |
| Dependencies not gated | Build fails without feature | Add `optional = true` to dep |
| Feature combinations fail | Compile error with specific combo | Check conditional compilation logic |
| Non-additive features | Enabling feature breaks build | Refactor to make features additive |

---

**Fix any issues before proceeding.**
~~~

**Exit Criteria:** All feature combinations compile successfully.

---

### Prompt 11: Final Commit

~~~
**Task:** Create final implementation commit(s).

---

### Pre-Commit Checklist

**Code Quality:**
- [ ] `cargo fmt` run (not just check)
- [ ] `cargo clippy --all-features -- -D warnings` passes
- [ ] All tests pass (`cargo test --all-features`)
- [ ] All feature combinations compile (Prompt 10)

**Documentation:**
- [ ] All public items have rustdoc comments
- [ ] Module-level docs present (`//!` at top of each file)
- [ ] README.md has usage examples
- [ ] CHANGELOG.md has initial entry (or git-cliff configured)

**No Leftover Issues:**
- [ ] No TODO/FIXME comments remain (or tracked in issues)
- [ ] No `#[allow(...)]` attributes without justification
- [ ] No `unwrap()` in library code (use proper error handling)

---

### Commit Strategy

| Strategy | When to Use | Example |
|----------|-------------|---------|
| **Atomic** | Clear phase boundaries | `feat(pty): implement rust-pty crate` |
| **Consolidated** | Small project | `feat: implement core library` |
| **Squash** | Messy history | Squash WIP commits before merge |

---

### Commit Message Rules

- Use conventional commits (`feat:`, `fix:`, `docs:`, `chore:`, `refactor:`)
- **NO AI branding** (Anthropic, Claude, OpenAI, ChatGPT, Copilot, etc.)
- Be specific: `feat(session): add timeout configuration` not `feat: updates`
- Use imperative mood: "add feature" not "added feature"

**Example:**

```git
git commit -m "feat(rust-expect): implement core session management

- Add Session struct with spawn, expect, send operations
- Implement pattern matching with regex and literal support
- Add timeout handling with configurable defaults
- Include comprehensive error types with context

Closes #123"
```
~~~

**Exit Criteria:** All code committed with proper conventional commit messages.

---

## Parallel Workstreams

While Rust implementation must be largely sequential (due to compilation dependencies), several workstreams can be developed in parallel by separate agents or sessions.

### Workstream Overview

<!-- Width: 73 chars -->
```
┌─────────────────────────────────────────────────────────────────────┐
│                    Sequential (Compilation Order)                   │
│  ┌──────────┐   ┌─────────┐   ┌─────────┐   ┌─────────┐             │
│  │Cargo.toml│ → │  Types  │ → │ Macros  │ → │  Core   │ → Tests     │
│  └──────────┘   └─────────┘   └─────────┘   └─────────┘             │
└─────────────────────────────────────────────────────────────────────┘
                 ↓ (can start immediately after scaffold)
┌─────────────────────────────────────────────────────────────────────┐
│                    Parallel (No Rust Compilation)                   │
│  ┌─────────┐   ┌─────────┐   ┌──────────┐   ┌──────────┐            │
│  │  CI/CD  │   │  Docs   │   │ Fixtures │   │ Templates│            │
│  └─────────┘   └─────────┘   └──────────┘   └──────────┘            │
└─────────────────────────────────────────────────────────────────────┘
```

### Workstream Details

| Workstream | Files | Blocks On | Can Start After |
|------------|-------|-----------|-----------------|
| **Rust Core** | `crates/*/src/**/*.rs` | Previous phases | Phase 1 (Cargo.toml) |
| **CI/CD** | `.github/workflows/*.yml` | Nothing | Scaffold (Phase 2) |
| **Documentation** | `*.md`, `README.md` files | Nothing | Scaffold (Phase 2) |
| **Fixtures** | `fixtures/**/*` | Nothing | Scaffold (Phase 2) |
| **GitHub Templates** | `.github/ISSUE_TEMPLATE/*`, `*.md` | Nothing | Scaffold (Phase 2) |
| **Config Files** | `rustfmt.toml`, `clippy.toml`, `deny.toml` | Nothing | Scaffold (Phase 2) |

### Parallelization Commands

When using AI agents that support parallel execution:

```markdown
## Parallel Agent Dispatch (Example)

Launch these agents simultaneously after scaffold commit:

1. **Agent: CI/CD Setup**
   - Implement all `.github/workflows/*.yml`
   - Configure Swatinem/rust-cache
   - Set up dependabot.yml

2. **Agent: Documentation**
   - Write README.md with usage examples
   - Complete CONTRIBUTING.md
   - Fill CODE_OF_CONDUCT.md (Contributor Covenant)
   - Write SECURITY.md

3. **Agent: Fixtures & Templates**
   - Create test fixtures (`fixtures/**/*`)
   - Write GitHub issue templates
   - Write PR template

4. **Agent: Rust Implementation** (main thread)
   - Follow Prompts 7-8 sequentially
```

### Efficiency Gains

| Project Size | Sequential Time | With Parallelization | Speedup |
|--------------|-----------------|----------------------|---------|
| Small (50 files) | 1x | ~1x | Minimal |
| Medium (150 files) | 1x | ~0.7x | 30% faster |
| Large (300+ files) | 1x | ~0.5x | 50% faster |

**Note:** Parallelization is most effective when CI/CD, docs, and fixtures are substantial.

---

## Special Cases

### Procedural Macro Crates

Proc-macro crates have special requirements that affect implementation order and structure.

#### Key Constraints

| Constraint | Reason | Impact |
|------------|--------|--------|
| Separate crate required | Compiler limitation | Cannot be a module in another crate |
| `proc-macro = true` in Cargo.toml | Enables macro compilation | Must be in `[lib]` section |
| Can only export proc macros | Compiler limitation | Types/functions must be in separate crate |
| Cannot use own macros | Compilation order | Test in integration tests or examples |

#### Cargo.toml Structure

```toml
# crates/my-macros/Cargo.toml
[package]
name = "my-macros"
version.workspace = true
edition.workspace = true

[lib]
proc-macro = true

[dependencies]
syn = { version = "2.0", features = ["full", "parsing", "printing"] }
quote = "1.0"
proc-macro2 = "1.0"

[dev-dependencies]
# For testing macro output
trybuild = "1.0"
```

#### Implementation Order

```
1. rust-pty (no dependencies)
       ↓
2. rust-expect-macros (proc-macro crate)
       ↓
3. rust-expect (depends on both above)
```

#### Best Practices

From [Nine Rules for Creating Procedural Macros in Rust](https://towardsdatascience.com/nine-rules-for-creating-procedural-macros-in-rust-595aa476a7ff/):

1. **Use `proc-macro2` internally** — More convenient and testable than `proc_macro`
2. **Use `syn` for parsing** — Handles all Rust syntax correctly
3. **Use `quote!` for code generation** — Type-safe token generation
4. **Accumulate errors** — Don't fail on first error; collect all issues
5. **Emit valid code even on error** — Helps rust-analyzer provide completions
6. **Test with `cargo expand`** — Verify generated code is correct
7. **Use `trybuild` for compile-fail tests** — Verify error messages

#### Publishing Order

If publishing to crates.io:

```bash
# Must publish in dependency order with delay between each
cargo publish -p rust-pty
sleep 10
cargo publish -p rust-expect-macros
sleep 10
cargo publish -p rust-expect
```

---

### Test Utility Binaries

Test utilities (`test-utils/`) are workspace members that provide controlled test fixtures.

#### Purpose

| Utility | Purpose | Example Usage |
|---------|---------|---------------|
| `test-echo` | Simple I/O verification | Basic spawn/expect tests |
| `test-prompt` | Configurable prompts | Shell simulation tests |
| `test-output` | Large/streaming output | Buffer handling tests |
| `test-signals` | Signal handling | SIGWINCH, SIGCHLD tests |
| `test-timing` | Precise timing control | Timeout tests |
| `test-hang` | Intentional hangs | Timeout/kill tests |

#### Cargo.toml Structure

```toml
# test-utils/test-echo/Cargo.toml
[package]
name = "test-echo"
version = "0.0.0"  # Internal only, not published
edition.workspace = true
publish = false    # Never publish to crates.io

[[bin]]
name = "test-echo"
path = "src/main.rs"
```

#### Workspace Configuration

```toml
# Root Cargo.toml
[workspace]
resolver = "3"
members = [
    "crates/*",
    "test-utils/test-echo",
    "test-utils/test-prompt",
    "test-utils/test-output",
    "test-utils/test-signals",
    "test-utils/test-timing",
    "test-utils/test-hang",
]
# Or use glob: "test-utils/*" if all subdirs are crates
```

#### Implementation Timing

Test utilities should be implemented:
- **After:** Core library types are defined (so you know what to test)
- **Before:** Integration tests that use them
- **Parallel with:** Examples and documentation

---

### Workspace Inheritance

Edition 2024 projects should maximize workspace inheritance for consistency and maintainability.

#### `[workspace.package]` — Shared Metadata

```toml
# Root Cargo.toml
[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.85"
license = "MIT OR Apache-2.0"
repository = "https://github.com/org/project"
authors = ["Author Name <email@example.com>"]
categories = ["development-tools"]
keywords = ["automation", "terminal", "pty"]
```

```toml
# Member Cargo.toml
[package]
name = "my-crate"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
authors.workspace = true
# Note: description CANNOT be inherited (must be crate-specific)
description = "This specific crate's description"
```

#### `[workspace.dependencies]` — Shared Dependencies

```toml
# Root Cargo.toml
[workspace.dependencies]
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
thiserror = "2.0"
tracing = "0.1"

# Optional dependencies (for feature gating)
russh = { version = "0.50", optional = true }
```

```toml
# Member Cargo.toml
[dependencies]
tokio.workspace = true
serde.workspace = true
thiserror.workspace = true

# Can add features to workspace dependency
serde = { workspace = true, features = ["rc"] }

# Optional in this crate (feature-gated)
russh = { workspace = true, optional = true }

[features]
ssh = ["dep:russh"]
```

#### `[workspace.lints]` — Shared Lint Configuration

```toml
# Root Cargo.toml
[workspace.lints.rust]
unsafe_code = "warn"
missing_docs = "warn"

[workspace.lints.clippy]
pedantic = "warn"
nursery = "warn"
unwrap_used = "warn"
expect_used = "warn"

# Allow specific pedantic lints that are too noisy
missing_errors_doc = "allow"
missing_panics_doc = "allow"
module_name_repetitions = "allow"
```

```toml
# Member Cargo.toml
[lints]
workspace = true
```

---

### Benchmarking with Criterion.rs

[Criterion.rs](https://bheisler.github.io/criterion.rs/book/getting_started.html) is the de facto standard for Rust benchmarking, providing statistical analysis and HTML reports.

#### Cargo.toml Setup

```toml
# Root Cargo.toml
[workspace.dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

# Crate Cargo.toml
[dev-dependencies]
criterion.workspace = true

[[bench]]
name = "my_benchmark"
harness = false  # Required: disable built-in harness
```

#### Benchmark File Structure

```rust
// benches/my_benchmark.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use my_crate::function_to_benchmark;

fn benchmark_function(c: &mut Criterion) {
    c.bench_function("function_name", |b| {
        b.iter(|| function_to_benchmark(black_box(input)))
    });
}

criterion_group!(benches, benchmark_function);
criterion_main!(benches);
```

#### Running Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench my_benchmark

# Save baseline for comparison
cargo bench -- --save-baseline main

# Compare against baseline
cargo bench -- --baseline main

# View HTML report
open target/criterion/report/index.html
```

#### Configuration Options

```rust
use criterion::{Criterion, SamplingMode};
use std::time::Duration;

fn custom_config() -> Criterion {
    Criterion::default()
        .sample_size(100)                    // Number of samples (min 10)
        .measurement_time(Duration::from_secs(5))  // Time per benchmark
        .warm_up_time(Duration::from_secs(3))      // Warm-up period
        .sampling_mode(SamplingMode::Auto)   // Auto, Linear, or Flat
}

criterion_group! {
    name = benches;
    config = custom_config();
    targets = benchmark_function
}
```

#### CI Integration

```yaml
# .github/workflows/bench.yml
- name: Run benchmarks
  run: cargo bench --all-features -- --noplot

# Optional: Compare with baseline
- name: Compare benchmarks
  run: |
    cargo bench -- --save-baseline pr
    # Use critcmp for comparison if needed
```

**Note:** Use `--noplot` in CI to skip gnuplot dependency. Reports are in `target/criterion/`.

---

### IDE Configuration (rust-analyzer)

Configure [rust-analyzer](https://rust-analyzer.github.io/) for optimal IDE support in workspaces.

#### VS Code Settings

```json
// .vscode/settings.json
{
  "rust-analyzer.cargo.features": "all",
  "rust-analyzer.check.command": "clippy",
  "rust-analyzer.check.allTargets": true,
  "rust-analyzer.procMacro.enable": true,
  "rust-analyzer.cargo.buildScripts.enable": true,
  "rust-analyzer.diagnostics.disabled": [],
  "rust-analyzer.inlayHints.parameterHints.enable": true,
  "rust-analyzer.inlayHints.typeHints.enable": true
}
```

#### rust-analyzer.toml (Experimental)

> **Note:** `rust-analyzer.toml` is still experimental and may change. Use `.vscode/settings.json` for stability.

```toml
# rust-analyzer.toml (workspace root)
# See: https://rust-analyzer.github.io/book/configuration.html

[cargo]
features = "all"
buildScripts.enable = true

[check]
command = "clippy"
allTargets = true

[procMacro]
enable = true
```

#### Workspace-Specific Considerations

For multi-crate workspaces:

```json
// .vscode/settings.json
{
  // Analyze all workspace members
  "rust-analyzer.linkedProjects": [],

  // Or specify specific crates if needed
  // "rust-analyzer.linkedProjects": [
  //   "./crates/my-crate/Cargo.toml"
  // ],

  // Run cargo check on save
  "rust-analyzer.checkOnSave": true,

  // Target directory for rust-analyzer (separate from cargo build)
  "rust-analyzer.cargo.targetDir": true
}
```

**Tip:** For large workspaces, set `"rust-analyzer.cargo.targetDir": true` to use a separate target directory, preventing conflicts with manual `cargo build` commands.

---

### CI/CD with Rust Caching

Use [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache) for faster CI builds.

#### Basic Setup

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: -Dwarnings

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Cache dependencies
        uses: Swatinem/rust-cache@v2
        with:
          # Optional: differentiate caches by job
          prefix-key: "v1-rust"
          # Optional: cache additional directories
          cache-directories: |
            ~/.cargo/bin/

      - name: Check formatting
        run: cargo fmt --all -- --check

      - name: Clippy
        run: cargo clippy --all-features -- -D warnings

      - name: Test
        run: cargo test --all-features
```

#### Advanced Caching Configuration

```yaml
- uses: Swatinem/rust-cache@v2
  with:
    # Prefix for cache key (bump to invalidate all caches)
    prefix-key: "v1-rust"

    # Share cache between jobs with same key
    shared-key: "stable-ubuntu"

    # Cache these additional directories
    cache-directories: |
      ~/.cargo/bin/
      ~/.cargo/registry/index/
      target/

    # Environment variables affecting compilation
    env-vars: |
      CARGO_TERM_COLOR
      RUSTFLAGS

    # Workspaces to cache (default: finds automatically)
    workspaces: |
      . -> target
```

#### Cache Key Components

The cache key is automatically generated from:
- Rust version (rustc release, host, hash)
- `Cargo.lock` / `Cargo.toml` files hash
- `rust-toolchain.toml` hash
- `.cargo/config.toml` hash
- GitHub job ID

#### MSRV Testing

```yaml
jobs:
  msrv:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install MSRV toolchain
        uses: dtolnay/rust-toolchain@master
        with:
          toolchain: "1.85"  # Your MSRV

      - uses: Swatinem/rust-cache@v2
        with:
          prefix-key: "msrv-1.85"

      - name: Check MSRV
        run: cargo check --all-features
```

#### Alternative: cargo-msrv verify

For more sophisticated MSRV verification, use [cargo-msrv](https://github.com/foresterre/cargo-msrv):

```yaml
      - name: Install cargo-msrv
        uses: taiki-e/install-action@cargo-msrv

      - name: Verify MSRV
        run: cargo msrv verify
```

#### Security Scanning

Add [rustsec/audit-check](https://github.com/rustsec/audit-check) for automated security audits:

```yaml
# .github/workflows/security.yml
name: Security Audit

on:
  push:
    paths:
      - '**/Cargo.toml'
      - '**/Cargo.lock'
  schedule:
    - cron: '0 0 * * *'  # Daily at midnight

jobs:
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Security audit
        uses: rustsec/audit-check@v2
        with:
          token: ${{ secrets.GITHUB_TOKEN }}
```

**Features:**
- Creates GitHub issues for new advisories when run on schedule
- Fails CI on security vulnerabilities
- Informational advisories don't affect check status
- Supports `audit.toml` for configuration

#### Faster Tool Installation with cargo-binstall

Use [cargo-binstall](https://github.com/cargo-bins/cargo-binstall) for ~100x faster CI tool installation:

```yaml
      - name: Install cargo-binstall
        uses: cargo-bins/cargo-binstall@main

      - name: Install tools (fast)
        run: |
          cargo binstall --no-confirm cargo-nextest cargo-hack cargo-deny cargo-audit
```

**Why cargo-binstall:**
- Downloads pre-built binaries instead of compiling from source
- ~2 seconds vs ~3+ minutes for tool installation
- Falls back to `cargo install` if no binary available
- Drop-in replacement for most use cases

#### Cross-Compilation Matrix

For multi-platform support, use a build matrix:

```yaml
jobs:
  build:
    strategy:
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
          - target: x86_64-unknown-linux-musl
            os: ubuntu-latest
          - target: x86_64-apple-darwin
            os: macos-latest
          - target: aarch64-apple-darwin
            os: macos-latest
          - target: x86_64-pc-windows-msvc
            os: windows-latest

    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - uses: Swatinem/rust-cache@v2
        with:
          prefix-key: ${{ matrix.target }}

      - name: Build
        run: cargo build --release --target ${{ matrix.target }}
```

**Note:** For `musl` targets on Ubuntu, install `musl-tools`:

```yaml
      - name: Install musl tools
        if: contains(matrix.target, 'musl')
        run: sudo apt-get install -y musl-tools
```

#### Supply Chain Security: SBOM & Provenance

For production deployments requiring [SLSA](https://slsa.dev/) compliance and software bill of materials:

##### SBOM Generation

```bash
# Install SBOM tools
cargo install cargo-cyclonedx cargo-sbom

# Generate CycloneDX SBOM (recommended for Rust)
cargo cyclonedx --format json > sbom.cdx.json

# Alternative: SPDX format
cargo sbom --output-format spdx_json_2_3 > sbom.spdx.json
```

##### CI Integration for SBOMs

```yaml
# .github/workflows/release.yml
- name: Generate SBOM
  run: |
    cargo install cargo-cyclonedx
    cargo cyclonedx --format json > sbom.cdx.json

- name: Upload SBOM as artifact
  uses: actions/upload-artifact@v4
  with:
    name: sbom
    path: sbom.cdx.json
```

##### GitHub Artifact Attestations (SLSA Level 2)

```yaml
# Requires repository setting: Settings > Actions > Artifact Attestations
- name: Build release
  run: cargo build --release

- name: Generate attestation
  uses: actions/attest-build-provenance@v2
  with:
    subject-path: 'target/release/my-binary'
```

##### Verifying Attestations

```bash
# Verify with GitHub CLI
gh attestation verify ./my-binary --repo owner/repo
```

##### cargo-auditable for Binary Auditing

Embed dependency info in binaries for vulnerability scanning:

```toml
# Cargo.toml
[profile.release]
# Enable cargo-auditable (requires cargo-auditable installed)
# Run: cargo auditable build --release
```

```bash
# Install and use
cargo install cargo-auditable cargo-audit

# Build with embedded dependency info
cargo auditable build --release

# Scan binary for vulnerabilities
cargo audit bin target/release/my-binary
```

**SLSA Levels for Rust (current state):**
- **Level 1:** Achievable today with cargo-auditable, cargo-audit, cargo-deny
- **Level 2:** Achievable with GitHub Artifact Attestations + SBOM
- **Level 3+:** Requires isolated build environments (GitHub Actions, reproducible builds)

**Note:** Full SLSA integration in Cargo is tracked in [rust-lang/cargo#12661](https://github.com/rust-lang/cargo/issues/12661).

---

## Troubleshooting

### tree2repo Issues

| Problem | Cause | Solution |
|---------|-------|----------|
| "No such file" | Wrong working directory | Run from PARENT of tree root directory |
| Files not created | Validation failed | Run with `--validate` to see specific errors |
| Wrong nested structure | Tree root not accounted for | Check first line of tree.txt matches expected root |
| Existing files overwritten | Used `--force` | Restore from git: `git checkout -- <file>` |
| Creates nested directory | Inside project dir but tree has root | `cd ..` and run from parent |
| Fewer files than expected | Some already existed | Normal behavior - existing files preserved |

### Cargo Issues

| Problem | Cause | Solution |
|---------|-------|----------|
| "can't find crate" | Workspace not configured | Ensure root Cargo.toml has `[workspace]` with `members` |
| "unresolved import" | Module not declared | Add `mod module_name;` in parent module |
| "feature not found" | Feature not in Cargo.toml | Add to `[features]` section |
| Circular dependency | Crate A uses B, B uses A | Extract shared code to third crate |
| "edition not found" | Edition 2024 not supported | Update to Rust 1.85+ |
| Version conflicts | Different versions in workspace | Use `workspace.dependencies` for shared deps |
| "workspace inheritance" error | Wrong syntax | Use `key.workspace = true`, not `key = { workspace = true }` |

### Feature Flag Issues

| Problem | Cause | Solution |
|---------|-------|----------|
| Feature breaks build | Non-additive feature | Ensure enabling feature doesn't require disabling another |
| "unresolved import" when feature off | Missing `#[cfg(feature = "...")]` | Add cfg guard to import and usage |
| Optional dep always included | Missing `optional = true` | Add `optional = true` to dependency |
| Feature has no effect | Not wired up | Check `[features]` enables the optional deps |

### Proc-Macro Issues

| Problem | Cause | Solution |
|---------|-------|----------|
| "can't use proc-macro crate" | Missing `proc-macro = true` | Add to `[lib]` section in Cargo.toml |
| "macro undefined" | Implementation not exported | Use `#[proc_macro]`, `#[proc_macro_derive]`, or `#[proc_macro_attribute]` |
| Types not found | Proc-macro can only export macros | Move types to separate crate, re-export from main crate |
| Compile errors hard to debug | Generated code issue | Use `cargo expand` to see generated code |

### Workspace Inheritance Issues

| Problem | Cause | Solution |
|---------|-------|----------|
| "field not found in workspace" | Key not in `workspace.package` | Add to root Cargo.toml `[workspace.package]` |
| Lint inheritance not working | Missing `[lints]` section | Add `[lints]\nworkspace = true` to member |
| Dependency not inheriting features | Features in wrong place | Features go in member's usage, not workspace definition |

### Common Mistakes

1. **Forgetting virtual manifest:** Root Cargo.toml needs `[workspace]` section, not `[package]`

2. **Wrong resolver:** Edition 2024 uses `resolver = "3"`, not `"2"`. For virtual workspaces, set explicitly:
   ```toml
   [workspace]
   resolver = "3"
   members = ["crates/*"]
   ```

3. **Duplicate dependency versions:** Use `workspace.dependencies`:
   ```toml
   # Root Cargo.toml
   [workspace.dependencies]
   tokio = { version = "1.0", features = ["full"] }

   # Crate Cargo.toml
   [dependencies]
   tokio.workspace = true
   ```

4. **Missing workspace members:** All crates must be listed (use globs for scalability):
   ```toml
   [workspace]
   members = [
       "crates/*",
       "test-utils/*",
   ]
   ```

5. **Uncommitted Cargo.lock:** Applications should commit Cargo.lock for reproducible builds

6. **Running tree2repo from wrong directory:** Always run from PARENT of tree root

7. **Not using workspace.lints:** Leads to inconsistent lint configuration across crates

8. **Proc-macro crate in wrong order:** Must be compiled before crates that use the macros

---

## Success Criteria

The scaffolding chain is complete when ALL criteria are met:

### Structure Criteria

- [ ] All directories from tree.txt exist
- [ ] All files from tree.txt exist and are non-empty
- [ ] tree.txt and actual structure match exactly
- [ ] No orphaned files (every file in a module, every module in a crate)

### Edition 2024 Criteria

- [ ] `edition = "2024"` in workspace.package (inherited by all crates)
- [ ] `resolver = "3"` in workspace Cargo.toml (or implied by edition)
- [ ] `rust-version = "1.85"` declared for MSRV (in workspace.package)
- [ ] No deprecated patterns (no `mod.rs`, no `extern crate`, no `try!`)

### Workspace Criteria

- [ ] `workspace.package` used for shared metadata
- [ ] `workspace.dependencies` used for shared dependencies
- [ ] `workspace.lints` used for shared lint configuration
- [ ] All crates inherit with `.workspace = true` pattern
- [ ] Glob patterns used for workspace members where appropriate

### Build Criteria

- [ ] `cargo build --all-features` succeeds
- [ ] `cargo build --no-default-features` succeeds
- [ ] `cargo build --release --all-features` succeeds
- [ ] All individual features compile in isolation
- [ ] All feature combinations compile

### Quality Criteria

- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy --all-features -- -D warnings` passes
- [ ] `cargo test --all-features` passes
- [ ] `cargo doc --all-features --no-deps` succeeds with no warnings
- [ ] No `unwrap()` in library code

### Documentation Criteria

- [ ] README.md has usage examples that compile
- [ ] All public items have rustdoc comments
- [ ] ARCHITECTURE.md matches implementation
- [ ] CHANGELOG.md has initial entry
- [ ] Crate READMEs present for docs.rs
- [ ] Optional: README synced with lib.rs docs (use [cargo-rdme](https://github.com/orium/cargo-rdme))

### CI/CD Criteria

- [ ] All workflow files are valid YAML
- [ ] CI pipeline runs successfully on push
- [ ] Swatinem/rust-cache configured for faster builds
- [ ] Security scanning configured (cargo-audit, cargo-deny)
- [ ] Release workflow configured
- [ ] MSRV verification in CI

---

## References

### Prompt Engineering

- [Anthropic: Prompt Engineering Best Practices](https://docs.anthropic.com/en/docs/build-with-claude/prompt-engineering/overview)
- [OpenAI: Prompt Engineering Guide](https://platform.openai.com/docs/guides/prompt-engineering)
- [Lakera: Ultimate Guide to Prompt Engineering 2025](https://www.lakera.ai/blog/prompt-engineering-guide)
- [Prompting Guide](https://www.promptingguide.ai/) — Comprehensive techniques reference

### Rust Project Structure

- [The Rust Book: Cargo Workspaces](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html)
- [The Cargo Book: Workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html)
- [Rust Edition Guide: 2024 Edition](https://doc.rust-lang.org/edition-guide/rust-2024/index.html)
- [Cargo Resolver v3 (MSRV-aware)](https://doc.rust-lang.org/edition-guide/rust-2024/cargo-resolver.html)
- [Vivek Shukla: How I Use Cargo Workspace](https://vivekshuk.la/tech/2025/use-cargo-workspace-rust/)
- [cargo-autoinherit: DRY up workspace dependencies](https://mainmatter.com/blog/2024/03/18/cargo-autoinherit/)

### Procedural Macros

- [The Rust Reference: Procedural Macros](https://doc.rust-lang.org/reference/procedural-macros.html)
- [Nine Rules for Creating Procedural Macros](https://towardsdatascience.com/nine-rules-for-creating-procedural-macros-in-rust-595aa476a7ff/)
- [LogRocket: Procedural Macros in Rust](https://blog.logrocket.com/procedural-macros-in-rust/)
- [Generalist Programmer: proc-macro2 Guide](https://generalistprogrammer.com/tutorials/proc-macro2-rust-crate-guide)

### CI/CD and Caching

- [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache) — Smart caching for Rust projects
- [dtolnay/rust-toolchain](https://github.com/dtolnay/rust-toolchain) — Install Rust toolchain in CI
- [taiki-e/install-action](https://github.com/taiki-e/install-action) — Install Rust tools in CI
- [cargo-binstall](https://github.com/cargo-bins/cargo-binstall) — Fast binary installation (~100x faster than cargo install)
- [GitHub Actions for Rust (actions-rs)](https://github.com/actions-rs/meta)
- [Optimizing Rust CI with Caching](https://jwsong.github.io/blog/ci-optimization/)
- [Infinyon: GitHub Actions Best Practices for Rust](https://www.infinyon.com/blog/2021/04/github-actions-best-practices/)

### Testing and Benchmarking

- [cargo-nextest](https://nexte.st/) — Fast test runner with better output
- [cargo-hack](https://github.com/taiki-e/cargo-hack) — Test feature combinations exhaustively
- [Criterion.rs](https://bheisler.github.io/criterion.rs/book/getting_started.html) — Statistics-driven benchmarking
- [trybuild](https://github.com/dtolnay/trybuild) — Test compile-fail cases

### Supply Chain Security

- [cargo-vet Documentation](https://mozilla.github.io/cargo-vet/)
- [cargo-deny Documentation](https://embarkstudios.github.io/cargo-deny/)
- [cargo-auditable](https://github.com/rust-secure-code/cargo-auditable) — Embed dependency info in binaries
- [rustsec/audit-check](https://github.com/rustsec/audit-check) — GitHub Action for security audits
- [cargo-cyclonedx](https://github.com/CycloneDX/cyclonedx-rust-cargo) — Generate CycloneDX SBOMs
- [cargo-sbom](https://crates.io/crates/cargo-sbom) — Generate SPDX/CycloneDX SBOMs
- [SLSA Specification v1.0](https://slsa.dev/spec/v1.0/)
- [GitHub Artifact Attestations](https://docs.github.com/en/actions/security-for-github-actions/using-artifact-attestations)
- [rust-lang/cargo#12661: SLSA Integration](https://github.com/rust-lang/cargo/issues/12661) — Tracking issue

### Linting and Formatting

- [Clippy Configuration](https://doc.rust-lang.org/clippy/configuration.html)
- [Clippy Lint List](https://rust-lang.github.io/rust-clippy/master/index.html)
- [Workspace Lints (RFC 3389)](https://rust-lang.github.io/rfcs/3389-manifest-lint.html)
- [coreyja: clippy::pedantic and Workspace Lints](https://coreyja.com/til/clippy-pedantic-workspace)

### MSRV Policy

- [cargo-msrv: Find MSRV](https://github.com/foresterre/cargo-msrv)
- [RFC 3537: MSRV-aware Resolver](https://rust-lang.github.io/rfcs/3537-msrv-resolver.html)
- [kube.rs Rust Version Policy](https://kube.rs/rust-version/)
- [API Guidelines: MSRV Discussion](https://github.com/rust-lang/api-guidelines/discussions/231)

### IDE and Editor Support

- [rust-analyzer](https://rust-analyzer.github.io/) — LSP server for Rust
- [rust-analyzer Configuration](https://rust-analyzer.github.io/book/configuration.html) — Configuration options
- [VS Code Rust Extension](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

### Documentation

- [cargo-rdme](https://github.com/orium/cargo-rdme) — Sync README with lib.rs docs
- [cargo-doc2readme](https://github.com/msrd0/cargo-doc2readme) — Alternative README generator
- [rustdoc Book](https://doc.rust-lang.org/rustdoc/) — Official rustdoc documentation

### Tooling

- [git-cliff: Changelog Generator](https://git-cliff.org/)
- [Dependabot for Rust](https://docs.github.com/en/code-security/dependabot)
- [Renovate](https://docs.renovatebot.com/modules/manager/cargo/) — Alternative to Dependabot

---

## Changelog

### 1.3.0 (2025-12-30)

**Added:**
- **Prompt 8:** Optional cargo-nextest integration for faster test execution with doctest caveat
- **Prompt 10:** Exhaustive feature testing with cargo-hack (`--feature-powerset`, `--depth`, `--no-dev-deps`)
- **Special Cases:** Benchmarking with Criterion.rs (setup, configuration, CI integration)
- **Special Cases:** IDE Configuration with rust-analyzer (VS Code settings, experimental rust-analyzer.toml)
- **CI/CD:** Security scanning with rustsec/audit-check workflow
- **CI/CD:** Faster tool installation with cargo-binstall (~100x speedup)
- **CI/CD:** Cross-compilation matrix example (Linux, macOS, Windows targets)
- **CI/CD:** cargo-msrv verify as alternative MSRV verification
- **CI/CD:** Supply chain security section with SBOM generation (cargo-cyclonedx, cargo-sbom)
- **CI/CD:** GitHub Artifact Attestations for SLSA Level 2 compliance
- **CI/CD:** cargo-auditable for binary vulnerability scanning
- **Success Criteria:** Optional cargo-rdme for README/lib.rs synchronization
- **References:** New sections for Testing/Benchmarking, IDE Support, Documentation tools
- **References:** Added cargo-nextest, cargo-hack, Criterion.rs, cargo-binstall, cargo-cyclonedx, cargo-sbom, rustsec/audit-check, cargo-rdme, rust-analyzer, Renovate

**Improved:**
- Supply chain security references updated with practical implementation guidance
- SLSA levels now include concrete achievability for Rust projects
- References reorganized with dedicated Testing/Benchmarking and IDE sections

### 1.2.1 (2025-12-30)

**Fixed:**
- ASCII diagrams reformatted to 73-character fixed width per formatting rules
- Border alignment corrected (all right `│` characters now align properly)
- Inner box padding standardized across all diagrams
- Trailing inline content padding (Rule 15): "→ Tests" line now has consistent padding with box lines
- Added `<!-- Width: 73 chars -->` comments for maintainability

### 1.2.0 (2025-12-30)

**Added:**
- **Parallel Workstreams section** with efficiency analysis and agent dispatch examples
- **Special Cases section** covering:
  - Procedural macro crates (constraints, Cargo.toml, best practices, publishing order)
  - Test utility binaries (purpose, configuration, timing)
  - Workspace inheritance (`workspace.package`, `workspace.dependencies`, `workspace.lints`)
  - CI/CD with Swatinem/rust-cache (setup, configuration, MSRV testing)
- Proc-macro implementation order in Prompt 6 dependency analysis
- Test utilities category in Prompt 6 file categorization
- `workspace.lints` to workspace requirements in Prompt 1
- Workspace inheritance examples in Prompt 7
- Proc-macro troubleshooting section
- Workspace inheritance troubleshooting section
- cargo-auditable to supply chain security references
- Comprehensive references for proc-macros, linting, MSRV policy

**Improved:**
- Prompt 1 now includes `workspace.lints` requirement
- Prompt 2 validation includes `workspace.lints` check
- Prompt 6 includes proc-macro crates and test utilities in categorization
- Troubleshooting expanded with proc-macro and workspace inheritance issues
- Success criteria includes workspace inheritance verification
- References reorganized into more specific categories

**Fixed:**
- Workspace members example now uses glob patterns for scalability

### 1.1.0 (2025-12-30)

**Breaking Changes:**
- Renumbered prompts: 9.5 → 10, 10 → 11

**Added:**
- Overview section with visual workflow diagram
- Critical directory context warnings throughout
- Detailed tree2repo behavior documentation
- Workspace.dependencies best practices
- Edition 2024 specific criteria
- Feature flag verification phase (Prompt 10)
- Parallelization guidance for implementation
- Expanded troubleshooting tables
- CI caching references
- Concrete examples for directory context

**Improved:**
- Prerequisites table with notes column
- All prompts have clearer structure with horizontal rules
- Exit criteria explicit for each prompt
- Commit message examples with full format
- References section reorganized by topic

**Fixed:**
- Prompt numbering (removed .5 suffix)
- Added missing stub verification commands

### 1.0.0 (2025-12-30)

- Initial release
- Complete prompt chain from structure to implementation
- Troubleshooting guide
- Success criteria checklist
- Reference links to authoritative sources
