//! Zero-copy I/O utilities.
//!
//! This module provides utilities for minimizing buffer copies during
//! I/O operations, improving performance for high-throughput terminal
//! automation.
//!
//! # Features
//!
//! - `BytesBuffer`: Reference-counted bytes for zero-copy slicing
//! - `VecWriter`: Efficient vectored write batching
//! - `BorrowedView`: Borrowed slice views with lifetime tracking
//!
//! # Example
//!
//! ```rust
//! use rust_expect::util::zerocopy::{BytesBuffer, VecWriter};
//!
//! // Create a buffer with zero-copy slicing
//! let mut buffer = BytesBuffer::new();
//! buffer.extend(b"hello world");
//!
//! // Slice without copying
//! let slice = buffer.slice(0..5);
//! assert_eq!(&slice[..], b"hello");
//!
//! // Batch multiple writes
//! let mut writer = VecWriter::new();
//! writer.push(&b"hello"[..]);
//! writer.push(&b" "[..]);
//! writer.push(&b"world"[..]);
//! let bytes = writer.freeze();
//! assert_eq!(&bytes[..], b"hello world");
//! ```

use std::io::{self, IoSlice, Write};
use std::ops::{Deref, Range, RangeBounds};

use bytes::{Buf, Bytes, BytesMut};

/// A reference-counted byte buffer that supports zero-copy slicing.
///
/// This is a wrapper around `bytes::Bytes` that provides convenient
/// methods for terminal automation use cases.
#[derive(Debug, Clone, Default)]
pub struct BytesBuffer {
    inner: BytesMut,
}

impl BytesBuffer {
    /// Create an empty buffer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: BytesMut::new(),
        }
    }

    /// Create a buffer with the specified capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: BytesMut::with_capacity(capacity),
        }
    }

    /// Create a buffer from existing bytes.
    #[must_use]
    pub fn from_bytes(data: impl Into<Bytes>) -> Self {
        let bytes: Bytes = data.into();
        let mut inner = BytesMut::with_capacity(bytes.len());
        inner.extend_from_slice(&bytes);
        Self { inner }
    }

    /// Get the length of the buffer.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if the buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get the capacity of the buffer.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    /// Extend the buffer with data.
    pub fn extend(&mut self, data: &[u8]) {
        self.inner.extend_from_slice(data);
    }

    /// Reserve additional capacity.
    pub fn reserve(&mut self, additional: usize) {
        self.inner.reserve(additional);
    }

    /// Clear the buffer, retaining capacity.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Get a zero-copy slice of the buffer.
    ///
    /// The returned `Bytes` shares ownership with the original buffer.
    #[must_use]
    pub fn slice(&self, range: Range<usize>) -> Bytes {
        self.inner.clone().freeze().slice(range)
    }

    /// Get a zero-copy slice using range bounds.
    #[must_use]
    pub fn slice_ref<R: RangeBounds<usize>>(&self, range: R) -> Bytes {
        use std::ops::Bound;

        let start = match range.start_bound() {
            Bound::Included(&n) => n,
            Bound::Excluded(&n) => n + 1,
            Bound::Unbounded => 0,
        };

        let end = match range.end_bound() {
            Bound::Included(&n) => n + 1,
            Bound::Excluded(&n) => n,
            Bound::Unbounded => self.len(),
        };

        self.slice(start..end)
    }

    /// Freeze the buffer into immutable bytes.
    ///
    /// This is a zero-copy operation.
    #[must_use]
    pub fn freeze(self) -> Bytes {
        self.inner.freeze()
    }

    /// Split off the first `at` bytes.
    ///
    /// Returns the split-off bytes, leaving the rest in the buffer.
    pub fn split_to(&mut self, at: usize) -> BytesMut {
        self.inner.split_to(at)
    }

    /// Split off bytes at the end.
    pub fn split_off(&mut self, at: usize) -> BytesMut {
        self.inner.split_off(at)
    }

    /// Consume `n` bytes from the front of the buffer.
    pub fn advance(&mut self, n: usize) {
        self.inner.advance(n);
    }

    /// Get an immutable view of the buffer.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.inner
    }

    /// Get the buffer as a string (lossy UTF-8 conversion).
    #[must_use]
    pub fn as_str_lossy(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.inner)
    }

    /// Find a byte pattern in the buffer.
    #[must_use]
    pub fn find(&self, needle: &[u8]) -> Option<usize> {
        self.inner
            .windows(needle.len())
            .position(|window| window == needle)
    }

    /// Find a string in the buffer.
    #[must_use]
    pub fn find_str(&self, needle: &str) -> Option<usize> {
        self.find(needle.as_bytes())
    }

    /// Get a view of the last `n` bytes.
    #[must_use]
    pub fn tail(&self, n: usize) -> &[u8] {
        let start = self.len().saturating_sub(n);
        &self.inner[start..]
    }

    /// Get a view of the first `n` bytes.
    #[must_use]
    pub fn head(&self, n: usize) -> &[u8] {
        let end = n.min(self.len());
        &self.inner[..end]
    }
}

