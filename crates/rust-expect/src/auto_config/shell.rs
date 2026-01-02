//! Shell detection and configuration.

use std::path::PathBuf;

/// Known shell types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellType {
    /// Bourne shell.
    Sh,
    /// Bash shell.
    Bash,
    /// Zsh shell.
    Zsh,
    /// Fish shell.
    Fish,
    /// Ksh shell.
    Ksh,
    /// Tcsh shell.
    Tcsh,
    /// Dash shell.
    Dash,
    /// `PowerShell`.
    PowerShell,
    /// Windows Command Prompt.
    Cmd,
    /// Unknown shell.
    Unknown,
}

impl ShellType {
    /// Get shell name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Sh => "sh",
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::Fish => "fish",
            Self::Ksh => "ksh",
            Self::Tcsh => "tcsh",
            Self::Dash => "dash",
            Self::PowerShell => "powershell",
            Self::Cmd => "cmd",
            Self::Unknown => "unknown",
        }
    }

    /// Check if shell supports ANSI sequences.
    #[must_use]
    pub const fn supports_ansi(&self) -> bool {
        !matches!(self, Self::Cmd)
    }

    /// Get typical prompt pattern.
    #[must_use]
    pub const fn prompt_pattern(&self) -> &'static str {
        match self {
            Self::Bash | Self::Sh | Self::Dash | Self::Ksh => r"[$#]\s*$",
            Self::Zsh => r"[%#$]\s*$",
            Self::Fish => r">\s*$",
            Self::Tcsh => r"[%>]\s*$",
            Self::PowerShell => r"PS[^>]*>\s*$",
            Self::Cmd => r">\s*$",
            Self::Unknown => r"[$#%>]\s*$",
        }
    }

    /// Get exit command.
    #[must_use]
    pub const fn exit_command(&self) -> &'static str {
        match self {
            Self::Cmd => "exit",
            Self::PowerShell => "exit",
            _ => "exit",
        }
    }
}

/// Detect shell type from environment.
#[must_use]
pub fn detect_shell() -> ShellType {
    // Check SHELL environment variable
    if let Ok(shell) = std::env::var("SHELL") {
        return detect_from_path(&shell);
    }

    // Windows: check COMSPEC
    #[cfg(windows)]
    if let Ok(comspec) = std::env::var("COMSPEC") {
        if comspec.to_lowercase().contains("powershell") {
            return ShellType::PowerShell;
        }
        return ShellType::Cmd;
    }

    ShellType::Unknown
}

/// Detect shell type from path.
#[must_use]
pub fn detect_from_path(path: &str) -> ShellType {
    let path_lower = path.to_lowercase();
    let path_buf = PathBuf::from(&path_lower);
    let name = path_buf
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&path_lower);

    match name {
        "sh" => ShellType::Sh,
        "bash" => ShellType::Bash,
        "zsh" => ShellType::Zsh,
        "fish" => ShellType::Fish,
        "ksh" | "ksh93" | "mksh" => ShellType::Ksh,
        "tcsh" | "csh" => ShellType::Tcsh,
        "dash" => ShellType::Dash,
        "pwsh" | "powershell" | "powershell.exe" => ShellType::PowerShell,
        "cmd" | "cmd.exe" => ShellType::Cmd,
        _ => ShellType::Unknown,
    }
}

/// Get default shell path.
#[must_use]
pub fn default_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| {
        #[cfg(unix)]
        {
            "/bin/sh".to_string()
        }
        #[cfg(windows)]
        {
            std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
        }
        #[cfg(not(any(unix, windows)))]
        {
            "sh".to_string()
        }
    })
}

/// Shell configuration options.
#[derive(Debug, Clone)]
pub struct ShellConfig {
    /// Shell type.
    pub shell_type: ShellType,
    /// Shell path.
    pub path: String,
    /// Additional arguments.
    pub args: Vec<String>,
    /// Environment variables.
    pub env: std::collections::HashMap<String, String>,
    /// Working directory.
    pub cwd: Option<PathBuf>,
}

impl Default for ShellConfig {
    fn default() -> Self {
        let path = default_shell();
        let shell_type = detect_from_path(&path);
        Self {
            shell_type,
            path,
            args: Vec::new(),
            env: std::collections::HashMap::new(),
            cwd: None,
        }
    }
}

impl ShellConfig {
    /// Create a new shell config.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set shell path.
    #[must_use]
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = path.into();
        self.shell_type = detect_from_path(&self.path);
        self
    }

    /// Add an argument.
    #[must_use]
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Set an environment variable.
    #[must_use]
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Set working directory.
    #[must_use]
    pub fn cwd(mut self, dir: impl Into<PathBuf>) -> Self {
        self.cwd = Some(dir.into());
        self
    }

    /// Get command and args for spawning.
    #[must_use]
    pub fn command(&self) -> (&str, &[String]) {
        (&self.path, &self.args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_bash() {
        assert_eq!(detect_from_path("/bin/bash"), ShellType::Bash);
        assert_eq!(detect_from_path("/usr/bin/bash"), ShellType::Bash);
    }

    #[test]
    fn detect_zsh() {
        assert_eq!(detect_from_path("/bin/zsh"), ShellType::Zsh);
    }

    #[test]
    fn shell_type_name() {
        assert_eq!(ShellType::Bash.name(), "bash");
        assert_eq!(ShellType::Zsh.name(), "zsh");
    }

    #[test]
    fn shell_config_default() {
        let config = ShellConfig::new();
        assert!(!config.path.is_empty());
    }
}
