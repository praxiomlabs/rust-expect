//! Regex cache for efficient pattern matching.
//!
//! This module provides a cache for compiled regular expressions,
//! avoiding the overhead of recompiling patterns on each use.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use regex::Regex;

/// Default maximum cache size.
pub const DEFAULT_CACHE_SIZE: usize = 100;

/// A cache for compiled regular expressions.
///
/// The cache uses LRU (Least Recently Used) eviction when full.
pub struct RegexCache {
    cache: RwLock<LruCache>,
    max_size: usize,
    /// Total cache hits (for statistics).
    total_hits: AtomicUsize,
    /// Total cache misses (for statistics).
    total_misses: AtomicUsize,
}

struct LruCache {
    entries: HashMap<String, CacheEntry>,
    order: Vec<String>,
}

struct CacheEntry {
    regex: Arc<Regex>,
    /// Number of times this pattern has been accessed.
    hits: AtomicUsize,
}

impl RegexCache {
    /// Create a new regex cache with the specified maximum size.
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: RwLock::new(LruCache {
                entries: HashMap::with_capacity(max_size),
                order: Vec::with_capacity(max_size),
            }),
            max_size,
            total_hits: AtomicUsize::new(0),
            total_misses: AtomicUsize::new(0),
        }
    }

    /// Create a new regex cache with default size.
    #[must_use]
    pub fn with_default_size() -> Self {
        Self::new(DEFAULT_CACHE_SIZE)
    }

    /// Get or compile a regex pattern.
    ///
    /// Returns a cached regex if available, otherwise compiles and caches it.
    ///
    /// # Errors
    ///
    /// Returns an error if the pattern is invalid.
    pub fn get_or_compile(&self, pattern: &str) -> Result<Arc<Regex>, regex::Error> {
        // Try read path first
        // Note: We recover from lock poisoning since the cache is just an optimization
        {
            let cache = self
                .cache
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(entry) = cache.entries.get(pattern) {
                // Track cache hit
                entry.hits.fetch_add(1, Ordering::Relaxed);
                self.total_hits.fetch_add(1, Ordering::Relaxed);
                return Ok(Arc::clone(&entry.regex));
            }
        }

        // Track cache miss
        self.total_misses.fetch_add(1, Ordering::Relaxed);

        // Compile the regex
        let regex = Regex::new(pattern)?;
        let regex = Arc::new(regex);

        // Update cache
        {
            let mut cache = self
                .cache
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            // Double-check after acquiring write lock (another thread may have inserted)
            if let Some(entry) = cache.entries.get(pattern) {
                // Count as hit since we're returning a cached entry
                entry.hits.fetch_add(1, Ordering::Relaxed);
                return Ok(Arc::clone(&entry.regex));
            }

            // Evict if necessary
            if cache.entries.len() >= self.max_size
                && let Some(oldest) = cache.order.first().cloned()
            {
                cache.entries.remove(&oldest);
                cache.order.remove(0);
            }

            // Insert new entry
            cache.entries.insert(
                pattern.to_string(),
                CacheEntry {
                    regex: Arc::clone(&regex),
                    hits: AtomicUsize::new(1), // First access
                },
            );
            cache.order.push(pattern.to_string());
        }

        Ok(regex)
    }

    /// Check if a pattern is cached.
    #[must_use]
    pub fn contains(&self, pattern: &str) -> bool {
        let cache = self
            .cache
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.entries.contains_key(pattern)
    }

    /// Get the current number of cached patterns.
    #[must_use]
    pub fn len(&self) -> usize {
        let cache = self
            .cache
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.entries.len()
    }

    /// Check if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear the cache.
    pub fn clear(&self) {
        let mut cache = self
            .cache
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.entries.clear();
        cache.order.clear();
    }

    /// Get the maximum cache size.
    #[must_use]
    pub const fn max_size(&self) -> usize {
        self.max_size
    }

    /// Get cache statistics.
    #[must_use]
    pub fn stats(&self) -> CacheStats {
        let cache = self
            .cache
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        CacheStats {
            size: cache.entries.len(),
            max_size: self.max_size,
            total_hits: self.total_hits.load(Ordering::Relaxed),
            total_misses: self.total_misses.load(Ordering::Relaxed),
        }
    }

    /// Get the total number of cache hits.
    #[must_use]
    pub fn total_hits(&self) -> usize {
        self.total_hits.load(Ordering::Relaxed)
    }

    /// Get the total number of cache misses.
    #[must_use]
    pub fn total_misses(&self) -> usize {
        self.total_misses.load(Ordering::Relaxed)
    }

    /// Get the cache hit rate as a ratio (0.0 to 1.0).
    ///
    /// Returns 1.0 if no accesses have been made.
    #[must_use]
    pub fn hit_rate(&self) -> f64 {
        let hits = self.total_hits.load(Ordering::Relaxed);
        let misses = self.total_misses.load(Ordering::Relaxed);
        let total = hits + misses;
        if total == 0 {
            1.0
        } else {
            hits as f64 / total as f64
        }
    }
}

