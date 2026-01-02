//! Windows ConPTY (Console Pseudo Terminal) management.
//!
//! This module provides the core ConPTY functionality, wrapping the Windows
//! Pseudo Console API introduced in Windows 10 1809.

use std::io;
use std::os::windows::io::{AsRawHandle, OwnedHandle, RawHandle};
use std::ptr;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE, S_OK};
use windows_sys::Win32::System::Console::{
    ClosePseudoConsole, CreatePseudoConsole, ResizePseudoConsole, COORD, HPCON,
};

use crate::config::WindowSize;
use crate::error::{PtyError, Result};

/// A wrapper around a Windows Pseudo Console (ConPTY).
#[derive(Debug)]
pub struct ConPty {
    /// The pseudo console handle.
    handle: HPCON,
    /// The input pipe (for writing to the PTY).
    input_write: OwnedHandle,
    /// The output pipe (for reading from the PTY).
    output_read: OwnedHandle,
}

// SAFETY: ConPTY handles can be safely sent between threads
unsafe impl Send for ConPty {}
unsafe impl Sync for ConPty {}

impl ConPty {
    /// Create a new ConPTY with the specified window size.
    ///
    /// # Arguments
    ///
    /// * `size` - The initial window size.
    /// * `input_read` - The read end of the input pipe (passed to ConPTY).
    /// * `output_write` - The write end of the output pipe (passed to ConPTY).
    /// * `input_write` - The write end of the input pipe (kept for writing).
    /// * `output_read` - The read end of the output pipe (kept for reading).
    ///
    /// # Errors
    ///
    /// Returns an error if ConPTY creation fails or is not available.
    pub fn new(
        size: WindowSize,
        input_read: OwnedHandle,
        output_write: OwnedHandle,
        input_write: OwnedHandle,
        output_read: OwnedHandle,
    ) -> Result<Self> {
        let coord = COORD {
            X: size.cols as i16,
            Y: size.rows as i16,
        };

        let mut hpc: HPCON = 0;

        // SAFETY: All handles are valid and the pointer is valid
        let result = unsafe {
            CreatePseudoConsole(
                coord,
                input_read.as_raw_handle() as HANDLE,
                output_write.as_raw_handle() as HANDLE,
                0, // dwFlags
                &mut hpc,
            )
        };

        if result != S_OK {
            return Err(PtyError::Windows {
                message: "failed to create pseudo console".into(),
                code: result as u32,
            });
        }

        if hpc == 0 {
            return Err(PtyError::ConPtyNotAvailable);
        }

        // The input_read and output_write handles are now owned by ConPTY
        // We intentionally leak them here since ConPTY will close them
        std::mem::forget(input_read);
        std::mem::forget(output_write);

        Ok(Self {
            handle: hpc,
            input_write,
            output_read,
        })
    }

    /// Get the ConPTY handle.
    #[must_use]
    pub fn handle(&self) -> HPCON {
        self.handle
    }

    /// Get a reference to the input write handle.
    #[must_use]
    pub fn input(&self) -> &OwnedHandle {
        &self.input_write
    }

    /// Get a reference to the output read handle.
    #[must_use]
    pub fn output(&self) -> &OwnedHandle {
        &self.output_read
    }

    /// Resize the ConPTY window.
    ///
    /// # Errors
    ///
    /// Returns an error if the resize operation fails.
    pub fn resize(&self, size: WindowSize) -> Result<()> {
        let coord = COORD {
            X: size.cols as i16,
            Y: size.rows as i16,
        };

        // SAFETY: handle is valid
        let result = unsafe { ResizePseudoConsole(self.handle, coord) };

        if result != S_OK {
            return Err(PtyError::Windows {
                message: "failed to resize pseudo console".into(),
                code: result as u32,
            });
        }

        Ok(())
    }
}

impl Drop for ConPty {
    fn drop(&mut self) {
        // SAFETY: handle was obtained from CreatePseudoConsole
        unsafe {
            ClosePseudoConsole(self.handle);
        }
    }
}

/// Check if ConPTY is available on this Windows version.
///
/// ConPTY was introduced in Windows 10 version 1809 (build 17763).
#[must_use]
pub fn is_conpty_available() -> bool {
    // Check Windows version
    // For simplicity, we try to create a minimal ConPTY to verify availability
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};

    // Check if CreatePseudoConsole exists in kernel32
    let kernel32 = unsafe { GetModuleHandleW(windows_sys::w!("kernel32.dll")) };
    if kernel32 == 0 {
        return false;
    }

    let proc = unsafe {
        GetProcAddress(
            kernel32,
            b"CreatePseudoConsole\0".as_ptr() as *const i8,
        )
    };

    proc.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_conpty_availability() {
        // This just tests that the function runs without panicking
        let _ = is_conpty_available();
    }
}
