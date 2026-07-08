# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.0] - 2026-07-08

A pre-1.0 release centered on the correctness of the Unix spawn path: the
hand-rolled `fork`/`exec` is replaced by `tokio::process` (via `rust-pty`),
together with a batch of PTY robustness fixes — terminal sizing, close-on-exec,
PID-reuse-safe signals, resilient allocation, and reliable final-output capture
on macOS and Windows. Includes low-level breaking API changes; the high-level
`Session`/`SyncSession` APIs are unchanged.

### Added

- Re-export `PersistentPattern` and `HandlerAction` at the crate root (they were
  previously reachable only via `rust_expect::expect::…`), so before/after
  ambient patterns can be built and registered without the longer module path.
  (#41)

### Changed

- **Unix spawn migrated off the hand-rolled `fork`/`exec` onto `tokio::process`
  (via `rust-pty`).** The previous Unix `Session` spawn did non-async-signal-safe
  work between `fork()` and `execvp` (environment mutation, heap allocation),
  which is unsound under a multi-threaded Tokio runtime — the default
  `#[tokio::main]`. The Unix transport now wraps rust-pty's
  `UnixPtyMaster`/`UnixPtyChild`, whose only between-fork-and-exec work is
  async-signal-safe `setsid` + `TIOCSCTTY`. This deletes a large block of
  `unsafe` and resolves intermittent empty-output spawns observed under
  multi-threaded load. `Session`/`SyncSession` public APIs are unchanged.
- **API (Unix, pre-1.0 breaking):** the low-level re-exported `PtyHandle` now
  wraps rust-pty's master/child instead of a raw fd, and its `wait()` method was
  removed (wait via `Session`/`SyncSession`). As part of the PID-reuse guard,
  `AsyncPty::signal`/`kill` now take `&mut self` (they perform an authoritative
  reap check), and the unguarded low-level `PtyHandle::signal`/`kill` methods
  have been removed — signal a child through `Session`/`SyncSession` instead
  (whose `signal`/`kill` keep their `&self` signatures). A null byte in the
  command or an argument is still rejected, now with std's "nul byte found in
  provided data" message rather than "... contains null byte".

### Fixed

- **Before/after ambient patterns now work.** `expect` never checked
  after-patterns (`PatternManager::add_after` was inert — the loop only ran
  before-patterns), and a before-pattern `Respond` re-fired on every poll
  because the triggering match was never consumed (e.g. a `password:` handler
  resent the password repeatedly). The expect loop now runs after-patterns as a
  fallback once the explicit patterns fail, and consumes the triggering match
  for `Respond`/`Return` (both before and after) so it cannot re-fire on the
  next poll or the next `expect` call against the same buffer.

- **`Pattern::Bytes(n)` now matches.** It was unreachable — `Matcher::try_match`,
  `Matcher::try_match_any`, and `Pattern::matches` all returned `None` for it, so
  `expect(Pattern::bytes(n))` never matched and blocked until the timeout. It now
  matches once at least `n` bytes are buffered and consumes the first `n`. (#28)

- **Spawning a non-existent program now returns a spawn error** instead of an
  apparently-successful `Session` whose child immediately exits. The migrated
  `tokio::process` path reports `exec` failures that the old hand-rolled fork
  path silently swallowed (the parent returned a pid before the child's `exec`
  failed).

- **Apply the configured terminal size at spawn.** The PTY was allocated with no
  initial window size, so a freshly spawned child saw a 0x0 terminal until an
  explicit `resize` — a full-screen (TUI) child would render into nothing. The
  PTY is now allocated at the session's configured dimensions, so `stty size`
  (and curses apps) report the right size from the first byte.

- **Set close-on-exec on the PTY master.** The master fd lacked `FD_CLOEXEC`, so
  under concurrent spawning it could leak into an unrelated child forked by
  another session. The master is now marked close-on-exec at allocation: atomic
  on Linux (`openpt` with `CLOEXEC`), and best-effort on macOS/BSD, whose
  `posix_openpt` has no atomic `O_CLOEXEC`, so a follow-up `fcntl` leaves a small
  open→`fcntl` window. It guards the master only.

- **Guard `signal()`/`kill()` against PID reuse.** After a child exits and is
  reaped, the OS can recycle its PID; the previous code called `libc::kill`
  unconditionally, so a later `Session::signal`/`kill` could land on an
  unrelated process. Both now perform an authoritative non-blocking reap check
  before signalling and return `SessionClosed` if the child has already exited;
  a raw `ESRCH` maps to the same. Genuine delivery failures (e.g. `EPERM`) are
  still surfaced as `Io`. Signalling a live child is unchanged.

- **Resilient PTY allocation on macOS/BSD.** macOS caps system-wide PTYs at
  `kern.tty.ptmx_max` (511 by default), far below Linux's dynamic allocation, so
  under heavy concurrent spawning PTY allocation can transiently fail (the BSD
  PTY-exhaustion code is `ENXIO`, "Device not configured") even though a slot
  frees moments later. `Session::spawn` now retries allocation with a short
  bounded backoff and, on a genuine failure, surfaces the underlying OS error
  instead of the opaque "Failed to open PTY". This removes intermittent
  `PtyAllocation` failures seen when running the test suite — or an app driving
  many sessions — in parallel on macOS. The retry fires on any allocation
  failure rather than a specific errno: our arguments are always valid, so the
  only realistic failure is exhaustion, and retrying unconditionally is simpler
  and more robust.

- **Recover a fast-exiting child's final output on Windows.** The ConPTY read
  path short-circuited to EOF as soon as the shared `open` flag was cleared,
  which the exit watcher does the instant the child exits — so bytes conhost had
  already written to the output pipe but that had not yet been read were
  discarded. Reads no longer gate on `open`; the pipe is drained until `ReadFile`
  reports `ERROR_BROKEN_PIPE`. Writes still fail with `BrokenPipe` after exit.
  (#28)

- **Capture a fast-exiting child's final PTY output on macOS.** A child spawned
  via `tokio::process` could lose its final output — `expect` returned
  `Eof { buffer: "" }` — because macOS discards the master's still-buffered bytes
  when the last slave fd closes around child exit, before the session's first
  read observes them. A dedicated drain (in `rust-pty`), started before the child
  spawns, reads the master into a userspace buffer the instant bytes arrive, so
  the output survives teardown. macOS-only; Linux and Windows are unaffected.
  (#40)

## [0.4.0] - 2026-07-02

Adds opt-in screen scrollback with a lossless streaming callback, and relaxes
the `tokio` requirement so downstreams can use a newer runtime. Additive; no
breaking changes.

### Added

- **Screen scrollback history** (`screen` feature). `Screen::with_scrollback`
  retains rows that scroll off the top of the viewport (bounded, oldest
  dropped first), readable via `Screen::scrollback()` and
  `Screen::full_text()`. `Screen::on_line_scrolled_out` streams each evicted
  row as it finalizes, for lossless capture independent of the ring size. A new
  public `Row` type exposes both `text()` and `cells()`.
  `Session::attach_screen_with_scrollback` and
  `Session::on_screen_line_scrolled_out` thread it through the session. Opt-in:
  `scrollback_lines = 0` preserves the previous behavior with no extra
  allocation. (#25)

### Changed

- Relaxed the `tokio` dependency requirement from `~1.49` to `1.49`
  (`>=1.49.0, <2.0.0`), so a downstream project can use a newer tokio in the
  same dependency graph. (#24)

## [0.3.0] - 2026-06-28

Fixes silently-ignored `working_dir` / `inherit_env`, a Windows compile
break, and a cross-platform post-exit write bug, and upgrades `russh` to
patch two DoS advisories. Contains one breaking change (see below); pre-1.0
semver allows breaking changes in minor versions
([RELEASING.md §Version Numbering](RELEASING.md#version-numbering)).

### Breaking changes

- `backend::PtyConfig` is now `#[non_exhaustive]` and gained a
  `working_directory: Option<PathBuf>` field. External code constructing it
  with a struct literal must switch to `PtyConfig::default()` (or the
  `SessionBuilder` path); future field additions will no longer be breaking.

### Fixed

- **`SessionBuilder::working_directory` was silently ignored on spawn**: the
  configured `working_dir` was dropped between `PtyConfig::from` and `execvp`,
  so the child ran in the parent's directory. The child now `chdir`s to the
  configured directory before exec on Unix and sets it on the Windows ConPTY
  path. A non-existent directory returns `SpawnError::InvalidWorkingDir`
  instead of being ignored.
- **`SessionConfig::inherit_env(false)` was a silent no-op**: the flag was
  never read, so the child always inherited the full parent environment. It
  now produces a cleared environment, leaving only explicit `env` overrides.
- **The crate failed to compile on Windows**: the Windows `ChildExit` impl
  matched a `#[cfg(unix)]`-only `ExitStatus::Signaled` variant. It now maps
  ConPTY's `Terminated(code)` to `ProcessExitStatus::Exited`.
- **A write to an already-exited child could buffer indefinitely on Linux and
  Windows** instead of reporting closure (only macOS surfaced it). `send` now
  reports `SessionClosed` once the child has exited, regardless of whether
  `wait()` was called first.

### Security

- Upgraded `russh` 0.56 → 0.61.2, patching RUSTSEC-2026-0153 and
  RUSTSEC-2026-0154 (denial-of-service via unbounded allocation on the SSH
  agent / transport paths; the latter was network-reachable in 0.56). The
  refreshed dependency tree also clears the aws-lc-rs, cmov, and libcrux-sha3
  advisories the old tree carried.

## [0.2.0] - 2026-05-20

Second public release. Contains breaking changes versus 0.1.0; see the
**Breaking changes** section below. Pre-1.0 semver allows breaking changes
in minor versions ([RELEASING.md §Version Numbering](RELEASING.md#version-numbering)).

### Breaking changes

- `ExpectError` is now `#[non_exhaustive]`. Exhaustive `match` arms over it
  without a wildcard will no longer compile; add `_ => {}` or handle the
  new variants explicitly.
- New `ExpectError` variants: `ScreenNotAttached` (screen-aware methods
  called without `attach_screen`) and `InvalidInput { api, reason }`
  (caller-supplied input rejected before any I/O).
- `screen::AnsiParser::parse(byte)` signature changed from
  `Option<ParseResult>` to `[Option<ParseResult>; 2]`. The second slot is
  populated only on UTF-8 malformed-sequence recovery so both the `U+FFFD`
  replacement and the recovery byte's own effect are emitted in the correct
  visual order. Callers iterate via `.into_iter().flatten()`.
  `Screen::process` already does this; direct `AnsiParser` consumers need
  a one-line change.
- `backend::PtyConfig` gained an `env: HashMap<String, String>` field used
  to plumb `SessionBuilder::env()` values through to the spawned child.
  Construction via `PtyConfig { .. }` literal (rather than the builder /
  `Default::default()`) now requires the new field. `#[non_exhaustive]`
  prevents this kind of break going forward.
- `Session::add_output_tap` now returns a `TapId` (was `()`). Existing call
  sites that ignore the return value continue to compile; sites that want
  to deregister later capture the id.

### Changed

- Updated development toolchain to Rust 1.92
- MSRV remains at 1.88 for Edition 2024 and let chains support
- `justfile` MSRV variable synced to 1.88 (was 1.85, drifted from `Cargo.toml`)
- `rust-toolchain.toml` header clarifies the 1.92 pin is a dev convenience and
  does not raise the published MSRV
- Removed unused `crossterm` dependency from `rust-expect` and the workspace
- `.cargo/audit.toml` advisory ignore list synced with `deny.toml` so both
  security tools agree (eight transitive russh-stack advisories with the
  same documented rationale appear in both files)

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
- `ExpectError::ScreenNotAttached` distinguishes "caller forgot to call
  `attach_screen`" from a runtime substring miss; the screen-aware expect
  methods now return this variant instead of conflating it with
  `PatternNotFound`. `ExpectError` itself is now `#[non_exhaustive]` so
  adding further variants is non-breaking.
- `Session::screen()`'s return type is `Option<&Arc<Mutex<Screen>>>`;
  `Screen::revision() -> u64` is a cheap monotonic counter that bumps
  once per `process()` call. `wait_screen_stable` polls this counter
  instead of materializing `screen.text()` every 50 ms.
- `TapId` is backed by `u64` and uses a non-wrapping `+= 1` increment so a
  hypothetical exhaustion would surface as a loud panic instead of silently
  colliding with a still-registered tap. Implements `fmt::Display` for
  ergonomic logging in downstream observability.
- The screen `AnsiParser::parse(byte)` signature changed from
  `Option<ParseResult>` to `[Option<ParseResult>; 2]` to correctly emit
  both `U+FFFD` and the recovery byte's own result when a malformed
  UTF-8 sequence is interrupted, in the right visual order. Callers
  iterate via `.into_iter().flatten()`.

### Security notes

- `apply_env_in_child` (`backend/pty.rs`) calls `setenv`/`unsetenv`
  between `fork` and `exec`. These functions are not async-signal-safe
  (they allocate), so the call is technically only sound in
  single-threaded contexts. This codebase forks before any tokio worker
  threads exist, which preserves the invariant **today**. If the
  workspace ever switches to a runtime that pre-spawns workers, or if
  spawning is moved into a multi-threaded context, this assumption
  breaks — callers must either re-introduce a single-threaded fork
  helper or switch to `posix_spawn`/`execve(envp)`.

### Release-prep polish

- README quick-start example now compiles against the real API
  (`Session::spawn(prog, &[]).await?`, `send_line`, `result.matched` field)
- Module-level doctests in `lib.rs`, `session.rs`, `prelude.rs`, and several
  others converted from `ignore` to `no_run` so `cargo test --doc` now
  compile-checks them. SSH/interact snippets that reference outdated
  example-only APIs remain `ignore`d pending an example rewrite.
- `ROADMAP.md` refreshed to reflect the current release-prep state and
  the as-shipped feature set, including the TUI-driving primitives
- All `unsafe` blocks across the workspace carry explicit SAFETY comments
  describing the invariant the caller relies on
- Production-path `.unwrap()` calls in `windows::async_adapter`,
  `util::backpressure::time_until_reset`, `util::zerocopy::freeze`, and
  `transcript::asciicast` escape parsing have been replaced with
  `.expect(...)` carrying the local invariant
- `ARCHITECTURE.md` header refreshed with a navigation note pointing first
  readers at `README.md` and rustdoc

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
