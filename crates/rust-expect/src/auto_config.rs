//! Automatic configuration detection.
//!
//! This module provides functionality for automatically detecting
//! and configuring terminal session parameters.

pub mod line_ending;
pub mod locale;
pub mod prompt;
pub mod shell;

pub use line_ending::{detect_line_ending, normalize_line_endings, to_crlf, to_lf, LineEnding, LineEndingConfig};
pub use locale::{detect_locale, is_utf8_environment, locale_env, LocaleInfo};
pub use prompt::{detect_prompt, ends_with_prompt, PromptConfig, PromptInfo};
pub use shell::{default_shell, detect_from_path, detect_shell, ShellConfig, ShellType};
