//! Ring buffer implementation for expect operations.
//!
//! This module provides a ring buffer optimized for terminal output processing,
//! supporting efficient append, search, and extraction operations.

use std::collections::VecDeque;
use std::fmt;

/// Default buffer capacity (1 MB).
pub const DEFAULT_CAPACITY: usize = 1024 * 1024;

/// A ring buffer for accumulating terminal output.
///
/// The buffer supports efficient append operations and automatically
/// discards oldest data when the maximum size is reached.
#[derive(Clone)]
pub struct RingBuffer {
    /// The underlying storage.
    data: VecDeque<u8>,
    /// Maximum capacity.
    max_size: usize,
    /// Total bytes written (may exceed `max_size` due to wrapping).
    total_written: usize,
    /// Bytes discarded due to overflow.
    bytes_discarded: usize,
}

impl RingBuffer {
    /// Create a new ring buffer with the specified maximum size.
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(max_size.min(DEFAULT_CAPACITY)),
            max_size,
            total_written: 0,
            bytes_discarded: 0,
        }
    }

    /// Create a new ring buffer with default capacity.
    #[must_use]
    pub fn with_default_capacity() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }

    /// Append data to the buffer.
    ///
    /// If the buffer would exceed its maximum size, oldest data is discarded.
    pub fn append(&mut self, data: &[u8]) {
        self.total_written += data.len();

        // If new data alone exceeds max_size, only keep the tail
        if data.len() >= self.max_size {
            self.data.clear();
            let start = data.len() - self.max_size;
            self.data.extend(&data[start..]);
            self.bytes_discarded += data.len() - self.max_size;
            return;
        }

        // Calculate how much space we need to free
        let needed_space = (self.data.len() + data.len()).saturating_sub(self.max_size);
        if needed_space > 0 {
            self.bytes_discarded += needed_space;
            for _ in 0..needed_space {
                self.data.pop_front();
            }
        }

        self.data.extend(data);
    }

    /// Get the current buffer contents as a contiguous slice.
    ///
    /// Note: This may need to reallocate if the buffer wraps around.
    #[must_use]
    pub fn as_slice(&mut self) -> &[u8] {
        self.data.make_contiguous()
    }

    /// Get the current buffer contents as a string (lossy UTF-8 conversion).
    #[must_use]
    pub fn as_str_lossy(&mut self) -> String {
        String::from_utf8_lossy(self.as_slice()).into_owned()
    }

    /// Get the current length of the buffer.
    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get the maximum size of the buffer.
    #[must_use]
    pub const fn max_size(&self) -> usize {
        self.max_size
    }

    /// Get the total bytes written to the buffer.
    #[must_use]
    pub const fn total_written(&self) -> usize {
        self.total_written
    }

    /// Get the number of bytes that have been discarded due to overflow.
    #[must_use]
    pub const fn bytes_discarded(&self) -> usize {
        self.bytes_discarded
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Find a byte sequence in the buffer.
    ///
    /// Returns the position of the first match.
    #[must_use]
    pub fn find(&mut self, needle: &[u8]) -> Option<usize> {
        if needle.is_empty() {
            return Some(0);
        }
        if needle.len() > self.data.len() {
            return None;
        }

        let data = self.as_slice();
        data.windows(needle.len())
            .position(|window| window == needle)
    }

    /// Find a string in the buffer.
    #[must_use]
    pub fn find_str(&mut self, needle: &str) -> Option<usize> {
        self.find(needle.as_bytes())
    }

    /// Consume data up to and including the specified position.
    ///
    /// Returns the consumed data.
    pub fn consume(&mut self, end: usize) -> Vec<u8> {
        let end = end.min(self.data.len());
        self.data.drain(..end).collect()
    }

    /// Consume data up to (but not including) the specified position.
    ///
    /// Returns the consumed data as a string (lossy conversion).
    pub fn consume_before(&mut self, pos: usize) -> String {
        let data = self.consume(pos);
        String::from_utf8_lossy(&data).into_owned()
    }

    /// Consume data up to and including a pattern match.
    ///
    /// Returns (`before_match`, `matched_text`) if found.
    pub fn consume_until(&mut self, needle: &str) -> Option<(String, String)> {
        let pos = self.find_str(needle)?;
        let before = self.consume_before(pos);
        let matched = self.consume(needle.len());
        Some((before, String::from_utf8_lossy(&matched).into_owned()))
    }

    /// Get a slice of the last N bytes in the buffer.
    #[must_use]
    pub fn tail(&mut self, n: usize) -> Vec<u8> {
        let data = self.as_slice();
        let start = data.len().saturating_sub(n);
        data[start..].to_vec()
    }

    /// Get a slice of the first N bytes in the buffer.
    #[must_use]
    pub fn head(&mut self, n: usize) -> Vec<u8> {
        let data = self.as_slice();
        let end = n.min(data.len());
        data[..end].to_vec()
    }

    /// Search within a window at the end of the buffer.
    ///
    /// This is more efficient for large buffers when patterns
    /// are expected near the end.
    #[must_use]
    pub fn find_in_tail(&mut self, needle: &[u8], window_size: usize) -> Option<usize> {
        let data = self.as_slice();
        let search_start = data.len().saturating_sub(window_size);
        let search_data = &data[search_start..];

        search_data
            .windows(needle.len())
            .position(|w| w == needle)
            .map(|pos| search_start + pos)
    }

    /// Apply a function to search the buffer contents.
    ///
    /// This is useful for complex pattern matching like regex.
    pub fn search<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&str) -> R,
    {
        let s = self.as_str_lossy();
        f(&s)
    }
}

