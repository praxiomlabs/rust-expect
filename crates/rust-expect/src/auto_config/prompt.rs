//! Prompt detection and configuration.

use std::sync::LazyLock;

use regex::Regex;

/// Common prompt patterns.
/// Order matters: more specific patterns must come before generic ones.
static PROMPT_PATTERNS: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    vec![
        // Most specific patterns first
        (
            "python",
            Regex::new(r">>>\s*$").expect("Python prompt pattern is a valid regex"),
        ),
        (
            "irb",
            Regex::new(r"irb\([^)]*\):\d+:\d+[>*]\s*$")
                .expect("IRB prompt pattern is a valid regex"),
        ),
        (
            "powershell",
            Regex::new(r"PS[^>]*>\s*$").expect("PowerShell prompt pattern is a valid regex"),
        ),
        (
            "mysql",
            Regex::new(r"mysql>\s*$").expect("MySQL prompt pattern is a valid regex"),
        ),
        (
            "postgres",
            Regex::new(r"[a-z_]+[=#]\s*$").expect("PostgreSQL prompt pattern is a valid regex"),
        ),
        // Root before bash/zsh (# is in both [$#] and [%#$>])
        (
            "root",
            Regex::new(r"^root@[^#]*#\s*$").expect("Root prompt pattern is a valid regex"),
        ),
        // General shell patterns
        (
            "bash",
            Regex::new(r"[$#]\s*$").expect("Bash prompt pattern is a valid regex"),
        ),
        (
            "zsh",
            Regex::new(r"%\s*$").expect("Zsh prompt pattern is a valid regex"),
        ),
        (
            "fish",
            Regex::new(r"[^>]>\s*$").expect("Fish prompt pattern is a valid regex"),
        ),
        (
            "cmd",
            Regex::new(r"[^>]>\s*$").expect("CMD prompt pattern is a valid regex"),
        ),
        (
            "node",
            Regex::new(r"[^>]>\s*$").expect("Node prompt pattern is a valid regex"),
        ),
    ]
});

/// Prompt detection result.
#[derive(Debug, Clone)]
pub struct PromptInfo {
    /// Detected prompt type.
    pub prompt_type: String,
    /// Matched prompt text.
    pub matched: String,
    /// Position in buffer.
    pub position: usize,
    /// Confidence (0.0-1.0).
    pub confidence: f32,
}

/// Detect prompt in text.
#[must_use]
pub fn detect_prompt(text: &str) -> Option<PromptInfo> {
    // Look at last few lines
    let lines: Vec<&str> = text.lines().collect();
    let last_lines: String = lines
        .iter()
        .rev()
        .take(3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .copied()
        .collect::<Vec<_>>()
        .join("\n");

    for (name, pattern) in PROMPT_PATTERNS.iter() {
        if let Some(m) = pattern.find(&last_lines) {
            return Some(PromptInfo {
                prompt_type: (*name).to_string(),
                matched: m.as_str().to_string(),
                position: text.len() - (last_lines.len() - m.start()),
                confidence: 0.8,
            });
        }
    }

    None
}

/// Check if text ends with a prompt.
#[must_use]
pub fn ends_with_prompt(text: &str) -> bool {
    detect_prompt(text).is_some()
}

/// Prompt configuration.
#[derive(Debug, Clone)]
pub struct PromptConfig {
    /// Custom prompt pattern.
    pub pattern: Option<String>,
    /// Compiled regex.
    regex: Option<Regex>,
    /// Wait for prompt after commands.
    pub wait_for_prompt: bool,
    /// Timeout for prompt detection.
    pub timeout_ms: u64,
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            pattern: None,
            regex: None,
            wait_for_prompt: true,
            timeout_ms: 5000,
        }
    }
}

impl PromptConfig {
    /// Create new prompt config.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set custom prompt pattern.
    #[must_use]
    pub fn with_pattern(mut self, pattern: &str) -> Self {
        self.pattern = Some(pattern.to_string());
        self.regex = Regex::new(pattern).ok();
        self
    }

    /// Set wait for prompt.
    #[must_use]
    pub const fn with_wait(mut self, wait: bool) -> Self {
        self.wait_for_prompt = wait;
        self
    }

    /// Set timeout.
    #[must_use]
    pub const fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Check if text matches prompt.
    #[must_use]
    pub fn matches(&self, text: &str) -> bool {
        if let Some(ref regex) = self.regex {
            regex.is_match(text)
        } else {
            ends_with_prompt(text)
        }
    }

    /// Find prompt in text.
    #[must_use]
    pub fn find(&self, text: &str) -> Option<PromptInfo> {
        if let Some(ref regex) = self.regex {
            regex.find(text).map(|m| PromptInfo {
                prompt_type: "custom".to_string(),
                matched: m.as_str().to_string(),
                position: m.start(),
                confidence: 1.0,
            })
        } else {
            detect_prompt(text)
        }
    }
}

/// Generate a unique prompt marker.
#[must_use]
pub fn generate_prompt_marker() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("__EXPECT_PROMPT_{timestamp}__")
}

/// Create a command that sets a unique prompt.
#[must_use]
pub fn set_prompt_command(marker: &str) -> String {
    format!("PS1='{marker} '")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_bash_prompt() {
        let text = "user@host:~$ ";
        let info = detect_prompt(text);
        assert!(info.is_some());
    }

    #[test]
    fn detect_root_prompt() {
        let text = "root@host:/# ";
        let info = detect_prompt(text);
        assert!(info.is_some());
        assert_eq!(info.unwrap().prompt_type, "root");
    }

    #[test]
    fn detect_python_prompt() {
        let text = ">>> ";
        let info = detect_prompt(text);
        assert!(info.is_some());
        assert_eq!(info.unwrap().prompt_type, "python");
    }

    #[test]
    fn prompt_config_custom() {
        let config = PromptConfig::new().with_pattern(r"myhost>\s*$");
        assert!(config.matches("myhost> "));
        assert!(!config.matches("other> "));
    }

    #[test]
    fn prompt_marker() {
        let marker = generate_prompt_marker();
        assert!(marker.starts_with("__EXPECT_PROMPT_"));
        assert!(marker.ends_with("__"));
    }
}
