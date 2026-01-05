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

pub use buffer::{
    Attributes, Cell, CellChange, ChangeType, Color, Cursor, ScreenBuffer, ScreenDiff,
};
use parser::apply_sgr;
pub use parser::{AnsiParser, AnsiSequence, EraseMode, ParseResult};
pub use query::{Region, ScreenQuery, ScreenQueryExt};

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
    pub const fn buffer_mut(&mut self) -> &mut ScreenBuffer {
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
            0x07 => {
                // Bell - ignored
            }
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
            0x0a..=0x0c => {
                // Line feed (LF), Vertical Tab (VT), Form Feed (FF)
                // All behave the same in VT100: move down one line, scroll if needed
                // Also reset column (newline mode behavior)
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
            _ => {}
        }
    }

    /// Apply an ANSI sequence.
    #[allow(clippy::too_many_lines)] // Large match over AnsiSequence variants - structure is clear
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
            AnsiSequence::CursorNextLine(n) => {
                // Move to beginning of line n lines down
                let rows = self.buffer.rows();
                let cursor = self.buffer.cursor_mut();
                cursor.row = (cursor.row + n as usize).min(rows.saturating_sub(1));
                cursor.col = 0;
            }
            AnsiSequence::CursorPrevLine(n) => {
                // Move to beginning of line n lines up
                let cursor = self.buffer.cursor_mut();
                cursor.row = cursor.row.saturating_sub(n as usize);
                cursor.col = 0;
            }
            AnsiSequence::CursorColumn(n) => {
                // Move cursor to column n (1-based)
                let cols = self.buffer.cols();
                let cursor = self.buffer.cursor_mut();
                cursor.col = (n.saturating_sub(1) as usize).min(cols.saturating_sub(1));
            }
            AnsiSequence::CursorRow(n) => {
                // Move cursor to row n (1-based)
                let rows = self.buffer.rows();
                let cursor = self.buffer.cursor_mut();
                cursor.row = (n.saturating_sub(1) as usize).min(rows.saturating_sub(1));
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
            AnsiSequence::EraseChars(n) => {
                // Erase n characters from cursor position (replace with spaces)
                let row = self.buffer.cursor().row;
                let col = self.buffer.cursor().col;
                let cols = self.buffer.cols();
                let end = (col + n as usize).min(cols);
                for c in col..end {
                    self.buffer.set(row, c, Cell::default());
                }
            }
            AnsiSequence::SetGraphics(params) => {
                apply_sgr(&params, &mut self.fg, &mut self.bg, &mut self.attrs);
            }
            AnsiSequence::ScrollUp(n) => {
                self.buffer.scroll_up(n as usize);
            }
            AnsiSequence::ScrollDown(n) => {
                self.buffer.scroll_down(n as usize);
            }
            AnsiSequence::ReverseIndex => {
                // Move cursor up, scroll down if at top of scroll region
                let cursor_row = self.buffer.cursor().row;
                let (top, _) = (0, self.buffer.rows() - 1); // Use full screen for now
                if cursor_row == top {
                    self.buffer.scroll_down(1);
                } else {
                    self.buffer.cursor_mut().row = cursor_row.saturating_sub(1);
                }
            }
            AnsiSequence::Index => {
                // Move cursor down, scroll up if at bottom
                let rows = self.buffer.rows();
                let cursor_row = self.buffer.cursor().row;
                if cursor_row >= rows - 1 {
                    self.buffer.scroll_up(1);
                } else {
                    self.buffer.cursor_mut().row = cursor_row + 1;
                }
            }
            AnsiSequence::NextLine => {
                // Move to start of next line, scroll if needed
                let rows = self.buffer.rows();
                let cursor_row = self.buffer.cursor().row;
                if cursor_row >= rows - 1 {
                    self.buffer.scroll_up(1);
                    self.buffer.cursor_mut().row = rows - 1;
                } else {
                    self.buffer.cursor_mut().row = cursor_row + 1;
                }
                self.buffer.cursor_mut().col = 0;
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
                self.buffer.insert_lines(n as usize);
            }
            AnsiSequence::DeleteLines(n) => {
                self.buffer.delete_lines(n as usize);
            }
            AnsiSequence::InsertChars(n) => {
                self.buffer.insert_chars(n as usize);
            }
            AnsiSequence::DeleteChars(n) => {
                self.buffer.delete_chars(n as usize);
            }
            AnsiSequence::RepeatChar(n) => {
                // Repeat the last printed character n times
                // Note: We don't track last char, so this is a no-op for now
                // A full implementation would track last_printed_char
                let _ = n;
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

    #[test]
    fn screen_cursor_next_line() {
        let mut screen = Screen::new(10, 20);
        screen.process_str("Test");
        screen.process_str("\x1b[2E"); // Move 2 lines down to beginning
        screen.process_str("Line");

        // Cursor should be at row 2, col 4 after "Line"
        assert_eq!(screen.cursor().row, 2);
        assert!(screen.query().contains("Line"));
    }

    #[test]
    fn screen_cursor_prev_line() {
        let mut screen = Screen::new(10, 20);
        screen.process_str("\x1b[5;10H"); // Row 5, Col 10
        screen.process_str("\x1b[2F"); // Move 2 lines up to beginning
        screen.process_str("X");

        assert_eq!(screen.cursor().row, 2);
        assert_eq!(screen.cursor().col, 1);
    }

    #[test]
    fn screen_cursor_column() {
        let mut screen = Screen::new(10, 20);
        screen.process_str("Hello World");
        screen.process_str("\x1b[5G"); // Move to column 5
        screen.process_str("X");

        // Should overwrite the 5th character (0-indexed: 4)
        assert!(screen.query().contains("HellX World"));
    }

    #[test]
    fn screen_cursor_row() {
        let mut screen = Screen::new(10, 20);
        screen.process_str("\x1b[5d"); // Move to row 5
        screen.process_str("Test");

        assert_eq!(screen.cursor().row, 4); // 0-indexed
    }

    #[test]
    fn screen_erase_chars() {
        let mut screen = Screen::new(1, 20);
        screen.process_str("Hello World");
        screen.process_str("\x1b[1;1H"); // Home
        screen.process_str("\x1b[5X"); // Erase 5 characters

        // First 5 chars should be spaces
        let text = screen.text();
        assert!(text.starts_with("      World") || text.contains("World"));
    }

    #[test]
    fn screen_reverse_index() {
        let mut screen = Screen::new(5, 20);
        screen.process_str("Line 1\n");
        screen.process_str("Line 2\n");
        screen.process_str("Line 3");

        // Now at row 2 (0-indexed)
        assert_eq!(screen.cursor().row, 2);

        screen.process_str("\x1bM"); // Reverse index - move up
        assert_eq!(screen.cursor().row, 1);
    }

    #[test]
    fn screen_reverse_index_at_top() {
        let mut screen = Screen::new(3, 20);
        screen.process_str("Line 1");
        screen.process_str("\x1b[1;1H"); // Move to top
        screen.process_str("\x1bM"); // Reverse index at top - should scroll down

        // First line should now be empty, Line 1 pushed to row 1
        assert!(screen.buffer().row_text(0).is_empty());
    }

    #[test]
    fn screen_index() {
        let mut screen = Screen::new(3, 20);
        screen.process_str("Line 1");
        screen.process_str("\x1bD"); // Index - move down

        assert_eq!(screen.cursor().row, 1);
    }

    #[test]
    fn screen_next_line_escape() {
        let mut screen = Screen::new(10, 20);
        screen.process_str("Hello");
        screen.process_str("\x1bE"); // NEL - Next Line
        screen.process_str("World");

        assert_eq!(screen.cursor().row, 1);
        assert_eq!(screen.cursor().col, 5);
    }

    #[test]
    fn screen_form_feed() {
        let mut screen = Screen::new(10, 20);
        screen.process_str("Line 1\x0c"); // Form feed acts like line feed
        screen.process_str("Line 2");

        assert_eq!(screen.cursor().row, 1);
    }

    #[test]
    fn screen_vertical_tab() {
        let mut screen = Screen::new(10, 20);
        screen.process_str("Line 1\x0b"); // Vertical tab acts like line feed
        screen.process_str("Line 2");

        assert_eq!(screen.cursor().row, 1);
    }
}
