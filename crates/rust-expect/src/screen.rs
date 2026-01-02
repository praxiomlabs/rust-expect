//! Virtual terminal screen emulation.
//!
//! This module provides a virtual terminal emulator that can process
//! ANSI escape sequences and maintain a screen buffer. This is useful
//! for screen-based expect operations and testing terminal applications.
//!
//! # Example
//!
//! ```rust
//! use rust_expect::screen::{Screen, ScreenQueryExt};
//!
//! let mut screen = Screen::new(24, 80);
//! screen.process(b"Hello, World!\x1b[2J\x1b[HNew content");
//!
//! // Query the screen content
//! assert!(screen.buffer().query().contains("New content"));
//! ```

pub mod buffer;
pub mod parser;
pub mod query;

pub use buffer::{Attributes, Cell, Color, Cursor, ScreenBuffer};
pub use parser::{AnsiParser, AnsiSequence, EraseMode, ParseResult};
pub use query::{Region, ScreenQuery, ScreenQueryExt};

use parser::apply_sgr;

/// A virtual terminal screen.
#[derive(Clone)]
pub struct Screen {
    /// The screen buffer.
    buffer: ScreenBuffer,
    /// The ANSI parser.
    parser: AnsiParser,
    /// Current foreground color.
    fg: Color,
    /// Current background color.
    bg: Color,
    /// Current text attributes.
    attrs: Attributes,
}

impl Screen {
    /// Create a new screen with the specified dimensions.
    #[must_use]
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            buffer: ScreenBuffer::new(rows, cols),
            parser: AnsiParser::new(),
            fg: Color::Default,
            bg: Color::Default,
            attrs: Attributes::empty(),
        }
    }

    /// Create a new screen with standard VT100 dimensions (24x80).
    #[must_use]
    pub fn vt100() -> Self {
        Self::new(24, 80)
    }

    /// Get the number of rows.
    #[must_use]
    pub const fn rows(&self) -> usize {
        self.buffer.rows()
    }

    /// Get the number of columns.
    #[must_use]
    pub const fn cols(&self) -> usize {
        self.buffer.cols()
    }

    /// Get the screen buffer.
    #[must_use]
    pub const fn buffer(&self) -> &ScreenBuffer {
        &self.buffer
    }

    /// Get mutable access to the buffer.
    pub fn buffer_mut(&mut self) -> &mut ScreenBuffer {
        &mut self.buffer
    }

    /// Get the cursor position.
    #[must_use]
    pub const fn cursor(&self) -> &Cursor {
        self.buffer.cursor()
    }

    /// Process input bytes.
    pub fn process(&mut self, data: &[u8]) {
        for byte in data {
            if let Some(result) = self.parser.parse(*byte) {
                self.apply_result(result);
            }
        }
    }

    /// Process a string.
    pub fn process_str(&mut self, s: &str) {
        self.process(s.as_bytes());
    }

    /// Apply a parse result to the screen.
    fn apply_result(&mut self, result: ParseResult) {
        match result {
            ParseResult::Print(c) => {
                self.buffer.set_style(self.fg, self.bg, self.attrs);
                self.buffer.write_char(c);
            }
            ParseResult::Control(c) => self.apply_control(c),
            ParseResult::Sequence(seq) => self.apply_sequence(seq),
        }
    }

    /// Apply a control character.
    fn apply_control(&mut self, c: u8) {
        match c {
            0x08 => {
                // Backspace
                let cursor = self.buffer.cursor_mut();
                if cursor.col > 0 {
                    cursor.col -= 1;
                }
            }
            0x09 => {
                // Tab - move to next tab stop (every 8 columns)
                let cols = self.buffer.cols();
                let cursor = self.buffer.cursor_mut();
                cursor.col = ((cursor.col / 8) + 1) * 8;
                if cursor.col >= cols {
                    cursor.col = cols - 1;
                }
            }
            0x0a => {
                // Line feed - also reset column (newline behavior)
                let rows = self.buffer.rows();
                let cursor_row = self.buffer.cursor().row + 1;
                if cursor_row >= rows {
                    self.buffer.scroll_up(1);
                    self.buffer.cursor_mut().row = rows - 1;
                } else {
                    self.buffer.cursor_mut().row = cursor_row;
                }
                self.buffer.cursor_mut().col = 0;
            }
            0x0d => {
                // Carriage return
                self.buffer.cursor_mut().col = 0;
            }
            0x07 => {
                // Bell - ignored
            }
            _ => {}
        }
    }

    /// Apply an ANSI sequence.
    fn apply_sequence(&mut self, seq: AnsiSequence) {
        match seq {
            AnsiSequence::CursorUp(n) => {
                let cursor = self.buffer.cursor_mut();
                cursor.row = cursor.row.saturating_sub(n as usize);
            }
            AnsiSequence::CursorDown(n) => {
                let rows = self.buffer.rows();
                let cursor = self.buffer.cursor_mut();
                cursor.row = (cursor.row + n as usize).min(rows.saturating_sub(1));
            }
            AnsiSequence::CursorForward(n) => {
                let cols = self.buffer.cols();
                let cursor = self.buffer.cursor_mut();
                cursor.col = (cursor.col + n as usize).min(cols.saturating_sub(1));
            }
            AnsiSequence::CursorBackward(n) => {
                let cursor = self.buffer.cursor_mut();
                cursor.col = cursor.col.saturating_sub(n as usize);
            }
            AnsiSequence::CursorPosition { row, col } => {
                self.buffer.goto(
                    (row.saturating_sub(1)) as usize,
                    (col.saturating_sub(1)) as usize,
                );
            }
            AnsiSequence::EraseDisplay(mode) => match mode {
                EraseMode::ToEnd => self.buffer.clear_to_end(),
                EraseMode::ToStart => self.buffer.clear_to_start(),
                EraseMode::All => self.buffer.clear(),
            },
            AnsiSequence::EraseLine(mode) => match mode {
                EraseMode::ToEnd => self.buffer.clear_line_to_end(),
                EraseMode::ToStart => {
                    // Clear from start of line to cursor
                    let row = self.buffer.cursor().row;
                    let col = self.buffer.cursor().col;
                    for c in 0..=col {
                        self.buffer.set(row, c, Cell::default());
                    }
                }
                EraseMode::All => self.buffer.clear_line(),
            },
            AnsiSequence::SetGraphics(params) => {
                apply_sgr(&params, &mut self.fg, &mut self.bg, &mut self.attrs);
            }
            AnsiSequence::ScrollUp(n) => {
                self.buffer.scroll_up(n as usize);
            }
            AnsiSequence::ScrollDown(n) => {
                self.buffer.scroll_down(n as usize);
            }
            AnsiSequence::SaveCursor => {
                self.buffer.save_cursor();
            }
            AnsiSequence::RestoreCursor => {
                self.buffer.restore_cursor();
            }
            AnsiSequence::SetScrollRegion { top, bottom } => {
                let top = (top.saturating_sub(1)) as usize;
                let bottom = if bottom == 0 {
                    self.buffer.rows() - 1
                } else {
                    (bottom.saturating_sub(1)) as usize
                };
                self.buffer.set_scroll_region(top, bottom);
            }
            AnsiSequence::ShowCursor => {
                self.buffer.cursor_mut().visible = true;
            }
            AnsiSequence::HideCursor => {
                self.buffer.cursor_mut().visible = false;
            }
            AnsiSequence::InsertLines(n) => {
                // Insert blank lines at cursor
                for _ in 0..n {
                    self.buffer.scroll_down(1);
                }
            }
            AnsiSequence::DeleteLines(n) => {
                // Delete lines at cursor
                for _ in 0..n {
                    self.buffer.scroll_up(1);
                }
            }
            AnsiSequence::InsertChars(n) => {
                // Insert blank chars at cursor (not fully implemented)
                let row = self.buffer.cursor().row;
                let col = self.buffer.cursor().col;
                for _ in 0..n {
                    self.buffer.set(row, col, Cell::default());
                }
            }
            AnsiSequence::DeleteChars(n) => {
                // Delete chars at cursor (not fully implemented)
                let row = self.buffer.cursor().row;
                let col = self.buffer.cursor().col;
                for c in col..self.buffer.cols().saturating_sub(n as usize) {
                    if let Some(cell) = self.buffer.get(row, c + n as usize).copied() {
                        self.buffer.set(row, c, cell);
                    }
                }
            }
            AnsiSequence::Reset => {
                self.buffer.clear();
                self.buffer.goto(0, 0);
                self.fg = Color::Default;
                self.bg = Color::Default;
                self.attrs = Attributes::empty();
            }
            AnsiSequence::Unknown(_) => {
                // Ignore unknown sequences
            }
        }
    }

    /// Get the text content of the screen.
    #[must_use]
    pub fn text(&self) -> String {
        self.buffer.text()
    }

    /// Clear the screen.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.buffer.goto(0, 0);
    }

    /// Resize the screen.
    pub fn resize(&mut self, rows: usize, cols: usize) {
        self.buffer.resize(rows, cols);
    }

    /// Query the screen content.
    #[must_use]
    pub const fn query(&self) -> ScreenQuery<'_> {
        ScreenQuery::new(&self.buffer)
    }
}

