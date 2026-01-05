//! Pattern types for expect operations.
//!
//! This module defines the pattern types that can be used with expect operations,
//! including literal strings, regular expressions, globs, EOF, and timeout.
//!
//! # Examples
//!
//! ```
//! use rust_expect::Pattern;
//! use std::time::Duration;
//!
//! // Literal pattern - matches exact text
//! let prompt = Pattern::literal("$ ");
//! assert!(prompt.matches("user@host:~ $ ").is_some());
//!
//! // Regex pattern - matches regular expressions
//! let version = Pattern::regex(r"\d+\.\d+\.\d+").unwrap();
//! assert!(version.matches("Version: 1.2.3").is_some());
//!
//! // Glob pattern - matches shell-style wildcards
//! let log = Pattern::glob("Error:*");
//! assert!(log.matches("Error: connection failed").is_some());
//!
//! // Timeout pattern - used with expect_any for timeouts
//! let timeout = Pattern::timeout(Duration::from_secs(5));
//! assert!(timeout.is_timeout());
//!
//! // EOF pattern - matches process termination
//! let eof = Pattern::eof();
//! assert!(eof.is_eof());
//! ```

use std::fmt;
use std::time::Duration;

use regex::Regex;

/// A pattern that can be matched against terminal output.
#[derive(Clone)]
pub enum Pattern {
    /// Match an exact string.
    Literal(String),

    /// Match a regular expression.
    Regex(CompiledRegex),

    /// Match a glob pattern.
    Glob(String),

    /// Match end of file (process terminated).
    Eof,

    /// Match after a timeout.
    Timeout(Duration),

    /// Match when N bytes have been received.
    Bytes(usize),
}

impl Pattern {
    /// Create a literal pattern.
    #[must_use]
    pub fn literal(s: impl Into<String>) -> Self {
        Self::Literal(s.into())
    }

    /// Create a regex pattern.
    ///
    /// # Errors
    ///
    /// Returns an error if the regex pattern is invalid.
    pub fn regex(pattern: &str) -> Result<Self, regex::Error> {
        let regex = Regex::new(pattern)?;
        Ok(Self::Regex(CompiledRegex::new(pattern.to_string(), regex)))
    }

    /// Create a glob pattern.
    #[must_use]
    pub fn glob(pattern: impl Into<String>) -> Self {
        Self::Glob(pattern.into())
    }

    /// Create an EOF pattern.
    #[must_use]
    pub const fn eof() -> Self {
        Self::Eof
    }

    /// Create a timeout pattern.
    #[must_use]
    pub const fn timeout(duration: Duration) -> Self {
        Self::Timeout(duration)
    }

    /// Create a bytes pattern.
    #[must_use]
    pub const fn bytes(n: usize) -> Self {
        Self::Bytes(n)
    }

