//! Screen buffer implementation.
//!
//! This module provides a 2D screen buffer for terminal emulation,
//! storing characters, attributes, and cursor position.

use std::fmt;

/// A single cell in the screen buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    /// The character in this cell.
    pub char: char,
    /// Foreground color.
    pub fg: Color,
    /// Background color.
    pub bg: Color,
    /// Text attributes.
    pub attrs: Attributes,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            char: ' ',
            fg: Color::Default,
            bg: Color::Default,
            attrs: Attributes::empty(),
        }
    }
}

impl Cell {
    /// Create a new cell with a character.
    #[must_use]
    pub fn new(char: char) -> Self {
        Self {
            char,
            ..Default::default()
        }
    }

    /// Set the foreground color.
    #[must_use]
    pub const fn with_fg(mut self, color: Color) -> Self {
        self.fg = color;
        self
    }

    /// Set the background color.
    #[must_use]
    pub const fn with_bg(mut self, color: Color) -> Self {
        self.bg = color;
        self
    }

    /// Set the attributes.
    #[must_use]
    pub const fn with_attrs(mut self, attrs: Attributes) -> Self {
        self.attrs = attrs;
        self
    }

    /// Check if this cell is empty (space with default colors).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.char == ' ' && self.fg == Color::Default && self.bg == Color::Default
    }
}

/// Terminal colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Color {
    /// Default terminal color.
    #[default]
    Default,
    /// Black.
    Black,
    /// Red.
    Red,
    /// Green.
    Green,
    /// Yellow.
    Yellow,
    /// Blue.
    Blue,
    /// Magenta.
    Magenta,
    /// Cyan.
    Cyan,
    /// White.
    White,
    /// Bright black (gray).
    BrightBlack,
    /// Bright red.
    BrightRed,
    /// Bright green.
    BrightGreen,
    /// Bright yellow.
    BrightYellow,
    /// Bright blue.
    BrightBlue,
    /// Bright magenta.
    BrightMagenta,
    /// Bright cyan.
    BrightCyan,
    /// Bright white.
    BrightWhite,
    /// 256-color palette index.
    Indexed(u8),
    /// RGB color.
    Rgb(u8, u8, u8),
}

impl Color {
    /// Convert from ANSI color code.
    #[must_use]
    pub const fn from_ansi(code: u8) -> Self {
        match code {
            0 => Self::Black,
            1 => Self::Red,
            2 => Self::Green,
            3 => Self::Yellow,
            4 => Self::Blue,
            5 => Self::Magenta,
            6 => Self::Cyan,
            7 => Self::White,
            8 => Self::BrightBlack,
            9 => Self::BrightRed,
            10 => Self::BrightGreen,
            11 => Self::BrightYellow,
            12 => Self::BrightBlue,
            13 => Self::BrightMagenta,
            14 => Self::BrightCyan,
            15 => Self::BrightWhite,
            _ => Self::Indexed(code),
        }
    }
}

bitflags::bitflags! {
    /// Text attributes for a cell.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct Attributes: u8 {
        /// Bold text.
        const BOLD = 0b0000_0001;
        /// Dim/faint text.
        const DIM = 0b0000_0010;
        /// Italic text.
        const ITALIC = 0b0000_0100;
        /// Underlined text.
        const UNDERLINE = 0b0000_1000;
        /// Blinking text.
        const BLINK = 0b0001_0000;
        /// Inverse video.
        const INVERSE = 0b0010_0000;
        /// Hidden text.
        const HIDDEN = 0b0100_0000;
        /// Strikethrough text.
        const STRIKETHROUGH = 0b1000_0000;
    }
}

/// Cursor position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cursor {
    /// Row (0-indexed).
    pub row: usize,
    /// Column (0-indexed).
    pub col: usize,
    /// Whether the cursor is visible.
    pub visible: bool,
}

impl Cursor {
    /// Create a new cursor at (0, 0).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            row: 0,
            col: 0,
            visible: true,
        }
    }

    /// Move the cursor to a position.
    pub fn goto(&mut self, row: usize, col: usize) {
        self.row = row;
        self.col = col;
    }

    /// Move the cursor relative to its current position.
    pub fn move_by(&mut self, rows: i32, cols: i32) {
        self.row = (self.row as i32 + rows).max(0) as usize;
        self.col = (self.col as i32 + cols).max(0) as usize;
    }
}

