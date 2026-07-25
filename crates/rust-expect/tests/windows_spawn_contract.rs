//! `ConPTY` spawn-contract integration tests (Windows).
//!
//! Peer libraries have shipped real Windows bugs where the spawn contract was
//! silently broken: arguments dropped (expectrl #63), environment modes ignored
//! (expectrl #69), the pseudo console created at 0x0, exit codes lost, or the
//! child handed the parent's standard handles instead of the pseudoconsole's
//! (#46). These tests spawn real `ConPTY` processes and assert each contract
//! end-to-end.
//!
//! To make assertions deterministic and immune to `ConPTY` escape-sequence
//! pollution, the child is a tiny purpose-built helper (`reflector.exe`,
//! compiled once with `rustc` into `CARGO_TARGET_TMPDIR`) that reflects its
//! observed state (argv / env / cwd / console size) into a file whose path is
//! passed as an argument. The test then reads that file after the child exits.

#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use rust_expect::{Session, SessionConfig};

/// Source of the reflector helper. Kept dependency-free so it compiles with a
/// bare `rustc` invocation (no Cargo, default edition). `winsize` uses raw
/// kernel32 FFI (kernel32 is linked by default for the msvc target).
const REFLECTOR_SRC: &str = r#"
use std::io::Write;

#[repr(C)]
struct Coord { x: i16, y: i16 }
#[repr(C)]
struct SmallRect { left: i16, top: i16, right: i16, bottom: i16 }
#[repr(C)]
struct Csbi { size: Coord, cursor: Coord, attrs: u16, window: SmallRect, max_size: Coord }

extern "system" {
    fn CreateFileW(
        name: *const u16,
        access: u32,
        share: u32,
        sa: *mut core::ffi::c_void,
        disposition: u32,
        flags: u32,
        template: *mut core::ffi::c_void,
    ) -> *mut core::ffi::c_void;
    fn GetConsoleScreenBufferInfo(h: *mut core::ffi::c_void, info: *mut Csbi) -> i32;
    fn GetStdHandle(which: u32) -> *mut core::ffi::c_void;
    fn GetFileType(h: *mut core::ffi::c_void) -> u32;
    fn GetConsoleMode(h: *mut core::ffi::c_void, mode: *mut u32) -> i32;
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // usage: reflector <mode> <outfile> [payload...]
    if args.len() < 2 {
        eprintln!("usage: reflector <mode> <outfile> [payload...]");
        std::process::exit(2);
    }
    let mode = args[0].as_str();
    let outfile = &args[1];
    let mut f = std::fs::File::create(outfile).expect("create outfile");

    match mode {
        // Reflect each payload argument on its own line, in order.
        "args" => {
            for a in &args[2..] {
                writeln!(f, "{}", a).unwrap();
            }
        }
        // Reflect NAME=value for each requested variable ("<UNSET>" if absent).
        "env" => {
            for name in &args[2..] {
                let val = std::env::var(name).unwrap_or_else(|_| "<UNSET>".to_string());
                writeln!(f, "{}={}", name, val).unwrap();
            }
        }
        // Reflect the child's current working directory.
        "cwd" => {
            let cwd = std::env::current_dir().unwrap();
            writeln!(f, "{}", cwd.display()).unwrap();
        }
        // Reflect the ConPTY window size via the console API. CONOUT$ always
        // resolves to the active console buffer, so it is queried directly
        // rather than relying on whatever the std handles happen to be.
        "winsize" => {
            const GENERIC_READ: u32 = 0x8000_0000;
            const GENERIC_WRITE: u32 = 0x4000_0000;
            const FILE_SHARE_READ: u32 = 0x0000_0001;
            const FILE_SHARE_WRITE: u32 = 0x0000_0002;
            const OPEN_EXISTING: u32 = 3;
            let name: Vec<u16> = "CONOUT$".encode_utf16().chain(std::iter::once(0)).collect();
            let h = unsafe {
                CreateFileW(
                    name.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    std::ptr::null_mut(),
                    OPEN_EXISTING,
                    0,
                    std::ptr::null_mut(),
                )
            };
            let mut info: Csbi = unsafe { std::mem::zeroed() };
            let ok = unsafe { GetConsoleScreenBufferInfo(h, &mut info) };
            if ok != 0 {
                let cols = info.window.right as i32 - info.window.left as i32 + 1;
                let rows = info.window.bottom as i32 - info.window.top as i32 + 1;
                writeln!(f, "cols={} rows={}", cols, rows).unwrap();
            } else {
                writeln!(f, "cols=ERR rows=ERR").unwrap();
            }
        }
        // Reflect the provenance of each standard handle. A correctly spawned
        // ConPTY child gets console handles; the parent's pipes/files reaching
        // the child is the issue #46 leak. `type` is GetFileType (2 =
        // FILE_TYPE_CHAR), `console` is whether GetConsoleMode succeeds —
        // needed because NUL is also a character device.
        "stdio" => {
            let handles = [("stdin", -10i32), ("stdout", -11i32), ("stderr", -12i32)];
            for &(label, id) in handles.iter() {
                let h = unsafe { GetStdHandle(id as u32) };
                let ft = unsafe { GetFileType(h) };
                let mut mode: u32 = 0;
                let console = unsafe { GetConsoleMode(h, &mut mode) } != 0;
                writeln!(f, "{}=type:{} console:{}", label, ft, console).unwrap();
            }
        }
        // Block on one line of stdin and reflect exactly what arrived. The
        // terminator is recorded via `{:?}` so the test can see whether it was
        // CR, LF or CRLF rather than only that a line was read at all.
        "readline" => {
            let mut line = String::new();
            match std::io::stdin().read_line(&mut line) {
                Ok(n) => writeln!(f, "read={} line={:?}", n, line).unwrap(),
                Err(e) => writeln!(f, "error={}", e).unwrap(),
            }
        }
        other => {
            eprintln!("unknown mode: {}", other);
            std::process::exit(3);
        }
    }
    f.flush().unwrap();
    // Marker on stdout so tests can also observe completion via the PTY.
    println!("REFLECTOR-DONE");
}
"#;

