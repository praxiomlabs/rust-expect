//! Windows child process management for ConPTY.
//!
//! This module provides child process spawning and management for Windows
//! ConPTY sessions, including Job Object integration for process lifetime management.

use std::ffi::OsStr;
use std::future::Future;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::pin::Pin;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use windows_sys::Win32::Foundation::{
    CloseHandle, BOOL, FALSE, HANDLE, INVALID_HANDLE_VALUE, TRUE,
};
use windows_sys::Win32::System::Console::HPCON;
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows_sys::Win32::System::Threading::{
    CreateProcessW, GetExitCodeProcess, InitializeProcThreadAttributeList,
    UpdateProcThreadAttribute, WaitForSingleObject, EXTENDED_STARTUPINFO_PRESENT,
    INFINITE, PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
    STARTUPINFOEXW, STARTUPINFOW, WAIT_OBJECT_0,
};

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
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Wait for the child process to exit.
    pub async fn wait(&mut self) -> Result<ExitStatus> {
        if let Some(status) = self.exit_status {
            return Ok(status);
        }

        let handle = self.process.as_raw_handle() as HANDLE;

        // Wait in a blocking task
        let exit_code = tokio::task::spawn_blocking(move || {
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
        .map_err(|e| PtyError::Wait(io::Error::new(io::ErrorKind::Other, e)))?
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
            GenerateConsoleCtrlEvent, CTRL_BREAK_EVENT, CTRL_C_EVENT,
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
            PtySignal::Terminate | PtySignal::Kill => self.kill(),
            PtySignal::Hangup => self.kill(),
            PtySignal::WindowChange => {
                // Window changes are handled via ConPTY resize
                Ok(())
            }
        }
    }

    /// Kill the child process.
    pub fn kill(&mut self) -> Result<()> {
        if let Some(ref job) = self.job {
            // Terminate all processes in the job
            // SAFETY: job handle is valid
            if unsafe { TerminateJobObject(job.as_raw_handle() as HANDLE, 1) } == FALSE {
                return Err(PtyError::Signal(io::Error::last_os_error()));
            }
        } else {
            use windows_sys::Win32::System::Threading::TerminateProcess;

            // SAFETY: process handle is valid
            if unsafe { TerminateProcess(self.process.as_raw_handle() as HANDLE, 1) } == FALSE {
                return Err(PtyError::Signal(io::Error::last_os_error()));
            }
        }

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
    // Build command line
    let mut cmdline = to_wide_string(program.as_ref());
    for arg in args {
        cmdline.push(b' ' as u16);
        // TODO: Proper escaping for Windows command line
        cmdline.extend(to_wide_string(arg.as_ref()));
    }
    cmdline.push(0); // Null terminator

    // Build environment block
    let env_block = build_environment_block(&config.effective_env());

    // Working directory
    let working_dir = config
        .working_directory
        .as_ref()
        .map(|p| {
            let mut w = to_wide_string(p.as_os_str());
            w.push(0);
            w
        });

    // Create job object for process management
    let job = create_job_object()?;

    // Set up startup info with pseudo console
    let (startup_info, _attr_list) = create_startup_info(hpc)?;

    let mut process_info: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };

    // SAFETY: All pointers are valid and properly initialized
    let result = unsafe {
        CreateProcessW(
            ptr::null(),
            cmdline.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            FALSE,
            EXTENDED_STARTUPINFO_PRESENT,
            if env_block.is_empty() {
                ptr::null()
            } else {
                env_block.as_ptr() as *const _
            },
            working_dir
                .as_ref()
                .map_or(ptr::null(), |w| w.as_ptr()),
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

    // Assign process to job
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

    Ok(WindowsPtyChild::new(
        process,
        process_info.dwProcessId,
        job,
    ))
}

/// Convert an OsStr to a wide string (UTF-16).
fn to_wide_string(s: &OsStr) -> Vec<u16> {
    s.encode_wide().collect()
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

    if job == 0 {
        return Ok(None);
    }

    // Configure job to kill child processes when job handle is closed
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

    let mut startup_info: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
    startup_info.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
    startup_info.lpAttributeList = attr_list.as_mut_ptr() as *mut _;

    Ok((startup_info, attr_list))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_string_conversion() {
        let s = OsStr::new("hello");
        let wide = to_wide_string(s);
        assert_eq!(wide, vec![b'h' as u16, b'e' as u16, b'l' as u16, b'l' as u16, b'o' as u16]);
    }
}