/// A 2D screen buffer.
#[derive(Clone)]
pub struct ScreenBuffer {
    /// Buffer dimensions.
    rows: usize,
    cols: usize,
    /// Cell storage (row-major order).
    cells: Vec<Cell>,
    /// Current cursor position.
    cursor: Cursor,
    /// Current style for new characters.
    current_style: Cell,
    /// Scroll region (top, bottom).
    scroll_region: (usize, usize),
    /// Saved cursor position.
    saved_cursor: Option<Cursor>,
}

impl ScreenBuffer {
    /// Create a new screen buffer.
    #[must_use]
    pub fn new(rows: usize, cols: usize) -> Self {
        let cells = vec![Cell::default(); rows * cols];
        Self {
            rows,
            cols,
            cells,
            cursor: Cursor::new(),
            current_style: Cell::default(),
            scroll_region: (0, rows.saturating_sub(1)),
            saved_cursor: None,
        }
    }

    /// Get the number of rows.
    #[must_use]
    pub const fn rows(&self) -> usize {
        self.rows
    }

    /// Get the number of columns.
    #[must_use]
    pub const fn cols(&self) -> usize {
        self.cols
    }

    /// Get a cell at the given position.
    #[must_use]
    pub fn get(&self, row: usize, col: usize) -> Option<&Cell> {
        if row < self.rows && col < self.cols {
            Some(&self.cells[row * self.cols + col])
        } else {
            None
        }
    }

    /// Get a mutable cell at the given position.
    pub fn get_mut(&mut self, row: usize, col: usize) -> Option<&mut Cell> {
        if row < self.rows && col < self.cols {
            Some(&mut self.cells[row * self.cols + col])
        } else {
            None
        }
    }

    /// Set a cell at the given position.
    pub fn set(&mut self, row: usize, col: usize, cell: Cell) {
        if row < self.rows && col < self.cols {
            self.cells[row * self.cols + col] = cell;
        }
    }

    /// Write a character at the current cursor position.
    pub fn write_char(&mut self, c: char) {
        if self.cursor.row < self.rows && self.cursor.col < self.cols {
            let idx = self.cursor.row * self.cols + self.cursor.col;
            self.cells[idx] = Cell {
                char: c,
                fg: self.current_style.fg,
                bg: self.current_style.bg,
                attrs: self.current_style.attrs,
            };
            self.cursor.col += 1;
            if self.cursor.col >= self.cols {
                self.cursor.col = 0;
                self.cursor.row += 1;
                if self.cursor.row >= self.rows {
                    self.scroll_up(1);
                    self.cursor.row = self.rows - 1;
                }
            }
        }
    }

    /// Get the current cursor position.
    #[must_use]
    pub const fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    /// Get mutable cursor.
    pub fn cursor_mut(&mut self) -> &mut Cursor {
        &mut self.cursor
    }

    /// Move cursor to position.
    pub fn goto(&mut self, row: usize, col: usize) {
        self.cursor.row = row.min(self.rows.saturating_sub(1));
        self.cursor.col = col.min(self.cols.saturating_sub(1));
    }

    /// Clear the entire screen.
    pub fn clear(&mut self) {
        self.cells.fill(Cell::default());
    }

    /// Clear from cursor to end of screen.
    pub fn clear_to_end(&mut self) {
        let start = self.cursor.row * self.cols + self.cursor.col;
        for cell in &mut self.cells[start..] {
            *cell = Cell::default();
        }
    }

    /// Clear from start of screen to cursor.
    pub fn clear_to_start(&mut self) {
        let end = self.cursor.row * self.cols + self.cursor.col + 1;
        for cell in &mut self.cells[..end] {
            *cell = Cell::default();
        }
    }

    /// Clear the current line.
    pub fn clear_line(&mut self) {
        let start = self.cursor.row * self.cols;
        let end = start + self.cols;
        for cell in &mut self.cells[start..end] {
            *cell = Cell::default();
        }
    }

    /// Clear from cursor to end of line.
    pub fn clear_line_to_end(&mut self) {
        let start = self.cursor.row * self.cols + self.cursor.col;
        let end = self.cursor.row * self.cols + self.cols;
        for cell in &mut self.cells[start..end] {
            *cell = Cell::default();
        }
    }