impl Default for RingBuffer {
    fn default() -> Self {
        Self::with_default_capacity()
    }
}

impl fmt::Debug for RingBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RingBuffer")
            .field("len", &self.len())
            .field("max_size", &self.max_size)
            .field("total_written", &self.total_written)
            .field("bytes_discarded", &self.bytes_discarded)
            .finish()
    }
}

impl std::io::Write for RingBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.append(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_append() {
        let mut buf = RingBuffer::new(100);
        buf.append(b"hello");
        assert_eq!(buf.len(), 5);
        assert_eq!(buf.as_slice(), b"hello");
    }

    #[test]
    fn overflow_discards_oldest() {
        let mut buf = RingBuffer::new(10);
        buf.append(b"12345");
        buf.append(b"67890");
        buf.append(b"abc");

        assert_eq!(buf.len(), 10);
        // After overflow, we should have the last 10 bytes
        assert_eq!(buf.as_str_lossy(), "4567890abc");
    }

    #[test]
    fn find_pattern() {
        let mut buf = RingBuffer::new(100);
        buf.append(b"hello world");
        assert_eq!(buf.find(b"world"), Some(6));
        assert_eq!(buf.find(b"foo"), None);
    }

    #[test]
    fn consume_until() {
        let mut buf = RingBuffer::new(100);
        buf.append(b"login: username");
        let result = buf.consume_until("login:");
        assert!(result.is_some());
        let (before, matched) = result.unwrap();
        assert_eq!(before, "");
        assert_eq!(matched, "login:");
        assert_eq!(buf.as_str_lossy(), " username");
    }

    #[test]
    fn tail_and_head() {
        let mut buf = RingBuffer::new(100);
        buf.append(b"hello world");
        assert_eq!(buf.tail(5), b"world".to_vec());
        assert_eq!(buf.head(5), b"hello".to_vec());
    }

    #[test]
    fn find_in_tail() {
        let mut buf = RingBuffer::new(100);
        buf.append(b"the quick brown fox jumps over the lazy dog");
        // Should find "lazy" in the last 20 bytes
        assert!(buf.find_in_tail(b"lazy", 20).is_some());
        // Should not find "quick" in the last 20 bytes
        assert!(buf.find_in_tail(b"quick", 20).is_none());
    }

    #[test]
    fn write_trait() {
        use std::io::Write;

        let mut buf = RingBuffer::new(100);
        write!(buf, "hello world").unwrap();
        assert_eq!(buf.as_str_lossy(), "hello world");
    }
}