impl std::fmt::Debug for Screen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Screen")
            .field("rows", &self.rows())
            .field("cols", &self.cols())
            .field("cursor", self.cursor())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_basic() {
        let mut screen = Screen::new(24, 80);
        screen.process_str("Hello, World!");
        assert!(screen.query().contains("Hello, World!"));
    }

    #[test]
    fn screen_cursor_movement() {
        let mut screen = Screen::new(24, 80);
        screen.process_str("Hello\x1b[1;1HWorld");
        assert!(screen.query().contains("World"));
    }

    #[test]
    fn screen_clear() {
        let mut screen = Screen::new(24, 80);
        screen.process_str("Hello\x1b[2J\x1b[HWorld");
        assert!(!screen.query().contains("Hello"));
        assert!(screen.query().contains("World"));
    }

    #[test]
    fn screen_colors() {
        let mut screen = Screen::new(24, 80);
        screen.process_str("\x1b[31mRed\x1b[0m Normal");

        // Check that cells have the right colors
        let cell = screen.buffer().get(0, 0).unwrap();
        assert_eq!(cell.char, 'R');
        assert_eq!(cell.fg, Color::Red);
    }

    #[test]
    fn screen_scroll() {
        let mut screen = Screen::new(3, 10);
        screen.process_str("Line 1\n");
        screen.process_str("Line 2\n");
        screen.process_str("Line 3\n");
        screen.process_str("Line 4");

        // Line 1 should have scrolled off
        assert!(!screen.query().contains("Line 1"));
        assert!(screen.query().contains("Line 4"));
    }
}
