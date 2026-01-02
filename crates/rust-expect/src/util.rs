//! Utility modules for expect operations.
//!
//! This module provides various utilities for timeout handling,
//! byte manipulation, and backpressure management.

pub mod backpressure;
pub mod bytes;
pub mod timeout;

// Re-export commonly used types
pub use backpressure::{Backpressure, RateLimiter, TokenBucket};
pub use bytes::{
    escape_bytes, find_all_patterns, find_pattern, hexdump, replace_pattern, strip_ansi,
    to_visible_string, unescape_bytes, EscapedBytes,
};
pub use timeout::{Deadline, TimeoutConfig, TimeoutExt};
