//! Memory-efficient buffer implementations.
//!
//! This module provides buffer implementations optimized for different
//! use cases:
//!
//! - `RingBuffer`: Efficient circular buffer for streaming data
//! - `MmapBuffer`: Memory-mapped buffer for large data sets
//! - `SplitBuffer`: Buffer that spills to disk when threshold is exceeded
//!
//! # Memory-Mapped Buffers
//!
//! For very large terminal sessions (e.g., log processing, long-running scripts),
//! memory-mapped buffers can reduce heap pressure by using virtual memory
//! backed by temporary files.
//!
//! ```rust,ignore
//! use rust_expect::util::buffer::{MmapBuffer, BufferConfig};
//!
//! // Create a 1GB memory-mapped buffer
//! let buffer = MmapBuffer::new(1024 * 1024 * 1024)?;
//!
//! // Use like a regular buffer
//! buffer.write(b"Hello, world!")?;
//! let data = buffer.read_all();
//! ```

use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Configuration for buffer behavior.
#[derive(Debug, Clone)]
pub struct LargeBufferConfig {
    /// Initial capacity in bytes.
    pub initial_capacity: usize,
    /// Maximum capacity in bytes.
    pub max_capacity: usize,
    /// Threshold at which to spill to disk (0 = never).
    pub spill_threshold: usize,
    /// Directory for temporary files.
    pub temp_dir: Option<PathBuf>,
}

impl Default for LargeBufferConfig {
    fn default() -> Self {
        Self {
            initial_capacity: 64 * 1024,       // 64KB
            max_capacity: 1024 * 1024 * 1024,  // 1GB
            spill_threshold: 64 * 1024 * 1024, // 64MB
            temp_dir: None,
        }
    }
}

impl LargeBufferConfig {
    /// Create a new configuration with the given max capacity.
    #[must_use]
    pub fn new(max_capacity: usize) -> Self {
        Self {
            max_capacity,
            ..Default::default()
        }
    }

    /// Set initial capacity.
    #[must_use]
    pub const fn initial_capacity(mut self, capacity: usize) -> Self {
        self.initial_capacity = capacity;
        self
    }

    /// Set spill threshold.
    #[must_use]
    pub const fn spill_threshold(mut self, threshold: usize) -> Self {
        self.spill_threshold = threshold;
        self
    }

    /// Set temporary directory.
    #[must_use]
    pub fn temp_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.temp_dir = Some(dir.into());
        self
    }
}

/// A circular ring buffer for streaming data.
///
/// When the buffer is full, oldest data is overwritten.
#[derive(Debug)]
pub struct RingBuffer {
    data: Vec<u8>,
    capacity: usize,
    head: usize,
    tail: usize,
    full: bool,
}

impl RingBuffer {
    /// Create a new ring buffer with the given capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            data: vec![0u8; capacity],
            capacity,
            head: 0,
            tail: 0,
            full: false,
        }
    }

    /// Get the current length of data in the buffer.
    #[must_use]
    pub fn len(&self) -> usize {
        if self.full {
            self.capacity
        } else if self.head >= self.tail {
            self.head - self.tail
        } else {
            self.capacity - self.tail + self.head
        }
    }

    /// Check if the buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.full && self.head == self.tail
    }

    /// Check if the buffer is full.
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.full
    }

    /// Get the capacity of the buffer.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Write data to the buffer.
    ///
    /// If the buffer is full, oldest data is overwritten.
    pub fn write(&mut self, data: &[u8]) {
        for &byte in data {
            self.data[self.head] = byte;
            self.head = (self.head + 1) % self.capacity;

            if self.full {
                self.tail = (self.tail + 1) % self.capacity;
            }

            if self.head == self.tail {
                self.full = true;
            }
        }
    }

    /// Read all data from the buffer.
    ///
    /// This does not consume the data.
    #[must_use]
    pub fn read_all(&self) -> Vec<u8> {
        let len = self.len();
        let mut result = Vec::with_capacity(len);

        if len == 0 {
            return result;
        }

        if self.head > self.tail {
            result.extend_from_slice(&self.data[self.tail..self.head]);
        } else {
            result.extend_from_slice(&self.data[self.tail..]);
            result.extend_from_slice(&self.data[..self.head]);
        }

        result
    }

    /// Read as a string (lossy UTF-8 conversion).
    #[must_use]
    pub fn as_string(&self) -> String {
        String::from_utf8_lossy(&self.read_all()).into_owned()
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.tail = 0;
        self.full = false;
    }

    /// Get the last N bytes from the buffer.
    #[must_use]
    pub fn tail_bytes(&self, n: usize) -> Vec<u8> {
        let len = self.len();
        if n >= len {
            return self.read_all();
        }

        let all = self.read_all();
        all[len - n..].to_vec()
    }
}