    /// Get the pattern as a string for display purposes.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Literal(s) => s,
            Self::Regex(r) => r.pattern(),
            Self::Glob(s) => s,
            Self::Eof => "<EOF>",
            Self::Timeout(_) => "<TIMEOUT>",
            Self::Bytes(_) => "<BYTES>",
        }
    }

    /// Check if this pattern matches the given text.
    ///
    /// Returns the match position and captures if successful.
    #[must_use]
    pub fn matches(&self, text: &str) -> Option<PatternMatch> {
        match self {
            Self::Literal(s) => text.find(s).map(|pos| PatternMatch {
                start: pos,
                end: pos + s.len(),
                captures: Vec::new(),
            }),
            Self::Regex(r) => r.find(text).map(|m| PatternMatch {
                start: m.start(),
                end: m.end(),
                captures: r.captures(text),
            }),
            Self::Glob(pattern) => glob_match(pattern, text).map(|pos| PatternMatch {
                start: pos,
                end: text.len(),
                captures: Vec::new(),
            }),
            Self::Eof | Self::Timeout(_) | Self::Bytes(_) => None,
        }
    }

    /// Check if this is a timeout pattern.
    #[must_use]
    pub const fn is_timeout(&self) -> bool {
        matches!(self, Self::Timeout(_))
    }

    /// Check if this is an EOF pattern.
    #[must_use]
    pub const fn is_eof(&self) -> bool {
        matches!(self, Self::Eof)
    }

    /// Get the timeout duration if this is a timeout pattern.
    #[must_use]
    pub const fn timeout_duration(&self) -> Option<Duration> {
        match self {
            Self::Timeout(d) => Some(*d),
            _ => None,
        }
    }

    // =========================================================================
    // Convenience pattern constructors
    // =========================================================================

    /// Create a pattern that matches common shell prompts.
    ///
    /// Matches prompts ending with `$`, `#`, `>`, or `%` followed by optional whitespace.
    /// This handles most Unix shells (bash, zsh, sh) and root prompts.
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_expect::Pattern;
    ///
    /// let prompt = Pattern::shell_prompt();
    /// assert!(prompt.matches("user@host:~$ ").is_some());
    /// assert!(prompt.matches("root@host:~# ").is_some());
    /// assert!(prompt.matches("> ").is_some());
    /// ```
    #[must_use]
    pub fn shell_prompt() -> Self {
        // Use a fallback to literal if regex somehow fails (it won't for this pattern)
        Self::regex(r"[$#>%]\s*$").unwrap_or_else(|_| Self::Literal("$ ".to_string()))
    }

    /// Create a pattern that matches any common prompt character.
    ///
    /// A simpler alternative to `shell_prompt()` that uses glob matching.
    /// Less precise but faster for simple cases.
    #[must_use]
    pub fn any_prompt() -> Self {
        Self::Glob("*$*".to_string())
    }

    /// Create a pattern that matches IPv4 addresses.
    ///
    /// # Errors
    ///
    /// Returns an error if the regex compilation fails (should not happen).
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_expect::Pattern;
    ///
    /// let ipv4 = Pattern::ipv4().unwrap();
    /// assert!(ipv4.matches("Server IP: 192.168.1.1").is_some());
    /// assert!(ipv4.matches("10.0.0.255 is local").is_some());
    /// ```
    pub fn ipv4() -> Result<Self, regex::Error> {
        Self::regex(
            r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b",
        )
    }

    /// Create a pattern that matches email addresses.
    ///
    /// # Errors
    ///
    /// Returns an error if the regex compilation fails (should not happen).
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_expect::Pattern;
    ///
    /// let email = Pattern::email().unwrap();
    /// assert!(email.matches("Contact: user@example.com").is_some());
    /// ```
    pub fn email() -> Result<Self, regex::Error> {
        Self::regex(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b")
    }

    /// Create a pattern that matches ISO 8601 timestamps.
    ///
    /// Matches formats like `2024-01-15T10:30:00` or `2024-01-15 10:30:00`.
    ///
    /// # Errors
    ///
    /// Returns an error if the regex compilation fails (should not happen).
    pub fn timestamp_iso8601() -> Result<Self, regex::Error> {
        Self::regex(r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}")
    }

    /// Create a pattern that matches common error indicators.
    ///
    /// Matches words like "error", "failed", "fatal" (case-insensitive).
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_expect::Pattern;
    ///
    /// let error = Pattern::error_indicator();
    /// assert!(error.matches("Error: connection refused").is_some());
    /// assert!(error.matches("Command FAILED").is_some());
    /// ```
    #[must_use]
    pub fn error_indicator() -> Self {
        Self::regex(r"(?i)\b(?:error|failed|fatal)\b")
            .unwrap_or_else(|_| Self::Glob("*[Ee]rror*".to_string()))
    }

    /// Create a pattern that matches common success indicators.
    ///
    /// Matches words like "success", "passed", "complete", "ok" (case-insensitive).
    #[must_use]
    pub fn success_indicator() -> Self {
        Self::regex(r"(?i)\b(?:success|successful|passed|complete|ok)\b")
            .unwrap_or_else(|_| Self::Glob("*[Ss]uccess*".to_string()))
    }

    /// Create a pattern that matches common password prompts.
    ///
    /// Matches prompts like "Password:", "password: ", "Passphrase:".
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_expect::Pattern;
    ///
    /// let pwd = Pattern::password_prompt();
    /// assert!(pwd.matches("Password: ").is_some());
    /// assert!(pwd.matches("Enter passphrase: ").is_some());
    /// ```
    #[must_use]
    pub fn password_prompt() -> Self {
        Self::regex(r"(?i)(?:password|passphrase)\s*:\s*$")
            .unwrap_or_else(|_| Self::Literal("password:".to_string()))
    }

    /// Create a pattern that matches common login/username prompts.
    ///
    /// Matches prompts like "login:", "Username:", "user: ".
    #[must_use]
    pub fn login_prompt() -> Self {
        Self::regex(r"(?i)(?:login|username|user)\s*:\s*$")
            .unwrap_or_else(|_| Self::Literal("login:".to_string()))
    }

    /// Create a pattern that matches common yes/no confirmation prompts.
    ///
    /// Matches prompts like "[y/n]", "(yes/no)", "[Y/n]".
    #[must_use]
    pub fn confirmation_prompt() -> Self {
        Self::regex(r"\[([yYnN])/([yYnN])\]|\(([yY]es)/([nN]o)\)")
            .unwrap_or_else(|_| Self::Glob("*[y/n]*".to_string()))
    }

    /// Create a pattern that matches common "continue?" prompts.
    ///
    /// Matches prompts like "Continue?", "Do you want to continue?", "Press any key".
    #[must_use]
    pub fn continue_prompt() -> Self {
        Self::regex(r"(?i)(?:continue\s*\?|press any key|hit enter)")
            .unwrap_or_else(|_| Self::Glob("*continue*".to_string()))
    }
}