/// Statistics about a regex cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheStats {
    /// Current number of cached patterns.
    pub size: usize,
    /// Maximum cache size.
    pub max_size: usize,
    /// Total cache hits.
    pub total_hits: usize,
    /// Total cache misses.
    pub total_misses: usize,
}

impl CacheStats {
    /// Get the cache hit rate as a ratio (0.0 to 1.0).
    ///
    /// Returns 1.0 if no accesses have been made.
    #[must_use]
    pub fn hit_rate(&self) -> f64 {
        let total = self.total_hits + self.total_misses;
        if total == 0 {
            1.0
        } else {
            self.total_hits as f64 / total as f64
        }
    }
}

impl Default for RegexCache {
    fn default() -> Self {
        Self::with_default_size()
    }
}

/// Global regex cache for shared use.
pub static GLOBAL_CACHE: std::sync::LazyLock<RegexCache> =
    std::sync::LazyLock::new(RegexCache::with_default_size);

/// Get or compile a regex using the global cache.
///
/// # Errors
///
/// Returns an error if the pattern is invalid.
pub fn get_regex(pattern: &str) -> Result<Arc<Regex>, regex::Error> {
    GLOBAL_CACHE.get_or_compile(pattern)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_basic() {
        let cache = RegexCache::new(10);

        let r1 = cache.get_or_compile(r"\d+").unwrap();
        let r2 = cache.get_or_compile(r"\d+").unwrap();

        // Should be the same Arc
        assert!(Arc::ptr_eq(&r1, &r2));
    }

    #[test]
    fn cache_eviction() {
        let cache = RegexCache::new(2);

        cache.get_or_compile(r"a+").unwrap();
        cache.get_or_compile(r"b+").unwrap();
        assert_eq!(cache.len(), 2);

        // This should evict "a+"
        cache.get_or_compile(r"c+").unwrap();
        assert_eq!(cache.len(), 2);
        assert!(!cache.contains(r"a+"));
        assert!(cache.contains(r"b+"));
        assert!(cache.contains(r"c+"));
    }

    #[test]
    fn cache_invalid_pattern() {
        let cache = RegexCache::new(10);
        let result = cache.get_or_compile(r"[invalid");
        assert!(result.is_err());
    }

    #[test]
    fn global_cache() {
        let r1 = get_regex(r"\w+").unwrap();
        let r2 = get_regex(r"\w+").unwrap();
        assert!(Arc::ptr_eq(&r1, &r2));
    }

    #[test]
    fn cache_stats_tracking() {
        let cache = RegexCache::new(10);

        // Initial state
        let stats = cache.stats();
        assert_eq!(stats.size, 0);
        assert_eq!(stats.total_hits, 0);
        assert_eq!(stats.total_misses, 0);

        // First access (miss)
        cache.get_or_compile(r"\d+").unwrap();
        assert_eq!(cache.total_misses(), 1);
        assert_eq!(cache.total_hits(), 0);

        // Second access (hit)
        cache.get_or_compile(r"\d+").unwrap();
        assert_eq!(cache.total_misses(), 1);
        assert_eq!(cache.total_hits(), 1);

        // Third access (hit)
        cache.get_or_compile(r"\d+").unwrap();
        assert_eq!(cache.total_hits(), 2);

        // New pattern (miss)
        cache.get_or_compile(r"\w+").unwrap();
        assert_eq!(cache.total_misses(), 2);

        // Check hit rate (2 hits out of 4 total = 0.5)
        let hit_rate = cache.hit_rate();
        assert!((hit_rate - 0.5).abs() < 0.001);
    }

    #[test]
    fn cache_stats_hit_rate_empty() {
        let cache = RegexCache::new(10);
        // Empty cache should return 1.0 hit rate (no failures yet)
        assert!((cache.hit_rate() - 1.0).abs() < 0.001);
    }
}