    /// Scroll the screen up by n lines.
    pub fn scroll_up(&mut self, n: usize) {
        let (top, bottom) = self.scroll_region;
        let scroll_height = bottom - top + 1;
        let n = n.min(scroll_height);

        if n == 0 {
            return;
        }

        // Move lines up
        if n <= bottom.saturating_sub(top) {
            for row in top..=bottom.saturating_sub(n) {
                let src_start = (row + n) * self.cols;
                let dst_start = row * self.cols;
                for col in 0..self.cols {
                    self.cells[dst_start + col] = self.cells[src_start + col];
                }
            }
        }

        // Clear new lines at bottom
        for row in bottom.saturating_sub(n).saturating_add(1)..=bottom {
            let start = row * self.cols;
            for col in 0..self.cols {
                self.cells[start + col] = Cell::default();
            }
        }
    }

    /// Scroll the screen down by n lines.
    pub fn scroll_down(&mut self, n: usize) {
        let (top, bottom) = self.scroll_region;
        let scroll_height = bottom - top + 1;
        let n = n.min(scroll_height);

        if n == 0 {
            return;
        }

        // Move lines down
        for row in (top + n..=bottom).rev() {
            let src_start = (row - n) * self.cols;
            let dst_start = row * self.cols;
            for col in 0..self.cols {
                self.cells[dst_start + col] = self.cells[src_start + col];
            }
        }

        // Clear new lines at top
        for row in top..top + n {
            let start = row * self.cols;
            for col in 0..self.cols {
                self.cells[start + col] = Cell::default();
            }
        }
    }

    /// Set the scroll region.
    pub fn set_scroll_region(&mut self, top: usize, bottom: usize) {
        let top = top.min(self.rows.saturating_sub(1));
        let bottom = bottom.min(self.rows.saturating_sub(1)).max(top);
        self.scroll_region = (top, bottom);
    }

    /// Reset the scroll region to the entire screen.
    pub fn reset_scroll_region(&mut self) {
        self.scroll_region = (0, self.rows.saturating_sub(1));
    }

    /// Save the current cursor position.
    pub fn save_cursor(&mut self) {
        self.saved_cursor = Some(self.cursor);
    }

    /// Restore the saved cursor position.
    pub fn restore_cursor(&mut self) {
        if let Some(cursor) = self.saved_cursor.take() {
            self.cursor = cursor;
        }
    }

    /// Set the current text style.
    pub fn set_style(&mut self, fg: Color, bg: Color, attrs: Attributes) {
        self.current_style.fg = fg;
        self.current_style.bg = bg;
        self.current_style.attrs = attrs;
    }

    /// Reset the current text style to defaults.
    pub fn reset_style(&mut self) {
        self.current_style = Cell::default();
    }

    /// Insert n blank characters at the cursor position.
    ///
    /// Characters at and after the cursor are shifted right. Characters shifted
    /// past the end of the line are lost.
    pub fn insert_chars(&mut self, n: usize) {
        if n == 0 || self.cursor.row >= self.rows || self.cursor.col >= self.cols {
            return;
        }

        let row = self.cursor.row;
        let col = self.cursor.col;
        let n = n.min(self.cols - col);

        // Shift characters to the right
        let row_start = row * self.cols;
        for c in (col + n..self.cols).rev() {
            self.cells[row_start + c] = self.cells[row_start + c - n];
        }

        // Fill inserted positions with blanks
        for c in col..col + n {
            self.cells[row_start + c] = Cell::default();
        }
    }

    /// Delete n characters at the cursor position.
    ///
    /// Characters after the deleted region are shifted left. Blank characters
    /// are inserted at the end of the line.
    pub fn delete_chars(&mut self, n: usize) {
        if n == 0 || self.cursor.row >= self.rows || self.cursor.col >= self.cols {
            return;
        }

        let row = self.cursor.row;
        let col = self.cursor.col;
        let n = n.min(self.cols - col);

        let row_start = row * self.cols;

        // Shift characters to the left
        for c in col..self.cols - n {
            self.cells[row_start + c] = self.cells[row_start + c + n];
        }

        // Fill trailing positions with blanks
        for c in self.cols - n..self.cols {
            self.cells[row_start + c] = Cell::default();
        }
    }

    /// Insert n blank lines at the cursor row.
    ///
    /// Lines at and below the cursor are pushed down. Lines pushed past the
    /// bottom of the scroll region are lost.
    pub fn insert_lines(&mut self, n: usize) {
        if n == 0 || self.cursor.row > self.scroll_region.1 {
            return;
        }

        let (top, bottom) = self.scroll_region;
        let start_row = self.cursor.row.max(top);
        let region_height = bottom - start_row + 1;
        let n = n.min(region_height);

        // Move lines down within the scroll region
        for row in (start_row + n..=bottom).rev() {
            let src_start = (row - n) * self.cols;
            let dst_start = row * self.cols;
            for col in 0..self.cols {
                self.cells[dst_start + col] = self.cells[src_start + col];
            }
        }

        // Clear the inserted lines
        for row in start_row..start_row + n {
            let row_start = row * self.cols;
            for col in 0..self.cols {
                self.cells[row_start + col] = Cell::default();
            }
        }
    }

