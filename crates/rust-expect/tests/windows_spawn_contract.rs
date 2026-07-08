//! `ConPTY` spawn-contract integration tests (Windows).
//!
//! Peer libraries have shipped real Windows bugs where the spawn contract was
//! silently broken: arguments dropped (expectrl #63), environment modes ignored
//! (expectrl #69), the pseudo console created at 0x0, or exit codes lost. These
//! tests spawn real `ConPTY` processes and assert each contract end-to-end.
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
        // Reflect the ConPTY window size via the console API. Under ConPTY the
        // std handles are pipes, so query the console buffer via CONOUT$.
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

/// Inherit mode (inherit_env=true, no explicit env): the child sees the parent's
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

/// Extend mode (inherit_env=true + explicit overrides): the child sees the
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

/// Clear mode (inherit_env=false + overrides): the child sees ONLY the explicit
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

/// The ConPTY must be created at the configured dimensions, NOT 0x0.
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

/// Custom dimensions must be honored by the ConPTY.
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
