//! Windows child process management for ConPTY.
//!
//! This module provides child process spawning and management for Windows
//! ConPTY sessions, including Job Object integration for process lifetime management.

use std::ffi::OsStr;
use std::future::Future;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{io, ptr};

use windows_sys::Win32::Foundation::{CloseHandle, DUPLICATE_SAME_ACCESS, DuplicateHandle, HANDLE};
use windows_sys::Win32::System::Console::HPCON;
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject,
};
use windows_sys::Win32::System::Threading::{
    CREATE_UNICODE_ENVIRONMENT, CreateProcessW, EXTENDED_STARTUPINFO_PRESENT, GetCurrentProcess,
    GetExitCodeProcess, INFINITE, InitializeProcThreadAttributeList,
    PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, PROCESS_INFORMATION, STARTF_USESTDHANDLES, STARTUPINFOEXW,
    UpdateProcThreadAttribute, WaitForSingleObject,
};

/// Windows BOOL type (i32 in windows-sys 0.61+)
type BOOL = i32;
/// Windows FALSE constant
const FALSE: BOOL = 0;
/// Windows TRUE constant (not currently used but kept for completeness)
#[allow(dead_code)]
const TRUE: BOOL = 1;
/// Wait result when object is signaled (value 0)
const WAIT_OBJECT_0: u32 = 0;

use crate::config::{PtyConfig, PtySignal};
use crate::error::{PtyError, Result};
use crate::traits::{ExitStatus, PtyChild};

/// Windows child process handle with Job Object support.
#[derive(Debug)]
pub struct WindowsPtyChild {
    /// The process handle.
    process: OwnedHandle,
    /// The process ID.
    pid: u32,
    /// The job object (for cleanup).
    job: Option<OwnedHandle>,
    /// Whether the process is still running.
    running: Arc<AtomicBool>,
    /// Cached exit status.
    exit_status: Option<ExitStatus>,
}

impl WindowsPtyChild {
    /// Create a new child process handle.
    pub fn new(process: OwnedHandle, pid: u32, job: Option<OwnedHandle>) -> Self {
        Self {
            process,
            pid,
            job,
            running: Arc::new(AtomicBool::new(true)),
            exit_status: None,
        }
    }

    /// Get the process ID.
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Check if the process is still running.
    ///
    /// This performs a non-blocking peek at the process handle rather than
    /// relying solely on the cached `running` flag, so it reports the truth
    /// immediately after the child exits — even before the exit watcher thread
    /// (or an explicit `wait`/`try_wait`) has had a chance to observe it.
    #[must_use]
    pub fn is_running(&self) -> bool {
        if !self.running.load(Ordering::SeqCst) {
            return false;
        }

        let handle = self.process.as_raw_handle() as HANDLE;
        // SAFETY: handle is a valid process handle owned by self. A zero timeout
        // makes this a non-blocking state query.
        let signaled = unsafe { WaitForSingleObject(handle, 0) } == WAIT_OBJECT_0;
        if signaled {
            self.running.store(false, Ordering::SeqCst);
        }
        !signaled
    }