impl Deref for BytesBuffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl AsRef<[u8]> for BytesBuffer {
    fn as_ref(&self) -> &[u8] {
        &self.inner
    }
}

impl From<Vec<u8>> for BytesBuffer {
    fn from(vec: Vec<u8>) -> Self {
        Self {
            inner: BytesMut::from(&vec[..]),
        }
    }
}

impl From<&[u8]> for BytesBuffer {
    fn from(slice: &[u8]) -> Self {
        Self {
            inner: BytesMut::from(slice),
        }
    }
}

impl From<&str> for BytesBuffer {
    fn from(s: &str) -> Self {
        Self::from(s.as_bytes())
    }
}

impl Write for BytesBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.extend(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A writer that batches multiple small writes for vectored I/O.
///
/// This reduces the number of system calls by accumulating writes
/// and sending them as a single vectored write operation.
#[derive(Debug, Default)]
pub struct VecWriter {
    chunks: Vec<Bytes>,
    total_len: usize,
}

impl VecWriter {
    /// Create a new vectored writer.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            chunks: Vec::new(),
            total_len: 0,
        }
    }

    /// Create a vectored writer with the specified chunk capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            chunks: Vec::with_capacity(capacity),
            total_len: 0,
        }
    }

    /// Add bytes to the writer.
    pub fn push(&mut self, data: impl Into<Bytes>) {
        let bytes: Bytes = data.into();
        self.total_len += bytes.len();
        self.chunks.push(bytes);
    }

    /// Add a slice to the writer (copies into a new Bytes).
    pub fn push_slice(&mut self, data: &[u8]) {
        self.push(Bytes::copy_from_slice(data));
    }

    /// Get the number of chunks.
    #[must_use]
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Get the total length across all chunks.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.total_len
    }

    /// Check if the writer is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.total_len == 0
    }

    /// Clear all chunks.
    pub fn clear(&mut self) {
        self.chunks.clear();
        self.total_len = 0;
    }

    /// Get the chunks as I/O slices for vectored writes.
    ///
    /// The returned slices can be passed to `write_vectored`.
    #[must_use]
    pub fn as_io_slices(&self) -> Vec<IoSlice<'_>> {
        self.chunks.iter().map(|b| IoSlice::new(b)).collect()
    }

    /// Freeze all chunks into a single contiguous buffer.
    ///
    /// This is useful when you need a single contiguous view.
    #[must_use]
    pub fn freeze(self) -> Bytes {
        if self.chunks.len() == 1 {
            // Fast path: single chunk
            return self.chunks.into_iter().next().unwrap();
        }

        let mut buffer = BytesMut::with_capacity(self.total_len);
        for chunk in self.chunks {
            buffer.extend_from_slice(&chunk);
        }
        buffer.freeze()
    }

    /// Write all chunks to a writer using vectored I/O.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<usize> {
        let slices = self.as_io_slices();
        writer.write_vectored(&slices)
    }
}