    /// Delete n lines at the cursor row.
    ///
    /// Lines below the deleted region are pulled up. Blank lines are inserted
    /// at the bottom of the scroll region.
    pub fn delete_lines(&mut self, n: usize) {
        if n == 0 || self.cursor.row > self.scroll_region.1 {
            return;
        }

        let (top, bottom) = self.scroll_region;
        let start_row = self.cursor.row.max(top);
        let region_height = bottom - start_row + 1;
        let n = n.min(region_height);

        // Move lines up within the scroll region
        for row in start_row..=bottom.saturating_sub(n) {
            let src_start = (row + n) * self.cols;
            let dst_start = row * self.cols;
            for col in 0..self.cols {
                self.cells[dst_start + col] = self.cells[src_start + col];
            }
        }

        // Clear the vacated lines at the bottom
        for row in bottom - n + 1..=bottom {
            let row_start = row * self.cols;
            for col in 0..self.cols {
                self.cells[row_start + col] = Cell::default();
            }
        }
    }

    /// Get a row as a string.
    #[must_use]
    pub fn row_text(&self, row: usize) -> String {
        if row >= self.rows {
            return String::new();
        }

        let start = row * self.cols;
        let end = start + self.cols;
        self.cells[start..end]
            .iter()
            .map(|c| c.char)
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    /// Get all content as a string.
    #[must_use]
    pub fn text(&self) -> String {
        (0..self.rows)
            .map(|r| self.row_text(r))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Resize the buffer.
    pub fn resize(&mut self, new_rows: usize, new_cols: usize) {
        let mut new_cells = vec![Cell::default(); new_rows * new_cols];

        for row in 0..new_rows.min(self.rows) {
            for col in 0..new_cols.min(self.cols) {
                new_cells[row * new_cols + col] = self.cells[row * self.cols + col];
            }
        }

        self.rows = new_rows;
        self.cols = new_cols;
        self.cells = new_cells;
        self.cursor.row = self.cursor.row.min(new_rows.saturating_sub(1));
        self.cursor.col = self.cursor.col.min(new_cols.saturating_sub(1));
        self.scroll_region = (0, new_rows.saturating_sub(1));
    }
}

impl fmt::Debug for ScreenBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScreenBuffer")
            .field("rows", &self.rows)
            .field("cols", &self.cols)
            .field("cursor", &self.cursor)
            .finish()
    }
}

/// Type of change detected in a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    /// No change.
    None,
    /// Character changed.
    Char,
    /// Foreground color changed.
    FgColor,
    /// Background color changed.
    BgColor,
    /// Attributes changed.
    Attrs,
    /// Multiple properties changed.
    Multiple,
}

/// A change to a single cell.
#[derive(Debug, Clone)]
pub struct CellChange {
    /// Row position.
    pub row: usize,
    /// Column position.
    pub col: usize,
    /// Old cell value.
    pub old: Cell,
    /// New cell value.
    pub new: Cell,
    /// Type of change.
    pub change_type: ChangeType,
}

impl CellChange {
    /// Create a new cell change.
    #[must_use]
    pub fn new(row: usize, col: usize, old: Cell, new: Cell) -> Self {
        let change_type = Self::compute_change_type(&old, &new);
        Self {
            row,
            col,
            old,
            new,
            change_type,
        }
    }

    /// Compute the type of change between two cells.
    fn compute_change_type(old: &Cell, new: &Cell) -> ChangeType {
        let mut changes = 0;
        let mut last_type = ChangeType::None;

        if old.char != new.char {
            changes += 1;
            last_type = ChangeType::Char;
        }
        if old.fg != new.fg {
            changes += 1;
            last_type = ChangeType::FgColor;
        }
        if old.bg != new.bg {
            changes += 1;
            last_type = ChangeType::BgColor;
        }
        if old.attrs != new.attrs {
            changes += 1;
            last_type = ChangeType::Attrs;
        }

        match changes {
            0 => ChangeType::None,
            1 => last_type,
            _ => ChangeType::Multiple,
        }
    }