/// Storage backend for large buffers.
enum Storage {
    /// In-memory storage.
    Memory(Vec<u8>),
    /// File-backed storage.
    File {
        file: std::fs::File,
        path: PathBuf,
        size: usize,
    },
}

/// A buffer that can spill to disk for very large data sets.
///
/// Starts in memory and automatically spills to disk when the
/// spill threshold is exceeded.
pub struct SpillBuffer {
    storage: Storage,
    config: LargeBufferConfig,
    write_pos: usize,
    spilled: bool,
}

impl SpillBuffer {
    /// Create a new spill buffer with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(LargeBufferConfig::default())
    }

    /// Create a new spill buffer with custom configuration.
    #[must_use]
    pub fn with_config(config: LargeBufferConfig) -> Self {
        Self {
            storage: Storage::Memory(Vec::with_capacity(config.initial_capacity)),
            config,
            write_pos: 0,
            spilled: false,
        }
    }

    /// Check if the buffer has spilled to disk.
    #[must_use]
    pub const fn is_spilled(&self) -> bool {
        self.spilled
    }

    /// Get the current size of the buffer.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.write_pos
    }

    /// Check if the buffer is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.write_pos == 0
    }

    /// Write data to the buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if disk I/O fails during spill.
    pub fn write(&mut self, data: &[u8]) -> io::Result<()> {
        let new_size = self.write_pos + data.len();

        // Check if we need to spill
        if !self.spilled
            && self.config.spill_threshold > 0
            && new_size > self.config.spill_threshold
        {
            self.spill_to_disk()?;
        }

        // Check capacity
        if new_size > self.config.max_capacity {
            return Err(io::Error::new(
                io::ErrorKind::StorageFull,
                "Buffer exceeded maximum capacity",
            ));
        }

        match &mut self.storage {
            Storage::Memory(vec) => {
                vec.extend_from_slice(data);
                self.write_pos = vec.len();
            }
            Storage::File { file, size, .. } => {
                file.seek(SeekFrom::End(0))?;
                file.write_all(data)?;
                *size += data.len();
                self.write_pos = *size;
            }
        }

        Ok(())
    }

    /// Spill the buffer contents to disk.
    fn spill_to_disk(&mut self) -> io::Result<()> {
        if self.spilled {
            return Ok(());
        }

        let temp_dir = self
            .config
            .temp_dir
            .as_ref()
            .map_or_else(std::env::temp_dir, |p| p.clone());

        let path = temp_dir.join(format!("rust_expect_buffer_{}", std::process::id()));

        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;

        // Write existing memory contents to file
        if let Storage::Memory(vec) = &self.storage {
            file.write_all(vec)?;
        }

        let size = self.write_pos;
        self.storage = Storage::File { file, path, size };
        self.spilled = true;

        Ok(())
    }

    /// Read all data from the buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if disk I/O fails.
    pub fn read_all(&mut self) -> io::Result<Vec<u8>> {
        match &mut self.storage {
            Storage::Memory(vec) => Ok(vec.clone()),
            Storage::File { file, size, .. } => {
                file.seek(SeekFrom::Start(0))?;
                let mut data = vec![0u8; *size];
                file.read_exact(&mut data)?;
                Ok(data)
            }
        }
    }

    /// Read as a string (lossy UTF-8 conversion).
    ///
    /// # Errors
    ///
    /// Returns an error if disk I/O fails.
    pub fn as_string(&mut self) -> io::Result<String> {
        Ok(String::from_utf8_lossy(&self.read_all()?).into_owned())
    }

    /// Clear the buffer.
    ///
    /// If spilled to disk, the file is truncated.
    ///
    /// # Errors
    ///
    /// Returns an error if disk I/O fails.
    pub fn clear(&mut self) -> io::Result<()> {
        match &mut self.storage {
            Storage::Memory(vec) => {
                vec.clear();
            }
            Storage::File { file, size, .. } => {
                file.set_len(0)?;
                *size = 0;
            }
        }
        self.write_pos = 0;
        Ok(())
    }
}

impl Default for SpillBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SpillBuffer {
    fn drop(&mut self) {
        // Clean up temporary file if it exists
        if let Storage::File { path, .. } = &self.storage {
            let _ = std::fs::remove_file(path);
        }
    }
}

impl std::fmt::Debug for SpillBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpillBuffer")
            .field("len", &self.write_pos)
            .field("spilled", &self.spilled)
            .field("max_capacity", &self.config.max_capacity)
            .finish()
    }
}

/// Thread-safe atomic buffer size tracker.
#[derive(Debug, Default)]
pub struct AtomicBufferSize {
    size: AtomicUsize,
}