    /// Duplicate the process handle into a new owned handle.
    ///
    /// Used to hand a process handle to the exit-watcher thread so it can
    /// outlive (or run independently of) this `WindowsPtyChild` without racing
    /// on the original handle's ownership.
    pub fn duplicate_process_handle(&self) -> Result<OwnedHandle> {
        let mut dup: HANDLE = ptr::null_mut();
        // SAFETY: source and target process handles come from GetCurrentProcess
        // (pseudo-handles, always valid), the source handle is owned by self,
        // and `dup` is a valid out-pointer.
        let ok = unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                self.process.as_raw_handle() as HANDLE,
                GetCurrentProcess(),
                &mut dup,
                0,
                FALSE,
                DUPLICATE_SAME_ACCESS,
            )
        };
        if ok == FALSE {
            return Err(PtyError::Spawn(io::Error::last_os_error()));
        }
        // SAFETY: DuplicateHandle succeeded, so `dup` is a fresh owned handle.
        Ok(unsafe { OwnedHandle::from_raw_handle(dup as RawHandle) })
    }

    /// Wait for the child process to exit.
    pub async fn wait(&mut self) -> Result<ExitStatus> {
        if let Some(status) = self.exit_status {
            return Ok(status);
        }

        // Cast to usize to make it Send (raw pointers are not Send)
        let handle_val = self.process.as_raw_handle() as usize;

        // Wait in a blocking task
        let exit_code = tokio::task::spawn_blocking(move || {
            let handle = handle_val as HANDLE;
            // SAFETY: handle is valid
            let wait_result = unsafe { WaitForSingleObject(handle, INFINITE) };
            if wait_result != WAIT_OBJECT_0 {
                return Err(io::Error::last_os_error());
            }

            let mut exit_code: u32 = 0;
            // SAFETY: handle is valid and exit_code is a valid pointer
            if unsafe { GetExitCodeProcess(handle, &mut exit_code) } == FALSE {
                return Err(io::Error::last_os_error());
            }

            Ok(exit_code)
        })
        .await
        .map_err(|e| PtyError::Wait(io::Error::other(e)))?
        .map_err(PtyError::Wait)?;

        let status = ExitStatus::Terminated(exit_code);
        self.exit_status = Some(status);
        self.running.store(false, Ordering::SeqCst);

        Ok(status)
    }

    /// Try to get the exit status without blocking.
    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>> {
        if let Some(status) = self.exit_status {
            return Ok(Some(status));
        }

        let handle = self.process.as_raw_handle() as HANDLE;

        // SAFETY: handle is valid
        let wait_result = unsafe { WaitForSingleObject(handle, 0) };

        if wait_result == WAIT_OBJECT_0 {
            let mut exit_code: u32 = 0;
            // SAFETY: handle is valid
            if unsafe { GetExitCodeProcess(handle, &mut exit_code) } == FALSE {
                return Err(PtyError::Wait(io::Error::last_os_error()));
            }

            let status = ExitStatus::Terminated(exit_code);
            self.exit_status = Some(status);
            self.running.store(false, Ordering::SeqCst);

            Ok(Some(status))
        } else {
            Ok(None)
        }
    }

    /// Send a signal to the child process.
    ///
    /// On Windows, most signals are emulated or not supported.
    pub fn signal(&self, signal: PtySignal) -> Result<()> {
        use windows_sys::Win32::System::Console::{
            CTRL_BREAK_EVENT, CTRL_C_EVENT, GenerateConsoleCtrlEvent,
        };

        match signal {
            PtySignal::Interrupt => {
                // SAFETY: pid is valid
                if unsafe { GenerateConsoleCtrlEvent(CTRL_C_EVENT, self.pid) } == FALSE {
                    Err(PtyError::Signal(io::Error::last_os_error()))
                } else {
                    Ok(())
                }
            }
            PtySignal::Quit => {
                // SAFETY: pid is valid
                if unsafe { GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, self.pid) } == FALSE {
                    Err(PtyError::Signal(io::Error::last_os_error()))
                } else {
                    Ok(())
                }
            }
            PtySignal::Terminate | PtySignal::Kill | PtySignal::Hangup => {
                // Terminate the process (Windows equivalent of SIGTERM/SIGKILL)
                self.terminate_impl()
            }
            PtySignal::WindowChange => {
                // Window changes are handled via ConPTY resize
                Ok(())
            }
        }
    }

    /// Internal termination implementation that doesn't require &mut self.
    fn terminate_impl(&self) -> Result<()> {
        use windows_sys::Win32::System::Threading::TerminateProcess;

        if let Some(ref job) = self.job {
            // Terminate all processes in the job
            // SAFETY: job handle is valid
            if unsafe { TerminateJobObject(job.as_raw_handle() as HANDLE, 1) } == FALSE {
                return Err(PtyError::Signal(io::Error::last_os_error()));
            }
        } else {
            // SAFETY: process handle is valid
            if unsafe { TerminateProcess(self.process.as_raw_handle() as HANDLE, 1) } == FALSE {
                return Err(PtyError::Signal(io::Error::last_os_error()));
            }
        }
        Ok(())
    }

    /// Kill the child process.
    pub fn kill(&mut self) -> Result<()> {
        self.terminate_impl()?;
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }
}

impl PtyChild for WindowsPtyChild {
    fn pid(&self) -> u32 {
        WindowsPtyChild::pid(self)
    }

    fn is_running(&self) -> bool {
        WindowsPtyChild::is_running(self)
    }

    fn wait(&mut self) -> Pin<Box<dyn Future<Output = Result<ExitStatus>> + Send + '_>> {
        Box::pin(WindowsPtyChild::wait(self))
    }

    fn try_wait(&mut self) -> Result<Option<ExitStatus>> {
        WindowsPtyChild::try_wait(self)
    }

    fn signal(&self, signal: PtySignal) -> Result<()> {
        WindowsPtyChild::signal(self, signal)
    }

    fn kill(&mut self) -> Result<()> {
        WindowsPtyChild::kill(self)
    }
}

