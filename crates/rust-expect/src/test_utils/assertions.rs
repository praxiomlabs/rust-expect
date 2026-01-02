//! Custom assertion helpers for expect tests.

use regex::Regex;

/// Assertion helpers for output testing.
pub trait OutputAssertions {
    /// Get the output as a string.
    fn as_str(&self) -> &str;

    /// Assert that output contains a literal string.
    fn assert_contains(&self, needle: &str) {
        let output = self.as_str();
        assert!(
            output.contains(needle),
            "Expected output to contain {needle:?}, but got:\n{output}"
        );
    }

    /// Assert that output matches a regex pattern.
    fn assert_matches(&self, pattern: &str) {
        let output = self.as_str();
        let re = Regex::new(pattern).expect("Invalid regex pattern");
        assert!(
            re.is_match(output),
            "Expected output to match pattern {pattern:?}, but got:\n{output}"
        );
    }

    /// Assert that output does not contain a literal string.
    fn assert_not_contains(&self, needle: &str) {
        let output = self.as_str();
        assert!(
            !output.contains(needle),
            "Expected output NOT to contain {needle:?}, but found it in:\n{output}"
        );
    }

    /// Assert that output starts with a literal string.
    fn assert_starts_with(&self, prefix: &str) {
        let output = self.as_str();
        assert!(
            output.starts_with(prefix),
            "Expected output to start with {prefix:?}, but got:\n{output}"
        );
    }

    /// Assert that output ends with a literal string.
    fn assert_ends_with(&self, suffix: &str) {
        let output = self.as_str();
        assert!(
            output.ends_with(suffix),
            "Expected output to end with {suffix:?}, but got:\n{output}"
        );
    }

    /// Assert that output is empty.
    fn assert_empty(&self) {
        let output = self.as_str();
        assert!(
            output.is_empty(),
            "Expected output to be empty, but got:\n{output}"
        );
    }

    /// Assert that output equals exactly.
    fn assert_eq(&self, expected: &str) {
        let output = self.as_str();
        assert_eq!(
            output, expected,
            "Expected output to equal {expected:?}, but got:\n{output}"
        );
    }

    /// Assert output line count.
    fn assert_line_count(&self, expected: usize) {
        let output = self.as_str();
        let count = output.lines().count();
        assert_eq!(
            count, expected,
            "Expected {expected} lines, but got {count} in:\n{output}"
        );
    }
}

impl OutputAssertions for str {
    fn as_str(&self) -> &str {
        self
    }
}

impl OutputAssertions for String {
    fn as_str(&self) -> &str {
        self.as_str()
    }
}

impl OutputAssertions for Vec<u8> {
    fn as_str(&self) -> &str {
        std::str::from_utf8(self).unwrap_or("<invalid utf8>")
    }
}

/// Assert that output contains a literal string.
pub fn assert_output_contains(output: &str, needle: &str) {
    output.assert_contains(needle);
}

/// Assert that output matches a regex pattern.
pub fn assert_output_matches(output: &str, pattern: &str) {
    output.assert_matches(pattern);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assert_contains() {
        let output = "Hello, World!";
        output.assert_contains("World");
    }

    #[test]
    fn test_assert_matches() {
        let output = "User: john123";
        output.assert_matches(r"User: \w+");
    }

    #[test]
    fn test_assert_not_contains() {
        let output = "Hello, World!";
        output.assert_not_contains("Goodbye");
    }

    #[test]
    fn test_vec_output() {
        let output = b"Hello".to_vec();
        output.assert_contains("Hello");
    }
}