/// A borrowed view of bytes with lifetime tracking.
///
/// This provides a way to pass borrowed slices around without
/// copying, while still allowing for owned data when needed.
#[derive(Debug)]
pub enum BorrowedView<'a> {
    /// A borrowed slice.
    Borrowed(&'a [u8]),
    /// Owned bytes (for cases where borrowing isn't possible).
    Owned(Bytes),
}

impl<'a> BorrowedView<'a> {
    /// Create a borrowed view.
    #[must_use]
    pub const fn borrowed(data: &'a [u8]) -> Self {
        Self::Borrowed(data)
    }

    /// Create an owned view.
    #[must_use]
    pub fn owned(data: impl Into<Bytes>) -> Self {
        Self::Owned(data.into())
    }

    /// Get the length of the view.
    #[must_use]
    pub const fn len(&self) -> usize {
        match self {
            Self::Borrowed(b) => b.len(),
            Self::Owned(b) => b.len(),
        }
    }

    /// Check if the view is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get a slice of the view.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        match self {
            Self::Borrowed(b) => b,
            Self::Owned(b) => b,
        }
    }

    /// Convert to owned bytes.
    ///
    /// This may involve copying if the view is borrowed.
    #[must_use]
    pub fn into_owned(self) -> Bytes {
        match self {
            Self::Borrowed(b) => Bytes::copy_from_slice(b),
            Self::Owned(b) => b,
        }
    }
}

impl Deref for BorrowedView<'_> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl AsRef<[u8]> for BorrowedView<'_> {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

/// Trait for types that can provide a zero-copy view of their contents.
pub trait ZeroCopySource {
    /// Get a borrowed view of the data.
    fn view(&self) -> BorrowedView<'_>;

    /// Get the length of the data.
    fn len(&self) -> usize {
        self.view().len()
    }

    /// Check if the data is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl ZeroCopySource for [u8] {
    fn view(&self) -> BorrowedView<'_> {
        BorrowedView::borrowed(self)
    }
}

impl ZeroCopySource for Vec<u8> {
    fn view(&self) -> BorrowedView<'_> {
        BorrowedView::borrowed(self)
    }
}

impl ZeroCopySource for Bytes {
    fn view(&self) -> BorrowedView<'_> {
        BorrowedView::Owned(self.clone())
    }
}

impl ZeroCopySource for BytesBuffer {
    fn view(&self) -> BorrowedView<'_> {
        BorrowedView::borrowed(&self.inner)
    }
}

impl ZeroCopySource for str {
    fn view(&self) -> BorrowedView<'_> {
        BorrowedView::borrowed(self.as_bytes())
    }
}

impl ZeroCopySource for String {
    fn view(&self) -> BorrowedView<'_> {
        BorrowedView::borrowed(self.as_bytes())
    }
}

/// A read buffer that minimizes copies during async reads.
///
/// This is designed to work with Tokio's `ReadBuf` pattern while
/// allowing for efficient buffer reuse.
#[derive(Debug)]
pub struct ReadPool {
    buffers: Vec<BytesMut>,
    buffer_size: usize,
}

impl ReadPool {
    /// Create a new read pool.
    #[must_use]
    pub const fn new(buffer_size: usize) -> Self {
        Self {
            buffers: Vec::new(),
            buffer_size,
        }
    }

    /// Get a buffer from the pool, or create a new one.
    pub fn acquire(&mut self) -> BytesMut {
        self.buffers
            .pop()
            .unwrap_or_else(|| BytesMut::with_capacity(self.buffer_size))
    }

