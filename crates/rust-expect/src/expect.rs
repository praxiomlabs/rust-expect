//! Expect pattern matching module.
//!
//! This module provides the core pattern matching functionality for expect operations,
//! including pattern types, buffer management, regex caching, and match handling.

mod before_after;
mod buffer;
mod cache;
mod large_buffer;
mod matcher;
mod pattern;

pub use before_after::{
    HandlerAction, PatternBuilder, PatternHandler, PatternManager, PersistentPattern,
};
pub use buffer::{RingBuffer, DEFAULT_CAPACITY};
pub use cache::{get_regex, RegexCache, DEFAULT_CACHE_SIZE, GLOBAL_CACHE};
pub use large_buffer::{AdaptiveBuffer, LargeBuffer, MMAP_THRESHOLD};
pub use matcher::{ExpectState, MatchResult, Matcher};
pub use pattern::{CompiledRegex, NamedPattern, Pattern, PatternMatch, PatternSet};
