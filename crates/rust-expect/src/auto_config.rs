//! Automatic configuration detection.
//!
//! This module provides functionality for automatically detecting
//! and configuring terminal session parameters.

pub mod line_ending;
pub mod locale;
pub mod prompt;
pub mod shell;

pub use line_ending::{
    LineEnding, LineEndingConfig, detect_line_ending, normalize_line_endings, to_crlf, to_lf,
};
pub use locale::{LocaleInfo, detect_locale, is_utf8_environment, locale_env};
pub use prompt::{PromptConfig, PromptInfo, detect_prompt, ends_with_prompt};
pub use shell::{ShellConfig, ShellType, default_shell, detect_from_path, detect_shell};