/// Compile the reflector helper once and return its path.
fn reflector() -> &'static Path {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
        let src = dir.join("reflector.rs");
        let exe = dir.join("reflector.exe");
        std::fs::write(&src, REFLECTOR_SRC).expect("write reflector source");
        let status = Command::new("rustc")
            .args(["-O", src.to_str().unwrap(), "-o", exe.to_str().unwrap()])
            .status()
            .expect("invoke rustc to build reflector helper");
        assert!(status.success(), "reflector helper failed to compile");
        exe
    })
    .as_path()
}

/// A unique output-file path under `CARGO_TARGET_TMPDIR` for one test.
fn unique_outfile(tag: &str) -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("out-{tag}-{n}.txt"))
}

/// Spawn the reflector with the given mode/payload and config, wait for it to
/// exit, and return the exit status plus the reflected file contents.
async fn run_reflector(
    mode: &str,
    outfile: &Path,
    payload: &[&str],
    config: SessionConfig,
) -> (rust_expect::ProcessExitStatus, String) {
    let exe = reflector().to_str().unwrap().to_string();
    let out = outfile.to_str().unwrap().to_string();

    let mut args: Vec<&str> = vec![mode, &out];
    args.extend_from_slice(payload);

    let mut session = Session::spawn_with_config(&exe, &args, config)
        .await
        .expect("spawn reflector");

    let status = session
        .wait_timeout(Duration::from_secs(20))
        .await
        .expect("reflector should exit");

    let contents = std::fs::read_to_string(outfile).unwrap_or_default();
    (status, contents)
}