/// Spawn a child process attached to a ConPTY.
pub fn spawn_child<S, I>(
    hpc: HPCON,
    program: S,
    args: I,
    config: &PtyConfig,
) -> Result<WindowsPtyChild>
where
    S: AsRef<OsStr>,
    I: IntoIterator,
    I::Item: AsRef<OsStr>,
{
    // Build command line with proper escaping
    // The program name is escaped the same way as arguments
    let mut cmdline = escape_argument(program.as_ref());
    for arg in args {
        cmdline.push(b' ' as u16);
        cmdline.extend(escape_argument(arg.as_ref()));
    }
    cmdline.push(0); // Null terminator

    // Build environment block
    let env_block = build_environment_block(&config.effective_env());

    // Working directory
    let working_dir = config.working_directory.as_ref().map(|p| {
        let mut w = to_wide_string(p.as_os_str());
        w.push(0);
        w
    });

    // Create job object for process management
    let job = create_job_object()?;

    // Set up startup info with pseudo console
    let (startup_info, _attr_list) = create_startup_info(hpc)?;

    // SAFETY: PROCESS_INFORMATION is plain-old-data; all-zero is a valid
    // (uninitialized) state that CreateProcessW will overwrite on success.
    let mut process_info: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };

    // SAFETY: All pointers are valid and properly initialized
    let result = unsafe {
        CreateProcessW(
            ptr::null(),
            cmdline.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            FALSE,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
            if env_block.is_empty() {
                ptr::null()
            } else {
                env_block.as_ptr() as *const _
            },
            working_dir.as_ref().map_or(ptr::null(), |w| w.as_ptr()),
            &startup_info.StartupInfo,
            &mut process_info,
        )
    };

    if result == FALSE {
        return Err(PtyError::Spawn(io::Error::last_os_error()));
    }

    // Close thread handle (we don't need it)
    // SAFETY: thread handle is valid
    unsafe {
        CloseHandle(process_info.hThread);
    }

    // Assign process to job.
    // SAFETY: CreateProcessW succeeded above (FALSE-check at line 294 returned early),
    // so process_info.hProcess is a freshly-opened, owned handle that we are taking
    // sole ownership of.
    let process = unsafe { OwnedHandle::from_raw_handle(process_info.hProcess as RawHandle) };

    if let Some(ref job_handle) = job {
        // SAFETY: handles are valid
        unsafe {
            AssignProcessToJobObject(
                job_handle.as_raw_handle() as HANDLE,
                process.as_raw_handle() as HANDLE,
            );
        }
    }

    Ok(WindowsPtyChild::new(process, process_info.dwProcessId, job))
}

/// Convert an OsStr to a wide string (UTF-16).
fn to_wide_string(s: &OsStr) -> Vec<u16> {
    s.encode_wide().collect()
}

/// Escape a command-line argument for Windows.
///
/// This implements proper Windows command-line escaping according to the
/// Microsoft C/C++ argument parsing rules:
/// - Arguments containing spaces, tabs, or quotes are wrapped in double quotes
/// - Backslashes before quotes are doubled
/// - Quotes inside the argument are escaped as \"
/// - Trailing backslashes are doubled (since they precede the closing quote)
///
/// # References
/// - <https://docs.microsoft.com/en-us/cpp/c-language/parsing-c-command-line-arguments>
/// - <https://daviddeley.com/autohotkey/parameters/parameters.htm>
fn escape_argument(arg: &OsStr) -> Vec<u16> {
    let arg_str = arg.to_string_lossy();

    // Check if argument needs quoting
    let needs_quoting = arg_str.is_empty()
        || arg_str.contains(' ')
        || arg_str.contains('\t')
        || arg_str.contains('"')
        || arg_str.contains('\\');

    if !needs_quoting {
        return to_wide_string(arg);
    }

    let mut result = Vec::new();
    result.push(b'"' as u16); // Opening quote

    let chars: Vec<char> = arg_str.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if c == '\\' {
            // Count consecutive backslashes
            let mut num_backslashes = 0;
            while i < chars.len() && chars[i] == '\\' {
                num_backslashes += 1;
                i += 1;
            }

            if i < chars.len() && chars[i] == '"' {
                // Backslashes before a quote: double them and escape the quote
                for _ in 0..(num_backslashes * 2) {
                    result.push(b'\\' as u16);
                }
                result.push(b'\\' as u16);
                result.push(b'"' as u16);
                i += 1;
            } else if i >= chars.len() {
                // Trailing backslashes: double them (they'll precede closing quote)
                for _ in 0..(num_backslashes * 2) {
                    result.push(b'\\' as u16);
                }
            } else {
                // Backslashes not before a quote: keep them as-is
                for _ in 0..num_backslashes {
                    result.push(b'\\' as u16);
                }
            }
        } else if c == '"' {
            // Quote without preceding backslash: escape it
            result.push(b'\\' as u16);
            result.push(b'"' as u16);
            i += 1;
        } else {
            // Regular character
            for code_unit in c.encode_utf16(&mut [0u16; 2]) {
                result.push(*code_unit);
            }
            i += 1;
        }
    }

    result.push(b'"' as u16); // Closing quote
    result
}

