//! Async I/O adapter for Windows pipes.
//!
//! This module provides async read/write operations for Windows pipes used
//! with ConPTY, bridging the gap between synchronous Windows I/O and Tokio's
//! async runtime.

use std::io;
use std::os::windows::io::{AsRawHandle, OwnedHandle, RawHandle};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::mpsc;

use windows_sys::Win32::Foundation::{HANDLE, FALSE};
use windows_sys::Win32::Storage::FileSystem::{ReadFile, WriteFile};

use crate::config::WindowSize;
use crate::error::{PtyError, Result};
use crate::traits::PtyMaster;

/// Async wrapper for Windows ConPTY I/O.
///
/// This struct provides async read/write operations by using blocking I/O
/// in spawn_blocking tasks, since Windows named pipes don't integrate well
/// with async runtimes.
pub struct WindowsPtyMaster {
    /// Handle for writing to the PTY (input to child).
    input: Arc<OwnedHandle>,
    /// Handle for reading from the PTY (output from child).
    output: Arc<OwnedHandle>,
    /// Resize callback.
    resize_fn: Option<Box<dyn Fn(WindowSize) -> Result<()> + Send + Sync>>,
    /// Whether the PTY is open.
    open: Arc<AtomicBool>,
    /// Current window size.
    window_size: WindowSize,
}

impl std::fmt::Debug for WindowsPtyMaster {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowsPtyMaster")
            .field("open", &self.open.load(Ordering::SeqCst))
            .field("window_size", &self.window_size)
            .finish()
    }
}

impl WindowsPtyMaster {
    /// Create a new async PTY master wrapper.
    pub fn new(
        input: OwnedHandle,
        output: OwnedHandle,
        resize_fn: impl Fn(WindowSize) -> Result<()> + Send + Sync + 'static,
        initial_size: WindowSize,
    ) -> Self {
        Self {
            input: Arc::new(input),
            output: Arc::new(output),
            resize_fn: Some(Box::new(resize_fn)),
            open: Arc::new(AtomicBool::new(true)),
            window_size: initial_size,
        }
    }

    /// Create without resize support.
    pub fn without_resize(input: OwnedHandle, output: OwnedHandle) -> Self {
        Self {
            input: Arc::new(input),
            output: Arc::new(output),
            resize_fn: None,
            open: Arc::new(AtomicBool::new(true)),
            window_size: WindowSize::default(),
        }
    }

    /// Read from the PTY output.
    async fn read_async(&self, buf: &mut [u8]) -> io::Result<usize> {
        if !self.is_open() {
            return Ok(0); // EOF
        }

        let handle = Arc::clone(&self.output);
        let mut temp_buf = vec![0u8; buf.len()];

        let result = tokio::task::spawn_blocking(move || {
            let raw = handle.as_raw_handle() as HANDLE;
            let mut bytes_read: u32 = 0;

            // SAFETY: handle and buffer are valid
            let success = unsafe {
                ReadFile(
                    raw,
                    temp_buf.as_mut_ptr(),
                    temp_buf.len() as u32,
                    &mut bytes_read,
                    std::ptr::null_mut(),
                )
            };

            if success == FALSE {
                Err(io::Error::last_os_error())
            } else {
                temp_buf.truncate(bytes_read as usize);
                Ok(temp_buf)
            }
        })
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))??;

        let len = result.len().min(buf.len());
        buf[..len].copy_from_slice(&result[..len]);
        Ok(len)
    }

    /// Write to the PTY input.
    async fn write_async(&self, buf: &[u8]) -> io::Result<usize> {
        if !self.is_open() {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "PTY closed"));
        }

        let handle = Arc::clone(&self.input);
        let data = buf.to_vec();

        tokio::task::spawn_blocking(move || {
            let raw = handle.as_raw_handle() as HANDLE;
            let mut bytes_written: u32 = 0;

            // SAFETY: handle and buffer are valid
            let success = unsafe {
                WriteFile(
                    raw,
                    data.as_ptr(),
                    data.len() as u32,
                    &mut bytes_written,
                    std::ptr::null_mut(),
                )
            };

            if success == FALSE {
                Err(io::Error::last_os_error())
            } else {
                Ok(bytes_written as usize)
            }
        })
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
    }
}

impl AsyncRead for WindowsPtyMaster {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // For simplicity, use a future-based approach
        // In production, you'd want proper overlapped I/O
        let this = self.get_mut();

        if !this.open.load(Ordering::SeqCst) {
            return Poll::Ready(Ok(())); // EOF
        }

        // Create a temporary buffer
        let unfilled = buf.initialize_unfilled();
        let handle = Arc::clone(&this.output);
        let buf_len = unfilled.len();

        // We need to spawn a blocking read
        // This is a simplified implementation - production code would use
        // proper overlapped I/O or IOCP
        let mut bytes_read: u32 = 0;

        // SAFETY: handle and buffer are valid
        let success = unsafe {
            ReadFile(
                handle.as_raw_handle() as HANDLE,
                unfilled.as_mut_ptr(),
                buf_len as u32,
                &mut bytes_read,
                std::ptr::null_mut(),
            )
        };

        if success == FALSE {
            let err = io::Error::last_os_error();
            // ERROR_BROKEN_PIPE means the child closed
            if err.raw_os_error() == Some(109) {
                return Poll::Ready(Ok(())); // EOF
            }
            return Poll::Ready(Err(err));
        }

        buf.advance(bytes_read as usize);
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for WindowsPtyMaster {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();

        if !this.open.load(Ordering::SeqCst) {
            return Poll::Ready(Err(io::Error::new(io::ErrorKind::BrokenPipe, "PTY closed")));
        }

        let handle = Arc::clone(&this.input);
        let mut bytes_written: u32 = 0;

        // SAFETY: handle and buffer are valid
        let success = unsafe {
            WriteFile(
                handle.as_raw_handle() as HANDLE,
                buf.as_ptr(),
                buf.len() as u32,
                &mut bytes_written,
                std::ptr::null_mut(),
            )
        };

        if success == FALSE {
            Poll::Ready(Err(io::Error::last_os_error()))
        } else {
            Poll::Ready(Ok(bytes_written as usize))
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.get_mut().open.store(false, Ordering::SeqCst);
        Poll::Ready(Ok(()))
    }
}

impl PtyMaster for WindowsPtyMaster {
    fn resize(&self, size: WindowSize) -> Result<()> {
        if let Some(ref resize_fn) = self.resize_fn {
            resize_fn(size)
        } else {
            Err(PtyError::Resize(io::Error::new(
                io::ErrorKind::Unsupported,
                "resize not supported",
            )))
        }
    }

    fn window_size(&self) -> Result<WindowSize> {
        Ok(self.window_size)
    }

    fn close(&mut self) -> Result<()> {
        self.open.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn is_open(&self) -> bool {
        self.open.load(Ordering::SeqCst)
    }

    fn as_raw_handle(&self) -> RawHandle {
        self.output.as_raw_handle()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests would require actual Windows environment
}
