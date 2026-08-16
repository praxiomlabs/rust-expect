# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Findings from a source architecture review, burned down one at a time. Every
fix carries a regression test that was run against the unfixed code first.

### Added

- **`Session` has an event stream.** `add_event_subscriber` observes
  `SessionEvent::{Output, Input, Resize, StateChanged, Matched, Error}`.
  `Input` had no observation point at all before: `send()` had no hook, so a
  transcript could never record what was typed. Output taps are unchanged and
  are now one kind of subscriber, sharing registration order with the rest.

- **Built-in observers attach to that stream**: `Session::attach_recorder`,
  `attach_redacted_recorder` (feature `pii-redaction`) and `attach_metrics`.
  `Recorder`, `SessionMetrics` and `StreamingRedactor` were previously
  unreachable from a session. Redaction sits between the stream and the
  transcript and nowhere else, so a caller expecting on a password prompt still
  matches it while the password stays out of the transcript.

- **`Session::shutdown()`** — ask the child's process group to exit, wait
  `config.timeout.close`, then kill it and reap. The graceful counterpart to
  dropping the session.

- **`Session::detach()`** — give up ownership so the child outlives the session.

- **`SessionState::Eof` and `SessionState::Failed(ErrorKind)`.** A session that
  had reached EOF, or that could not be read from at all, previously went on
  reporting itself `Running`.

- `Histogram::sum()`, and `StreamingRedactor::redactor()`.

### Changed

- **Breaking: dropping a `Session` kills the child's process group.** Closing
  the PTY master hangs up the child's controlling terminal, and that `SIGHUP`
  cleans up an ordinary child — but a child that ignores `SIGHUP` outlived its
  session with nothing holding it and nothing that would ever reap it. Use
  `Session::detach()` to keep the previous behaviour for a child you mean to
  outlive its session.

- **Breaking: `signal()` and `kill()` deliver to the child's process group**,
  which is what a terminal does — Ctrl-C signals the foreground group, not one
  process. A shell's background jobs previously survived `kill()`. Guarded:
  a child that does not lead a group of its own is signalled alone.

- **Breaking: `Session::interact()` takes `&mut self`** and is now the session's
  read driver rather than a second one. Output it reads lands in the session's
  buffer, its writes are the session's writes, and EOF or a read failure moves
  the session's state, so a caller that interacts and then expects sees what
  happened in between. `InteractBuilder::with_buffer_size` is gone — the
  session's buffer bounds the interaction now. Output hooks rewrite what the
  terminal shows, not what the patterns match.

- **Breaking: `Session::transport()` is removed.** Handing out the transport is
  how a second reader gets built; process control no longer travels through the
  transport lock, so it is not needed for that either.

- **Breaking: `Session::set_state` is removed** and `SessionState` is
  `#[non_exhaustive]`. One state machine owns every transition.

- **Breaking: `Session::pid()` and `is_running()` return `Option`.** They
  previously fabricated `0` and `true` when they could not acquire a lock, which
  a caller could not distinguish from a real answer.

- **Breaking: removed public items with no callers** — `LifecycleManager`,
  `LifecycleEvent`, `LifecycleCallback`, `ShutdownConfig`, `ShutdownStrategy`
  (a second, dead lifecycle model), the `Signal` enum,
  `DialogExecutor::step_pattern` and `DialogExecutor::execute_step_sync`.

- **Breaking: configuration that nothing read is gone, and what remains is
  honoured.** Every field of `SessionConfig` and `InteractionMode` now has a
  reader.
  - Removed: `LoggingConfig` and `LogFormat` (`attach_recorder` is the API),
    `EncodingConfig`, `Encoding` and `EncodingErrorHandling`, `InteractConfig`
    and `InteractHook` (the third place interact mode was configured),
    `BufferConfig::ring_buffer` (the buffer is always a ring; there was no
    other mode for `false` to select), `TimeoutConfig::spawn` (the spawn
    completes on its first poll, so no timeout around it can fire), and
    `SessionBuilder::{logging, log_to_file, encoding}`.
  - `BufferConfig::search_window` now bounds the matcher's search to the tail
    of the buffer, as documented.
  - `SessionConfig::delay_before_send` is now waited before every scripted
    send. **Its default is now zero, not 50 ms**: nothing read it before, so
    zero is what every caller has always observed. Keystrokes forwarded by
    `interact()` are not delayed.
  - `InteractionMode::local_echo` echoes keystrokes to the terminal, and
    `InteractionMode::crlf` (default on) rewrites a bare LF from the child as
    CRLF for the terminal — a no-op for a PTY child, which already sends CRLF,
    and the fix for stair-stepped output over any transport that does not.
    `InteractionMode::{buffer_size, exit_char, escape_char}` are removed;
    the escape sequence is `InteractBuilder::with_escape`.

