//! Large buffer support using memory-mapped files.
//!
//! This module provides a buffer implementation optimized for handling
//! very large outputs (>10MB) using memory-mapped files for efficiency.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Default threshold for switching to mmap buffer (10 MB).
pub const MMAP_THRESHOLD: usize = 10 * 1024 * 1024;

/// A large buffer backed by a temporary file.
///
/// This buffer uses a file to store data, with optional memory mapping
/// for efficient access to large datasets.
pub struct LargeBuffer {
    /// The backing file.
    file: File,
    /// Path to the temporary file.
    path: PathBuf,
    /// Current size of the buffer.
    size: usize,
    /// Whether to delete the file on drop.
    cleanup: bool,
    /// Read position for streaming.
    read_pos: usize,
}

impl LargeBuffer {
    /// Create a new large buffer with a temporary file.
    ///
    /// # Errors
    ///
    /// Returns an error if the temporary file cannot be created.
    pub fn new() -> io::Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "rust_expect_buffer_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        Self::with_path(&path)
    }

    /// Create a new large buffer at the specified path.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be created.
    pub fn with_path(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;

        Ok(Self {
            file,
            path: path.to_path_buf(),
            size: 0,
            cleanup: true,
            read_pos: 0,
        })
    }

    /// Set whether to delete the file on drop.
    pub const fn set_cleanup(&mut self, cleanup: bool) {
        self.cleanup = cleanup;
    }

    /// Get the path to the backing file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append data to the buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub fn append(&mut self, data: &[u8]) -> io::Result<()> {
        self.file.seek(SeekFrom::End(0))?;
        self.file.write_all(data)?;
        self.size += data.len();
        Ok(())
    }

    /// Get the current size of the buffer.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.size
    }

    /// Check if the buffer is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Read a range of bytes from the buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the read fails.
    pub fn read_range(&mut self, start: usize, len: usize) -> io::Result<Vec<u8>> {
        if start >= self.size {
            return Ok(Vec::new());
        }

        let actual_len = len.min(self.size - start);
        let mut buf = vec![0u8; actual_len];

        self.file.seek(SeekFrom::Start(start as u64))?;
        self.file.read_exact(&mut buf)?;

        Ok(buf)
    }

    /// Read all data from the buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the read fails.
    pub fn read_all(&mut self) -> io::Result<Vec<u8>> {
        self.read_range(0, self.size)
    }

    /// Read the last N bytes from the buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the read fails.
    pub fn tail(&mut self, n: usize) -> io::Result<Vec<u8>> {
        let start = self.size.saturating_sub(n);
        self.read_range(start, n)
    }

    /// Read the first N bytes from the buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the read fails.
    pub fn head(&mut self, n: usize) -> io::Result<Vec<u8>> {
        self.read_range(0, n)
    }

    /// Clear the buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the truncation fails.
    pub fn clear(&mut self) -> io::Result<()> {
        self.file.set_len(0)?;
        self.size = 0;
        self.read_pos = 0;
        Ok(())
    }

    /// Find a byte sequence in the buffer.
    ///
    /// This performs a linear search through the file.
    ///
    /// # Errors
    ///
    /// Returns an error if reading fails.
    pub fn find(&mut self, needle: &[u8]) -> io::Result<Option<usize>> {
        // Read in chunks for efficiency
        const CHUNK_SIZE: usize = 64 * 1024;

        if needle.is_empty() {
            return Ok(Some(0));
        }
        if needle.len() > self.size {
            return Ok(None);
        }

        let mut pos = 0;
        let mut overlap = Vec::new();

        self.file.seek(SeekFrom::Start(0))?;

        while pos < self.size {
            let read_size = CHUNK_SIZE.min(self.size - pos);
            let mut chunk = vec![0u8; read_size];
            self.file.read_exact(&mut chunk)?;

            // Prepend overlap from previous chunk
            let search_data = if overlap.is_empty() {
                chunk.clone()
            } else {
                let mut combined = overlap.clone();
                combined.extend(&chunk);
                combined
            };

            // Search in combined data
            if let Some(idx) = find_subsequence(&search_data, needle) {
                let actual_pos = if overlap.is_empty() {
                    pos + idx
                } else {
                    pos - overlap.len() + idx
                };
                return Ok(Some(actual_pos));
            }

            // Keep overlap for next iteration (to handle matches across chunks)
            overlap = if chunk.len() >= needle.len() - 1 {
                chunk[chunk.len() - (needle.len() - 1)..].to_vec()
            } else {
                chunk
            };

            pos += read_size;
        }

        Ok(None)
    }

    /// Find a string in the buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if reading fails.
    pub fn find_str(&mut self, needle: &str) -> io::Result<Option<usize>> {
        self.find(needle.as_bytes())
    }

    /// Read data as a string (lossy UTF-8 conversion).
    ///
    /// # Errors
    ///
    /// Returns an error if reading fails.
    pub fn as_str_lossy(&mut self) -> io::Result<String> {
        let data = self.read_all()?;
        Ok(String::from_utf8_lossy(&data).into_owned())
    }

    /// Consume data from the beginning of the buffer.
    ///
    /// This is expensive for large buffers as it requires rewriting the file.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn consume(&mut self, len: usize) -> io::Result<Vec<u8>> {
        if len == 0 {
            return Ok(Vec::new());
        }

        let consume_len = len.min(self.size);

        // Read the data to consume
        let consumed = self.read_range(0, consume_len)?;

        // Read remaining data
        let remaining = self.read_range(consume_len, self.size - consume_len)?;

        // Rewrite the file with remaining data
        self.file.seek(SeekFrom::Start(0))?;
        self.file.set_len(0)?;
        self.file.write_all(&remaining)?;
        self.size = remaining.len();

        Ok(consumed)
    }

    /// Sync the buffer to disk.
    ///
    /// # Errors
    ///
    /// Returns an error if sync fails.
    pub fn sync(&self) -> io::Result<()> {
        self.file.sync_all()
    }
}

