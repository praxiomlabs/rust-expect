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
pub use buffer::{DEFAULT_CAPACITY, RingBuffer};
pub use cache::{DEFAULT_CACHE_SIZE, GLOBAL_CACHE, RegexCache, get_regex};
pub use large_buffer::{AdaptiveBuffer, LargeBuffer, MMAP_THRESHOLD};
pub use matcher::{ExpectState, MatchResult, Matcher};
pub use pattern::{CompiledRegex, NamedPattern, Pattern, PatternMatch, PatternSet};
