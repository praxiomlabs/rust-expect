//! Screen buffer integration for sessions.
//!
//! This module provides integration between sessions and the screen buffer,
//! allowing for terminal emulation and screen-based operations.

use crate::types::Dimensions;

/// Screen position (row, column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Position {
    /// Row (0-indexed).
    pub row: usize,
    /// Column (0-indexed).
    pub col: usize,
}

impl Position {
    /// Create a new position.
    #[must_use]
    pub const fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }
}

/// A rectangular region of the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    /// Top-left corner.
    pub start: Position,
    /// Bottom-right corner (exclusive).
    pub end: Position,
}

impl Region {
    /// Create a new region.
    #[must_use]
    pub const fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    /// Create a region from coordinates.
    #[must_use]
    pub const fn from_coords(
        start_row: usize,
        start_col: usize,
        end_row: usize,
        end_col: usize,
    ) -> Self {
        Self {
            start: Position::new(start_row, start_col),
            end: Position::new(end_row, end_col),
        }
    }

    /// Get the width of the region.
    #[must_use]
    pub const fn width(&self) -> usize {
        self.end.col.saturating_sub(self.start.col)
    }

    /// Get the height of the region.
    #[must_use]
    pub const fn height(&self) -> usize {
        self.end.row.saturating_sub(self.start.row)
    }

    /// Check if a position is within this region.
    #[must_use]
    pub const fn contains(&self, pos: Position) -> bool {
        pos.row >= self.start.row
            && pos.row < self.end.row
            && pos.col >= self.start.col
            && pos.col < self.end.col
    }
}

/// Text attributes for a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct CellAttributes {
    /// Bold text.
    pub bold: bool,
    /// Italic text.
    pub italic: bool,
    /// Underlined text.
    pub underline: bool,
    /// Blinking text.
    pub blink: bool,
    /// Inverse video.
    pub inverse: bool,
    /// Hidden text.
    pub hidden: bool,
    /// Strikethrough text.
    pub strikethrough: bool,
    /// Foreground color (ANSI color code or RGB).
    pub foreground: Option<Color>,
    /// Background color (ANSI color code or RGB).
    pub background: Option<Color>,
}

/// Color representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    /// ANSI color index (0-255).
    Indexed(u8),
    /// RGB color.
    Rgb(u8, u8, u8),
}

/// A single cell in the screen buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// The character in this cell.
    pub char: char,
    /// Text attributes.
    pub attrs: CellAttributes,
    /// Width of the character (1 for normal, 2 for wide chars).
    pub width: u8,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            char: ' ',
            attrs: CellAttributes::default(),
            width: 1,
        }
    }
}

/// A simple screen buffer for terminal content.
///
/// This provides basic screen buffer functionality. For full terminal
/// emulation, use the `screen` feature which provides a more complete
/// implementation.
pub struct ScreenBuffer {
    /// Screen cells.
    cells: Vec<Vec<Cell>>,
    /// Screen dimensions.
    dimensions: Dimensions,
    /// Cursor position.
    cursor: Position,
    /// Saved cursor position.
    saved_cursor: Option<Position>,
    /// Scroll region.
    scroll_region: Option<(usize, usize)>,
}

impl ScreenBuffer {
    /// Create a new screen buffer.
    #[must_use]
    pub fn new(dimensions: Dimensions) -> Self {
        let rows = dimensions.rows as usize;
        let cols = dimensions.cols as usize;

        let cells = (0..rows).map(|_| vec![Cell::default(); cols]).collect();

        Self {
            cells,
            dimensions,
            cursor: Position::default(),
            saved_cursor: None,
            scroll_region: None,
        }
    }

    /// Get the screen dimensions.
    #[must_use]
    pub const fn dimensions(&self) -> Dimensions {
        self.dimensions
    }

    /// Get the cursor position.
    #[must_use]
    pub const fn cursor(&self) -> Position {
        self.cursor
    }

    /// Set the cursor position.
    pub fn set_cursor(&mut self, pos: Position) {
        self.cursor = Position {
            row: pos.row.min(self.dimensions.rows as usize - 1),
            col: pos.col.min(self.dimensions.cols as usize - 1),
        };
    }

    /// Move the cursor.
    pub fn move_cursor(&mut self, rows: isize, cols: isize) {
        let new_row = (self.cursor.row as isize + rows)
            .max(0)
            .min(self.dimensions.rows as isize - 1) as usize;
        let new_col = (self.cursor.col as isize + cols)
            .max(0)
            .min(self.dimensions.cols as isize - 1) as usize;
        self.cursor = Position::new(new_row, new_col);
    }

    /// Save the cursor position.
    pub const fn save_cursor(&mut self) {
        self.saved_cursor = Some(self.cursor);
    }

    /// Restore the cursor position.
    pub const fn restore_cursor(&mut self) {
        if let Some(pos) = self.saved_cursor {
            self.cursor = pos;
        }
    }

    /// Get a cell at a position.
    #[must_use]
    pub fn get(&self, row: usize, col: usize) -> Option<&Cell> {
        self.cells.get(row).and_then(|r| r.get(col))
    }

    /// Get a mutable cell at a position.
    pub fn get_mut(&mut self, row: usize, col: usize) -> Option<&mut Cell> {
        self.cells.get_mut(row).and_then(|r| r.get_mut(col))
    }