// ---------------------------------------------------------------------------
// argv correctness (mirrors expectrl #63)
// ---------------------------------------------------------------------------

/// All arguments must arrive at the child intact and in order, including args
/// with spaces, `=`, and trailing backslashes (which exercise `escape_argument`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn argv_arrives_intact_and_in_order() {
    let out = unique_outfile("argv");
    let payload = [
        "alpha",
        "beta gamma",
        "--flag=value",
        "with\"quote",
        "trailing\\",
    ];
    let (status, contents) = run_reflector("args", &out, &payload, SessionConfig::default()).await;

    assert!(status.success(), "reflector exited non-zero: {status}");
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(
        lines, payload,
        "child argv did not round-trip exactly (got {lines:?})"
    );
}

// ---------------------------------------------------------------------------
// environment modes (mirrors expectrl #69)
// ---------------------------------------------------------------------------

/// Inherit mode (`inherit_env=true`, no explicit env): the child sees the parent's
/// environment. `SystemRoot` always exists in a Windows process env.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn env_inherit_sees_parent() {
    let out = unique_outfile("env-inherit");
    let mut config = SessionConfig::default();
    config.env.clear(); // exact Inherit branch: inherit_env=true + empty env
    let (status, contents) =
        run_reflector("env", &out, &["SystemRoot", "RUST_EXPECT_ABSENT"], config).await;

    assert!(status.success(), "reflector exited non-zero: {status}");
    assert!(
        contents.contains("SystemRoot=") && !contents.contains("SystemRoot=<UNSET>"),
        "Inherit mode should expose the parent's SystemRoot (got: {contents:?})"
    );
    assert!(
        contents.contains("RUST_EXPECT_ABSENT=<UNSET>"),
        "a never-set variable must be absent (got: {contents:?})"
    );
}

/// Extend mode (`inherit_env=true` + explicit overrides): the child sees the
/// parent's environment AND the overrides.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn env_extend_merges_parent_and_overrides() {
    let out = unique_outfile("env-extend");
    let config = SessionConfig::default().env("RUST_EXPECT_EXTEND", "extend-value");
    let (status, contents) =
        run_reflector("env", &out, &["SystemRoot", "RUST_EXPECT_EXTEND"], config).await;

    assert!(status.success(), "reflector exited non-zero: {status}");
    assert!(
        contents.contains("SystemRoot=") && !contents.contains("SystemRoot=<UNSET>"),
        "Extend mode should still inherit the parent env (got: {contents:?})"
    );
    assert!(
        contents.contains("RUST_EXPECT_EXTEND=extend-value"),
        "Extend mode should apply the override (got: {contents:?})"
    );
}

/// Clear mode (`inherit_env=false` + overrides): the child sees ONLY the explicit
/// variables, not the parent's environment.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn env_clear_hides_parent() {
    let out = unique_outfile("env-clear");
    let config = SessionConfig::default()
        .inherit_env(false)
        .env("RUST_EXPECT_CLEAR", "only-this");
    let (status, contents) =
        run_reflector("env", &out, &["SystemRoot", "RUST_EXPECT_CLEAR"], config).await;

    assert!(status.success(), "reflector exited non-zero: {status}");
    assert!(
        contents.contains("RUST_EXPECT_CLEAR=only-this"),
        "Clear mode should still apply explicit overrides (got: {contents:?})"
    );
    assert!(
        contents.contains("SystemRoot=<UNSET>"),
        "Clear mode must not leak the parent's SystemRoot (got: {contents:?})"
    );
}

// ---------------------------------------------------------------------------
// working directory
// ---------------------------------------------------------------------------

/// `working_directory` must set the child's CWD.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn working_directory_is_honored() {
    let dir = std::env::temp_dir();
    let canonical = std::fs::canonicalize(&dir).expect("canonicalize temp dir");
    let out = unique_outfile("cwd");

    let config = SessionConfig::default().working_dir(&canonical);
    let (status, contents) = run_reflector("cwd", &out, &[], config).await;

    assert!(status.success(), "reflector exited non-zero: {status}");
    let reported = std::fs::canonicalize(contents.trim()).expect("child cwd should canonicalize");
    assert_eq!(
        reported, canonical,
        "child did not run in the configured working directory"
    );
}

