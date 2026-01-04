//! Screen query utilities.
//!
//! This module provides utilities for querying screen buffer contents,
//! including text extraction, pattern matching, and region selection.

use super::buffer::{Cell, ScreenBuffer};
use regex::Regex;

/// A rectangular region on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    /// Top row (inclusive).
    pub top: usize,
    /// Left column (inclusive).
    pub left: usize,
    /// Bottom row (inclusive).
    pub bottom: usize,
    /// Right column (inclusive).
    pub right: usize,
}

impl Region {
    /// Create a new region.
    #[must_use]
    pub const fn new(top: usize, left: usize, bottom: usize, right: usize) -> Self {
        Self {
            top,
            left,
            bottom,
            right,
        }
    }

    /// Create a region for a single cell.
    #[must_use]
    pub const fn cell(row: usize, col: usize) -> Self {
        Self::new(row, col, row, col)
    }

    /// Create a region for a single row.
    #[must_use]
    pub const fn row(row: usize, cols: usize) -> Self {
        Self::new(row, 0, row, cols.saturating_sub(1))
    }

    /// Create a region for the entire screen.
    #[must_use]
    pub const fn full(rows: usize, cols: usize) -> Self {
        Self::new(0, 0, rows.saturating_sub(1), cols.saturating_sub(1))
    }

    /// Get the width of the region.
    #[must_use]
    pub const fn width(&self) -> usize {
        self.right.saturating_sub(self.left) + 1
    }

    /// Get the height of the region.
    #[must_use]
    pub const fn height(&self) -> usize {
        self.bottom.saturating_sub(self.top) + 1
    }

    /// Check if a position is within the region.
    #[must_use]
    pub const fn contains(&self, row: usize, col: usize) -> bool {
        row >= self.top && row <= self.bottom && col >= self.left && col <= self.right
    }

    /// Clamp the region to fit within bounds.
    #[must_use]
    pub fn clamp(self, max_rows: usize, max_cols: usize) -> Self {
        Self {
            top: self.top.min(max_rows.saturating_sub(1)),
            left: self.left.min(max_cols.saturating_sub(1)),
            bottom: self.bottom.min(max_rows.saturating_sub(1)),
            right: self.right.min(max_cols.saturating_sub(1)),
        }
    }
}

/// A query for screen content.
pub struct ScreenQuery<'a> {
    buffer: &'a ScreenBuffer,
    region: Option<Region>,
}

impl<'a> ScreenQuery<'a> {
    /// Create a new query for the entire screen.
    #[must_use]
    pub const fn new(buffer: &'a ScreenBuffer) -> Self {
        Self {
            buffer,
            region: None,
        }
    }

    /// Limit the query to a specific region.
    #[must_use]
    pub fn region(mut self, region: Region) -> Self {
        self.region = Some(region.clamp(self.buffer.rows(), self.buffer.cols()));
        self
    }

    /// Limit the query to a specific row.
    #[must_use]
    pub fn row(self, row: usize) -> Self {
        let cols = self.buffer.cols();
        self.region(Region::row(row, cols))
    }

    /// Get the effective region.
    fn effective_region(&self) -> Region {
        self.region
            .unwrap_or_else(|| Region::full(self.buffer.rows(), self.buffer.cols()))
    }

    /// Get the text content of the query region.
    #[must_use]
    pub fn text(&self) -> String {
        let region = self.effective_region();
        let mut lines = Vec::new();

        for row in region.top..=region.bottom {
            let mut line = String::new();
            for col in region.left..=region.right {
                if let Some(cell) = self.buffer.get(row, col) {
                    line.push(cell.char);
                }
            }
            lines.push(line.trim_end().to_string());
        }

        lines.join("\n")
    }

    /// Get the text content without trailing whitespace on each line.
    #[must_use]
    pub fn trimmed_text(&self) -> String {
        self.text()
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
            .trim_end()
            .to_string()
    }

    /// Find a literal string in the region.
    #[must_use]
    pub fn find(&self, needle: &str) -> Option<(usize, usize)> {
        let region = self.effective_region();
        let text = self.text();

        // Search in the combined text
        if let Some(pos) = text.find(needle) {
            // Convert byte position to row/col
            let mut row = region.top;
            let mut byte_pos = 0;

            for line in text.lines() {
                let line_bytes = line.len() + 1; // +1 for newline
                if byte_pos + line_bytes > pos {
                    let col = region.left + (pos - byte_pos);
                    return Some((row, col));
                }
                byte_pos += line_bytes;
                row += 1;
            }
        }

        None
    }