    /// Put a character at the cursor position.
    pub fn put_char(&mut self, c: char, attrs: CellAttributes) {
        if self.cursor.row < self.cells.len() && self.cursor.col < self.cells[0].len() {
            self.cells[self.cursor.row][self.cursor.col] = Cell {
                char: c,
                attrs,
                width: if c.is_ascii() { 1 } else { 2 },
            };
            self.cursor.col += 1;
            if self.cursor.col >= self.dimensions.cols as usize {
                self.cursor.col = 0;
                self.cursor.row += 1;
            }
        }
    }

    /// Get a line as a string.
    #[must_use]
    pub fn line(&self, row: usize) -> Option<String> {
        self.cells.get(row).map(|cells| {
            cells
                .iter()
                .map(|c| c.char)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
    }

    /// Get all lines as strings.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        (0..self.dimensions.rows as usize)
            .filter_map(|row| self.line(row))
            .collect()
    }

    /// Get the screen content as a single string.
    #[must_use]
    pub fn content(&self) -> String {
        self.lines().join("\n")
    }

    /// Get text in a region.
    #[must_use]
    pub fn region_text(&self, region: Region) -> String {
        let mut result = String::new();
        for row in region.start.row..region.end.row.min(self.cells.len()) {
            if row < self.cells.len() {
                let start = region.start.col;
                let end = region.end.col.min(self.cells[row].len());
                for col in start..end {
                    result.push(self.cells[row][col].char);
                }
                if row < region.end.row - 1 {
                    result.push('\n');
                }
            }
        }
        result.trim_end().to_string()
    }

    /// Clear the screen.
    pub fn clear(&mut self) {
        for row in &mut self.cells {
            for cell in row {
                *cell = Cell::default();
            }
        }
        self.cursor = Position::default();
    }

    /// Clear a region.
    pub fn clear_region(&mut self, region: Region) {
        for row in region.start.row..region.end.row.min(self.cells.len()) {
            let start = region.start.col;
            let end = region.end.col.min(self.cells[row].len());
            for col in start..end {
                self.cells[row][col] = Cell::default();
            }
        }
    }

    /// Scroll the screen up by n lines.
    pub fn scroll_up(&mut self, n: usize) {
        let (start, end) = self
            .scroll_region
            .unwrap_or((0, self.dimensions.rows as usize));

        for _ in 0..n {
            if start < end && end <= self.cells.len() {
                self.cells.remove(start);
                self.cells.insert(
                    end - 1,
                    vec![Cell::default(); self.dimensions.cols as usize],
                );
            }
        }
    }

    /// Scroll the screen down by n lines.
    pub fn scroll_down(&mut self, n: usize) {
        let (start, end) = self
            .scroll_region
            .unwrap_or((0, self.dimensions.rows as usize));

        for _ in 0..n {
            if start < end && end <= self.cells.len() {
                self.cells.remove(end - 1);
                self.cells
                    .insert(start, vec![Cell::default(); self.dimensions.cols as usize]);
            }
        }
    }

    /// Set the scroll region.
    pub const fn set_scroll_region(&mut self, top: usize, bottom: usize) {
        if top < bottom && bottom <= self.dimensions.rows as usize {
            self.scroll_region = Some((top, bottom));
        } else {
            self.scroll_region = None;
        }
    }

    /// Resize the screen.
    pub fn resize(&mut self, dimensions: Dimensions) {
        let new_rows = dimensions.rows as usize;
        let new_cols = dimensions.cols as usize;

        // Resize rows
        self.cells
            .resize_with(new_rows, || vec![Cell::default(); new_cols]);

        // Resize columns in each row
        for row in &mut self.cells {
            row.resize_with(new_cols, Cell::default);
        }

        self.dimensions = dimensions;

        // Adjust cursor if necessary
        self.cursor.row = self.cursor.row.min(new_rows.saturating_sub(1));
        self.cursor.col = self.cursor.col.min(new_cols.saturating_sub(1));
    }
}

impl std::fmt::Debug for ScreenBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScreenBuffer")
            .field("dimensions", &self.dimensions)
            .field("cursor", &self.cursor)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_buffer_basic() {
        let mut screen = ScreenBuffer::new(Dimensions { rows: 24, cols: 80 });

        screen.put_char('H', CellAttributes::default());
        screen.put_char('i', CellAttributes::default());

        assert_eq!(screen.line(0), Some("Hi".to_string()));
    }

    #[test]
    fn screen_buffer_region() {
        let mut screen = ScreenBuffer::new(Dimensions { rows: 24, cols: 80 });

        for c in "Hello".chars() {
            screen.put_char(c, CellAttributes::default());
        }

        let text = screen.region_text(Region::from_coords(0, 0, 1, 5));
        assert_eq!(text, "Hello");
    }

    #[test]
    fn screen_buffer_resize() {
        let mut screen = ScreenBuffer::new(Dimensions { rows: 24, cols: 80 });
        screen.resize(Dimensions {
            rows: 40,
            cols: 120,
        });

        assert_eq!(screen.dimensions().rows, 40);
        assert_eq!(screen.dimensions().cols, 120);
    }

    #[test]
    fn position_region() {
        let region = Region::from_coords(0, 0, 10, 20);

        assert!(region.contains(Position::new(5, 10)));
        assert!(!region.contains(Position::new(10, 10)));
        assert!(!region.contains(Position::new(5, 20)));

        assert_eq!(region.width(), 20);
        assert_eq!(region.height(), 10);
    }
}