impl fmt::Debug for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Literal(s) => write!(f, "Literal({s:?})"),
            Self::Regex(r) => write!(f, "Regex({:?})", r.pattern()),
            Self::Glob(s) => write!(f, "Glob({s:?})"),
            Self::Eof => write!(f, "Eof"),
            Self::Timeout(d) => write!(f, "Timeout({d:?})"),
            Self::Bytes(n) => write!(f, "Bytes({n})"),
        }
    }
}

impl From<&str> for Pattern {
    fn from(s: &str) -> Self {
        Self::Literal(s.to_string())
    }
}

impl From<String> for Pattern {
    fn from(s: String) -> Self {
        Self::Literal(s)
    }
}

/// A compiled regular expression with its source pattern.
#[derive(Clone)]
pub struct CompiledRegex {
    pattern: String,
    regex: Regex,
}

impl CompiledRegex {
    /// Create a new compiled regex.
    #[must_use]
    pub const fn new(pattern: String, regex: Regex) -> Self {
        Self { pattern, regex }
    }

    /// Get the source pattern.
    #[must_use]
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Find the first match in the text.
    #[must_use]
    pub fn find<'a>(&self, text: &'a str) -> Option<regex::Match<'a>> {
        self.regex.find(text)
    }

    /// Get capture groups from a match.
    #[must_use]
    pub fn captures(&self, text: &str) -> Vec<String> {
        self.regex
            .captures(text)
            .map(|caps| {
                caps.iter()
                    .skip(1) // Skip the full match
                    .filter_map(|m| m.map(|m| m.as_str().to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Result of a successful pattern match.
#[derive(Debug, Clone)]
pub struct PatternMatch {
    /// Start position of the match in the text.
    pub start: usize,
    /// End position of the match in the text.
    pub end: usize,
    /// Capture groups (for regex patterns).
    pub captures: Vec<String>,
}

impl PatternMatch {
    /// Get the matched text from the original input.
    #[must_use]
    pub fn as_str<'a>(&self, text: &'a str) -> &'a str {
        &text[self.start..self.end]
    }

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

/// A set of patterns for multi-pattern matching.
#[derive(Debug, Clone, Default)]
pub struct PatternSet {
    patterns: Vec<NamedPattern>,
}

/// A pattern with an optional name.
#[derive(Clone)]
pub struct NamedPattern {
    /// The pattern.
    pub pattern: Pattern,
    /// Optional name for the pattern.
    pub name: Option<String>,
    /// Index in the pattern set.
    pub index: usize,
}

impl fmt::Debug for NamedPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NamedPattern")
            .field("pattern", &self.pattern)
            .field("name", &self.name)
            .field("index", &self.index)
            .finish()
    }
}

impl PatternSet {
    /// Create a new empty pattern set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a pattern set from a vector of patterns.
    #[must_use]
    pub fn from_patterns(patterns: Vec<Pattern>) -> Self {
        let patterns = patterns
            .into_iter()
            .enumerate()
            .map(|(index, pattern)| NamedPattern {
                pattern,
                name: None,
                index,
            })
            .collect();
        Self { patterns }
    }

    /// Add a pattern to the set.
    pub fn add(&mut self, pattern: Pattern) -> &mut Self {
        let index = self.patterns.len();
        self.patterns.push(NamedPattern {
            pattern,
            name: None,
            index,
        });
        self
    }

    /// Add a named pattern to the set.
    pub fn add_named(&mut self, name: impl Into<String>, pattern: Pattern) -> &mut Self {
        let index = self.patterns.len();
        self.patterns.push(NamedPattern {
            pattern,
            name: Some(name.into()),
            index,
        });
        self
    }

    /// Get the number of patterns in the set.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.patterns.len()
    }

    /// Check if the set is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// Find the first matching pattern in the text.
    ///
    /// Returns the pattern index and match details.
    #[must_use]
    pub fn find_match(&self, text: &str) -> Option<(usize, PatternMatch)> {
        let mut best_match: Option<(usize, PatternMatch)> = None;

        for (idx, named) in self.patterns.iter().enumerate() {
            if let Some(m) = named.pattern.matches(text) {
                match &best_match {
                    None => best_match = Some((idx, m)),
                    Some((_, current)) if m.start < current.start => {
                        best_match = Some((idx, m));
                    }
                    _ => {}
                }
            }
        }

        best_match
    }

    /// Get a pattern by index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&NamedPattern> {
        self.patterns.get(index)
    }

    /// Get the minimum timeout from timeout patterns.
    #[must_use]
    pub fn min_timeout(&self) -> Option<Duration> {
        self.patterns
            .iter()
            .filter_map(|p| p.pattern.timeout_duration())
            .min()
    }

    /// Check if any pattern is an EOF pattern.
    #[must_use]
    pub fn has_eof(&self) -> bool {
        self.patterns.iter().any(|p| p.pattern.is_eof())
    }

    /// Get iterator over patterns.
    pub fn iter(&self) -> impl Iterator<Item = &NamedPattern> {
        self.patterns.iter()
    }
}