    /// Find all occurrences of a literal string.
    #[must_use]
    pub fn find_all(&self, needle: &str) -> Vec<(usize, usize)> {
        let region = self.effective_region();
        let mut results = Vec::new();

        for row in region.top..=region.bottom {
            let line_text = self.row_text(row);
            let mut start = 0;
            while let Some(pos) = line_text[start..].find(needle) {
                let col = region.left + start + pos;
                results.push((row, col));
                start += pos + 1;
            }
        }

        results
    }

    /// Find a regex pattern in the region.
    #[must_use]
    pub fn find_regex(&self, pattern: &Regex) -> Option<(usize, usize, String)> {
        let region = self.effective_region();
        let text = self.text();

        if let Some(m) = pattern.find(&text) {
            let pos = m.start();
            let mut row = region.top;
            let mut byte_pos = 0;

            for line in text.lines() {
                let line_bytes = line.len() + 1;
                if byte_pos + line_bytes > pos {
                    let col = region.left + (pos - byte_pos);
                    return Some((row, col, m.as_str().to_string()));
                }
                byte_pos += line_bytes;
                row += 1;
            }
        }

        None
    }

    /// Check if the region contains a literal string.
    #[must_use]
    pub fn contains(&self, needle: &str) -> bool {
        self.find(needle).is_some()
    }

    /// Check if the region matches a regex pattern.
    #[must_use]
    pub fn matches(&self, pattern: &Regex) -> bool {
        pattern.is_match(&self.text())
    }

    /// Get the text of a specific row.
    fn row_text(&self, row: usize) -> String {
        let region = self.effective_region();
        if row < region.top || row > region.bottom {
            return String::new();
        }

        let mut line = String::new();
        for col in region.left..=region.right {
            if let Some(cell) = self.buffer.get(row, col) {
                line.push(cell.char);
            }
        }
        line
    }

    /// Get cells in the region.
    #[must_use]
    pub fn cells(&self) -> Vec<&Cell> {
        let region = self.effective_region();
        let mut cells = Vec::new();

        for row in region.top..=region.bottom {
            for col in region.left..=region.right {
                if let Some(cell) = self.buffer.get(row, col) {
                    cells.push(cell);
                }
            }
        }

        cells
    }

    /// Count non-empty cells in the region.
    #[must_use]
    pub fn count_non_empty(&self) -> usize {
        self.cells().iter().filter(|c| !c.is_empty()).count()
    }

    /// Check if the region is empty (all whitespace).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cells().iter().all(|c| c.is_empty())
    }
}

/// Extension trait for screen buffer queries.
pub trait ScreenQueryExt {
    /// Create a query for this buffer.
    fn query(&self) -> ScreenQuery<'_>;
}

impl ScreenQueryExt for ScreenBuffer {
    fn query(&self) -> ScreenQuery<'_> {
        ScreenQuery::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_buffer(text: &str) -> ScreenBuffer {
        let lines: Vec<&str> = text.lines().collect();
        let rows = lines.len().max(1);
        // Add 1 to cols to prevent cursor wrap on last character
        let cols = lines.iter().map(|l| l.len()).max().unwrap_or(80) + 1;
        let mut buf = ScreenBuffer::new(rows, cols);

        for (row, line) in lines.iter().enumerate() {
            buf.goto(row, 0);
            for c in line.chars() {
                buf.write_char(c);
            }
        }

        buf
    }

    #[test]
    fn query_text() {
        let buf = make_buffer("Hello\nWorld");
        let text = buf.query().text();
        assert_eq!(text, "Hello\nWorld");
    }

    #[test]
    fn query_find() {
        let buf = make_buffer("Hello World");
        let result = buf.query().find("World");
        assert_eq!(result, Some((0, 6)));
    }

    #[test]
    fn query_contains() {
        let buf = make_buffer("Login: ");
        assert!(buf.query().contains("Login"));
        assert!(!buf.query().contains("Password"));
    }

    #[test]
    fn query_region() {
        let buf = make_buffer("ABCDE\nFGHIJ\nKLMNO");
        let text = buf.query().region(Region::new(0, 1, 1, 3)).text();
        assert_eq!(text, "BCD\nGHI");
    }

    #[test]
    fn region_contains() {
        let region = Region::new(5, 10, 15, 20);
        assert!(region.contains(10, 15));
        assert!(!region.contains(4, 15));
        assert!(!region.contains(10, 21));
    }
}
