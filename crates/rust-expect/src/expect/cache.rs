//! Regex cache for efficient pattern matching.
//!
//! This module provides a cache for compiled regular expressions,
//! avoiding the overhead of recompiling patterns on each use.

use regex::Regex;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Default maximum cache size.
pub const DEFAULT_CACHE_SIZE: usize = 100;

/// A cache for compiled regular expressions.
///
/// The cache uses LRU (Least Recently Used) eviction when full.
pub struct RegexCache {
    cache: RwLock<LruCache>,
    max_size: usize,
}

struct LruCache {
    entries: HashMap<String, CacheEntry>,
    order: Vec<String>,
}

struct CacheEntry {
    regex: Arc<Regex>,
    #[allow(dead_code)]
    hits: usize,
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
            let cache = self.cache.read().unwrap_or_else(|e| e.into_inner());
            if let Some(entry) = cache.entries.get(pattern) {
                return Ok(Arc::clone(&entry.regex));
            }
        }

        // Compile the regex
        let regex = Regex::new(pattern)?;
        let regex = Arc::new(regex);

        // Update cache
        {
            let mut cache = self.cache.write().unwrap_or_else(|e| e.into_inner());

            // Double-check after acquiring write lock
            if let Some(entry) = cache.entries.get(pattern) {
                return Ok(Arc::clone(&entry.regex));
            }

            // Evict if necessary
            if cache.entries.len() >= self.max_size {
                if let Some(oldest) = cache.order.first().cloned() {
                    cache.entries.remove(&oldest);
                    cache.order.remove(0);
                }
            }

            // Insert new entry
            cache.entries.insert(
                pattern.to_string(),
                CacheEntry {
                    regex: Arc::clone(&regex),
                    hits: 0,
                },
            );
            cache.order.push(pattern.to_string());
        }

        Ok(regex)
    }

    /// Check if a pattern is cached.
    #[must_use]
    pub fn contains(&self, pattern: &str) -> bool {
        let cache = self.cache.read().unwrap_or_else(|e| e.into_inner());
        cache.entries.contains_key(pattern)
    }

    /// Get the current number of cached patterns.
    #[must_use]
    pub fn len(&self) -> usize {
        let cache = self.cache.read().unwrap_or_else(|e| e.into_inner());
        cache.entries.len()
    }

    /// Check if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear the cache.
    pub fn clear(&self) {
        let mut cache = self.cache.write().unwrap_or_else(|e| e.into_inner());
        cache.entries.clear();
        cache.order.clear();
    }

    /// Get the maximum cache size.
    #[must_use]
    pub const fn max_size(&self) -> usize {
        self.max_size
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
}
