//! Line ending detection and handling.

/// Line ending style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    /// Unix-style LF (\n).
    Lf,
    /// Windows-style CRLF (\r\n).
    CrLf,
    /// Old Mac-style CR (\r).
    Cr,
    /// Unknown or mixed.
    Unknown,
}

impl LineEnding {
    /// Get the line ending bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &'static [u8] {
        match self {
            Self::Lf => b"\n",
            Self::CrLf => b"\r\n",
            Self::Cr => b"\r",
            Self::Unknown => b"\n",
        }
    }

    /// Get the line ending string.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
            Self::Cr => "\r",
            Self::Unknown => "\n",
        }
    }

    /// Get name of line ending.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Lf => "LF",
            Self::CrLf => "CRLF",
            Self::Cr => "CR",
            Self::Unknown => "Unknown",
        }
    }
}

impl Default for LineEnding {
    fn default() -> Self {
        #[cfg(windows)]
        {
            Self::CrLf
        }
        #[cfg(not(windows))]
        {
            Self::Lf
        }
    }
}

/// Detect line ending style in data.
#[must_use]
pub fn detect_line_ending(data: &[u8]) -> LineEnding {
    let mut lf_count = 0;
    let mut crlf_count = 0;
    let mut cr_count = 0;

    let mut i = 0;
    while i < data.len() {
        if i + 1 < data.len() && data[i] == b'\r' && data[i + 1] == b'\n' {
            crlf_count += 1;
            i += 2;
        } else if data[i] == b'\n' {
            lf_count += 1;
            i += 1;
        } else if data[i] == b'\r' {
            cr_count += 1;
            i += 1;
        } else {
            i += 1;
        }
    }

    // Determine dominant style
    if crlf_count > lf_count && crlf_count > cr_count {
        LineEnding::CrLf
    } else if lf_count > crlf_count && lf_count > cr_count {
        LineEnding::Lf
    } else if cr_count > 0 && lf_count == 0 && crlf_count == 0 {
        LineEnding::Cr
    } else if lf_count == 0 && crlf_count == 0 && cr_count == 0 {
        LineEnding::Unknown
    } else {
        // Mixed - return platform default
        LineEnding::default()
    }
}

/// Normalize line endings in data.
#[must_use]
pub fn normalize_line_endings(data: &[u8], target: LineEnding) -> Vec<u8> {
    let target_bytes = target.as_bytes();
    let mut result = Vec::with_capacity(data.len());
    let mut i = 0;

    while i < data.len() {
        if i + 1 < data.len() && data[i] == b'\r' && data[i + 1] == b'\n' {
            // CRLF
            result.extend_from_slice(target_bytes);
            i += 2;
        } else if data[i] == b'\n' {
            // LF
            result.extend_from_slice(target_bytes);
            i += 1;
        } else if data[i] == b'\r' {
            // CR (not followed by LF)
            result.extend_from_slice(target_bytes);
            i += 1;
        } else {
            result.push(data[i]);
            i += 1;
        }
    }

    result
}

/// Convert line endings to LF.
#[must_use]
pub fn to_lf(data: &[u8]) -> Vec<u8> {
    normalize_line_endings(data, LineEnding::Lf)
}

/// Convert line endings to CRLF.
#[must_use]
pub fn to_crlf(data: &[u8]) -> Vec<u8> {
    normalize_line_endings(data, LineEnding::CrLf)
}

/// Line ending configuration.
#[derive(Debug, Clone)]
pub struct LineEndingConfig {
    /// Input line ending (what to send).
    pub input: LineEnding,
    /// Output line ending (normalize received output).
    pub output: Option<LineEnding>,
    /// Auto-detect from first output.
    pub auto_detect: bool,
}

impl Default for LineEndingConfig {
    fn default() -> Self {
        Self {
            input: LineEnding::default(),
            output: None,
            auto_detect: true,
        }
    }
}

impl LineEndingConfig {
    /// Create new config.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set input line ending.
    #[must_use]
    pub const fn with_input(mut self, ending: LineEnding) -> Self {
        self.input = ending;
        self
    }

    /// Set output normalization.
    #[must_use]
    pub const fn with_output(mut self, ending: LineEnding) -> Self {
        self.output = Some(ending);
        self
    }

    /// Enable auto-detection.
    #[must_use]
    pub const fn with_auto_detect(mut self, auto: bool) -> Self {
        self.auto_detect = auto;
        self
    }

    /// Process input (add line endings to send).
    #[must_use]
    pub fn process_input(&self, line: &str) -> Vec<u8> {
        let mut result = line.as_bytes().to_vec();
        if !line.ends_with('\n') && !line.ends_with('\r') {
            result.extend_from_slice(self.input.as_bytes());
        }
        result
    }

    /// Process output (normalize received data).
    #[must_use]
    pub fn process_output(&self, data: &[u8]) -> Vec<u8> {
        if let Some(target) = self.output {
            normalize_line_endings(data, target)
        } else {
            data.to_vec()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_lf() {
        let data = b"line1\nline2\nline3\n";
        assert_eq!(detect_line_ending(data), LineEnding::Lf);
    }

    #[test]
    fn detect_crlf() {
        let data = b"line1\r\nline2\r\nline3\r\n";
        assert_eq!(detect_line_ending(data), LineEnding::CrLf);
    }

    #[test]
    fn normalize_to_lf() {
        let data = b"line1\r\nline2\r\n";
        let result = to_lf(data);
        assert_eq!(result, b"line1\nline2\n");
    }

    #[test]
    fn normalize_to_crlf() {
        let data = b"line1\nline2\n";
        let result = to_crlf(data);
        assert_eq!(result, b"line1\r\nline2\r\n");
    }

    #[test]
    fn line_ending_bytes() {
        assert_eq!(LineEnding::Lf.as_bytes(), b"\n");
        assert_eq!(LineEnding::CrLf.as_bytes(), b"\r\n");
    }
}
