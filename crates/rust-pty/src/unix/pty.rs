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
#[cfg(not(target_os = "macos"))]
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
    /// macOS-only background drain of the master (see [`macos_drain`]).
    ///
    /// When present, `poll_read` serves bytes from this drain instead of the fd.
    /// Started before the child is spawned so the child's output is captured the
    /// instant it is written — before macOS/`tokio::process` can discard it on a
    /// fast-exiting child's exit (issue #40). `None` until `start_read_drain`.
    #[cfg(target_os = "macos")]
    drain: Option<macos_drain::Drain>,
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
        // Open master PTY.
        //
        // L1: request close-on-exec so the master fd can't leak into unrelated
        // children spawned concurrently. On Linux rustix honors `CLOEXEC`
        // atomically inside `posix_openpt` (no open->set race). Other platforms'
        // `posix_openpt` has no atomic `O_CLOEXEC`, so there we set `FD_CLOEXEC`
        // with a follow-up fcntl (best-effort; a small open->fcntl window
        // remains, closed only once the platform gains an atomic path).
        #[cfg(target_os = "linux")]
        let master_fd = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY | OpenptFlags::CLOEXEC)
            .map_err(|e| PtyError::Create(io::Error::from_raw_os_error(e.raw_os_error())))?;
        #[cfg(not(target_os = "linux"))]
        let master_fd = {
            let fd = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY)
                .map_err(|e| PtyError::Create(io::Error::from_raw_os_error(e.raw_os_error())))?;
            let flags = rustix::io::fcntl_getfd(&fd)
                .map_err(|e| PtyError::Create(io::Error::from_raw_os_error(e.raw_os_error())))?;
            rustix::io::fcntl_setfd(&fd, flags | rustix::io::FdFlags::CLOEXEC)
                .map_err(|e| PtyError::Create(io::Error::from_raw_os_error(e.raw_os_error())))?;
            fd
        };

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
                #[cfg(target_os = "macos")]
                drain: None,
            },
            slave_path,
        ))
    }

    /// Start the macOS background read drain (see the `macos_drain` module).
    ///
    /// Must be called **before** the child is spawned so no output is missed.
    /// A dedicated OS thread reads a `dup` of the master into a userspace buffer
    /// the instant bytes arrive, so a fast-exiting child's final output is
    /// captured before macOS/`tokio::process` can discard it on exit (#40).
    /// After this, `poll_read` serves from the drain rather than the fd.
    ///
    /// macOS-only: on other targets the master's output survives the child's
    /// exit, so there is nothing to drain and this method does not exist (the
    /// call in `UnixPtySystem::spawn` is likewise `cfg`-gated).
    ///
    /// # Errors
    ///
    /// Returns an error if the master fd cannot be duplicated or the drain
    /// thread cannot be spawned.
    #[cfg(target_os = "macos")]
    pub(crate) fn start_read_drain(&mut self) -> Result<()> {
        if self.drain.is_none() {
            let drain =
                macos_drain::Drain::start(self.async_fd.as_raw_fd()).map_err(PtyError::Io)?;
            self.drain = Some(drain);
        }
        Ok(())
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

        // On macOS, use libc::ioctl directly with TIOCSWINSZ
        #[cfg(target_os = "macos")]
        {
            #[allow(clippy::struct_field_names)]
            #[repr(C)]
            struct LibcWinsize {
                ws_row: libc::c_ushort,
                ws_col: libc::c_ushort,
                ws_xpixel: libc::c_ushort,
                ws_ypixel: libc::c_ushort,
            }

            let winsize = LibcWinsize {
                ws_row: size.rows,
                ws_col: size.cols,
                ws_xpixel: size.xpixel,
                ws_ypixel: size.ypixel,
            };

            // SAFETY: ioctl with TIOCSWINSZ is the standard way to set terminal window size.
            // We're passing a valid winsize struct to a valid file descriptor.
            let result = unsafe {
                libc::ioctl(
                    self.async_fd.as_raw_fd(),
                    libc::TIOCSWINSZ,
                    &raw const winsize,
                )
            };

            if result == -1 {
                return Err(PtyError::Resize(io::Error::last_os_error()));
            }
            Ok(())
        }

        // On other Unix systems, use rustix
        #[cfg(not(target_os = "macos"))]
        {
            let winsize = Winsize {
                ws_col: size.cols,
                ws_row: size.rows,
                ws_xpixel: size.xpixel,
                ws_ypixel: size.ypixel,
            };

            tcsetwinsize(self.async_fd.get_ref(), winsize)
                .map_err(|e| PtyError::Resize(io::Error::from_raw_os_error(e.raw_os_error())))
        }
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

        // macOS: serve from the background drain, which captured the child's
        // output before it could be discarded on exit (#40).
        #[cfg(target_os = "macos")]
        if let Some(drain) = self.drain.as_ref() {
            return drain.poll_read(cx.waker(), buf);
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
                }
                Err(rustix::io::Errno::INTR) => {
                    // Interrupted by a signal before any bytes moved; the fd is
                    // still ready, so loop and retry the read rather than
                    // surfacing a spurious error (E1).
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
                }
                Err(rustix::io::Errno::INTR) => {
                    // Interrupted by a signal before any bytes moved; the fd is
                    // still ready, so loop and retry the write rather than
                    // surfacing a spurious error (E1).
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
    use std::path::Path;

    use rustix::fs::{Mode, OFlags, open};

    // CLOEXEC: the child receives the slave via three explicit `dup`ed stdio
    // fds; this original handle (used only for the pre_exec `TIOCSCTTY`) must
    // not additionally leak across the exec. `dup` does not copy the flag, so
    // the stdio copies remain inheritable as intended.
    let fd = open(
        Path::new(path),
        OFlags::RDWR | OFlags::NOCTTY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|e| PtyError::Create(io::Error::from_raw_os_error(e.raw_os_error())))?;

    Ok(fd)
}

/// macOS-only background drain of a PTY master fd.
///
/// # Why this exists (issue #40)
///
/// On macOS, a child spawned via `tokio::process` can lose its final PTY output:
/// when a fast-exiting child (e.g. `echo hello`) exits, `tokio::process`'s exit
/// handling discards the master's still-buffered output before the session's
/// first read observes it, so `expect` sees EOF with an empty buffer. This was
/// root-caused on-device: the loss is **not** caused by reaping (the child is an
/// un-reaped zombie at loss time) and does not happen with a raw `fork`/`exec`
/// child — only `tokio::process` triggers it, and only on macOS. Linux
/// (`EIO`-based EOF) and Windows (`ConPTY`) are unaffected.
///
/// # How it fixes it
///
/// A dedicated OS thread (independent of the Tokio runtime, so it behaves the
/// same for multi-thread and current-thread runtimes) `poll`s a `dup` of the
/// master and reads bytes into a userspace buffer the instant they are written.
/// Because [`UnixPtyMaster::start_read_drain`] runs it **before the child is
/// spawned**, the reader is already waiting when the child writes, so the output
/// is captured before the exit-time discard can happen. [`UnixPtyMaster::poll_read`]
/// then serves this buffer; EOF (`read` returns 0, which macOS surfaces on the
/// child's *exit* without a reap) is recorded and reported once the buffer is
/// empty. A Tokio task or an inline `AsyncFd` read is *not* sufficient — both are
/// subject to runtime scheduling latency and still lose the race under load.
#[cfg(target_os = "macos")]
pub(crate) mod macos_drain {
    use std::collections::VecDeque;
    use std::io;
    use std::os::unix::io::RawFd;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::task::{Poll, Waker};

    use tokio::io::ReadBuf;

    /// Soft cap on the userspace buffer. When reached, the drain thread stops
    /// reading so the kernel PTY buffer fills and the child sees backpressure —
    /// preserving flow control for a producer whose output is never read.
    const BUFFER_CAP: usize = 1 << 20; // 1 MiB

    struct Shared {
        buf: VecDeque<u8>,
        eof: bool,
        err: Option<io::ErrorKind>,
        waker: Option<Waker>,
    }

    /// Handle to the background drain thread and its shared buffer.
    pub(crate) struct Drain {
        shared: Arc<Mutex<Shared>>,
        stop: Arc<AtomicBool>,
    }

    impl Drain {
        /// Start draining a `dup` of `master_fd` on a dedicated OS thread.
        pub(crate) fn start(master_fd: RawFd) -> io::Result<Self> {
            // A private read-only handle to the same master, so the reader thread
            // never contends with `poll_write` on the original fd.
            // SAFETY: `master_fd` is a live PTY master fd owned by the caller for
            // the duration of this call.
            let read_fd = unsafe { libc::dup(master_fd) };
            if read_fd < 0 {
                return Err(io::Error::last_os_error());
            }
            // Best-effort: don't let this fd leak into concurrently-spawned
            // children. SAFETY: `read_fd` is a valid fd created just above.
            unsafe {
                libc::fcntl(read_fd, libc::F_SETFD, libc::FD_CLOEXEC);
            }

            let shared = Arc::new(Mutex::new(Shared {
                buf: VecDeque::new(),
                eof: false,
                err: None,
                waker: None,
            }));
            let stop = Arc::new(AtomicBool::new(false));

            let thread_shared = Arc::clone(&shared);
            let thread_stop = Arc::clone(&stop);
            // Spawn detached: the thread exits on EOF/error, or within one poll
            // interval of `stop` being set on drop — no join, so teardown never
            // blocks.
            std::thread::Builder::new()
                .name("pty-macos-drain".into())
                .spawn(move || drain_loop(read_fd, &thread_shared, &thread_stop))
                .inspect_err(|_e| {
                    // SAFETY: `read_fd` is the fd we created and still own here.
                    unsafe { libc::close(read_fd) };
                })?;

            Ok(Self { shared, stop })
        }

        /// Serve buffered bytes / EOF / error to `poll_read`.
        pub(crate) fn poll_read(
            &self,
            waker: &Waker,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            let mut sh = self.shared.lock().unwrap();
            if !sh.buf.is_empty() {
                let n = sh.buf.len().min(buf.remaining());
                // `VecDeque` storage may wrap; copy the contiguous run(s).
                let (front, back) = sh.buf.as_slices();
                let take_front = front.len().min(n);
                buf.put_slice(&front[..take_front]);
                let take_back = n - take_front;
                if take_back > 0 {
                    buf.put_slice(&back[..take_back]);
                }
                sh.buf.drain(..n);
                return Poll::Ready(Ok(()));
            }
            if let Some(kind) = sh.err {
                return Poll::Ready(Err(kind.into()));
            }
            if sh.eof {
                // 0-byte read == EOF, now that every buffered byte is delivered.
                return Poll::Ready(Ok(()));
            }
            sh.waker = Some(waker.clone());
            Poll::Pending
        }
    }

    impl Drop for Drain {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
        }
    }

    /// Append bytes and wake any pending `poll_read`. The lock is dropped before
    /// waking so we never wake under the lock.
    fn push(shared: &Arc<Mutex<Shared>>, data: &[u8]) {
        let mut sh = shared.lock().unwrap();
        sh.buf.extend(data);
        let waker = sh.waker.take();
        drop(sh);
        if let Some(w) = waker {
            w.wake();
        }
    }

    /// Record terminal state — EOF (`err = None`) or an error — and wake any
    /// pending `poll_read`. The lock is dropped before waking.
    fn finish(shared: &Arc<Mutex<Shared>>, err: Option<io::ErrorKind>) {
        let mut sh = shared.lock().unwrap();
        match err {
            Some(kind) if sh.err.is_none() => sh.err = Some(kind),
            Some(_) => {}
            None => sh.eof = true,
        }
        let waker = sh.waker.take();
        drop(sh);
        if let Some(w) = waker {
            w.wake();
        }
    }

    fn drain_loop(fd: RawFd, shared: &Arc<Mutex<Shared>>, stop: &Arc<AtomicBool>) {
        use std::cmp::Ordering as CmpOrdering;

        let mut tmp = [0u8; 4096];
        'outer: loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            // Apply backpressure: if the userspace buffer is full, stop reading so
            // the kernel PTY buffer fills and the child blocks.
            let full = { shared.lock().unwrap().buf.len() >= BUFFER_CAP };
            if full {
                std::thread::sleep(std::time::Duration::from_millis(2));
                continue;
            }
            let mut pfd = libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            };
            // 50ms timeout so `stop` is observed promptly after drop.
            // SAFETY: `pfd` is a valid single-element pollfd for a live fd.
            let pr = unsafe { libc::poll(std::ptr::from_mut(&mut pfd), 1, 50) };
            if stop.load(Ordering::Relaxed) {
                break;
            }
            if pr < 0 {
                let e = io::Error::last_os_error();
                if e.raw_os_error() == Some(libc::EINTR) {
                    continue;
                }
                finish(shared, Some(e.kind()));
                break;
            }
            if pr == 0 {
                continue; // timeout, no data
            }
            // Readable: drain everything currently available.
            loop {
                // SAFETY: `fd` is our live dup of the master; `tmp` is a valid
                // local buffer of `tmp.len()` bytes.
                let n = unsafe { libc::read(fd, tmp.as_mut_ptr().cast(), tmp.len()) };
                match n.cmp(&0) {
                    CmpOrdering::Greater => {
                        // `n > 0`, so the cast is lossless.
                        push(shared, &tmp[..n as usize]);
                    }
                    CmpOrdering::Equal => {
                        finish(shared, None); // EOF
                        break 'outer;
                    }
                    CmpOrdering::Less => {
                        let e = io::Error::last_os_error();
                        match e.raw_os_error() {
                            Some(libc::EAGAIN) => break, // drained; back to poll
                            Some(libc::EINTR) => {}      // retry
                            _ => {
                                finish(shared, Some(e.kind()));
                                break 'outer;
                            }
                        }
                    }
                }
            }
        }
        // SAFETY: `fd` is our dup; nothing else uses it after this thread ends.
        unsafe { libc::close(fd) };
    }
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
        // Linux uses /dev/pts/N, macOS uses /dev/ttys* (slave), BSD may use /dev/ttyp* or /dev/pty*
        assert!(
            slave_path.starts_with("/dev/pts/")
                || slave_path.starts_with("/dev/ttys")
                || slave_path.starts_with("/dev/ttyp")
                || slave_path.starts_with("/dev/pty")
        );
    }

    /// L1: the master fd must be close-on-exec so it can't leak into unrelated
    /// children spawned concurrently (atomic on Linux via `OpenptFlags`, via a
    /// follow-up fcntl elsewhere).
    #[tokio::test]
    async fn master_is_close_on_exec() {
        let (master, _slave_path) = UnixPtyMaster::open().expect("open");
        let flags = rustix::io::fcntl_getfd(master.async_fd.get_ref()).expect("F_GETFD");
        assert!(
            flags.contains(rustix::io::FdFlags::CLOEXEC),
            "master fd is missing FD_CLOEXEC"
        );
    }

    #[tokio::test]
    async fn window_size_operations() {
        let (master, _slave_path) = UnixPtyMaster::open().unwrap();

        // On macOS, we need to open the slave before setting window size works reliably
        #[cfg(target_os = "macos")]
        let _slave_fd = open_slave(&_slave_path).unwrap();

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
