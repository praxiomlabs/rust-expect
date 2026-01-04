//! Async I/O adapter for Windows pipes.
//!
//! This module provides async read/write operations for Windows pipes used
//! with ConPTY, bridging the gap between synchronous Windows I/O and Tokio's
//! async runtime.

use std::io;
use std::os::windows::io::{AsRawHandle, OwnedHandle, RawHandle};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll, Waker};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Storage::FileSystem::{ReadFile, WriteFile};

/// Windows FALSE constant (0)
const FALSE: i32 = 0;

use crate::config::WindowSize;
use crate::error::{PtyError, Result};
use crate::traits::PtyMaster;

/// Result from a pending read operation.
#[derive(Debug)]
enum PendingReadState {
    /// No read in progress.
    Idle,
    /// Read is in progress, will wake the waker when done.
    InProgress(Option<Waker>),
    /// Read completed with data.
    Ready(io::Result<Vec<u8>>),
}

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
    /// Pending read state (protected by mutex for Sync).
    pending_read: Arc<Mutex<PendingReadState>>,
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
            pending_read: Arc::new(Mutex::new(PendingReadState::Idle)),
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
            pending_read: Arc::new(Mutex::new(PendingReadState::Idle)),
        }
    }
}

impl AsyncRead for WindowsPtyMaster {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();

        if !this.open.load(Ordering::SeqCst) {
            return Poll::Ready(Ok(())); // EOF
        }

        // Check current state
        let mut state = this.pending_read.lock().unwrap();
        match std::mem::replace(&mut *state, PendingReadState::Idle) {
            PendingReadState::Idle => {
                // Start a new blocking read operation
                let handle = Arc::clone(&this.output);
                let open = Arc::clone(&this.open);
                let pending_read = Arc::clone(&this.pending_read);
                let buf_capacity = buf.remaining();

                // Store waker before spawning
                *state = PendingReadState::InProgress(Some(cx.waker().clone()));
                drop(state); // Release lock before spawning

                // Spawn the blocking read
                tokio::task::spawn(async move {
                    let result = tokio::task::spawn_blocking(move || {
                        if !open.load(Ordering::SeqCst) {
                            return Ok(Vec::new()); // EOF
                        }

                        let mut buffer = vec![0u8; buf_capacity.min(4096)];
                        let mut bytes_read: u32 = 0;

                        // Cast handle to usize for Send (handle values are pointers)
                        let raw_handle = handle.as_raw_handle() as usize;

                        // SAFETY: handle and buffer are valid
                        let success = unsafe {
                            ReadFile(
                                raw_handle as HANDLE,
                                buffer.as_mut_ptr(),
                                buffer.len() as u32,
                                &mut bytes_read,
                                std::ptr::null_mut(),
                            )
                        };

                        if success == FALSE {
                            let err = io::Error::last_os_error();
                            // ERROR_BROKEN_PIPE means the child closed
                            if err.raw_os_error() == Some(109) {
                                return Ok(Vec::new()); // EOF
                            }
                            return Err(err);
                        }

                        buffer.truncate(bytes_read as usize);
                        Ok(buffer)
                    })
                    .await
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
                    .and_then(|r| r);

                    // Store result and wake
                    let mut state = pending_read.lock().unwrap();
                    let waker = match std::mem::replace(&mut *state, PendingReadState::Ready(result)) {
                        PendingReadState::InProgress(waker) => waker,
                        _ => None,
                    };
                    drop(state);
                    if let Some(w) = waker {
                        w.wake();
                    }
                });

                Poll::Pending
            }
            PendingReadState::InProgress(waker) => {
                // Update waker in case it changed
                *state = PendingReadState::InProgress(Some(cx.waker().clone()));
                drop(waker); // Drop old waker
                Poll::Pending
            }
            PendingReadState::Ready(result) => {
                // Leave state as Idle (already set by mem::replace)
                match result {
                    Ok(data) => {
                        if data.is_empty() {
                            Poll::Ready(Ok(())) // EOF
                        } else {
                            let unfilled = buf.initialize_unfilled();
                            let to_copy = data.len().min(unfilled.len());
                            unfilled[..to_copy].copy_from_slice(&data[..to_copy]);
                            buf.advance(to_copy);
                            Poll::Ready(Ok(()))
                        }
                    }
                    Err(e) => Poll::Ready(Err(e)),
                }
            }
        }
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
    // Tests would require actual Windows environment with ConPTY support.
    // Integration tests should be run manually on a Windows machine.
}
