//! File-based configuration loading.

use crate::error::{ExpectError, Result};
use std::collections::HashMap;
use std::path::Path;

/// Configuration file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    /// TOML format.
    Toml,
    /// JSON format.
    Json,
    /// YAML format.
    Yaml,
    /// INI format.
    Ini,
}

impl ConfigFormat {
    /// Detect format from file extension.
    #[must_use]
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "toml" => Some(Self::Toml),
            "json" => Some(Self::Json),
            "yaml" | "yml" => Some(Self::Yaml),
            "ini" | "cfg" | "conf" => Some(Self::Ini),
            _ => None,
        }
    }

    /// Detect format from path.
    #[must_use]
    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|e| e.to_str())
            .and_then(Self::from_extension)
    }
}

/// Configuration value.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigValue {
    /// String value.
    String(String),
    /// Integer value.
    Integer(i64),
    /// Float value.
    Float(f64),
    /// Boolean value.
    Boolean(bool),
    /// Array value.
    Array(Vec<ConfigValue>),
    /// Table/object value.
    Table(HashMap<String, ConfigValue>),
    /// Null value.
    Null,
}

impl ConfigValue {
    /// Get as string.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// Get as integer.
    #[must_use]
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// Get as float.
    #[must_use]
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(f) => Some(*f),
            Self::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// Get as boolean.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Get as array.
    #[must_use]
    pub fn as_array(&self) -> Option<&Vec<ConfigValue>> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Get as table.
    #[must_use]
    pub fn as_table(&self) -> Option<&HashMap<String, ConfigValue>> {
        match self {
            Self::Table(t) => Some(t),
            _ => None,
        }
    }

    /// Get value by key (for tables).
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&ConfigValue> {
        self.as_table().and_then(|t| t.get(key))
    }

    /// Get nested value by path (e.g., "section.key").
    #[must_use]
    pub fn get_path(&self, path: &str) -> Option<&ConfigValue> {
        let mut current = self;
        for key in path.split('.') {
            current = current.get(key)?;
        }
        Some(current)
    }
}

/// Configuration file loader.
#[derive(Debug, Default)]
pub struct ConfigLoader {
    /// Search paths.
    search_paths: Vec<std::path::PathBuf>,
    /// Default format.
    default_format: Option<ConfigFormat>,
}

impl ConfigLoader {
    /// Create a new loader.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a search path.
    #[must_use]
    pub fn add_path(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        self.search_paths.push(path.into());
        self
    }

    /// Set default format.
    #[must_use]
    pub fn with_format(mut self, format: ConfigFormat) -> Self {
        self.default_format = Some(format);
        self
    }

    /// Find a config file.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<std::path::PathBuf> {
        let extensions = ["toml", "json", "yaml", "yml", "ini", "cfg"];

        for search_path in &self.search_paths {
            // Try exact name first
            let path = search_path.join(name);
            if path.exists() {
                return Some(path);
            }

            // Try with extensions
            for ext in &extensions {
                let path = search_path.join(format!("{}.{}", name, ext));
                if path.exists() {
                    return Some(path);
                }
            }
        }

        None
    }

    /// Load a config file.
    pub fn load(&self, path: &Path) -> Result<ConfigValue> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ExpectError::Io(e))?;

        let format = ConfigFormat::from_path(path)
            .or(self.default_format)
            .ok_or_else(|| ExpectError::config("Unknown config format"))?;

        parse_config(&content, format)
    }

    /// Load by name (searches paths).
    pub fn load_by_name(&self, name: &str) -> Result<ConfigValue> {
        let path = self.find(name)
            .ok_or_else(|| ExpectError::config(format!("Config file not found: {}", name)))?;
        self.load(&path)
    }
}