    /// Check if this is a meaningful change (not just whitespace).
    #[must_use]
    pub fn is_significant(&self) -> bool {
        // A change is significant if either old or new is not an empty cell
        !self.old.is_empty() || !self.new.is_empty()
    }
}

/// A diff between two screen buffers.
#[derive(Debug, Clone)]
pub struct ScreenDiff {
    /// Changed cells.
    changes: Vec<CellChange>,
    /// Whether cursor position changed.
    cursor_changed: bool,
    /// Old cursor position.
    old_cursor: Cursor,
    /// New cursor position.
    new_cursor: Cursor,
    /// Whether dimensions changed.
    dimensions_changed: bool,
    /// Old dimensions (rows, cols).
    old_dims: (usize, usize),
    /// New dimensions (rows, cols).
    new_dims: (usize, usize),
}

impl ScreenDiff {
    /// Compute the diff between two screen buffers.
    #[must_use]
    pub fn compute(old: &ScreenBuffer, new: &ScreenBuffer) -> Self {
        let mut changes = Vec::new();

        let dimensions_changed = old.rows != new.rows || old.cols != new.cols;
        let cursor_changed = old.cursor != new.cursor;

        // Compare cells in the overlapping region
        let min_rows = old.rows.min(new.rows);
        let min_cols = old.cols.min(new.cols);

        for row in 0..min_rows {
            for col in 0..min_cols {
                let old_cell = old.get(row, col).copied().unwrap_or_default();
                let new_cell = new.get(row, col).copied().unwrap_or_default();

                if old_cell != new_cell {
                    changes.push(CellChange::new(row, col, old_cell, new_cell));
                }
            }
        }

        // Handle cells in rows that only exist in the new buffer
        if new.rows > old.rows {
            for row in old.rows..new.rows {
                for col in 0..new.cols {
                    let new_cell = new.get(row, col).copied().unwrap_or_default();
                    if !new_cell.is_empty() {
                        changes.push(CellChange::new(row, col, Cell::default(), new_cell));
                    }
                }
            }
        }

        // Handle cells in columns that only exist in the new buffer
        if new.cols > old.cols {
            for row in 0..min_rows {
                for col in old.cols..new.cols {
                    let new_cell = new.get(row, col).copied().unwrap_or_default();
                    if !new_cell.is_empty() {
                        changes.push(CellChange::new(row, col, Cell::default(), new_cell));
                    }
                }
            }
        }

        Self {
            changes,
            cursor_changed,
            old_cursor: old.cursor,
            new_cursor: new.cursor,
            dimensions_changed,
            old_dims: (old.rows, old.cols),
            new_dims: (new.rows, new.cols),
        }
    }

