//! Windows pipe I/O handling for ConPTY.
//!
//! This module provides utilities for creating and managing the pipes
//! used for ConPTY input and output.

use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::{io, ptr};

use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Pipes::CreatePipe;

/// A pair of connected pipes for PTY I/O.
#[derive(Debug)]
pub struct PipePair {
    /// Read end of the pipe.
    pub read: OwnedHandle,
    /// Write end of the pipe.
    pub write: OwnedHandle,
}

impl PipePair {
    /// Create a new pipe pair.
    ///
    /// # Errors
    ///
    /// Returns an error if pipe creation fails.
    pub fn new() -> io::Result<Self> {
        let mut read_handle: HANDLE = INVALID_HANDLE_VALUE;
        let mut write_handle: HANDLE = INVALID_HANDLE_VALUE;

        // SAFETY: We're passing valid pointers and the handles are initialized
        let result = unsafe { CreatePipe(&mut read_handle, &mut write_handle, ptr::null(), 0) };

        if result == 0 {
            return Err(io::Error::last_os_error());
        }

        // SAFETY: The handles are valid after successful CreatePipe
        Ok(Self {
            read: unsafe { OwnedHandle::from_raw_handle(read_handle as RawHandle) },
            write: unsafe { OwnedHandle::from_raw_handle(write_handle as RawHandle) },
        })
    }
}

/// Create a pair of pipes for ConPTY input.
///
/// Returns (pty_input_write, pty_input_read) where:
/// - `pty_input_write` is used by the application to write to the PTY
/// - `pty_input_read` is passed to ConPTY
pub fn create_input_pipe() -> io::Result<PipePair> {
    PipePair::new()
}

/// Create a pair of pipes for ConPTY output.
///
/// Returns (pty_output_write, pty_output_read) where:
/// - `pty_output_write` is passed to ConPTY
/// - `pty_output_read` is used by the application to read from the PTY
pub fn create_output_pipe() -> io::Result<PipePair> {
    PipePair::new()
}

/// Set a handle to be inheritable by child processes.
pub fn set_inheritable(handle: &OwnedHandle, inheritable: bool) -> io::Result<()> {
    use windows_sys::Win32::Foundation::{HANDLE_FLAG_INHERIT, SetHandleInformation};

    let flags = if inheritable { HANDLE_FLAG_INHERIT } else { 0 };

    // SAFETY: handle is valid
    let result = unsafe {
        SetHandleInformation(handle.as_raw_handle() as HANDLE, HANDLE_FLAG_INHERIT, flags)
    };

    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_pipe_pair() {
        let pair = PipePair::new();
        assert!(pair.is_ok());
    }
}