    /// Return a buffer to the pool for reuse.
    pub fn release(&mut self, mut buffer: BytesMut) {
        buffer.clear();
        // Only keep buffers that haven't grown too large
        if buffer.capacity() <= self.buffer_size * 2 {
            self.buffers.push(buffer);
        }
    }

    /// Clear the pool, releasing all buffers.
    pub fn clear(&mut self) {
        self.buffers.clear();
    }

    /// Get the number of buffers in the pool.
    #[must_use]
    pub fn available(&self) -> usize {
        self.buffers.len()
    }
}

impl Default for ReadPool {
    fn default() -> Self {
        Self::new(8192) // 8KB default buffer size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_buffer_basic() {
        let mut buffer = BytesBuffer::new();
        buffer.extend(b"hello");
        buffer.extend(b" world");

        assert_eq!(buffer.len(), 11);
        assert_eq!(buffer.as_bytes(), b"hello world");
    }

    #[test]
    fn bytes_buffer_slicing() {
        let mut buffer = BytesBuffer::with_capacity(20);
        buffer.extend(b"hello world");

        let slice = buffer.slice(0..5);
        assert_eq!(&slice[..], b"hello");

        let slice = buffer.slice_ref(6..);
        assert_eq!(&slice[..], b"world");
    }

    #[test]
    fn bytes_buffer_find() {
        let buffer = BytesBuffer::from("the quick brown fox");

        assert_eq!(buffer.find(b"quick"), Some(4));
        assert_eq!(buffer.find_str("fox"), Some(16));
        assert_eq!(buffer.find(b"lazy"), None);
    }

    #[test]
    fn bytes_buffer_head_tail() {
        let buffer = BytesBuffer::from("hello world");

        assert_eq!(buffer.head(5), b"hello");
        assert_eq!(buffer.tail(5), b"world");
        assert_eq!(buffer.head(100), b"hello world");
        assert_eq!(buffer.tail(100), b"hello world");
    }

    #[test]
    fn vec_writer_basic() {
        let mut writer = VecWriter::new();
        writer.push(b"hello".as_slice());
        writer.push(b" ".as_slice());
        writer.push(b"world".as_slice());

        assert_eq!(writer.chunk_count(), 3);
        assert_eq!(writer.len(), 11);

        let bytes = writer.freeze();
        assert_eq!(&bytes[..], b"hello world");
    }

    #[test]
    fn vec_writer_io_slices() {
        let mut writer = VecWriter::new();
        writer.push_slice(b"hello");
        writer.push_slice(b"world");

        let slices = writer.as_io_slices();
        assert_eq!(slices.len(), 2);
    }

    #[test]
    fn borrowed_view() {
        let data = b"hello world";
        let borrowed = BorrowedView::borrowed(data);

        assert_eq!(borrowed.len(), 11);
        assert_eq!(borrowed.as_slice(), data);

        let owned = borrowed.into_owned();
        assert_eq!(&owned[..], data);
    }

    #[test]
    fn zero_copy_source_trait() {
        let vec: Vec<u8> = b"hello".to_vec();
        let view = vec.view();
        assert_eq!(view.len(), 5);

        let string = "world".to_string();
        let view = string.view();
        assert_eq!(view.len(), 5);
    }

    #[test]
    fn read_pool() {
        let mut pool = ReadPool::new(4096);

        assert_eq!(pool.available(), 0);

        let buf1 = pool.acquire();
        assert!(buf1.capacity() >= 4096);

        let buf2 = pool.acquire();
        pool.release(buf1);
        assert_eq!(pool.available(), 1);

        let buf3 = pool.acquire();
        assert_eq!(pool.available(), 0);

        pool.release(buf2);
        pool.release(buf3);
        assert_eq!(pool.available(), 2);
    }

    #[test]
    fn write_trait() {
        use std::io::Write;

        let mut buffer = BytesBuffer::new();
        write!(buffer, "hello {}", 42).unwrap();
        assert_eq!(buffer.as_str_lossy(), "hello 42");
    }
}