    /// Check if there are any changes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty() && !self.cursor_changed && !self.dimensions_changed
    }

    /// Get the number of cell changes.
    #[must_use]
    pub fn change_count(&self) -> usize {
        self.changes.len()
    }

    /// Get all cell changes.
    #[must_use]
    pub fn changes(&self) -> &[CellChange] {
        &self.changes
    }

    /// Get significant changes only (non-whitespace).
    #[must_use]
    pub fn significant_changes(&self) -> Vec<&CellChange> {
        self.changes.iter().filter(|c| c.is_significant()).collect()
    }

    /// Check if cursor changed.
    #[must_use]
    pub fn cursor_changed(&self) -> bool {
        self.cursor_changed
    }

    /// Get the old cursor position.
    #[must_use]
    pub fn old_cursor(&self) -> Cursor {
        self.old_cursor
    }

    /// Get the new cursor position.
    #[must_use]
    pub fn new_cursor(&self) -> Cursor {
        self.new_cursor
    }

    /// Check if dimensions changed.
    #[must_use]
    pub fn dimensions_changed(&self) -> bool {
        self.dimensions_changed
    }

    /// Get changed rows (sorted, deduplicated).
    #[must_use]
    pub fn changed_rows(&self) -> Vec<usize> {
        let mut rows: Vec<usize> = self.changes.iter().map(|c| c.row).collect();
        rows.sort_unstable();
        rows.dedup();
        rows
    }

    /// Get changes for a specific row.
    #[must_use]
    pub fn row_changes(&self, row: usize) -> Vec<&CellChange> {
        self.changes.iter().filter(|c| c.row == row).collect()
    }

    /// Generate a human-readable diff report.
    #[must_use]
    pub fn report(&self) -> String {
        let mut output = String::new();

        if self.is_empty() {
            output.push_str("No changes detected.\n");
            return output;
        }

        if self.dimensions_changed {
            output.push_str(&format!(
                "Dimensions changed: {}x{} -> {}x{}\n",
                self.old_dims.1, self.old_dims.0,
                self.new_dims.1, self.new_dims.0
            ));
        }

        if self.cursor_changed {
            output.push_str(&format!(
                "Cursor moved: ({}, {}) -> ({}, {})\n",
                self.old_cursor.row, self.old_cursor.col,
                self.new_cursor.row, self.new_cursor.col
            ));
        }

        let significant = self.significant_changes();
        if !significant.is_empty() {
            output.push_str(&format!("{} cell(s) changed:\n", significant.len()));

            for row in self.changed_rows() {
                let row_changes: Vec<_> = significant.iter()
                    .filter(|c| c.row == row)
                    .collect();

                if !row_changes.is_empty() {
                    output.push_str(&format!("  Row {}:\n", row));
                    for change in row_changes {
                        let old_char = if change.old.char.is_control() {
                            format!("\\x{:02x}", change.old.char as u8)
                        } else {
                            change.old.char.to_string()
                        };
                        let new_char = if change.new.char.is_control() {
                            format!("\\x{:02x}", change.new.char as u8)
                        } else {
                            change.new.char.to_string()
                        };
                        output.push_str(&format!(
                            "    Col {}: '{}' -> '{}' ({:?})\n",
                            change.col, old_char, new_char, change.change_type
                        ));
                    }
                }
            }
        }

        output
    }

    /// Generate a visual side-by-side diff for a specific row.
    #[must_use]
    pub fn row_visual_diff(&self, old: &ScreenBuffer, new: &ScreenBuffer, row: usize) -> String {
        let old_text = old.row_text(row);
        let new_text = new.row_text(row);

        if old_text == new_text {
            format!("  {}: {}", row, old_text)
        } else {
            format!("- {}: {}\n+ {}: {}", row, old_text, row, new_text)
        }
    }

    /// Generate a complete visual diff output.
    #[must_use]
    pub fn visual_diff(&self, old: &ScreenBuffer, new: &ScreenBuffer) -> String {
        let mut output = String::new();
        let max_rows = old.rows.max(new.rows);

        for row in 0..max_rows {
            let old_text = if row < old.rows { old.row_text(row) } else { String::new() };
            let new_text = if row < new.rows { new.row_text(row) } else { String::new() };

            if old_text != new_text {
                if !old_text.is_empty() || row < old.rows {
                    output.push_str(&format!("- {}: {}\n", row, old_text));
                }
                if !new_text.is_empty() || row < new.rows {
                    output.push_str(&format!("+ {}: {}\n", row, new_text));
                }
            } else if !old_text.is_empty() {
                output.push_str(&format!("  {}: {}\n", row, old_text));
            }
        }

        output
    }
}

impl ScreenBuffer {
    /// Compute the diff between this buffer and another.
    #[must_use]
    pub fn diff(&self, other: &ScreenBuffer) -> ScreenDiff {
        ScreenDiff::compute(self, other)
    }

    /// Check if this buffer equals another (content and cursor).
    #[must_use]
    pub fn equals(&self, other: &ScreenBuffer) -> bool {
        self.diff(other).is_empty()
    }

