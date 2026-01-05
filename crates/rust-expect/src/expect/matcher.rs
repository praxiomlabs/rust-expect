//! Pattern matching engine for expect operations.
//!
//! This module provides the core matching engine that combines
//! patterns, buffers, and timeouts into a cohesive expect operation.

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::buffer::RingBuffer;
use super::cache::RegexCache;
use super::pattern::{Pattern, PatternSet};
use crate::types::Match;

/// The pattern matching engine.
pub struct Matcher {
    /// The output buffer.
    buffer: RingBuffer,
    /// Regex cache for compiled patterns.
    cache: Arc<RegexCache>,
    /// Default timeout for expect operations.
    default_timeout: Duration,
    /// Search window size (for performance optimization).
    search_window: Option<usize>,
}

impl Matcher {
    /// Create a new matcher with the specified buffer size.
    #[must_use]
    pub fn new(buffer_size: usize) -> Self {
        Self {
            buffer: RingBuffer::new(buffer_size),
            cache: Arc::new(RegexCache::with_default_size()),
            default_timeout: Duration::from_secs(30),
            search_window: None,
        }
    }

    /// Create a new matcher with shared regex cache.
    #[must_use]
    pub fn with_cache(buffer_size: usize, cache: Arc<RegexCache>) -> Self {
        Self {
            buffer: RingBuffer::new(buffer_size),
            cache,
            default_timeout: Duration::from_secs(30),
            search_window: None,
        }
    }

    /// Set the default timeout.
    pub const fn set_default_timeout(&mut self, timeout: Duration) {
        self.default_timeout = timeout;
    }

    /// Set the search window size.
    ///
    /// When set, pattern matching will only search the last N bytes
    /// of the buffer, improving performance for large buffers.
    pub const fn set_search_window(&mut self, size: Option<usize>) {
        self.search_window = size;
    }

    /// Append data to the buffer.
    pub fn append(&mut self, data: &[u8]) {
        self.buffer.append(data);
    }

    /// Get the current buffer.
    #[must_use]
    pub const fn buffer(&self) -> &RingBuffer {
        &self.buffer
    }

    /// Get the current buffer contents as a string.
    #[must_use]
    pub fn buffer_str(&mut self) -> String {
        self.buffer.as_str_lossy()
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Try to match a single pattern against the buffer.
    #[must_use]
    pub fn try_match(&mut self, pattern: &Pattern) -> Option<MatchResult> {
        let text = self.get_search_text();

        match pattern {
            Pattern::Literal(s) => text.find(s).map(|pos| MatchResult {
                pattern_index: 0,
                start: self.adjust_position(pos),
                end: self.adjust_position(pos + s.len()),
                captures: Vec::new(),
            }),
            Pattern::Regex(compiled) => compiled.find(&text).map(|m| {
                let captures = compiled.captures(&text);
                MatchResult {
                    pattern_index: 0,
                    start: self.adjust_position(m.start()),
                    end: self.adjust_position(m.end()),
                    captures,
                }
            }),
            Pattern::Glob(glob) => {
                self.try_glob_match(glob, &text)
                    .map(|(start, end)| MatchResult {
                        pattern_index: 0,
                        start: self.adjust_position(start),
                        end: self.adjust_position(end),
                        captures: Vec::new(),
                    })
            }
            Pattern::Eof | Pattern::Timeout(_) | Pattern::Bytes(_) => None,
        }
    }

    /// Try to match any pattern from a set against the buffer.
    #[must_use]
    pub fn try_match_any(&mut self, patterns: &PatternSet) -> Option<MatchResult> {
        let text = self.get_search_text();
        let mut best: Option<MatchResult> = None;

        for (idx, named) in patterns.iter().enumerate() {
            if let Some(pm) = named.pattern.matches(&text) {
                let result = MatchResult {
                    pattern_index: idx,
                    start: self.adjust_position(pm.start),
                    end: self.adjust_position(pm.end),
                    captures: pm.captures,
                };

                match &best {
                    None => best = Some(result),
                    Some(current) if result.start < current.start => best = Some(result),
                    _ => {}
                }
            }
        }

        best
    }

    /// Consume matched content from the buffer and return a Match.
    pub fn consume_match(&mut self, result: &MatchResult) -> Match {
        let before = self.buffer.consume_before(result.start);
        let matched_bytes = self.buffer.consume(result.end - result.start);
        let matched = String::from_utf8_lossy(&matched_bytes).into_owned();
        let after = self.buffer_str();

        Match::new(result.pattern_index, matched, before, after)
            .with_captures(result.captures.clone())
    }

    /// Get the timeout for a pattern set.
    #[must_use]
    pub fn get_timeout(&self, patterns: &PatternSet) -> Duration {
        patterns.min_timeout().unwrap_or(self.default_timeout)
    }

    /// Get the regex cache.
    #[must_use]
    pub const fn cache(&self) -> &Arc<RegexCache> {
        &self.cache
    }

    /// Get the text to search, applying search window if set.
    fn get_search_text(&mut self) -> String {
        match self.search_window {
            Some(window) => {
                let tail = self.buffer.tail(window);
                String::from_utf8_lossy(&tail).into_owned()
            }
            None => self.buffer.as_str_lossy(),
        }
    }

    /// Adjust position when using search window.
    fn adjust_position(&self, pos: usize) -> usize {
        match self.search_window {
            Some(window) => {
                let buffer_len = self.buffer.len();
                let offset = buffer_len.saturating_sub(window);
                offset + pos
            }
            None => pos,
        }
    }

    /// Simple glob matching.
    #[allow(clippy::unused_self)]
    fn try_glob_match(&self, pattern: &str, text: &str) -> Option<(usize, usize)> {
        // Convert glob to a simple search
        // For now, just handle * as prefix/suffix
        if let Some(rest) = pattern.strip_prefix('*') {
            if let Some(inner) = rest.strip_suffix('*') {
                // Pattern like *inner*
                text.find(inner).map(|pos| (pos, pos + inner.len()))
            } else {
                // Pattern like *suffix
                let suffix = rest;
                if text.ends_with(suffix) {
                    let start = text.len() - suffix.len();
                    Some((start, text.len()))
                } else {
                    None
                }
            }
        } else if let Some(prefix) = pattern.strip_suffix('*') {
            // Pattern like prefix*
            if text.starts_with(prefix) {
                Some((0, prefix.len()))
            } else {
                None
            }
        } else {
            text.find(pattern).map(|pos| (pos, pos + pattern.len()))
        }
    }
}

impl Default for Matcher {
    fn default() -> Self {
        Self::new(super::buffer::DEFAULT_CAPACITY)
    }
}

/// Result of a pattern match.
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// Index of the pattern that matched.
    pub pattern_index: usize,
    /// Start position in the buffer.
    pub start: usize,
    /// End position in the buffer.
    pub end: usize,
    /// Capture groups.
    pub captures: Vec<String>,
}

