//! Before/after pattern handlers for expect operations.
//!
//! This module provides persistent pattern handlers that are automatically
//! checked during every expect operation. These are useful for handling
//! common patterns like error messages or prompts that can appear at any time.

use super::pattern::{Pattern, PatternSet};
use std::collections::HashMap;

/// Handler function type for before/after patterns.
pub type PatternHandler = Box<dyn Fn(&str) -> HandlerAction + Send + Sync>;

/// Action to take after a pattern handler executes.
#[derive(Debug, Clone, Default)]
pub enum HandlerAction {
    /// Continue with the expect operation.
    #[default]
    Continue,
    /// Stop the expect operation and return success with this match.
    Return(String),
    /// Stop the expect operation and return an error.
    Abort(String),
    /// Send a response and continue.
    Respond(String),
}

/// A persistent pattern with its handler.
pub struct PersistentPattern {
    /// The pattern to match.
    pub pattern: Pattern,
    /// The handler to execute on match.
    pub handler: PatternHandler,
    /// Whether this pattern is currently enabled.
    pub enabled: bool,
    /// Priority (lower = higher priority).
    pub priority: i32,
}

impl PersistentPattern {
    /// Create a new persistent pattern.
    #[must_use]
    pub fn new(pattern: Pattern, handler: PatternHandler) -> Self {
        Self {
            pattern,
            handler,
            enabled: true,
            priority: 0,
        }
    }

    /// Create a pattern with a simple response.
    pub fn with_response(pattern: Pattern, response: impl Into<String>) -> Self {
        let response = response.into();
        Self::new(
            pattern,
            Box::new(move |_| HandlerAction::Respond(response.clone())),
        )
    }

    /// Create a pattern that aborts on match.
    pub fn with_abort(pattern: Pattern, message: impl Into<String>) -> Self {
        let message = message.into();
        Self::new(
            pattern,
            Box::new(move |_| HandlerAction::Abort(message.clone())),
        )
    }

    /// Set the priority for this pattern.
    #[must_use]
    pub const fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Disable this pattern.
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Enable this pattern.
    pub fn enable(&mut self) {
        self.enabled = true;
    }
}

impl std::fmt::Debug for PersistentPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PersistentPattern")
            .field("pattern", &self.pattern)
            .field("enabled", &self.enabled)
            .field("priority", &self.priority)
            .finish_non_exhaustive()
    }
}

/// Manager for before/after patterns.
///
/// Before patterns are checked before every expect operation.
/// After patterns are checked after each expect operation completes.
#[derive(Default)]
pub struct PatternManager {
    /// Patterns checked before each expect.
    before_patterns: HashMap<String, PersistentPattern>,
    /// Patterns checked after each expect.
    after_patterns: HashMap<String, PersistentPattern>,
    /// Counter for generating unique IDs.
    next_id: usize,
}

impl PatternManager {
    /// Create a new pattern manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a before pattern and return its ID.
    pub fn add_before(&mut self, pattern: PersistentPattern) -> String {
        let id = self.generate_id("before");
        self.before_patterns.insert(id.clone(), pattern);
        id
    }

    /// Add an after pattern and return its ID.
    pub fn add_after(&mut self, pattern: PersistentPattern) -> String {
        let id = self.generate_id("after");
        self.after_patterns.insert(id.clone(), pattern);
        id
    }

    /// Remove a before pattern by ID.
    pub fn remove_before(&mut self, id: &str) -> Option<PersistentPattern> {
        self.before_patterns.remove(id)
    }

    /// Remove an after pattern by ID.
    pub fn remove_after(&mut self, id: &str) -> Option<PersistentPattern> {
        self.after_patterns.remove(id)
    }

    /// Get a before pattern by ID.
    #[must_use]
    pub fn get_before(&self, id: &str) -> Option<&PersistentPattern> {
        self.before_patterns.get(id)
    }

    /// Get a mutable before pattern by ID.
    pub fn get_before_mut(&mut self, id: &str) -> Option<&mut PersistentPattern> {
        self.before_patterns.get_mut(id)
    }

