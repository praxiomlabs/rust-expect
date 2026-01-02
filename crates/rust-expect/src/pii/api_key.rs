//! API key and token detection.

use regex::Regex;
use std::sync::LazyLock;

/// Common API key patterns.
static API_KEY_PATTERNS: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    vec![
        ("AWS Access Key", Regex::new(r"AKIA[A-Z0-9]{16}")
            .expect("AWS access key pattern is a valid regex")),
        ("AWS Secret Key", Regex::new(r#"(?i)aws[_-]?secret[_-]?access[_-]?key\s*[:=]\s*['"]?([A-Za-z0-9/+=]{40})['"]?"#)
            .expect("AWS secret key pattern is a valid regex")),
        ("GitHub Token", Regex::new(r"gh[pousr]_[A-Za-z0-9_]{36,}")
            .expect("GitHub token pattern is a valid regex")),
        ("Slack Token", Regex::new(r"xox[baprs]-[0-9a-zA-Z-]+")
            .expect("Slack token pattern is a valid regex")),
        ("Google API Key", Regex::new(r"AIza[0-9A-Za-z_-]{35}")
            .expect("Google API key pattern is a valid regex")),
        ("Stripe Key", Regex::new(r"sk_(?:live|test)_[0-9a-zA-Z]{24,}")
            .expect("Stripe key pattern is a valid regex")),
        ("Generic API Key", Regex::new(r#"(?i)(?:api[_-]?key|apikey|access[_-]?token|auth[_-]?token)\s*[:=]\s*['"]?([A-Za-z0-9_-]{16,})['"]?"#)
            .expect("Generic API key pattern is a valid regex")),
    ]
});

/// API key type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiKeyType {
    /// AWS Access Key ID (starts with AKIA).
    AwsAccessKey,
    /// AWS Secret Access Key.
    AwsSecretKey,
    /// GitHub personal access token or app token.
    GitHubToken,
    /// Slack API token (bot, user, or app).
    SlackToken,
    /// Google Cloud API key.
    GoogleApiKey,
    /// Stripe API key (live or test).
    StripeKey,
    /// Generic API key detected by common patterns.
    Generic,
    /// Unknown or unclassified key type.
    Unknown,
}

/// A detected API key.
#[derive(Debug, Clone)]
pub struct ApiKeyMatch {
    /// Type of API key.
    pub key_type: ApiKeyType,
    /// Start position.
    pub start: usize,
    /// End position.
    pub end: usize,
    /// The matched text.
    pub text: String,
}

/// Detect API keys in text.
#[must_use]
pub fn detect(text: &str) -> Vec<ApiKeyMatch> {
    let mut matches = Vec::new();

    for (name, pattern) in API_KEY_PATTERNS.iter() {
        for m in pattern.find_iter(text) {
            let key_type = match *name {
                "AWS Access Key" => ApiKeyType::AwsAccessKey,
                "AWS Secret Key" => ApiKeyType::AwsSecretKey,
                "GitHub Token" => ApiKeyType::GitHubToken,
                "Slack Token" => ApiKeyType::SlackToken,
                "Google API Key" => ApiKeyType::GoogleApiKey,
                "Stripe Key" => ApiKeyType::StripeKey,
                "Generic API Key" => ApiKeyType::Generic,
                _ => ApiKeyType::Unknown,
            };

            matches.push(ApiKeyMatch {
                key_type,
                start: m.start(),
                end: m.end(),
                text: m.as_str().to_string(),
            });
        }
    }

    matches.sort_by_key(|m| m.start);
    matches
}

/// Check if text contains any API keys.
#[must_use]
pub fn contains_api_key(text: &str) -> bool {
    !detect(text).is_empty()
}

/// Mask an API key (show only first and last few characters).
#[must_use]
pub fn mask(key: &str) -> String {
    if key.len() <= 8 {
        return "*".repeat(key.len());
    }

    format!(
        "{}...{}",
        &key[..4],
        &key[key.len() - 4..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_aws_key() {
        let matches = detect("Key: AKIAIOSFODNN7EXAMPLE");
        assert!(!matches.is_empty());
        assert_eq!(matches[0].key_type, ApiKeyType::AwsAccessKey);
    }

    #[test]
    fn detect_github_token() {
        let matches = detect("Token: ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx1234");
        assert!(!matches.is_empty());
        assert_eq!(matches[0].key_type, ApiKeyType::GitHubToken);
    }

    #[test]
    fn mask_key() {
        assert_eq!(mask("AKIAIOSFODNN7EXAMPLE"), "AKIA...MPLE");
    }

    #[test]
    fn contains_key() {
        assert!(contains_api_key("api_key=my_secret_key_12345678"));
        assert!(!contains_api_key("no keys here"));
    }
}