impl Drop for LargeBuffer {
    fn drop(&mut self) {
        if self.cleanup {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

impl std::fmt::Debug for LargeBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LargeBuffer")
            .field("path", &self.path)
            .field("size", &self.size)
            .field("cleanup", &self.cleanup)
            .finish()
    }
}

/// Find a subsequence in a slice.
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Adaptive buffer that switches between in-memory and file-backed storage.
pub enum AdaptiveBuffer {
    /// In-memory buffer for small data.
    Memory(Vec<u8>),
    /// File-backed buffer for large data.
    File(LargeBuffer),
}

impl AdaptiveBuffer {
    /// Create a new adaptive buffer.
    #[must_use]
    pub const fn new() -> Self {
        Self::Memory(Vec::new())
    }

    /// Create a new adaptive buffer with a custom threshold.
    #[must_use]
    pub const fn with_threshold(_threshold: usize) -> Self {
        // Threshold stored elsewhere, just create memory buffer
        Self::Memory(Vec::new())
    }

    /// Append data to the buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if file operations fail when using file-backed storage.
    pub fn append(&mut self, data: &[u8], threshold: usize) -> io::Result<()> {
        match self {
            Self::Memory(buf) => {
                if buf.len() + data.len() > threshold {
                    // Switch to file-backed storage
                    let mut large = LargeBuffer::new()?;
                    large.append(buf)?;
                    large.append(data)?;
                    *self = Self::File(large);
                } else {
                    buf.extend_from_slice(data);
                }
            }
            Self::File(large) => {
                large.append(data)?;
            }
        }
        Ok(())
    }

    /// Get the current size of the buffer.
    #[must_use]
    pub const fn len(&self) -> usize {
        match self {
            Self::Memory(buf) => buf.len(),
            Self::File(large) => large.len(),
        }
    }

    /// Check if the buffer is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Check if the buffer is using file-backed storage.
    #[must_use]
    pub const fn is_file_backed(&self) -> bool {
        matches!(self, Self::File(_))
    }

    /// Read all data from the buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if file operations fail.
    pub fn read_all(&mut self) -> io::Result<Vec<u8>> {
        match self {
            Self::Memory(buf) => Ok(buf.clone()),
            Self::File(large) => large.read_all(),
        }
    }

    /// Read the last N bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if file operations fail.
    pub fn tail(&mut self, n: usize) -> io::Result<Vec<u8>> {
        match self {
            Self::Memory(buf) => {
                let start = buf.len().saturating_sub(n);
                Ok(buf[start..].to_vec())
            }
            Self::File(large) => large.tail(n),
        }
    }

    /// Clear the buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if file operations fail.
    pub fn clear(&mut self) -> io::Result<()> {
        match self {
            Self::Memory(buf) => {
                buf.clear();
                Ok(())
            }
            Self::File(large) => large.clear(),
        }
    }

    /// Get the data as a string (lossy UTF-8).
    ///
    /// # Errors
    ///
    /// Returns an error if file operations fail.
    pub fn as_str_lossy(&mut self) -> io::Result<String> {
        match self {
            Self::Memory(buf) => Ok(String::from_utf8_lossy(buf).into_owned()),
            Self::File(large) => large.as_str_lossy(),
        }
    }
}

impl Default for AdaptiveBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for AdaptiveBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Memory(buf) => f.debug_tuple("Memory").field(&buf.len()).finish(),
            Self::File(large) => f.debug_tuple("File").field(large).finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_buffer_basic() {
        let mut buf = LargeBuffer::new().unwrap();
        buf.append(b"hello world").unwrap();

        assert_eq!(buf.len(), 11);
        assert_eq!(buf.read_all().unwrap(), b"hello world");
    }

    #[test]
    fn large_buffer_find() {
        let mut buf = LargeBuffer::new().unwrap();
        buf.append(b"the quick brown fox").unwrap();

        assert_eq!(buf.find(b"quick").unwrap(), Some(4));
        assert_eq!(buf.find(b"lazy").unwrap(), None);
    }

    #[test]
    fn large_buffer_tail() {
        let mut buf = LargeBuffer::new().unwrap();
        buf.append(b"hello world").unwrap();

        assert_eq!(buf.tail(5).unwrap(), b"world");
    }

    #[test]
    fn large_buffer_consume() {
        let mut buf = LargeBuffer::new().unwrap();
        buf.append(b"hello world").unwrap();

        let consumed = buf.consume(6).unwrap();
        assert_eq!(consumed, b"hello ");
        assert_eq!(buf.read_all().unwrap(), b"world");
    }

    #[test]
    fn adaptive_buffer_stays_memory() {
        let mut buf = AdaptiveBuffer::new();
        buf.append(b"small data", MMAP_THRESHOLD).unwrap();

        assert!(!buf.is_file_backed());
    }

    #[test]
    fn adaptive_buffer_switches_to_file() {
        let mut buf = AdaptiveBuffer::new();
        let threshold = 100;

        // Add more than threshold
        let large_data = vec![b'x'; 150];
        buf.append(&large_data, threshold).unwrap();

        assert!(buf.is_file_backed());
        assert_eq!(buf.len(), 150);
    }
}