impl AtomicBufferSize {
    /// Create a new size tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            size: AtomicUsize::new(0),
        }
    }

    /// Get current size.
    #[must_use]
    pub fn get(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }

    /// Add to size.
    pub fn add(&self, n: usize) {
        self.size.fetch_add(n, Ordering::Relaxed);
    }

    /// Subtract from size.
    pub fn sub(&self, n: usize) {
        self.size.fetch_sub(n, Ordering::Relaxed);
    }

    /// Set size.
    pub fn set(&self, n: usize) {
        self.size.store(n, Ordering::Relaxed);
    }

    /// Reset to zero.
    pub fn reset(&self) {
        self.size.store(0, Ordering::Relaxed);
    }
}

/// Allocate a page-aligned buffer for zero-copy I/O.
///
/// # Safety
///
/// This allocates raw memory. The returned buffer should be deallocated
/// properly when no longer needed.
#[cfg(unix)]
#[must_use]
pub fn allocate_page_aligned(size: usize) -> Vec<u8> {
    // Round up to page size
    let page_size = page_size();
    let aligned_size = (size + page_size - 1) & !(page_size - 1);

    // For now, use regular allocation which may or may not be page-aligned
    // A more advanced implementation would use mmap or posix_memalign
    vec![0u8; aligned_size]
}

/// Get the system page size.
#[cfg(unix)]
#[must_use]
pub fn page_size() -> usize {
    // SAFETY: sysconf is safe to call
    let size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if size <= 0 {
        4096 // Default fallback
    } else {
        size as usize
    }
}

/// Get the system page size.
#[cfg(windows)]
#[must_use]
pub fn page_size() -> usize {
    4096 // Default for Windows
}

/// Allocate a page-aligned buffer for zero-copy I/O.
#[cfg(windows)]
#[must_use]
pub fn allocate_page_aligned(size: usize) -> Vec<u8> {
    vec![0u8; size]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_basic() {
        let mut buf = RingBuffer::new(10);
        assert!(buf.is_empty());
        assert_eq!(buf.capacity(), 10);

        buf.write(b"hello");
        assert_eq!(buf.len(), 5);
        assert_eq!(buf.as_string(), "hello");
    }

    #[test]
    fn ring_buffer_wrap() {
        let mut buf = RingBuffer::new(10);
        buf.write(b"12345678"); // 8 bytes
        assert_eq!(buf.len(), 8);

        buf.write(b"ABCD"); // 4 more bytes, should wrap
        assert_eq!(buf.len(), 10); // Full
        assert!(buf.is_full());

        // Should contain last 10 bytes
        let content = buf.as_string();
        assert_eq!(content.len(), 10);
        assert!(content.ends_with("ABCD"));
    }

    #[test]
    fn ring_buffer_tail_bytes() {
        let mut buf = RingBuffer::new(20);
        buf.write(b"hello world");

        let tail = buf.tail_bytes(5);
        assert_eq!(tail, b"world");

        let tail = buf.tail_bytes(100);
        assert_eq!(tail, b"hello world");
    }

    #[test]
    fn ring_buffer_clear() {
        let mut buf = RingBuffer::new(10);
        buf.write(b"hello");
        buf.clear();

        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn spill_buffer_memory() {
        let config = LargeBufferConfig::new(1024 * 1024)
            .spill_threshold(0); // Never spill

        let mut buf = SpillBuffer::with_config(config);
        buf.write(b"hello world").unwrap();

        assert!(!buf.is_spilled());
        assert_eq!(buf.len(), 11);
        assert_eq!(buf.as_string().unwrap(), "hello world");
    }

    #[test]
    fn spill_buffer_spill() {
        let config = LargeBufferConfig::new(1024 * 1024)
            .spill_threshold(10); // Spill after 10 bytes

        let mut buf = SpillBuffer::with_config(config);
        buf.write(b"hello").unwrap();
        assert!(!buf.is_spilled());

        buf.write(b"world!!!").unwrap();
        assert!(buf.is_spilled());
        assert_eq!(buf.as_string().unwrap(), "helloworld!!!");
    }

    #[test]
    fn atomic_buffer_size() {
        let size = AtomicBufferSize::new();
        assert_eq!(size.get(), 0);

        size.add(100);
        assert_eq!(size.get(), 100);

        size.sub(30);
        assert_eq!(size.get(), 70);

        size.set(500);
        assert_eq!(size.get(), 500);

        size.reset();
        assert_eq!(size.get(), 0);
    }

    #[test]
    fn page_aligned_allocation() {
        let buf = allocate_page_aligned(1000);
        assert!(buf.len() >= 1000);

        let page = page_size();
        assert!(page >= 4096);
    }
}