/// Simple glob pattern matching.
///
/// Supports `*` (any characters) and `?` (single character).
fn glob_match(pattern: &str, text: &str) -> Option<usize> {
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();

    (0..=text_chars.len()).find(|&start| glob_match_from(&pattern_chars, &text_chars[start..]))
}

const fn glob_match_from(pattern: &[char], text: &[char]) -> bool {
    let mut p = 0;
    let mut t = 0;
    let mut star_p = None;
    let mut star_t = 0;

    while p < pattern.len() {
        if pattern[p] == '*' {
            star_p = Some(p);
            star_t = t;
            p += 1;
        } else if t < text.len() && (pattern[p] == '?' || pattern[p] == text[t]) {
            p += 1;
            t += 1;
        } else if let Some(sp) = star_p {
            p = sp + 1;
            star_t += 1;
            if star_t > text.len() {
                return false;
            }
            t = star_t;
        } else {
            return false;
        }
    }

    // Pattern matched - we don't require text to be fully consumed
    // (we're looking for the pattern within the text)
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_pattern_matches() {
        let pattern = Pattern::literal("hello");
        let result = pattern.matches("say hello world");
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(m.start, 4);
        assert_eq!(m.end, 9);
    }

    #[test]
    fn regex_pattern_matches() {
        let pattern = Pattern::regex(r"\d+").unwrap();
        let result = pattern.matches("test 123 value");
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(m.as_str("test 123 value"), "123");
    }

    #[test]
    fn regex_pattern_captures() {
        let pattern = Pattern::regex(r"(\w+)@(\w+)").unwrap();
        let result = pattern.matches("email: user@domain here");
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(m.captures, vec!["user", "domain"]);
    }

    #[test]
    fn glob_pattern_matches() {
        let pattern = Pattern::glob("hello*world");
        let result = pattern.matches("say hello beautiful world!");
        assert!(result.is_some());
    }

    #[test]
    fn pattern_set_finds_first() {
        let mut set = PatternSet::new();
        set.add(Pattern::literal("world"))
            .add(Pattern::literal("hello"));

        let result = set.find_match("hello world");
        assert!(result.is_some());
        let (idx, _) = result.unwrap();
        // "hello" comes first in the text
        assert_eq!(idx, 1);
    }

    #[test]
    fn pattern_set_min_timeout() {
        let mut set = PatternSet::new();
        set.add(Pattern::timeout(Duration::from_secs(10)))
            .add(Pattern::timeout(Duration::from_secs(5)));

        assert_eq!(set.min_timeout(), Some(Duration::from_secs(5)));
    }
}