// ---------------------------------------------------------------------------
// initial window size (backlog W1)
// ---------------------------------------------------------------------------

/// The `ConPTY` must be created at the configured dimensions, NOT 0x0.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conpty_created_at_default_dimensions() {
    let out = unique_outfile("winsize-default");
    // Default SessionConfig dimensions are 80x24.
    let (status, contents) = run_reflector("winsize", &out, &[], SessionConfig::default()).await;

    assert!(status.success(), "reflector exited non-zero: {status}");
    assert_eq!(
        contents.trim(),
        "cols=80 rows=24",
        "ConPTY should be created at the default 80x24, not 0x0 (got: {contents:?})"
    );
}

/// Custom dimensions must be honored by the `ConPTY`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conpty_honors_custom_dimensions() {
    let out = unique_outfile("winsize-custom");
    let config = SessionConfig::default().dimensions(100, 40);
    let (status, contents) = run_reflector("winsize", &out, &[], config).await;

    assert!(status.success(), "reflector exited non-zero: {status}");
    assert_eq!(
        contents.trim(),
        "cols=100 rows=40",
        "ConPTY should honor configured dimensions (got: {contents:?})"
    );
}

// ---------------------------------------------------------------------------
// exit status (v0.4.0 regression: Terminated(code) -> Exited(code))
// ---------------------------------------------------------------------------

/// A normal non-zero exit code must be reported as `Exited(code)`, not `Unknown`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exit_code_nonzero_is_reported() {
    let mut session = Session::spawn("cmd.exe", &["/c", "exit 7"])
        .await
        .expect("spawn cmd.exe");
    let status = session
        .wait_timeout(Duration::from_secs(10))
        .await
        .expect("child should exit");
    assert_eq!(
        status,
        rust_expect::ProcessExitStatus::Exited(7),
        "expected Exited(7), got {status}"
    );
}

/// A clean exit must report `Exited(0)` and `success()`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exit_code_zero_is_success() {
    let mut session = Session::spawn("cmd.exe", &["/c", "exit 0"])
        .await
        .expect("spawn cmd.exe");
    let status = session
        .wait_timeout(Duration::from_secs(10))
        .await
        .expect("child should exit");
    assert_eq!(status, rust_expect::ProcessExitStatus::Exited(0));
    assert!(status.success());
}

// ---------------------------------------------------------------------------
// standard-handle provenance (issue #46)
// ---------------------------------------------------------------------------

/// Whether this process's own stdout is a console handle.
#[expect(
    unsafe_code,
    reason = "GetStdHandle/GetConsoleMode are raw Win32 FFI with no safe wrapper here"
)]
fn own_stdout_is_console() -> bool {
    use windows_sys::Win32::System::Console::{GetConsoleMode, GetStdHandle, STD_OUTPUT_HANDLE};

    // SAFETY: GetStdHandle returns a borrowed handle we do not close, and
    // GetConsoleMode only reads through it.
    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        let mut mode = 0u32;
        GetConsoleMode(handle, &raw mut mode) != 0
    }
}

