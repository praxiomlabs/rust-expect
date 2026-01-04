//! Utility modules for expect operations.
//!
//! This module provides various utilities for timeout handling,
//! byte manipulation, backpressure management, memory-efficient buffers,
//! and zero-copy I/O operations.

pub mod backpressure;
pub mod buffer;
pub mod bytes;
pub mod timeout;
pub mod zerocopy;

// Re-export commonly used types
pub use backpressure::{Backpressure, RateLimiter, TokenBucket};
pub use buffer::{
    AtomicBufferSize, LargeBufferConfig, RingBuffer, SpillBuffer, allocate_page_aligned, page_size,
};
pub use bytes::{
    EscapedBytes, escape_bytes, find_all_patterns, find_pattern, hexdump, replace_pattern,
    strip_ansi, to_visible_string, unescape_bytes,
};
pub use timeout::{Deadline, TimeoutConfig, TimeoutExt};
pub use zerocopy::{BorrowedView, BytesBuffer, ReadPool, VecWriter, ZeroCopySource};