impl MatchResult {
    /// Get the length of the match.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.end - self.start
    }

    /// Check if the match is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// State machine for async expect operations.
pub struct ExpectState {
    /// The patterns being matched.
    patterns: PatternSet,
    /// Start time of the expect operation.
    start_time: Instant,
    /// Timeout duration.
    timeout: Duration,
    /// Whether EOF has been detected.
    eof_detected: bool,
}

impl ExpectState {
    /// Create a new expect state.
    #[must_use]
    pub fn new(patterns: PatternSet, timeout: Duration) -> Self {
        Self {
            patterns,
            start_time: Instant::now(),
            timeout,
            eof_detected: false,
        }
    }

    /// Check if the operation has timed out.
    #[must_use]
    pub fn is_timed_out(&self) -> bool {
        self.start_time.elapsed() >= self.timeout
    }

    /// Get the remaining time until timeout.
    #[must_use]
    pub fn remaining_time(&self) -> Duration {
        self.timeout.saturating_sub(self.start_time.elapsed())
    }

    /// Mark EOF as detected.
    pub const fn set_eof(&mut self) {
        self.eof_detected = true;
    }

    /// Check if EOF was detected.
    #[must_use]
    pub const fn is_eof(&self) -> bool {
        self.eof_detected
    }

    /// Get the patterns.
    #[must_use]
    pub const fn patterns(&self) -> &PatternSet {
        &self.patterns
    }

    /// Check if the patterns include an EOF pattern.
    #[must_use]
    pub fn expects_eof(&self) -> bool {
        self.patterns.has_eof()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matcher_literal() {
        let mut matcher = Matcher::new(1024);
        matcher.append(b"hello world");

        let pattern = Pattern::literal("world");
        let result = matcher.try_match(&pattern);
        assert!(result.is_some());

        let m = result.unwrap();
        assert_eq!(m.start, 6);
        assert_eq!(m.end, 11);
    }

    #[test]
    fn matcher_regex() {
        let mut matcher = Matcher::new(1024);
        matcher.append(b"value: 42");

        let pattern = Pattern::regex(r"\d+").unwrap();
        let result = matcher.try_match(&pattern);
        assert!(result.is_some());

        let m = result.unwrap();
        assert_eq!(m.start, 7);
        assert_eq!(m.end, 9);
    }

    #[test]
    fn matcher_consume() {
        let mut matcher = Matcher::new(1024);
        matcher.append(b"prefix|match|suffix");

        let pattern = Pattern::literal("match");
        let result = matcher.try_match(&pattern).unwrap();
        let m = matcher.consume_match(&result);

        assert_eq!(m.before, "prefix|");
        assert_eq!(m.matched, "match");
        assert_eq!(m.after, "|suffix");
    }

    #[test]
    fn matcher_pattern_set() {
        let mut matcher = Matcher::new(1024);
        matcher.append(b"error: something went wrong");

        let mut patterns = PatternSet::new();
        patterns
            .add(Pattern::literal("success"))
            .add(Pattern::literal("error"));

        let result = matcher.try_match_any(&patterns);
        assert!(result.is_some());
        assert_eq!(result.unwrap().pattern_index, 1);
    }

    #[test]
    fn expect_state_timeout() {
        let patterns = PatternSet::from_patterns(vec![Pattern::literal("test")]);
        let state = ExpectState::new(patterns, Duration::from_millis(10));

        assert!(!state.is_timed_out());
        std::thread::sleep(Duration::from_millis(20));
        assert!(state.is_timed_out());
    }
}