/// Build a Windows environment block from a HashMap.
fn build_environment_block(
    env: &std::collections::HashMap<std::ffi::OsString, std::ffi::OsString>,
) -> Vec<u16> {
    let mut block = Vec::new();

    for (key, value) in env {
        block.extend(to_wide_string(key));
        block.push(b'=' as u16);
        block.extend(to_wide_string(value));
        block.push(0);
    }

    block.push(0); // Double null terminator
    block
}

/// Create a job object for process management.
fn create_job_object() -> Result<Option<OwnedHandle>> {
    // SAFETY: null parameters create an unnamed job
    let job = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };

    if job.is_null() {
        return Ok(None);
    }

    // Configure job to kill child processes when job handle is closed.
    // SAFETY: JOBOBJECT_EXTENDED_LIMIT_INFORMATION is plain-old-data; zero-init is
    // the documented starting state before setting LimitFlags below.
    let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

    // SAFETY: job handle and info are valid
    let result = unsafe {
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const _,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    };

    if result == FALSE {
        // SAFETY: job handle is valid
        unsafe {
            CloseHandle(job);
        }
        return Ok(None);
    }

    // SAFETY: `job` is the handle returned by CreateJobObjectW above and is non-null
    // (early return on null at line 427). Wrapping it in OwnedHandle transfers ownership.
    Ok(Some(unsafe {
        OwnedHandle::from_raw_handle(job as RawHandle)
    }))
}

