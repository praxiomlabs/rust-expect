//! Terminal interaction modes.

use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Terminal mode for interactive sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMode {
    /// Raw mode - no processing.
    Raw,
    /// Cooked mode - line buffering.
    Cooked,
    /// Cbreak mode - character at a time, no echo.
    Cbreak,
}

/// Terminal size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    /// Number of columns.
    pub cols: u16,
    /// Number of rows.
    pub rows: u16,
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self { cols: 80, rows: 24 }
    }
}

impl TerminalSize {
    /// Create a new terminal size.
    #[must_use]
    pub const fn new(cols: u16, rows: u16) -> Self {
        Self { cols, rows }
    }
}

/// Terminal state for saving/restoring.
#[derive(Debug, Clone)]
pub struct TerminalState {
    /// Current mode.
    pub mode: TerminalMode,
    /// Echo enabled.
    pub echo: bool,
    /// Canonical mode.
    pub canonical: bool,
}

impl Default for TerminalState {
    fn default() -> Self {
        Self {
            mode: TerminalMode::Cooked,
            echo: true,
            canonical: true,
        }
    }
}

/// A terminal handle for interactive sessions.
pub struct Terminal {
    /// Running flag.
    running: Arc<AtomicBool>,
    /// Current mode.
    mode: TerminalMode,
    /// Saved state.
    saved_state: Option<TerminalState>,
}

impl Terminal {
    /// Create a new terminal.
    #[must_use]
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            mode: TerminalMode::Cooked,
            saved_state: None,
        }
    }

    /// Check if the terminal is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Set the running state.
    pub fn set_running(&self, running: bool) {
        self.running.store(running, Ordering::SeqCst);
    }

    /// Get the running flag for sharing.
    #[must_use]
    pub fn running_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.running)
    }

    /// Get the current mode.
    #[must_use]
    pub const fn mode(&self) -> TerminalMode {
        self.mode
    }

    /// Set terminal mode.
    pub fn set_mode(&mut self, mode: TerminalMode) {
        self.mode = mode;
    }

    /// Save current state.
    pub fn save_state(&mut self) {
        self.saved_state = Some(TerminalState {
            mode: self.mode,
            echo: true,
            canonical: matches!(self.mode, TerminalMode::Cooked),
        });
    }

    /// Restore saved state.
    pub fn restore_state(&mut self) {
        if let Some(state) = self.saved_state.take() {
            self.mode = state.mode;
        }
    }

    /// Get terminal size.
    #[must_use]
    pub fn size() -> io::Result<TerminalSize> {
        // Use environment variables or defaults
        let cols = std::env::var("COLUMNS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(80);
        let rows = std::env::var("LINES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(24);
        Ok(TerminalSize::new(cols, rows))
    }

    /// Check if stdin is a TTY.
    #[must_use]
    pub fn is_tty() -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            unsafe { libc::isatty(std::io::stdin().as_raw_fd()) != 0 }
        }
        #[cfg(not(unix))]
        {
            false
        }
    }
}

impl Default for Terminal {
    fn default() -> Self {
        Self::new()
    }
}

/// Read input with timeout.
pub fn read_with_timeout(timeout_ms: u64) -> io::Result<Option<u8>> {
    use std::time::{Duration, Instant};

    let deadline = Instant::now() + Duration::from_millis(timeout_ms);

    loop {
        // Non-blocking read attempt
        let mut buf = [0u8; 1];
        match io::stdin().read(&mut buf) {
            Ok(0) => return Ok(None),
            Ok(_) => return Ok(Some(buf[0])),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Ok(None);
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => return Err(e),
        }
    }
}

/// Write output immediately.
pub fn write_immediate(data: &[u8]) -> io::Result<()> {
    let mut stdout = io::stdout();
    stdout.write_all(data)?;
    stdout.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_default() {
        let term = Terminal::new();
        assert!(!term.is_running());
        assert_eq!(term.mode(), TerminalMode::Cooked);
    }

    #[test]
    fn terminal_size_default() {
        let size = TerminalSize::default();
        assert_eq!(size.cols, 80);
        assert_eq!(size.rows, 24);
    }

    #[test]
    fn terminal_running_flag() {
        let term = Terminal::new();
        let flag = term.running_flag();

        assert!(!term.is_running());
        flag.store(true, Ordering::SeqCst);
        assert!(term.is_running());
    }
}