- **Breaking: one screen model, one multi-session model.**
  `rust_expect::session::ScreenBuffer` (and its `Position`, `Region`, `Cell`,
  `CellAttributes`, `Color`) is removed — `screen::ScreenBuffer` behind the
  `screen` feature is the one the session uses. `SessionGroup`,
  `GroupBuilder`, `GroupManager` and `GroupResult` are removed: they held no
  `Session` at all, only labels and a `String`; `MultiSessionManager` is the
  multi-session model.

- **Breaking: removed the empty `Backend` trait** (zero implementations; the
  capability traits `ProcessControl`, `Resizable` and `ChildExit` are what
  backends implement) and the `AsyncPty` / `WindowsAsyncPty` /
  `WindowsPtyHandle` process pass-throughs (`signal`, `kill`, `is_running`,
  `try_wait`), which had no callers. `Session::signal` and `Session::kill`
  are the API.

- **Breaking: `Dialog` no longer starts at the first *named* step.** A dialog
  whose opening steps were unnamed began part-way through itself.

- **Breaking: the `patterns!` and `dialog!` macros expand against the real
  runtime API.** Neither had ever compiled in any program; both are now covered
  by compile fixtures in CI.

- A read that fails with a non-EOF error now moves the session to `Failed` and
  makes it unwritable, rather than leaving it reporting a healthy session.

- `send` after EOF is rejected by the state machine rather than by the transport,
  so the error is the same on every backend.

### Fixed

- **Concurrently spawned sessions inherited each other's PTY slave fd.** The
  three stdio descriptors were duplicated with `dup`, which does not carry
  `FD_CLOEXEC`, so any process forked by another thread in that window inherited
  them. A session's master then never reached EOF while an unrelated child held
  its slave, hanging `wait`, `wait_timeout` and `expect_eof` for as long as that
  sibling lived. Six sessions spawned at once produced six children each holding
  two to four foreign pts devices.

- **Pattern matching consumed the wrong bytes after invalid UTF-8.** Patterns
  matched against lossily-decoded text while the buffer was consumed by raw byte
  offset, and the two spaces drifted two bytes per replacement character.

- **`interact()` dropped any output chunk that ended mid-character**, silently,
  from the buffer its own patterns matched against — routine, since a PTY read
  ends wherever the kernel buffer does.

- **`interact()` spun at full CPU when its terminal input was closed** (under
  `< /dev/null`, or an exhausted pipe): 3.2 million reads in 300ms. Its
  configured timeout was also never something the loop waited on, and only
  appeared to work because that spin kept waking it.

- **Trimming the interaction buffer could panic** by slicing a `String` at a raw
  byte offset, as could `screen::query`'s `find_all`.

- **`screen::query` reported byte offsets as column indices**, so any multibyte
  character earlier in a row shifted every reported column after it.

- **Dialog steps advanced by name rather than position**, so every unnamed step
  resolved back to step 0, and an unknown branch target reported success.

- **The mock transport never woke a parked read**, ignored scripted delays, and
  dropped a scenario step's `expect`, so several tests were passing for the
  wrong reason.

- **SSH: a channel message carrying no bytes was read as EOF**, ending the
  session at the first such message.

- **`StreamingRedactor` panicked on multibyte input**, and `Histogram::observe`
  accumulated float bit patterns with integer addition — `1.5 + 2.25` read back
  as `NaN`.

- **`tests/integration` was never compiled.** No test root declared the module,
  so eleven tests had rotted against an API they no longer matched while the
  suite stayed green.

- Windows: a failed `AssignProcessToJobObject` no longer leaves the crate
  holding a job handle whose kill-on-close guarantee is absent.


## [0.6.0] - 2026-07-31

### Changed

