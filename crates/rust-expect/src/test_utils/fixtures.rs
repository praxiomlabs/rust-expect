//! Test fixtures helpers.

use std::collections::HashMap;
use std::path::PathBuf;

/// A single test fixture.
#[derive(Debug, Clone)]
pub struct TestFixture {
    /// Fixture name.
    pub name: String,
    /// Fixture content.
    pub content: Vec<u8>,
    /// Expected output (if any).
    pub expected: Option<Vec<u8>>,
    /// Metadata.
    pub metadata: HashMap<String, String>,
}

impl TestFixture {
    /// Create a new fixture.
    #[must_use]
    pub fn new(name: impl Into<String>, content: impl Into<Vec<u8>>) -> Self {
        Self {
            name: name.into(),
            content: content.into(),
            expected: None,
            metadata: HashMap::new(),
        }
    }

    /// Create from a string.
    #[must_use]
    pub fn from_str(name: impl Into<String>, content: &str) -> Self {
        Self::new(name, content.as_bytes().to_vec())
    }

    /// Set expected output.
    #[must_use]
    pub fn with_expected(mut self, expected: impl Into<Vec<u8>>) -> Self {
        self.expected = Some(expected.into());
        self
    }

    /// Add metadata.
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Get content as string.
    #[must_use]
    pub fn content_str(&self) -> &str {
        std::str::from_utf8(&self.content).unwrap_or("<invalid utf8>")
    }

    /// Get expected as string.
    #[must_use]
    pub fn expected_str(&self) -> Option<&str> {
        self.expected
            .as_ref()
            .and_then(|e| std::str::from_utf8(e).ok())
    }
}

/// Collection of test fixtures.
#[derive(Debug, Default)]
pub struct Fixtures {
    /// Loaded fixtures.
    fixtures: HashMap<String, TestFixture>,
    /// Base path for fixture files.
    base_path: Option<PathBuf>,
}

impl Fixtures {
    /// Create empty fixtures collection.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the base path for loading fixtures from files.
    #[must_use]
    pub fn with_base_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.base_path = Some(path.into());
        self
    }

    /// Add a fixture.
    pub fn add(&mut self, fixture: TestFixture) {
        self.fixtures.insert(fixture.name.clone(), fixture);
    }

    /// Add a fixture from inline content.
    pub fn add_inline(&mut self, name: impl Into<String>, content: &str) {
        self.add(TestFixture::from_str(name, content));
    }

    /// Load a fixture from a file.
    pub fn load(&mut self, name: impl Into<String>, filename: &str) -> std::io::Result<()> {
        let name = name.into();
        let path = if let Some(base) = &self.base_path {
            base.join(filename)
        } else {
            PathBuf::from(filename)
        };

        let content = std::fs::read(&path)?;
        self.add(TestFixture::new(name, content));
        Ok(())
    }

    /// Get a fixture by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&TestFixture> {
        self.fixtures.get(name)
    }

    /// Get fixture content as string.
    #[must_use]
    pub fn content(&self, name: &str) -> Option<&str> {
        self.get(name).map(TestFixture::content_str)
    }

    /// Get fixture content as bytes.
    #[must_use]
    pub fn bytes(&self, name: &str) -> Option<&[u8]> {
        self.get(name).map(|f| f.content.as_slice())
    }

    /// List all fixture names.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.fixtures.keys().map(String::as_str).collect()
    }

    /// Get the number of fixtures.
    #[must_use]
    pub fn len(&self) -> usize {
        self.fixtures.len()
    }

    /// Check if empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fixtures.is_empty()
    }

    /// Create standard terminal fixtures.
    #[must_use]
    pub fn terminal_fixtures() -> Self {
        let mut fixtures = Self::new();

        // Login prompt
        fixtures.add_inline("login_prompt", "Login: ");

        // Password prompt
        fixtures.add_inline("password_prompt", "Password: ");

        // Shell prompt
        fixtures.add_inline("shell_prompt", "$ ");

        // Bash prompt
        fixtures.add_inline("bash_prompt", "[user@host ~]$ ");

        // Root prompt
        fixtures.add_inline("root_prompt", "# ");

        // Confirmation prompt
        fixtures.add_inline("confirm_prompt", "Are you sure? [y/N] ");

        // Progress output
        fixtures.add_inline(
            "progress_output",
            "Processing... 10%\nProcessing... 50%\nProcessing... 100%\nDone.",
        );

        // Error output
        fixtures.add_inline(
            "error_output",
            "Error: Something went wrong\nPlease try again.",
        );

        // ANSI colored output
        fixtures.add(TestFixture::new(
            "ansi_output",
            b"\x1b[31mRed\x1b[0m \x1b[32mGreen\x1b[0m \x1b[34mBlue\x1b[0m".to_vec(),
        ));

        // Multi-line output
        fixtures.add_inline(
            "multiline",
            "Line 1\nLine 2\nLine 3\nLine 4\nLine 5",
        );

        fixtures
    }
}

/// Find the fixtures directory relative to the project root.
///
/// Searches up to 5 parent directories looking for either a `fixtures`
/// directory or a `tests/fixtures` directory.
///
/// # Returns
///
/// Returns `Some(path)` if a fixtures directory is found, `None` otherwise.
///
/// # Example
///
/// ```rust,no_run
/// use rust_expect::test_utils::find_fixtures_dir;
///
/// if let Some(fixtures_path) = find_fixtures_dir() {
///     println!("Found fixtures at: {}", fixtures_path.display());
/// }
/// ```
#[must_use]
pub fn find_fixtures_dir() -> Option<PathBuf> {
    let mut path = std::env::current_dir().ok()?;

    // Look for fixtures directory
    for _ in 0..5 {
        let fixtures = path.join("fixtures");
        if fixtures.is_dir() {
            return Some(fixtures);
        }
        let test_fixtures = path.join("tests").join("fixtures");
        if test_fixtures.is_dir() {
            return Some(test_fixtures);
        }
        path = path.parent()?.to_path_buf();
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixture_creation() {
        let fixture = TestFixture::from_str("test", "Hello, World!")
            .with_expected(b"Expected".to_vec())
            .with_metadata("type", "greeting");

        assert_eq!(fixture.name, "test");
        assert_eq!(fixture.content_str(), "Hello, World!");
        assert_eq!(fixture.expected_str(), Some("Expected"));
        assert_eq!(fixture.metadata.get("type"), Some(&"greeting".to_string()));
    }

    #[test]
    fn test_fixtures_collection() {
        let mut fixtures = Fixtures::new();
        fixtures.add_inline("prompt", "$ ");
        fixtures.add_inline("greeting", "Hello!");

        assert_eq!(fixtures.len(), 2);
        assert_eq!(fixtures.content("prompt"), Some("$ "));
        assert!(fixtures.names().contains(&"greeting"));
    }

    #[test]
    fn test_terminal_fixtures() {
        let fixtures = Fixtures::terminal_fixtures();

        assert!(fixtures.get("login_prompt").is_some());
        assert!(fixtures.get("shell_prompt").is_some());
        assert!(fixtures.get("ansi_output").is_some());
    }

    #[test]
    fn test_find_fixtures_dir() {
        // This function searches for fixtures directories relative to cwd
        // It may or may not find one depending on where tests are run
        let result = find_fixtures_dir();
        // The function should return Some if a fixtures dir exists, None otherwise
        // We just verify it doesn't panic and returns a valid Option
        if let Some(path) = result {
            assert!(path.is_dir(), "found path should be a directory");
        }
    }
}