    /// Get an after pattern by ID.
    #[must_use]
    pub fn get_after(&self, id: &str) -> Option<&PersistentPattern> {
        self.after_patterns.get(id)
    }

    /// Get a mutable after pattern by ID.
    pub fn get_after_mut(&mut self, id: &str) -> Option<&mut PersistentPattern> {
        self.after_patterns.get_mut(id)
    }

    /// Check before patterns against the buffer.
    ///
    /// Returns the first matching handler action, or None if no patterns match.
    #[must_use]
    pub fn check_before(&self, buffer: &str) -> Option<(String, HandlerAction)> {
        self.check_patterns(&self.before_patterns, buffer)
    }

    /// Check after patterns against the buffer.
    ///
    /// Returns the first matching handler action, or None if no patterns match.
    #[must_use]
    pub fn check_after(&self, buffer: &str) -> Option<(String, HandlerAction)> {
        self.check_patterns(&self.after_patterns, buffer)
    }

    /// Get all before patterns as a `PatternSet` for matching.
    #[must_use]
    pub fn before_pattern_set(&self) -> PatternSet {
        self.patterns_to_set(&self.before_patterns)
    }

    /// Get all after patterns as a `PatternSet` for matching.
    #[must_use]
    pub fn after_pattern_set(&self) -> PatternSet {
        self.patterns_to_set(&self.after_patterns)
    }

    /// Clear all before patterns.
    pub fn clear_before(&mut self) {
        self.before_patterns.clear();
    }

    /// Clear all after patterns.
    pub fn clear_after(&mut self) {
        self.after_patterns.clear();
    }

    /// Clear all patterns.
    pub fn clear_all(&mut self) {
        self.before_patterns.clear();
        self.after_patterns.clear();
    }

    /// Get the number of before patterns.
    #[must_use]
    pub fn before_count(&self) -> usize {
        self.before_patterns.len()
    }

    /// Get the number of after patterns.
    #[must_use]
    pub fn after_count(&self) -> usize {
        self.after_patterns.len()
    }

    fn generate_id(&mut self, prefix: &str) -> String {
        let id = format!("{prefix}_{}", self.next_id);
        self.next_id += 1;
        id
    }

    fn check_patterns(
        &self,
        patterns: &HashMap<String, PersistentPattern>,
        buffer: &str,
    ) -> Option<(String, HandlerAction)> {
        // Collect enabled patterns sorted by priority
        let mut sorted: Vec<_> = patterns.iter().filter(|(_, p)| p.enabled).collect();
        sorted.sort_by_key(|(_, p)| p.priority);

        for (id, persistent) in sorted {
            if persistent.pattern.matches(buffer).is_some() {
                let action = (persistent.handler)(buffer);
                if !matches!(action, HandlerAction::Continue) {
                    return Some((id.clone(), action));
                }
            }
        }
        None
    }

    fn patterns_to_set(&self, patterns: &HashMap<String, PersistentPattern>) -> PatternSet {
        let mut sorted: Vec<_> = patterns.iter().filter(|(_, p)| p.enabled).collect();
        sorted.sort_by_key(|(_, p)| p.priority);

        PatternSet::from_patterns(sorted.into_iter().map(|(_, p)| p.pattern.clone()).collect())
    }
}

impl std::fmt::Debug for PatternManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PatternManager")
            .field("before_count", &self.before_patterns.len())
            .field("after_count", &self.after_patterns.len())
            .finish()
    }
}

/// Builder for common before/after pattern configurations.
pub struct PatternBuilder {
    manager: PatternManager,
}

