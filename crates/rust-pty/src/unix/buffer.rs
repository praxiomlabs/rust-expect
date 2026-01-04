//! Zero-copy buffer for efficient PTY I/O.
//!
//! This module provides a ring buffer optimized for PTY communication,
//! minimizing memory allocations and copies during read/write operations.

use std::io;

/// Default buffer capacity (16KB).
pub const DEFAULT_CAPACITY: usize = 16 * 1024;

/// A ring buffer for efficient PTY I/O operations.
///
/// This buffer is designed for the producer-consumer pattern typical
/// of PTY communication, where data is written in chunks and read
/// as it becomes available.
#[derive(Debug)]
pub struct PtyBuffer {
    /// The underlying storage.
    data: Box<[u8]>,
    /// Read position (consumer).
    read_pos: usize,
    /// Write position (producer).
    write_pos: usize,
}

impl PtyBuffer {
    /// Create a new buffer with the specified capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            data: vec![0u8; capacity].into_boxed_slice(),
            read_pos: 0,
            write_pos: 0,
        }
    }

    /// Create a new buffer with default capacity.
    #[must_use]
    pub fn with_default_capacity() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }

    /// Returns the total capacity of the buffer.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.data.len()
    }

    /// Returns the number of bytes available to read.
    #[must_use]
    pub const fn len(&self) -> usize {
        if self.write_pos >= self.read_pos {
            self.write_pos - self.read_pos
        } else {
            self.capacity() - self.read_pos + self.write_pos
        }
    }

    /// Returns true if the buffer contains no data.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.read_pos == self.write_pos
    }

    /// Returns the number of bytes that can be written.
    #[must_use]
    pub const fn available(&self) -> usize {
        // Reserve one byte to distinguish full from empty
        self.capacity() - self.len() - 1
    }

    /// Returns true if the buffer is full.
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.available() == 0
    }

    /// Get a slice of contiguous readable data.
    ///
    /// This may not return all available data if it wraps around the buffer.
    /// Call `consume()` after processing, then call again for remaining data.
    #[must_use]
    pub fn readable(&self) -> &[u8] {
        if self.write_pos >= self.read_pos {
            &self.data[self.read_pos..self.write_pos]
        } else {
            // Return data up to the end of the buffer
            &self.data[self.read_pos..]
        }
    }

    /// Get a mutable slice for writing data.
    ///
    /// This may not return all available space if it wraps around the buffer.
    /// Call `produce()` after writing, then call again for remaining space.
    #[must_use]
    pub fn writable(&mut self) -> &mut [u8] {
        let cap = self.capacity();
        if self.write_pos >= self.read_pos {
            // Can write to end of buffer (but not wrap to position 0 if read_pos is 0)
            let end = if self.read_pos == 0 { cap - 1 } else { cap };
            &mut self.data[self.write_pos..end]
        } else {
            // Can write up to read_pos - 1 (leave one byte gap)
            &mut self.data[self.write_pos..self.read_pos - 1]
        }
    }

    /// Mark `count` bytes as consumed (read).
    ///
    /// # Panics
    ///
    /// Panics if `count` exceeds the available data.
    pub fn consume(&mut self, count: usize) {
        assert!(count <= self.len(), "cannot consume more than available");
        self.read_pos = (self.read_pos + count) % self.capacity();
    }

    /// Mark `count` bytes as produced (written).
    ///
    /// # Panics
    ///
    /// Panics if `count` exceeds the available space.
    pub fn produce(&mut self, count: usize) {
        assert!(count <= self.available(), "cannot produce more than available");
        self.write_pos = (self.write_pos + count) % self.capacity();
    }

    /// Clear all data from the buffer.
    pub fn clear(&mut self) {
        self.read_pos = 0;
        self.write_pos = 0;
    }

    /// Read data from the buffer into the provided slice.
    ///
    /// Returns the number of bytes read.
    pub fn read(&mut self, buf: &mut [u8]) -> usize {
        let mut total = 0;

        while total < buf.len() && !self.is_empty() {
            let readable = self.readable();
            let to_copy = readable.len().min(buf.len() - total);
            buf[total..total + to_copy].copy_from_slice(&readable[..to_copy]);
            self.consume(to_copy);
            total += to_copy;
        }

        total
    }

    /// Write data to the buffer from the provided slice.
    ///
    /// Returns the number of bytes written.
    pub fn write(&mut self, data: &[u8]) -> usize {
        let mut total = 0;

        while total < data.len() && !self.is_full() {
            let writable = self.writable();
            let to_copy = writable.len().min(data.len() - total);
            writable[..to_copy].copy_from_slice(&data[total..total + to_copy]);
            self.produce(to_copy);
            total += to_copy;
        }

        total
    }
}

impl Default for PtyBuffer {
    fn default() -> Self {
        Self::with_default_capacity()
    }
}

impl io::Read for PtyBuffer {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        Ok(self.read(buf))
    }
}

impl io::Write for PtyBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(self.write(buf))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_buffer_is_empty() {
        let buf = PtyBuffer::new(1024);
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
        assert_eq!(buf.capacity(), 1024);
    }

    #[test]
    fn write_and_read() {
        let mut buf = PtyBuffer::new(1024);
        let data = b"hello world";

        let written = buf.write(data);
        assert_eq!(written, data.len());
        assert_eq!(buf.len(), data.len());

        let mut output = [0u8; 32];
        let read = buf.read(&mut output);
        assert_eq!(read, data.len());
        assert_eq!(&output[..read], data);
        assert!(buf.is_empty());
    }

    #[test]
    fn wrap_around() {
        let mut buf = PtyBuffer::new(16);

        // Fill most of the buffer
        let data1 = b"12345678901";
        buf.write(data1);

        // Consume some
        let mut tmp = [0u8; 8];
        buf.read(&mut tmp);

        // Write more (should wrap)
        let data2 = b"abcdefgh";
        let written = buf.write(data2);
        assert!(written > 0);

        // Read all
        let mut output = [0u8; 32];
        let total = buf.read(&mut output);
        assert!(!output[..total].is_empty());
    }

    #[test]
    fn clear_resets_buffer() {
        let mut buf = PtyBuffer::new(1024);
        buf.write(b"test data");
        assert!(!buf.is_empty());

        buf.clear();
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
    }
}