- **Windows (behaviour change): `SessionConfig::line_ending` now defaults to `Cr`
  instead of `Lf`.** `ConPTY` does not merely disagree with a bare LF — it discards
  it: a lone `\n` written to the pseudoconsole completes no line read, queues
  nothing, is not echoed, and does not reach the child even as a key event. So
  `send_line` on Windows previously sent a payload followed by an inert terminator
  and could never submit a line, against a Rust child, `cmd.exe`, or
  `powershell.exe` alike. The default now tracks the terminator a terminal sends
  for the ENTER key, which is not the platform's text-file convention. Unix is
  unchanged (`Lf`). Callers who were explicitly setting `line_ending` are
  unaffected; anyone relying on the Windows default to be `Lf` now gets `Cr`. To
  normalise *text* to CRLF, use `encoding::LineEndingStyle`, which is a separate
  concern and unchanged. (#50)

- **Windows (behaviour change): `SessionBuilder::windows_line_endings()` now sets
  `Cr` instead of `CrLf`**, and therefore so do `QuickSession::cmd()` and
  `QuickSession::powershell()`. Its sibling `unix_line_endings()` sets `Lf`, which
  is the Unix ENTER rather than a text convention, so the pair means "the
  terminator this platform's terminal sends for ENTER" and `CrLf` was the
  inconsistent member. `CrLf` does submit a line today, but only because conhost
  swallows the trailing LF; against a child with `ENABLE_LINE_INPUT` disabled an LF
  that did arrive would submit a second ENTER. Ask for CRLF explicitly with
  `.line_ending(LineEnding::CrLf)` if you need it. (#50)

### Fixed

- **Windows: a `ConPTY` child no longer receives the host's redirected standard
  handles.** `create_startup_info` left `dwFlags` at `0`, and the `hStd*` fields are
  honoured only when `STARTF_USESTDHANDLES` is set. Without it Windows *duplicates*
  the parent's standard handles into a console-subsystem child — a legacy path that
  `bInheritHandles = FALSE` does not disable, and which `ConPTY`'s startup only
  undoes for copied *console* handles. A host whose own stdio was redirected to a
  pipe or file (a service, a daemon, a CI runner, `host.exe > log.txt`, or a test
  binary under `cargo test`) therefore leaked those handles to the child: its output
  bypassed the pseudoconsole and never reached `WindowsPtyMaster`, and its input
  came from the host's stream rather than the PTY. The flag is now set with all
  three handle fields left NULL, which is how Windows Terminal spawns `ConPTY`
  children. Reported by @DamoyY. (#46)

  This also explains a symptom previously recorded in this repo's own Windows tests
  as unavoidable conhost behaviour ("does not forward a child's rendered output to
  the read pipe on some configurations"). It was this bug; those tests now assert on
  real child output rather than only the `ConPTY` handshake frame.

- **Windows: `InteractContext::send_line` ignored the configured line ending**,
  hardcoding `'\n'`. Combined with the above, the interactive path could not submit
  a line on Windows at all and no configuration could work around it. It now uses
  the platform default. (#50)

- **`screen`: the screen buffer now defers wrapping at the right margin, per
  VT100/xterm.** `write_char` advanced the cursor to the next row the instant the
  last column was filled. A real terminal instead raises a pending-wrap flag and
  leaves the cursor on the final column, taking the wrap only when the next
  printable character arrives — and any explicit cursor movement in between
  cancels it. The distinction is invisible for line-oriented output but
  load-bearing for full-screen TUIs, which emit lines exactly `cols` wide (box
  borders, horizontal rules) followed by CRLF: wrapping eagerly burned an extra
  row on every one of them, so the emulated screen's row accounting drifted from
  the application's and subsequent absolute cursor addressing overwrote the wrong
  rows, leaving stale text visible underneath a corrupted viewport. Any consumer
  driving a full-width TUI through `Screen` was affected. `cursor_mut()` clears
  the flag, covering CR, LF, CUP and the other positioning paths that reach the
  cursor through it; `goto()`, `restore_cursor()` and `resize()` clear it
  explicitly. DECTCEM show/hide now routes through a new
  `ScreenBuffer::set_cursor_visible()` so toggling cursor visibility cannot
  cancel a pending wrap.

### Added

- **`ScreenBuffer::set_cursor_visible()`** — set cursor visibility without
  touching its position or the pending-wrap state.

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