    /// Create a snapshot (clone) of this buffer for later comparison.
    #[must_use]
    pub fn snapshot(&self) -> ScreenBuffer {
        self.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_buffer_basic() {
        let mut buf = ScreenBuffer::new(24, 80);
        assert_eq!(buf.rows(), 24);
        assert_eq!(buf.cols(), 80);

        buf.write_char('H');
        buf.write_char('i');
        assert_eq!(buf.row_text(0), "Hi");
    }

    #[test]
    fn screen_buffer_cursor() {
        let mut buf = ScreenBuffer::new(24, 80);
        buf.goto(5, 10);
        assert_eq!(buf.cursor().row, 5);
        assert_eq!(buf.cursor().col, 10);
    }

    #[test]
    fn screen_buffer_clear() {
        let mut buf = ScreenBuffer::new(24, 80);
        buf.write_char('A');
        buf.clear();
        assert!(buf.row_text(0).is_empty());
    }

    #[test]
    fn screen_buffer_scroll() {
        let mut buf = ScreenBuffer::new(3, 10);
        buf.goto(0, 0);
        for c in "Line 1".chars() {
            buf.write_char(c);
        }
        buf.goto(1, 0);
        for c in "Line 2".chars() {
            buf.write_char(c);
        }
        buf.goto(2, 0);
        for c in "Line 3".chars() {
            buf.write_char(c);
        }

        buf.scroll_up(1);
        assert_eq!(buf.row_text(0), "Line 2");
        assert_eq!(buf.row_text(1), "Line 3");
        assert!(buf.row_text(2).is_empty());
    }

    #[test]
    fn insert_chars_shifts_right() {
        let mut buf = ScreenBuffer::new(1, 10);
        // Write "ABCDE" at start
        for c in "ABCDE".chars() {
            buf.write_char(c);
        }
        // Cursor is now at col 5
        // Move cursor to col 2 and insert 2 chars
        buf.goto(0, 2);
        buf.insert_chars(2);

        // Result should be "AB  CDE" (C, D, E shifted right by 2)
        assert_eq!(buf.row_text(0), "AB  CDE");
    }

    #[test]
    fn insert_chars_at_end_of_line() {
        let mut buf = ScreenBuffer::new(1, 5);
        for c in "ABCDE".chars() {
            buf.write_char(c);
        }
        // Move to col 3 and insert 3 chars
        buf.goto(0, 3);
        buf.insert_chars(3);

        // D and E should be pushed off, result is "ABC"
        assert_eq!(buf.row_text(0), "ABC");
    }

    #[test]
    fn delete_chars_shifts_left() {
        let mut buf = ScreenBuffer::new(1, 10);
        for c in "ABCDEFGH".chars() {
            buf.write_char(c);
        }
        // Move to col 2 and delete 3 chars
        buf.goto(0, 2);
        buf.delete_chars(3);

        // C, D, E deleted; F, G, H shift left
        // Result: "ABFGH"
        assert_eq!(buf.row_text(0), "ABFGH");
    }

    #[test]
    fn insert_lines_pushes_down() {
        let mut buf = ScreenBuffer::new(5, 10);
        for (i, text) in ["Line 0", "Line 1", "Line 2", "Line 3", "Line 4"]
            .iter()
            .enumerate()
        {
            buf.goto(i, 0);
            for c in text.chars() {
                buf.write_char(c);
            }
        }

        // Insert 2 lines at row 1
        buf.goto(1, 0);
        buf.insert_lines(2);

        assert_eq!(buf.row_text(0), "Line 0");
        assert!(buf.row_text(1).is_empty()); // Inserted blank
        assert!(buf.row_text(2).is_empty()); // Inserted blank
        assert_eq!(buf.row_text(3), "Line 1"); // Pushed from row 1
        assert_eq!(buf.row_text(4), "Line 2"); // Pushed from row 2
        // Line 3 and Line 4 pushed off bottom
    }

    #[test]
    fn delete_lines_pulls_up() {
        let mut buf = ScreenBuffer::new(5, 10);
        for (i, text) in ["Line 0", "Line 1", "Line 2", "Line 3", "Line 4"]
            .iter()
            .enumerate()
        {
            buf.goto(i, 0);
            for c in text.chars() {
                buf.write_char(c);
            }
        }

        // Delete 2 lines at row 1
        buf.goto(1, 0);
        buf.delete_lines(2);

        assert_eq!(buf.row_text(0), "Line 0");
        assert_eq!(buf.row_text(1), "Line 3"); // Pulled from row 3
        assert_eq!(buf.row_text(2), "Line 4"); // Pulled from row 4
        assert!(buf.row_text(3).is_empty()); // Cleared
        assert!(buf.row_text(4).is_empty()); // Cleared
    }

    #[test]
    fn diff_no_changes() {
        let buf1 = ScreenBuffer::new(3, 10);
        let buf2 = ScreenBuffer::new(3, 10);

        let diff = buf1.diff(&buf2);
        assert!(diff.is_empty());
        assert_eq!(diff.change_count(), 0);
        assert!(!diff.cursor_changed());
        assert!(!diff.dimensions_changed());
    }

    #[test]
    fn diff_character_changes() {
        let mut buf1 = ScreenBuffer::new(3, 10);
        let mut buf2 = ScreenBuffer::new(3, 10);

        for c in "Hello".chars() {
            buf1.write_char(c);
        }
        buf1.goto(0, 0);

        for c in "World".chars() {
            buf2.write_char(c);
        }
        buf2.goto(0, 0);

        let diff = buf1.diff(&buf2);
        assert!(!diff.is_empty());
        // "Hello" vs "World": H!=W, e!=o, l!=r, l==l (same!), o!=d = 4 changes
        assert_eq!(diff.change_count(), 4);
        assert_eq!(diff.changed_rows(), vec![0]);
    }

    #[test]
    fn diff_cursor_moved() {
        let mut buf1 = ScreenBuffer::new(3, 10);
        let mut buf2 = buf1.snapshot();

        buf2.goto(2, 5);

        let diff = buf1.diff(&buf2);
        assert!(!diff.is_empty());
        assert!(diff.cursor_changed());
        assert_eq!(diff.old_cursor().row, 0);
        assert_eq!(diff.old_cursor().col, 0);
        assert_eq!(diff.new_cursor().row, 2);
        assert_eq!(diff.new_cursor().col, 5);
    }

    #[test]
    fn diff_dimensions_changed() {
        let buf1 = ScreenBuffer::new(3, 10);
        let buf2 = ScreenBuffer::new(5, 15);

        let diff = buf1.diff(&buf2);
        assert!(diff.dimensions_changed());
    }

    #[test]
    fn diff_significant_changes() {
        let mut buf1 = ScreenBuffer::new(3, 10);
        let mut buf2 = ScreenBuffer::new(3, 10);

        for c in "Hi".chars() {
            buf2.write_char(c);
        }
        buf2.goto(0, 0);

        let diff = buf1.diff(&buf2);
        let significant = diff.significant_changes();
        assert_eq!(significant.len(), 2); // Only "Hi" are significant
    }

    #[test]
    fn diff_report() {
        let buf1 = ScreenBuffer::new(3, 10);
        let mut buf2 = ScreenBuffer::new(3, 10);

        for c in "ABC".chars() {
            buf2.write_char(c);
        }

        let diff = buf1.diff(&buf2);
        let report = diff.report();

        assert!(report.contains("3 cell(s) changed"));
        assert!(report.contains("Row 0"));
    }

    #[test]
    fn diff_visual_output() {
        let mut buf1 = ScreenBuffer::new(3, 10);
        let mut buf2 = ScreenBuffer::new(3, 10);

        for c in "Line 1".chars() {
            buf1.write_char(c);
        }
        buf1.goto(0, 0);

        for c in "Line 2".chars() {
            buf2.write_char(c);
        }
        buf2.goto(0, 0);

        let diff = buf1.diff(&buf2);
        let visual = diff.visual_diff(&buf1, &buf2);

        assert!(visual.contains("- 0: Line 1"));
        assert!(visual.contains("+ 0: Line 2"));
    }

    #[test]
    fn buffer_equals() {
        let buf1 = ScreenBuffer::new(3, 10);
        let buf2 = ScreenBuffer::new(3, 10);
        let buf3 = ScreenBuffer::new(5, 10);

        assert!(buf1.equals(&buf2));
        assert!(!buf1.equals(&buf3));
    }

    #[test]
    fn buffer_snapshot() {
        let mut buf = ScreenBuffer::new(3, 10);
        for c in "Test".chars() {
            buf.write_char(c);
        }

        let snapshot = buf.snapshot();
        assert!(buf.equals(&snapshot));

        buf.clear();
        assert!(!buf.equals(&snapshot));
    }

    #[test]
    fn cell_change_types() {
        let old = Cell::new('A');
        let new_char = Cell::new('B');
        let change = CellChange::new(0, 0, old, new_char);
        assert_eq!(change.change_type, ChangeType::Char);

        let new_fg = Cell::new('A').with_fg(Color::Red);
        let change = CellChange::new(0, 0, old, new_fg);
        assert_eq!(change.change_type, ChangeType::FgColor);

        let new_multiple = Cell::new('B').with_fg(Color::Red);
        let change = CellChange::new(0, 0, old, new_multiple);
        assert_eq!(change.change_type, ChangeType::Multiple);
    }

    #[test]
    fn row_changes_filter() {
        let buf1 = ScreenBuffer::new(3, 10);
        let mut buf2 = ScreenBuffer::new(3, 10);

        buf2.goto(0, 0);
        for c in "Row 0".chars() {
            buf2.write_char(c);
        }
        buf2.goto(1, 0);
        for c in "Row 1".chars() {
            buf2.write_char(c);
        }

        let diff = buf1.diff(&buf2);
        let row0_changes = diff.row_changes(0);
        let row1_changes = diff.row_changes(1);

        // "Row 0" and "Row 1" each have a space at position 3 that matches
        // the default cell (space), so only 4 characters differ per row
        assert_eq!(row0_changes.len(), 4); // R, o, w, 0
        assert_eq!(row1_changes.len(), 4); // R, o, w, 1
    }
}