/// A `ConPTY` child's stdin/stdout/stderr must be the pseudoconsole's handles,
/// never the parent's.
///
/// Regression test for issue #46. `create_startup_info` did not set
/// `STARTF_USESTDHANDLES`, so Windows duplicated *this* process's standard
/// handles into the child — a legacy path that `bInheritHandles = FALSE` does
/// not disable and that `ConPTY` only undoes for copied *console* handles. The
/// child then wrote to the test binary's stdout pipe instead of the
/// pseudoconsole, so its output never reached the master. That is also the real
/// cause of what this suite once recorded as conhost "not forwarding rendered
/// output" on some configurations.
///
/// `FILE_TYPE_CHAR` alone would be insufficient evidence, because `NUL` is also
/// a character device. `GetConsoleMode` succeeding is what proves the handle is
/// a real console handle.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn std_handles_are_conpty_not_parents() {
    // The leak can only happen when the parent's handles are *not* console
    // handles, so a console stdout would make the assertions pass vacuously.
    // Under `cargo test` stdout is always a pipe, which is the case that matters.
    //
    // libtest discards a *passing* test's output, so a skip is indistinguishable
    // from a real pass in a CI log. On CI an absent precondition is therefore a
    // hard failure rather than a silent skip: otherwise this test could quietly
    // stop exercising anything and no run would ever report it.
    if own_stdout_is_console() {
        assert!(
            std::env::var_os("CI").is_none(),
            "precondition absent on CI: this process's stdout is a console, so the \
             parent-handle leak cannot occur and this test would pass vacuously"
        );
        eprintln!(
            "SKIPPED std_handles_are_conpty_not_parents: this process's stdout is a \
             console, so the parent-handle leak cannot occur here and the assertions \
             would pass vacuously. Run under `cargo test` to exercise it."
        );
        return;
    }

    let out = unique_outfile("stdio");
    let (status, contents) = run_reflector("stdio", &out, &[], SessionConfig::default()).await;

    assert!(status.success(), "reflector exited non-zero: {status}");
    assert_eq!(
        contents.lines().count(),
        3,
        "expected one line each for stdin/stdout/stderr, got {contents:?}"
    );

    for line in contents.lines() {
        assert!(
            line.contains("type:2") && line.contains("console:true"),
            "every standard handle of a ConPTY child must be a console handle, but \
             {line:?} is not: the child received this process's redirected handle \
             instead of the pseudoconsole (issue #46). Full report: {contents:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// input direction (issue #50)
// ---------------------------------------------------------------------------

/// `send_line` must actually submit a line that a `ConPTY` child's blocking read
/// completes on.
///
/// Regression test for issue #50. `LineEnding::default()` was `Lf` on every
/// platform, and `ConPTY` does not merely disagree with a bare LF — it discards it,
/// completing no read and queuing nothing. `send_line` therefore sent a payload
/// with an inert terminator and this child would block until the timeout.
///
/// This asserts the whole public path rather than the terminator byte: reverting
/// the Windows default to `Lf` makes it fail. It is also the first automated cover
/// for the input direction at all — until now nothing wrote to a master and checked
/// that the child received it, so only the output direction was protected.
///
/// The child reflects the terminator it saw, which is expected to be `\r\n` no
/// matter which working terminator was sent: console cooked mode synthesizes the
/// CRLF rather than passing through the bytes written to the master.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn send_line_reaches_the_child() {
    const MARKER: &str = "PING-51724";

    let out = unique_outfile("readline");
    let exe = reflector().to_str().unwrap().to_string();
    let out_arg = out.to_str().unwrap().to_string();

    let mut session =
        Session::spawn_with_config(&exe, &["readline", &out_arg], SessionConfig::default())
            .await
            .expect("spawn reflector");

    session
        .send_line(MARKER)
        .await
        .expect("writing a line to the ConPTY master should succeed");

    let status = session.wait_timeout(Duration::from_secs(20)).await.expect(
        "the child should complete its read and exit; a timeout here means the line was \
             never submitted (issue #50)",
    );
    assert!(status.success(), "reflector exited non-zero: {status}");

    let contents = std::fs::read_to_string(&out).unwrap_or_default();
    assert!(
        contents.contains(MARKER),
        "the child's read_line should have received {MARKER:?}, got {contents:?}"
    );
    assert!(
        !contents.starts_with("read=0"),
        "the child saw EOF rather than a line, so input did not reach it: {contents:?}"
    );
}