/// Parse config content.
pub fn parse_config(content: &str, format: ConfigFormat) -> Result<ConfigValue> {
    match format {
        ConfigFormat::Toml => parse_simple_toml(content),
        ConfigFormat::Json => parse_simple_json(content),
        ConfigFormat::Yaml => parse_simple_yaml(content),
        ConfigFormat::Ini => parse_simple_ini(content),
    }
}

/// Simple TOML parser (basic key=value).
fn parse_simple_toml(content: &str) -> Result<ConfigValue> {
    let mut table = HashMap::new();
    let mut current_section = String::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Section header
        if line.starts_with('[') && line.ends_with(']') {
            current_section = line[1..line.len()-1].to_string();
            continue;
        }

        // Key = value
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim();
            let value = line[eq_pos+1..].trim();

            let full_key = if current_section.is_empty() {
                key.to_string()
            } else {
                format!("{}.{}", current_section, key)
            };

            table.insert(full_key, parse_value(value));
        }
    }

    Ok(ConfigValue::Table(table))
}

/// Simple JSON parser (basic objects).
fn parse_simple_json(content: &str) -> Result<ConfigValue> {
    // Very basic JSON parsing
    let content = content.trim();
    if content.starts_with('{') && content.ends_with('}') {
        let mut table = HashMap::new();
        let inner = &content[1..content.len()-1];

        for pair in inner.split(',') {
            let pair = pair.trim();
            if let Some(colon_pos) = pair.find(':') {
                let key = pair[..colon_pos].trim().trim_matches('"');
                let value = pair[colon_pos+1..].trim();
                table.insert(key.to_string(), parse_value(value));
            }
        }

        Ok(ConfigValue::Table(table))
    } else {
        Ok(parse_value(content))
    }
}

/// Simple YAML parser (basic key: value).
fn parse_simple_yaml(content: &str) -> Result<ConfigValue> {
    let mut table = HashMap::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(colon_pos) = line.find(':') {
            let key = line[..colon_pos].trim();
            let value = line[colon_pos+1..].trim();
            if !value.is_empty() {
                table.insert(key.to_string(), parse_value(value));
            }
        }
    }

    Ok(ConfigValue::Table(table))
}

/// Simple INI parser.
fn parse_simple_ini(content: &str) -> Result<ConfigValue> {
    parse_simple_toml(content) // INI is similar to basic TOML
}

/// Parse a single value.
fn parse_value(value: &str) -> ConfigValue {
    let value = value.trim().trim_matches('"').trim_matches('\'');

    if value == "true" {
        ConfigValue::Boolean(true)
    } else if value == "false" {
        ConfigValue::Boolean(false)
    } else if value == "null" || value == "~" {
        ConfigValue::Null
    } else if let Ok(i) = value.parse::<i64>() {
        ConfigValue::Integer(i)
    } else if let Ok(f) = value.parse::<f64>() {
        ConfigValue::Float(f)
    } else {
        ConfigValue::String(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_toml_basic() {
        let content = r#"
            name = "test"
            value = 42
            enabled = true
        "#;

        let config = parse_simple_toml(content).unwrap();
        assert_eq!(config.get("name").unwrap().as_str(), Some("test"));
        assert_eq!(config.get("value").unwrap().as_int(), Some(42));
        assert_eq!(config.get("enabled").unwrap().as_bool(), Some(true));
    }

    #[test]
    fn parse_yaml_basic() {
        let content = r#"
            name: test
            value: 42
        "#;

        let config = parse_simple_yaml(content).unwrap();
        assert_eq!(config.get("name").unwrap().as_str(), Some("test"));
        assert_eq!(config.get("value").unwrap().as_int(), Some(42));
    }

    #[test]
    fn config_format_detection() {
        assert_eq!(ConfigFormat::from_extension("toml"), Some(ConfigFormat::Toml));
        assert_eq!(ConfigFormat::from_extension("json"), Some(ConfigFormat::Json));
        assert_eq!(ConfigFormat::from_extension("yaml"), Some(ConfigFormat::Yaml));
    }
}
