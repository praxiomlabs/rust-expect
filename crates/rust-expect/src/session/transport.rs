//! Shared transport handle with per-poll locking.
//!
//! The session and the interaction loop both need to reach the transport, and
//! a session must stay writable while a read is parked waiting for output.
//! Holding a [`tokio::sync::Mutex`] across the read's `await` — what this crate
//! used to do — makes that impossible: the lock is held for as long as the
//! child stays quiet, which is most of a session's life.
//!
//! [`SharedTransport`] takes the lock *inside* each `poll_*` call and releases
//! it when that poll returns, including when the read returns `Pending`. A
//! parked read therefore holds nothing, and a concurrent write proceeds. This
//! is the same mechanism [`tokio::io::split`] uses; the difference is that
//! `split` consumes the transport and hands back read/write views only, which
//! would cut off the [`Resizable`](crate::backend::Resizable) and
//! [`ChildExit`](crate::backend::ChildExit) capabilities that need `&mut T`.
//!
//! The lock is a [`std::sync::Mutex`], deliberately. Every hold lasts one poll
//! and never spans an await, so there is nothing for an async-aware lock to do
//! but add cost.

use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// A cloneable handle to a session's transport.
///
/// Clones share one transport. See the module docs for why the lock is held
/// per poll rather than across an await.
pub(crate) struct SharedTransport<T>(Arc<Mutex<T>>);

// Implemented by hand: `derive(Clone)` would demand `T: Clone`, which no
// transport is — it is the handle that clones, not the transport.
impl<T> Clone for SharedTransport<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T> SharedTransport<T> {
    /// Wrap a transport in a shareable handle.
    pub(crate) fn new(transport: T) -> Self {
        Self(Arc::new(Mutex::new(transport)))
    }

    /// Lock the transport, recovering from poisoning.
    ///
    /// A panic inside one `poll_*` must not make the session permanently
    /// unusable — the transport itself is still valid, only the lock was
    /// tainted. Mirrors the screen mutex's recovery in `session::handle`.
    pub(crate) fn lock(&self) -> MutexGuard<'_, T> {
        match self.0.lock() {
            Ok(guard) => guard,
            Err(poison) => {
                tracing::warn!("transport mutex was poisoned; recovering inner state");
                poison.into_inner()
            }
        }
    }

    /// Run `f` against the transport.
    ///
    /// For the capabilities that need `&mut T` rather than byte I/O — resize,
    /// and the non-blocking reap after EOF. Both are short synchronous calls.
    pub(crate) fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        f(&mut self.lock())
    }
}

impl<T: AsyncRead + Unpin> AsyncRead for SharedTransport<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut *self.lock()).poll_read(cx, buf)
    }
}

impl<T: AsyncWrite + Unpin> AsyncWrite for SharedTransport<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut *self.lock()).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut *self.lock()).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut *self.lock()).poll_shutdown(cx)
    }
}

#[cfg(all(test, feature = "mock"))]
mod tests {
    use std::time::Duration;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::SharedTransport;
    use crate::mock::MockTransport;

    /// The property this type exists for: a read that is parked waiting for
    /// output must not block a write.
    ///
    /// Control run against the previous discipline — an
    /// `Arc<tokio::sync::Mutex<T>>` guard held across the read's await — the
    /// same shape failed: the write never acquired the lock and hit the 2 s
    /// timeout. Here it completes in tens of milliseconds.
    #[tokio::test]
    async fn a_parked_read_does_not_block_a_write() {
        let mock = MockTransport::new();
        let inspect = mock.clone();
        let transport = SharedTransport::new(mock);

        // Park a read on a transport with nothing queued.
        let mut reader = transport.clone();
        let read_task = tokio::spawn(async move {
            let mut buf = [0u8; 64];
            reader.read(&mut buf).await
        });

        // Give the read a chance to register as pending before writing.
        tokio::time::sleep(Duration::from_millis(50)).await;

        let mut writer = transport.clone();
        tokio::time::timeout(Duration::from_secs(2), writer.write_all(b"ping"))
            .await
            .expect("write must not wait on the parked read")
            .expect("write must succeed");

        assert_eq!(inspect.take_input(), b"ping");

        read_task.abort();
    }

    /// Reaching the transport for a non-I/O capability must not wait on a
    /// parked read either — this is the path `resize_pty` and the post-EOF
    /// reap take.
    #[tokio::test]
    async fn a_parked_read_does_not_block_capability_access() {
        let transport = SharedTransport::new(MockTransport::new());

        let mut reader = transport.clone();
        let read_task = tokio::spawn(async move {
            let mut buf = [0u8; 64];
            reader.read(&mut buf).await
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        let reached = tokio::time::timeout(
            Duration::from_secs(2),
            tokio::task::spawn_blocking({
                let transport = transport.clone();
                move || transport.with(|_| true)
            }),
        )
        .await
        .expect("capability access must not wait on the parked read")
        .expect("join");

        assert!(reached);

        read_task.abort();
    }
}