impl PatternBuilder {
    /// Create a new pattern builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            manager: PatternManager::new(),
        }
    }

    /// Add a password prompt handler.
    #[must_use]
    pub fn with_password_handler(mut self, password: impl Into<String>) -> Self {
        let password = password.into();
        let pattern = PersistentPattern::with_response(
            Pattern::regex(r"[Pp]assword:?\s*$").unwrap_or_else(|_| Pattern::literal("Password:")),
            format!("{password}\n"),
        );
        self.manager.add_before(pattern);
        self
    }

    /// Add a sudo password handler.
    #[must_use]
    pub fn with_sudo_handler(mut self, password: impl Into<String>) -> Self {
        let password = password.into();
        let pattern = PersistentPattern::with_response(
            Pattern::regex(r"\[sudo\] password")
                .unwrap_or_else(|_| Pattern::literal("[sudo] password")),
            format!("{password}\n"),
        );
        self.manager.add_before(pattern);
        self
    }

    /// Add an error pattern that aborts.
    #[must_use]
    pub fn with_error_pattern(mut self, pattern: Pattern, message: impl Into<String>) -> Self {
        let persistent = PersistentPattern::with_abort(pattern, message);
        self.manager.add_before(persistent);
        self
    }

    /// Add a yes/no prompt handler that responds with yes.
    #[must_use]
    pub fn with_yes_handler(mut self) -> Self {
        let pattern = PersistentPattern::with_response(
            Pattern::regex(r"\(yes/no\)\??\s*$").unwrap_or_else(|_| Pattern::literal("(yes/no)")),
            "yes\n",
        );
        self.manager.add_before(pattern);
        self
    }

    /// Add a y/n prompt handler that responds with y.
    #[must_use]
    pub fn with_yn_handler(mut self) -> Self {
        let pattern = PersistentPattern::with_response(
            Pattern::regex(r"\[y/n\]\??\s*$").unwrap_or_else(|_| Pattern::literal("[y/n]")),
            "y\n",
        );
        self.manager.add_before(pattern);
        self
    }

    /// Add a continue prompt handler.
    #[must_use]
    pub fn with_continue_handler(mut self) -> Self {
        let pattern = PersistentPattern::with_response(
            Pattern::regex(r"Press (?:Enter|any key) to continue")
                .unwrap_or_else(|_| Pattern::literal("Press Enter")),
            "\n",
        );
        self.manager.add_before(pattern);
        self
    }

    /// Build the pattern manager.
    #[must_use]
    pub fn build(self) -> PatternManager {
        self.manager
    }
}

impl Default for PatternBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_manager_before() {
        let mut manager = PatternManager::new();

        let pattern = PersistentPattern::with_response(Pattern::literal("password:"), "secret\n");
        let id = manager.add_before(pattern);

        let result = manager.check_before("Enter password: ");
        assert!(result.is_some());

        let (matched_id, action) = result.unwrap();
        assert_eq!(matched_id, id);
        assert!(matches!(action, HandlerAction::Respond(_)));
    }

    #[test]
    fn pattern_manager_priority() {
        let mut manager = PatternManager::new();

        let low = PersistentPattern::new(
            Pattern::literal("test"),
            Box::new(|_| HandlerAction::Respond("low".into())),
        )
        .with_priority(10);

        let high = PersistentPattern::new(
            Pattern::literal("test"),
            Box::new(|_| HandlerAction::Respond("high".into())),
        )
        .with_priority(1);

        manager.add_before(low);
        manager.add_before(high);

        let result = manager.check_before("test");
        assert!(result.is_some());

        if let Some((_, HandlerAction::Respond(s))) = result {
            assert_eq!(s, "high");
        } else {
            panic!("Expected Respond action");
        }
    }

    #[test]
    fn pattern_manager_disable() {
        let mut manager = PatternManager::new();

        let pattern = PersistentPattern::with_response(Pattern::literal("test"), "response");
        let id = manager.add_before(pattern);

        // Should match when enabled
        assert!(manager.check_before("test").is_some());

        // Disable and check again
        manager.get_before_mut(&id).unwrap().disable();
        assert!(manager.check_before("test").is_none());
    }

    #[test]
    fn pattern_builder() {
        let manager = PatternBuilder::new()
            .with_password_handler("secret")
            .with_yes_handler()
            .build();

        assert_eq!(manager.before_count(), 2);
    }
}
