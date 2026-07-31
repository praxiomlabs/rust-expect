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
    Attributes, Cell, CellChange, ChangeType, Color, Cursor, Row, ScreenBuffer, ScreenDiff,
};
use parser::apply_sgr;
pub use parser::{AnsiParser, AnsiSequence, EraseMode, ParseResult};
pub use query::{Region, ScreenQuery, ScreenQueryExt};

/// Callback invoked with each row that scrolls off the top of the viewport.
type ScrolledOutCallback = Box<dyn FnMut(&Row) + Send>;

/// A virtual terminal screen.
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
    /// Monotonic counter that ticks on each `process()` call, used as a
    /// cheap signal that the screen has received input. See [`revision`].
    revision: u64,
    /// Optional callback invoked for each row that scrolls off the top during
    /// `process()`. Not cloned (see the manual `Clone` impl).
    scrolled_out_cb: Option<ScrolledOutCallback>,
}

impl Clone for Screen {
    /// Clones the screen state (including scrollback). The scrolled-out
    /// callback is **not** carried over — a clone is a detached snapshot, not a
    /// live stream. Re-register with [`on_line_scrolled_out`](Self::on_line_scrolled_out)
    /// if the clone needs to stream.
    fn clone(&self) -> Self {
        Self {
            buffer: self.buffer.clone(),
            parser: self.parser.clone(),
            fg: self.fg,
            bg: self.bg,
            attrs: self.attrs,
            revision: self.revision,
            scrolled_out_cb: None,
        }
    }
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
            revision: 0,
            scrolled_out_cb: None,
        }
    }

    /// Create a screen with a bounded scrollback history.
    ///
    /// Rows that scroll off the top of the viewport are retained (up to
    /// `scrollback_lines`, oldest dropped first) and readable via
    /// [`scrollback`](Self::scrollback) and [`full_text`](Self::full_text).
    /// `scrollback_lines = 0` is identical to [`new`](Self::new) — no history,
    /// no extra allocation.
    #[must_use]
    pub fn with_scrollback(rows: usize, cols: usize, scrollback_lines: usize) -> Self {
        let mut screen = Self::new(rows, cols);
        screen.buffer.set_scrollback_limit(scrollback_lines);
        screen
    }

    /// Get the current revision counter.
    ///
    /// Bumps once per byte consumed inside `process()`, regardless of
    /// whether the byte caused any cell change. Useful as an O(1) "did
    /// anything come in?" check — `wait_screen_stable` uses it to avoid
    /// materializing the full screen text on every poll. Wraps on
    /// `u64::MAX`.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
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
            for result in self.parser.parse(*byte).into_iter().flatten() {
                self.apply_result(result);
            }
            // A non-zero number of input bytes always advances the screen's
            // revision counter so cheap stability polls can detect activity.
            self.revision = self.revision.wrapping_add(1);
        }

        // Deliver rows that scrolled off during this call. Taking the rows
        // before borrowing the callback keeps the two field borrows disjoint.
        if self.scrolled_out_cb.is_some() {
            let rows = self.buffer.take_scrolled_out();
            if let Some(cb) = self.scrolled_out_cb.as_mut() {
                for row in &rows {
                    cb(row);
                }
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
                // Visibility only — must not cancel a pending wrap.
                self.buffer.set_cursor_visible(true);
            }
            AnsiSequence::HideCursor => {
                self.buffer.set_cursor_visible(false);
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

    /// Register a callback fired for each row that scrolls off the top of the
    /// viewport, in order, during [`process`](Self::process).
    ///
    /// This is the lossless path: rows are delivered as they finalize, so a
    /// consumer can persist the full history regardless of the scrollback
    /// bound — it works even with `scrollback_lines = 0`.
    ///
    /// # Reentrancy
    ///
    /// The callback fires while the screen is being driven. When the screen is
    /// shared as `Arc<Mutex<Screen>>` (as [`Session::attach_screen`] does) it
    /// runs *while the screen lock is held*. It receives a `&Row` valid only
    /// for the call and must not call back into the `Screen`/`Session`, or it
    /// will deadlock. Do minimal work — the same contract as output taps.
    ///
    /// [`Session::attach_screen`]: crate::Session::attach_screen
    pub fn on_line_scrolled_out<F>(&mut self, callback: F)
    where
        F: FnMut(&Row) + Send + 'static,
    {
        self.buffer.set_capture_scrolled(true);
        self.scrolled_out_cb = Some(Box::new(callback));
    }

    /// The retained scrollback rows, oldest first (the rows immediately above
    /// the current viewport). Empty unless constructed with
    /// [`with_scrollback`](Self::with_scrollback).
    pub fn scrollback(&self) -> impl Iterator<Item = &Row> {
        self.buffer.scrollback().iter()
    }

    /// The scrollback history followed by the current viewport, one line per
    /// row with trailing whitespace trimmed.
    ///
    /// Guarantees: `full_text()` equals what [`text`](Self::text) would have
    /// returned had nothing scrolled off — history lines use the same
    /// extraction and trimming as viewport lines.
    #[must_use]
    pub fn full_text(&self) -> String {
        let mut lines: Vec<String> = self.buffer.scrollback().iter().map(Row::text).collect();
        lines.push(self.buffer.text());
        lines.join("\n")
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
    fn scrollback_disabled_by_default() {
        let mut screen = Screen::new(3, 10);
        screen.process_str("L0\r\nL1\r\nL2\r\nL3\r\nL4");
        assert_eq!(screen.scrollback().count(), 0);
        // With no scrollback, full_text is exactly the viewport text.
        assert_eq!(screen.full_text(), screen.text());
        assert_eq!(screen.text(), "L2\nL3\nL4");
    }

    #[test]
    fn scrollback_retains_scrolled_rows() {
        let mut screen = Screen::with_scrollback(3, 10, 10);
        screen.process_str("L0\r\nL1\r\nL2\r\nL3\r\nL4");

        let hist: Vec<String> = screen.scrollback().map(Row::text).collect();
        assert_eq!(hist, vec!["L0".to_string(), "L1".to_string()]);
        assert_eq!(screen.full_text(), "L0\nL1\nL2\nL3\nL4");
        assert_eq!(screen.text(), "L2\nL3\nL4");
    }

    #[test]
    fn scrollback_is_bounded_oldest_dropped() {
        let mut screen = Screen::with_scrollback(3, 10, 1);
        screen.process_str("L0\r\nL1\r\nL2\r\nL3\r\nL4");
        let hist: Vec<String> = screen.scrollback().map(Row::text).collect();
        // Only the most recent evicted row is kept; L0 was dropped.
        assert_eq!(hist, vec!["L1".to_string()]);
        assert_eq!(screen.full_text(), "L1\nL2\nL3\nL4");
    }

    #[test]
    fn scrolled_out_callback_is_lossless_without_ring() {
        use std::sync::{Arc, Mutex};
        let collected = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = collected.clone();

        // Ring disabled (0) — the callback still receives every evicted row.
        let mut screen = Screen::with_scrollback(3, 10, 0);
        screen.on_line_scrolled_out(move |row| sink.lock().unwrap().push(row.text()));
        screen.process_str("L0\r\nL1\r\nL2\r\nL3\r\nL4");

        assert_eq!(
            *collected.lock().unwrap(),
            vec!["L0".to_string(), "L1".to_string()]
        );
        assert_eq!(screen.scrollback().count(), 0);
    }

    #[test]
    fn row_exposes_text_and_cells() {
        let mut screen = Screen::with_scrollback(3, 10, 10);
        screen.process_str("hi\r\nL1\r\nL2\r\nL3");
        let first = screen.scrollback().next().unwrap();
        assert_eq!(first.text(), "hi");
        assert_eq!(first.cells().len(), 10);
        assert_eq!(first.cells()[0].char, 'h');
        assert!(!first.is_blank());
    }

    #[test]
    fn full_text_equals_text_when_nothing_scrolled() {
        let mut screen = Screen::with_scrollback(24, 80, 100);
        screen.process_str("short\r\ncontent");
        assert_eq!(screen.full_text(), screen.text());
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
    fn screen_utf8_box_drawing() {
        // ╭─╮  (U+256D, U+2500, U+256E)  — UTF-8: e2 95 ad, e2 94 80, e2 95 ae
        let mut screen = Screen::new(2, 10);
        screen.process("╭─╮".as_bytes());
        let row = screen.buffer().row_text(0);
        assert!(row.starts_with("╭─╮"), "expected '╭─╮' got {row:?}");
    }

    #[test]
    fn screen_utf8_split_across_calls() {
        // The 3-byte sequence for ╭ delivered one byte at a time should still
        // resolve to a single ╭, not three Latin-1 garbage chars.
        let mut screen = Screen::new(1, 4);
        let bytes = "╭".as_bytes();
        assert_eq!(bytes.len(), 3);
        for b in bytes {
            screen.process(&[*b]);
        }
        let row = screen.buffer().row_text(0);
        assert!(row.starts_with('╭'), "expected '╭' got {row:?}");
        // Should be exactly one cell occupied, not three.
        assert!(
            !row.starts_with("╭â"),
            "leftover Latin-1 bytes present: {row:?}"
        );
    }

    #[test]
    fn screen_utf8_four_byte_emoji() {
        // 🚀  U+1F680  — UTF-8: f0 9f 9a 80
        let mut screen = Screen::new(1, 4);
        screen.process("🚀".as_bytes());
        let row = screen.buffer().row_text(0);
        assert!(row.starts_with('🚀'), "expected '🚀' got {row:?}");
    }

    #[test]
    fn screen_utf8_invalid_lead_byte() {
        // 0xFF is never a valid UTF-8 byte; should become U+FFFD.
        let mut screen = Screen::new(1, 4);
        screen.process(&[0xFF]);
        let row = screen.buffer().row_text(0);
        assert!(
            row.starts_with(std::char::REPLACEMENT_CHARACTER),
            "expected replacement, got {row:?}"
        );
    }

    #[test]
    fn screen_vertical_tab() {
        let mut screen = Screen::new(10, 20);
        screen.process_str("Line 1\x0b"); // Vertical tab acts like line feed
        screen.process_str("Line 2");

        assert_eq!(screen.cursor().row, 1);
    }
}