/// Create extended startup info with pseudo console attribute.
fn create_startup_info(hpc: HPCON) -> Result<(STARTUPINFOEXW, Vec<u8>)> {
    // Get required attribute list size
    let mut size: usize = 0;
    // SAFETY: Getting size with null buffer
    unsafe {
        InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut size);
    }

    // Allocate attribute list
    let mut attr_list = vec![0u8; size];

    // Initialize attribute list
    // SAFETY: buffer is properly sized
    let result = unsafe {
        InitializeProcThreadAttributeList(attr_list.as_mut_ptr() as *mut _, 1, 0, &mut size)
    };

    if result == FALSE {
        return Err(PtyError::Spawn(io::Error::last_os_error()));
    }

    // Set pseudo console attribute
    // SAFETY: attribute list is initialized
    let result = unsafe {
        UpdateProcThreadAttribute(
            attr_list.as_mut_ptr() as *mut _,
            0,
            PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
            hpc as *mut _,
            std::mem::size_of::<HPCON>(),
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };

    if result == FALSE {
        return Err(PtyError::Spawn(io::Error::last_os_error()));
    }

    // SAFETY: STARTUPINFOEXW is plain-old-data; zero-init is the documented
    // baseline. cb and lpAttributeList are set on the next two lines.
    let mut startup_info: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
    startup_info.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
    startup_info.lpAttributeList = attr_list.as_mut_ptr() as *mut _;

    // Opt out of receiving this process's standard handles.
    //
    // Without STARTF_USESTDHANDLES, Windows *duplicates* the parent's standard
    // handles into a console-subsystem child — a legacy convenience path that
    // `bInheritHandles = FALSE` does not disable. ConPTY's startup only undoes
    // that for copied *console* handles, so a parent whose own stdio is
    // redirected to a pipe or file (a service, a daemon, a CI runner,
    // `host.exe > log.txt`) leaks those handles to the child: its output
    // bypasses the pseudoconsole entirely and its input comes from the parent's
    // stream rather than the PTY.
    //
    // Setting the flag with all three handle fields left NULL is how Windows
    // Terminal spawns ConPTY children. `bInheritHandles` stays FALSE because no
    // real handles are supplied; the pseudoconsole attribute set above is what
    // gives the child its console.
    //
    // See https://github.com/microsoft/terminal/issues/11276 and the
    // `std_handles_are_conpty_not_parents` regression test.
    startup_info.StartupInfo.dwFlags |= STARTF_USESTDHANDLES;
    // hStdInput/hStdOutput/hStdError intentionally remain NULL from zeroed().

    Ok((startup_info, attr_list))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_string_conversion() {
        let s = OsStr::new("hello");
        let wide = to_wide_string(s);
        assert_eq!(
            wide,
            vec![
                b'h' as u16,
                b'e' as u16,
                b'l' as u16,
                b'l' as u16,
                b'o' as u16
            ]
        );
    }

    /// Helper to convert escaped Vec<u16> back to String for testing.
    fn wide_to_string(wide: &[u16]) -> String {
        String::from_utf16_lossy(wide)
    }

    #[test]
    fn escape_simple_argument() {
        // Simple argument without special characters - no quoting needed
        let arg = OsStr::new("hello");
        let escaped = escape_argument(arg);
        assert_eq!(wide_to_string(&escaped), "hello");
    }

    #[test]
    fn escape_argument_with_space() {
        // Argument with space needs quoting
        let arg = OsStr::new("hello world");
        let escaped = escape_argument(arg);
        assert_eq!(wide_to_string(&escaped), "\"hello world\"");
    }

    #[test]
    fn escape_argument_with_tab() {
        // Argument with tab needs quoting
        let arg = OsStr::new("hello\tworld");
        let escaped = escape_argument(arg);
        assert_eq!(wide_to_string(&escaped), "\"hello\tworld\"");
    }

    #[test]
    fn escape_empty_argument() {
        // Empty argument needs quoting
        let arg = OsStr::new("");
        let escaped = escape_argument(arg);
        assert_eq!(wide_to_string(&escaped), "\"\"");
    }

    #[test]
    fn escape_argument_with_quote() {
        // Embedded quote needs escaping
        let arg = OsStr::new("say \"hello\"");
        let escaped = escape_argument(arg);
        assert_eq!(wide_to_string(&escaped), "\"say \\\"hello\\\"\"");
    }

    #[test]
    fn escape_argument_with_backslash() {
        // Backslash not before quote - kept as-is inside quotes
        let arg = OsStr::new("C:\\Users\\test");
        let escaped = escape_argument(arg);
        assert_eq!(wide_to_string(&escaped), "\"C:\\Users\\test\"");
    }

    #[test]
    fn escape_argument_with_trailing_backslash() {
        // Trailing backslashes need to be doubled (they precede closing quote)
        let arg = OsStr::new("C:\\Users\\");
        let escaped = escape_argument(arg);
        assert_eq!(wide_to_string(&escaped), "\"C:\\Users\\\\\"");
    }

    #[test]
    fn escape_argument_with_multiple_trailing_backslashes() {
        // Multiple trailing backslashes all need doubling
        let arg = OsStr::new("path\\\\");
        let escaped = escape_argument(arg);
        assert_eq!(wide_to_string(&escaped), "\"path\\\\\\\\\"");
    }

    #[test]
    fn escape_argument_backslash_before_quote() {
        // Backslash before quote: double the backslash and escape the quote
        let arg = OsStr::new("test\\\"value");
        let escaped = escape_argument(arg);
        // \\ before " -> \\\\ + \"
        assert_eq!(wide_to_string(&escaped), "\"test\\\\\\\"value\"");
    }

    #[test]
    fn escape_argument_multiple_backslashes_before_quote() {
        // Multiple backslashes before quote
        let arg = OsStr::new("test\\\\\"value");
        let escaped = escape_argument(arg);
        // \\\\ before " -> \\\\\\\\ + \"
        assert_eq!(wide_to_string(&escaped), "\"test\\\\\\\\\\\"value\"");
    }

    #[test]
    fn escape_complex_path() {
        // Complex Windows path with spaces
        let arg = OsStr::new("C:\\Program Files\\My App\\bin");
        let escaped = escape_argument(arg);
        assert_eq!(
            wide_to_string(&escaped),
            "\"C:\\Program Files\\My App\\bin\""
        );
    }

    #[test]
    fn escape_unc_path() {
        // UNC path
        let arg = OsStr::new("\\\\server\\share\\folder");
        let escaped = escape_argument(arg);
        assert_eq!(wide_to_string(&escaped), "\"\\\\server\\share\\folder\"");
    }
}
