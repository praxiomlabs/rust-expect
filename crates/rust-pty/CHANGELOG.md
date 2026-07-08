# Changelog

All notable changes to `rust-pty` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Changes before 0.5.0 predate this file; see the git history.

## [Unreleased]

## [0.5.0] - 2026-07-08

Hardens the Unix spawn and PTY I/O paths and adds reliable final-output capture
on macOS and Windows. Released together with the rest of the workspace.

### Changed

- **Unix spawn runs the child via `tokio::process`.** The only work between
  `fork()` and `exec` is async-signal-safe (`setsid` + `TIOCSCTTY`), so spawning
  is sound under a multi-threaded Tokio runtime. `try_wait` now reaps through the
  tokio child rather than a raw `waitpid`, so exit status and reaping stay
  consistent.

### Fixed

- **Default Unix spawn no longer fails with `EPERM`.** The child no longer calls
  `setsid` after `process_group(0)` had already created a new session — the
  redundant `setsid` returned `EPERM` and broke the default spawn path.
- **The PTY master is opened close-on-exec.** Atomic on Linux (`openpt` with
  `CLOEXEC`); on macOS/BSD, whose `posix_openpt` has no atomic `O_CLOEXEC`, a
  follow-up `fcntl` sets it (a small open→`fcntl` window remains). Prevents the
  master leaking into an unrelated concurrently-spawned child.
- **Retry PTY allocation on transient exhaustion.** macOS caps system-wide PTYs
  at `kern.tty.ptmx_max`, so allocation can transiently fail under heavy
  concurrent spawning; allocation now retries with a short bounded backoff and
  surfaces the underlying OS error on genuine failure.
- **Retry `poll_read`/`poll_write` on `EINTR`** rather than surfacing a spurious
  error when a signal interrupts the syscall before any bytes move.
- **macOS: open the slave before setting the initial window size**, so the
  `TIOCSWINSZ` applied at allocation takes effect.
- **Harden child stdio setup** with checked `dup` and close-on-exec on the slave
  handle used only for the `pre_exec` `TIOCSCTTY`.
- **Windows: recover a fast-exiting child's final output.** The ConPTY read path
  stopped as soon as the `open` flag was cleared on child exit, discarding bytes
  already written to the output pipe. Reads now drain the pipe until `ReadFile`
  reports `ERROR_BROKEN_PIPE`.
- **macOS: capture a fast-exiting child's final PTY output.** macOS discards the
  master's still-buffered bytes when the last slave fd closes around child exit.
  A dedicated drain thread, started before the child spawns, reads the master
  into a userspace buffer the instant bytes arrive, so the output survives
  teardown.
