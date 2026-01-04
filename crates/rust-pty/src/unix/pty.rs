//! Unix PTY allocation and management.
//!
//! This module provides the core PTY master implementation for Unix systems,
//! using rustix for low-level PTY operations.

use std::io;
use std::os::unix::io::{AsRawFd, OwnedFd, RawFd};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};

use rustix::fs::{OFlags, fcntl_setfl};
use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
use rustix::termios::{Winsize, tcsetwinsize};
use tokio::io::unix::AsyncFd;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::config::WindowSize;
use crate::error::{PtyError, Result};
use crate::traits::PtyMaster;

/// Unix PTY master implementation.
///
/// This struct wraps the master side of a Unix pseudo-terminal, providing
/// async read/write operations and terminal control.
pub struct UnixPtyMaster {
    /// The master file descriptor wrapped for async I/O.
    async_fd: AsyncFd<OwnedFd>,
    /// Whether the PTY is still open.
    open: Arc<AtomicBool>,
}

impl std::fmt::Debug for UnixPtyMaster {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnixPtyMaster")
            .field("fd", &self.async_fd.as_raw_fd())
            .field("open", &self.open.load(Ordering::SeqCst))
            .finish()
    }
}

impl UnixPtyMaster {
    /// Open a new PTY master.
    ///
    /// This allocates a new pseudo-terminal pair and returns the master side.
    ///
    /// # Errors
    ///
    /// Returns an error if PTY allocation fails.
    pub fn open() -> Result<(Self, String)> {
        // Open master PTY
        let master_fd = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY)
            .map_err(|e| PtyError::Create(io::Error::from_raw_os_error(e.raw_os_error())))?;

        // Grant access to slave
        grantpt(&master_fd)
            .map_err(|e| PtyError::Create(io::Error::from_raw_os_error(e.raw_os_error())))?;

        // Unlock slave
        unlockpt(&master_fd)
            .map_err(|e| PtyError::Create(io::Error::from_raw_os_error(e.raw_os_error())))?;

        // Get slave name
        let slave_name = ptsname(&master_fd, Vec::new())
            .map_err(|e| PtyError::Create(io::Error::from_raw_os_error(e.raw_os_error())))?;
        let slave_path = slave_name
            .to_str()
            .map_err(|_| {
                PtyError::Create(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid slave path encoding",
                ))
            })?
            .to_string();

        // Set non-blocking mode
        fcntl_setfl(&master_fd, OFlags::NONBLOCK)
            .map_err(|e| PtyError::Create(io::Error::from_raw_os_error(e.raw_os_error())))?;

        // Wrap for async I/O
        let async_fd = AsyncFd::new(master_fd).map_err(PtyError::Create)?;

        Ok((
            Self {
                async_fd,
                open: Arc::new(AtomicBool::new(true)),
            },
            slave_path,
        ))
    }

    /// Get the slave PTY path.
    ///
    /// This can be used to open the slave side for a child process.
    pub fn slave_name(&self) -> Result<String> {
        let name = ptsname(self.async_fd.get_ref(), Vec::new())
            .map_err(|e| PtyError::Io(io::Error::from_raw_os_error(e.raw_os_error())))?;
        name.to_str()
            .map(std::string::ToString::to_string)
            .map_err(|_| {
                PtyError::Io(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid slave path encoding",
                ))
            })
    }

    /// Check if the PTY is still open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open.load(Ordering::SeqCst)
    }

    /// Set the window size.
    pub fn set_window_size(&self, size: WindowSize) -> Result<()> {
        if !self.is_open() {
            return Err(PtyError::Closed);
        }

        let winsize = Winsize {
            ws_col: size.cols,
            ws_row: size.rows,
            ws_xpixel: size.xpixel,
            ws_ypixel: size.ypixel,
        };

        tcsetwinsize(self.async_fd.get_ref(), winsize)
            .map_err(|e| PtyError::Resize(io::Error::from_raw_os_error(e.raw_os_error())))
    }

    /// Get the current window size.
    pub fn get_window_size(&self) -> Result<WindowSize> {
        if !self.is_open() {
            return Err(PtyError::Closed);
        }

        let winsize = rustix::termios::tcgetwinsize(self.async_fd.get_ref())
            .map_err(|e| PtyError::GetAttributes(io::Error::from_raw_os_error(e.raw_os_error())))?;

        Ok(WindowSize {
            cols: winsize.ws_col,
            rows: winsize.ws_row,
            xpixel: winsize.ws_xpixel,
            ypixel: winsize.ws_ypixel,
        })
    }

    /// Close the PTY master.
    pub fn close(&mut self) -> Result<()> {
        self.open.store(false, Ordering::SeqCst);
        Ok(())
    }
}

