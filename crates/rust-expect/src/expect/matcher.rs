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
        let search = self.get_search_text();
        let text = &search.text;

        match pattern {
            Pattern::Literal(s) => text.find(s.as_str()).map(|pos| MatchResult {
                pattern_index: 0,
                start: search.raw_offset(pos),
                end: search.raw_offset(pos + s.len()),
                captures: Vec::new(),
            }),
            Pattern::Regex(compiled) => compiled.find(text).map(|m| {
                let captures = compiled.captures(text);
                MatchResult {
                    pattern_index: 0,
                    start: search.raw_offset(m.start()),
                    end: search.raw_offset(m.end()),
                    captures,
                }
            }),
            Pattern::Glob(glob) => {
                self.try_glob_match(glob, text)
                    .map(|(start, end)| MatchResult {
                        pattern_index: 0,
                        start: search.raw_offset(start),
                        end: search.raw_offset(end),
                        captures: Vec::new(),
                    })
            }
            // `Bytes(n)` matches once at least `n` raw bytes are buffered, and
            // consumes the first `n` of them. It is resolved here (not in
            // `Pattern::matches`) because it depends on the raw buffer length
            // rather than the search text.
            Pattern::Bytes(n) => (self.buffer.len() >= *n).then_some(MatchResult {
                pattern_index: 0,
                start: 0,
                end: *n,
                captures: Vec::new(),
            }),
            Pattern::Eof | Pattern::Timeout(_) => None,
        }
    }

    /// Try to match any pattern from a set against the buffer.
    #[must_use]
    pub fn try_match_any(&mut self, patterns: &PatternSet) -> Option<MatchResult> {
        let search = self.get_search_text();
        let text = &search.text;
        let buffer_len = self.buffer.len();
        let mut best: Option<MatchResult> = None;

        for (idx, named) in patterns.iter().enumerate() {
            // `Bytes(n)` depends on the raw buffer length, so it is matched
            // directly here rather than via `Pattern::matches` (which only sees
            // the search text and always returns `None` for `Bytes`).
            let result = if let Pattern::Bytes(n) = &named.pattern {
                (buffer_len >= *n).then_some(MatchResult {
                    pattern_index: idx,
                    start: 0,
                    end: *n,
                    captures: Vec::new(),
                })
            } else {
                named.pattern.matches(text).map(|pm| MatchResult {
                    pattern_index: idx,
                    start: search.raw_offset(pm.start),
                    end: search.raw_offset(pm.end),
                    captures: pm.captures,
                })
            };

            if let Some(result) = result {
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
    fn get_search_text(&mut self) -> SearchText {
        if let Some(window) = self.search_window {
            let base = self.buffer.len().saturating_sub(window);
            let tail = self.buffer.tail(window);
            SearchText::decode(&tail, base)
        } else {
            SearchText::decode(self.buffer.as_slice(), 0)
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

/// One run of the decoded search text, mapped back to the raw buffer.
///
/// A run is either verbatim (valid UTF-8, decoded byte-for-byte) or a single
/// replacement character standing in for `raw_len` invalid bytes.
struct Segment {
    /// Offset of this run in the decoded text.
    decoded_start: usize,
    /// Offset of this run in the raw region that was decoded.
    raw_start: usize,
    /// Number of raw bytes this run covers.
    raw_len: usize,
    /// Whether the run decoded byte-for-byte.
    verbatim: bool,
}

/// The text patterns are matched against, plus the mapping back to raw
/// buffer offsets.
///
/// Patterns match on text, but the buffer is consumed by raw byte offset.
/// Lossy decoding expands each invalid byte sequence into a three-byte
/// replacement character, so a decoded offset is not a raw offset once the
/// output contains a single invalid byte — the two drift apart by two bytes
/// per replacement, and consuming at the decoded offset takes the wrong bytes.
/// Every offset handed to [`MatchResult`] therefore goes through
/// [`SearchText::raw_offset`].
struct SearchText {
    /// The decoded text.
    text: String,
    /// Run map, `None` when the region was valid UTF-8 and decoded offsets
    /// are already raw offsets.
    segments: Option<Vec<Segment>>,
    /// Raw buffer offset the decoded region starts at. Non-zero when a search
    /// window restricts matching to the tail of the buffer.
    base: usize,
    /// Length in raw bytes of the decoded region.
    raw_len: usize,
}

impl SearchText {
    /// Decode `bytes` (which start at raw offset `base`) into searchable text.
    fn decode(bytes: &[u8], base: usize) -> Self {
        // Fast path: valid UTF-8 decodes byte-for-byte, so no map is needed.
        if let Ok(text) = std::str::from_utf8(bytes) {
            return Self {
                text: text.to_owned(),
                segments: None,
                base,
                raw_len: bytes.len(),
            };
        }

        let mut text = String::with_capacity(bytes.len());
        let mut segments = Vec::new();
        let mut rest = bytes;
        let mut raw_pos = 0;

        loop {
            let err = match std::str::from_utf8(rest) {
                Ok(valid) => {
                    if !valid.is_empty() {
                        segments.push(Segment {
                            decoded_start: text.len(),
                            raw_start: raw_pos,
                            raw_len: valid.len(),
                            verbatim: true,
                        });
                        text.push_str(valid);
                    }
                    break;
                }
                Err(e) => e,
            };

            let valid_up_to = err.valid_up_to();
            if valid_up_to > 0 {
                segments.push(Segment {
                    decoded_start: text.len(),
                    raw_start: raw_pos,
                    raw_len: valid_up_to,
                    verbatim: true,
                });
                // Borrowed (not re-allocated): this prefix is valid by
                // construction, so the lossy call cannot substitute anything.
                text.push_str(&String::from_utf8_lossy(&rest[..valid_up_to]));
            }

            // `error_len() == None` means the region ends mid-sequence, which
            // lossy decoding renders as one trailing replacement character.
            let incomplete_tail = err.error_len().is_none();
            let invalid_len = err.error_len().unwrap_or(rest.len() - valid_up_to);
            segments.push(Segment {
                decoded_start: text.len(),
                raw_start: raw_pos + valid_up_to,
                raw_len: invalid_len,
                verbatim: false,
            });
            text.push(char::REPLACEMENT_CHARACTER);

            let consumed = valid_up_to + invalid_len;
            raw_pos += consumed;
            rest = &rest[consumed..];
            if incomplete_tail {
                break;
            }
        }

        Self {
            text,
            segments: Some(segments),
            base,
            raw_len: bytes.len(),
        }
    }

    /// Map an offset in the decoded text to its offset in the raw buffer.
    ///
    /// Decoded offsets always land on a character boundary, so they fall
    /// either inside a verbatim run (where the mapping is linear) or exactly
    /// at a run boundary.
    fn raw_offset(&self, decoded: usize) -> usize {
        let Some(segments) = self.segments.as_ref() else {
            return self.base + decoded.min(self.raw_len);
        };

        match segments.binary_search_by_key(&decoded, |s| s.decoded_start) {
            Ok(i) => self.base + segments[i].raw_start,
            // Before the first run, or an empty map: the region start.
            Err(0) => self.base,
            Err(i) => {
                let seg = &segments[i - 1];
                if seg.verbatim {
                    self.base + seg.raw_start + (decoded - seg.decoded_start)
                } else {
                    // Past a replacement character: the end of the bytes it
                    // stands for.
                    self.base + seg.raw_start + seg.raw_len
                }
            }
        }
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
    fn matcher_bytes_waits_then_matches() {
        let mut matcher = Matcher::new(1024);
        let pattern = Pattern::bytes(5);

        // Fewer than 5 bytes: no match.
        matcher.append(b"abc");
        assert!(
            matcher.try_match(&pattern).is_none(),
            "Bytes(5) must not match with only 3 bytes buffered"
        );

        // Reaching 5 bytes: matches, consuming exactly the first 5.
        matcher.append(b"defgh");
        let result = matcher.try_match(&pattern).expect("Bytes(5) should match");
        assert_eq!(result.start, 0);
        assert_eq!(result.end, 5);

        let m = matcher.consume_match(&result);
        assert_eq!(m.matched, "abcde");
    }

    #[test]
    fn matcher_bytes_in_pattern_set() {
        let mut matcher = Matcher::new(1024);
        matcher.append(b"abcdef");

        let mut patterns = PatternSet::new();
        patterns.add(Pattern::literal("zzz")).add(Pattern::bytes(4));

        let result = matcher
            .try_match_any(&patterns)
            .expect("Bytes(4) should match in the set");
        assert_eq!(result.pattern_index, 1);
        assert_eq!(result.end - result.start, 4);
    }

    /// Invalid UTF-8 ahead of the match used to shift the decoded offsets away
    /// from the raw ones (each bad byte becomes a three-byte replacement
    /// character), so the buffer was consumed at the wrong position and the
    /// match came back empty.
    #[test]
    fn matcher_offsets_survive_invalid_utf8() {
        let mut matcher = Matcher::new(1024);
        matcher.append(&[0xFF, 0xFF, b'A', b'B']);

        let pattern = Pattern::literal("AB");
        let result = matcher.try_match(&pattern).expect("AB should match");
        assert_eq!(result.start, 2, "start must be a raw byte offset");
        assert_eq!(result.end, 4, "end must be a raw byte offset");

        let m = matcher.consume_match(&result);
        assert_eq!(m.matched, "AB");
        assert_eq!(m.before, "\u{FFFD}\u{FFFD}");
    }

    /// The same drift applies to a match found *after* an incomplete trailing
    /// sequence has been buffered ahead of it.
    #[test]
    fn matcher_offsets_survive_truncated_sequence() {
        let mut matcher = Matcher::new(1024);
        // 0xE2 0x82 starts a three-byte sequence that never completes.
        matcher.append(&[0xE2, 0x82, b'x', b'y', b'z']);

        let pattern = Pattern::literal("yz");
        let result = matcher.try_match(&pattern).expect("yz should match");
        let m = matcher.consume_match(&result);
        assert_eq!(m.matched, "yz");
    }

    /// Multibyte but valid UTF-8 takes the no-map fast path; offsets are byte
    /// offsets in both spaces and must stay unchanged.
    #[test]
    fn matcher_offsets_with_valid_multibyte() {
        let mut matcher = Matcher::new(1024);
        matcher.append("héllo wörld".as_bytes());

        let pattern = Pattern::literal("wörld");
        let result = matcher.try_match(&pattern).expect("wörld should match");
        let m = matcher.consume_match(&result);
        assert_eq!(m.matched, "wörld");
        assert_eq!(m.before, "héllo ");
    }

    /// With a search window the offsets are relative to the tail, and the tail
    /// can itself start mid-character. Both shifts have to compose.
    #[test]
    fn matcher_offsets_with_search_window_and_invalid_utf8() {
        let mut matcher = Matcher::new(1024);
        matcher.set_search_window(Some(6));
        matcher.append(b"prefix");
        matcher.append(&[0xFF, b'f', b'i', b'n', b'd', b'!']);

        let pattern = Pattern::literal("find");
        let result = matcher.try_match(&pattern).expect("find should match");
        assert_eq!(result.start, 7, "offset must be absolute in the buffer");

        let m = matcher.consume_match(&result);
        assert_eq!(m.matched, "find");
        assert_eq!(m.after, "!");
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
