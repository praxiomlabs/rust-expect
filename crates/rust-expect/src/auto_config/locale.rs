//! Locale detection and configuration.

use std::collections::HashMap;

/// Locale information.
#[derive(Debug, Clone, Default)]
pub struct LocaleInfo {
    /// Language code (e.g., "en").
    pub language: Option<String>,
    /// Territory/country (e.g., "US").
    pub territory: Option<String>,
    /// Codeset (e.g., "UTF-8").
    pub codeset: Option<String>,
    /// Modifier (e.g., "euro").
    pub modifier: Option<String>,
}

impl LocaleInfo {
    /// Parse a locale string (e.g., "en_US.UTF-8").
    #[must_use]
    pub fn parse(locale: &str) -> Self {
        let mut info = Self::default();

        // Handle empty or "C"/"POSIX" locale
        if locale.is_empty() || locale == "C" || locale == "POSIX" {
            info.language = Some("C".to_string());
            return info;
        }

        let mut remaining = locale;

        // Extract modifier (@modifier)
        if let Some(at_pos) = remaining.rfind('@') {
            info.modifier = Some(remaining[at_pos + 1..].to_string());
            remaining = &remaining[..at_pos];
        }

        // Extract codeset (.codeset)
        if let Some(dot_pos) = remaining.rfind('.') {
            info.codeset = Some(remaining[dot_pos + 1..].to_string());
            remaining = &remaining[..dot_pos];
        }

        // Extract territory (_territory)
        if let Some(under_pos) = remaining.rfind('_') {
            info.territory = Some(remaining[under_pos + 1..].to_string());
            remaining = &remaining[..under_pos];
        }

        // Remaining is the language
        if !remaining.is_empty() {
            info.language = Some(remaining.to_string());
        }

        info
    }

    /// Check if this is a UTF-8 locale.
    #[must_use]
    pub fn is_utf8(&self) -> bool {
        self.codeset.as_ref().is_some_and(|c| {
            let c = c.to_lowercase().replace('-', "");
            c == "utf8"
        })
    }

    /// Format as locale string.
    #[must_use]
    pub fn to_string(&self) -> String {
        let mut result = String::new();

        if let Some(ref lang) = self.language {
            result.push_str(lang);
        }
        if let Some(ref territory) = self.territory {
            result.push('_');
            result.push_str(territory);
        }
        if let Some(ref codeset) = self.codeset {
            result.push('.');
            result.push_str(codeset);
        }
        if let Some(ref modifier) = self.modifier {
            result.push('@');
            result.push_str(modifier);
        }

        result
    }
}

/// Detect current locale from environment.
#[must_use]
pub fn detect_locale() -> LocaleInfo {
    // Check LC_ALL first, then LANG
    let locale = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default();

    LocaleInfo::parse(&locale)
}

/// Get all locale-related environment variables.
#[must_use]
pub fn locale_env() -> HashMap<String, String> {
    let vars = [
        "LANG",
        "LC_ALL",
        "LC_CTYPE",
        "LC_NUMERIC",
        "LC_TIME",
        "LC_COLLATE",
        "LC_MONETARY",
        "LC_MESSAGES",
        "LC_PAPER",
        "LC_NAME",
        "LC_ADDRESS",
        "LC_TELEPHONE",
        "LC_MEASUREMENT",
        "LC_IDENTIFICATION",
    ];

    let mut result = HashMap::new();
    for var in vars {
        if let Ok(value) = std::env::var(var) {
            result.insert(var.to_string(), value);
        }
    }
    result
}

/// Check if the environment supports UTF-8.
#[must_use]
pub fn is_utf8_environment() -> bool {
    detect_locale().is_utf8()
}

/// Recommended environment for UTF-8 support.
#[must_use]
pub fn utf8_environment() -> HashMap<String, String> {
    let mut env = HashMap::new();
    env.insert("LANG".to_string(), "en_US.UTF-8".to_string());
    env.insert("LC_ALL".to_string(), "en_US.UTF-8".to_string());
    env
}

/// Force UTF-8 locale environment.
#[must_use]
pub fn force_utf8_env() -> HashMap<String, String> {
    let mut env = locale_env();
    env.insert("LC_ALL".to_string(), "en_US.UTF-8".to_string());
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_locale() {
        let info = LocaleInfo::parse("en_US.UTF-8");
        assert_eq!(info.language, Some("en".to_string()));
        assert_eq!(info.territory, Some("US".to_string()));
        assert_eq!(info.codeset, Some("UTF-8".to_string()));
        assert!(info.is_utf8());
    }

    #[test]
    fn parse_locale_with_modifier() {
        let info = LocaleInfo::parse("de_DE.UTF-8@euro");
        assert_eq!(info.language, Some("de".to_string()));
        assert_eq!(info.territory, Some("DE".to_string()));
        assert_eq!(info.modifier, Some("euro".to_string()));
    }

    #[test]
    fn parse_c_locale() {
        let info = LocaleInfo::parse("C");
        assert_eq!(info.language, Some("C".to_string()));
        assert!(!info.is_utf8());
    }

    #[test]
    fn locale_to_string() {
        let info = LocaleInfo {
            language: Some("en".to_string()),
            territory: Some("US".to_string()),
            codeset: Some("UTF-8".to_string()),
            modifier: None,
        };
        assert_eq!(info.to_string(), "en_US.UTF-8");
    }
}