impl AsRawFd for UnixPtyMaster {
    fn as_raw_fd(&self) -> RawFd {
        self.async_fd.as_raw_fd()
    }
}

impl AsyncRead for UnixPtyMaster {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if !self.open.load(Ordering::SeqCst) {
            return Poll::Ready(Ok(())); // EOF
        }

        loop {
            let mut guard = match self.async_fd.poll_read_ready(cx) {
                Poll::Ready(Ok(guard)) => guard,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            };

            let unfilled = buf.initialize_unfilled();
            match rustix::io::read(self.async_fd.get_ref(), unfilled) {
                Ok(0) => {
                    // EOF
                    return Poll::Ready(Ok(()));
                }
                Ok(n) => {
                    buf.advance(n);
                    return Poll::Ready(Ok(()));
                }
                Err(rustix::io::Errno::AGAIN) => {
                    guard.clear_ready();
                    continue;
                }
                Err(e) => {
                    return Poll::Ready(Err(io::Error::from_raw_os_error(e.raw_os_error())));
                }
            }
        }
    }
}

impl AsyncWrite for UnixPtyMaster {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if !self.open.load(Ordering::SeqCst) {
            return Poll::Ready(Err(io::Error::new(io::ErrorKind::BrokenPipe, "PTY closed")));
        }

        loop {
            let mut guard = match self.async_fd.poll_write_ready(cx) {
                Poll::Ready(Ok(guard)) => guard,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            };

            match rustix::io::write(self.async_fd.get_ref(), buf) {
                Ok(n) => return Poll::Ready(Ok(n)),
                Err(rustix::io::Errno::AGAIN) => {
                    guard.clear_ready();
                    continue;
                }
                Err(e) => {
                    return Poll::Ready(Err(io::Error::from_raw_os_error(e.raw_os_error())));
                }
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.open.store(false, Ordering::SeqCst);
        Poll::Ready(Ok(()))
    }
}

impl PtyMaster for UnixPtyMaster {
    fn resize(&self, size: WindowSize) -> Result<()> {
        self.set_window_size(size)
    }

    fn window_size(&self) -> Result<WindowSize> {
        self.get_window_size()
    }

    fn close(&mut self) -> Result<()> {
        Self::close(self)
    }

    fn is_open(&self) -> bool {
        Self::is_open(self)
    }

    fn as_raw_fd(&self) -> RawFd {
        AsRawFd::as_raw_fd(self)
    }
}

/// Open the slave side of a PTY.
///
/// # Safety
///
/// The caller must ensure the path is a valid PTY slave path.
pub fn open_slave(path: &str) -> Result<OwnedFd> {
    use rustix::fs::{Mode, OFlags, open};
    use std::path::Path;

    let fd = open(
        Path::new(path),
        OFlags::RDWR | OFlags::NOCTTY,
        Mode::empty(),
    )
    .map_err(|e| PtyError::Create(io::Error::from_raw_os_error(e.raw_os_error())))?;

    Ok(fd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn open_pty() {
        let result = UnixPtyMaster::open();
        assert!(result.is_ok());

        let (master, slave_path) = result.unwrap();
        assert!(master.is_open());
        assert!(slave_path.starts_with("/dev/pts/") || slave_path.starts_with("/dev/pty"));
    }

    #[tokio::test]
    async fn window_size_operations() {
        let (master, _) = UnixPtyMaster::open().unwrap();

        // Set window size
        let size = WindowSize::new(120, 40);
        assert!(master.set_window_size(size).is_ok());

        // Get window size
        let retrieved = master.get_window_size().unwrap();
        assert_eq!(retrieved.cols, 120);
        assert_eq!(retrieved.rows, 40);
    }

    #[tokio::test]
    async fn close_pty() {
        let (mut master, _) = UnixPtyMaster::open().unwrap();
        assert!(master.is_open());

        master.close().unwrap();
        assert!(!master.is_open());
    }
}
