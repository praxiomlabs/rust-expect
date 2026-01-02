//! Integration tests for screen buffer functionality.

#![cfg(feature = "screen")]

use rust_expect::screen::buffer::{Color, Cursor};
use rust_expect::{Attributes, Cell, Dimensions, ScreenBuffer};

#[test]
fn screen_buffer_new() {
    let buffer = ScreenBuffer::new(24, 80);
    assert_eq!(buffer.rows(), 24);
    assert_eq!(buffer.cols(), 80);
}

#[test]
fn screen_buffer_dimensions_match_standard() {
    let buffer = ScreenBuffer::new(24, 80);
    assert_eq!(buffer.rows(), 24);
    assert_eq!(buffer.cols(), 80);
}

#[test]
fn dimensions_struct() {
    let dims = Dimensions::new(80, 24);
    assert_eq!(dims.cols, 80);
    assert_eq!(dims.rows, 24);
}

#[test]
fn dimensions_default() {
    let dims = Dimensions::default();
    assert_eq!(dims, Dimensions::STANDARD);
    assert_eq!(dims.cols, 80);
    assert_eq!(dims.rows, 24);
}

#[test]
fn dimensions_from_tuple() {
    let dims: Dimensions = (120, 40).into();
    assert_eq!(dims.cols, 120);
    assert_eq!(dims.rows, 40);
}

#[test]
fn screen_buffer_resize() {
    let mut buffer = ScreenBuffer::new(24, 80);
    buffer.resize(40, 120);

    assert_eq!(buffer.rows(), 40);
    assert_eq!(buffer.cols(), 120);
}

#[test]
fn screen_buffer_write_char() {
    let mut buffer = ScreenBuffer::new(24, 80);
    buffer.write_char('A');

    let cell = buffer.get(0, 0);
    assert!(cell.is_some());
    assert_eq!(cell.unwrap().char, 'A');
}

#[test]
fn screen_buffer_cursor() {
    let buffer = ScreenBuffer::new(24, 80);
    let cursor = buffer.cursor();

    // Should start at origin
    assert_eq!(cursor.col, 0);
    assert_eq!(cursor.row, 0);
}

#[test]
fn screen_buffer_goto() {
    let mut buffer = ScreenBuffer::new(24, 80);
    buffer.goto(10, 5);

    let cursor = buffer.cursor();
    assert_eq!(cursor.row, 10);
    assert_eq!(cursor.col, 5);
}

#[test]
fn screen_buffer_clear() {
    let mut buffer = ScreenBuffer::new(24, 80);
    buffer.write_char('X');
    buffer.clear();

    // After clear, first cell should be empty space
    let cell = buffer.get(0, 0);
    assert!(cell.is_some());
    assert_eq!(cell.unwrap().char, ' ');
}

#[test]
fn screen_buffer_row_text() {
    let mut buffer = ScreenBuffer::new(24, 80);
    for c in "Hello".chars() {
        buffer.write_char(c);
    }

    let text = buffer.row_text(0);
    assert_eq!(text, "Hello");
}

#[test]
fn screen_buffer_text() {
    let mut buffer = ScreenBuffer::new(24, 80);
    for c in "Line 1".chars() {
        buffer.write_char(c);
    }
    buffer.goto(1, 0);
    for c in "Line 2".chars() {
        buffer.write_char(c);
    }

    let text = buffer.text();
    assert!(text.contains("Line 1"));
    assert!(text.contains("Line 2"));
}

#[test]
fn screen_buffer_get_cell() {
    let mut buffer = ScreenBuffer::new(24, 80);
    buffer.goto(5, 10);
    buffer.write_char('Z');

    let cell = buffer.get(5, 10);
    assert!(cell.is_some());
    assert_eq!(cell.unwrap().char, 'Z');
}

#[test]
fn screen_buffer_get_out_of_bounds() {
    let buffer = ScreenBuffer::new(24, 80);
    assert!(buffer.get(100, 100).is_none());
}

#[test]
fn screen_buffer_clear_line() {
    let mut buffer = ScreenBuffer::new(24, 80);
    buffer.goto(5, 0);
    for c in "Test line".chars() {
        buffer.write_char(c);
    }
    buffer.goto(5, 0);
    buffer.clear_line();

    assert!(buffer.row_text(5).is_empty());
}

#[test]
fn screen_buffer_scroll_up() {
    let mut buffer = ScreenBuffer::new(3, 10);
    buffer.goto(0, 0);
    for c in "Line 1".chars() {
        buffer.write_char(c);
    }
    buffer.goto(1, 0);
    for c in "Line 2".chars() {
        buffer.write_char(c);
    }
    buffer.goto(2, 0);
    for c in "Line 3".chars() {
        buffer.write_char(c);
    }

    buffer.scroll_up(1);

    assert_eq!(buffer.row_text(0), "Line 2");
    assert_eq!(buffer.row_text(1), "Line 3");
    assert!(buffer.row_text(2).is_empty());
}

#[test]
fn screen_buffer_scroll_down() {
    let mut buffer = ScreenBuffer::new(3, 10);
    buffer.goto(0, 0);
    for c in "Line 1".chars() {
        buffer.write_char(c);
    }
    buffer.goto(1, 0);
    for c in "Line 2".chars() {
        buffer.write_char(c);
    }

    buffer.scroll_down(1);

    assert!(buffer.row_text(0).is_empty());
    assert_eq!(buffer.row_text(1), "Line 1");
    assert_eq!(buffer.row_text(2), "Line 2");
}

#[test]
fn screen_buffer_save_restore_cursor() {
    let mut buffer = ScreenBuffer::new(24, 80);
    buffer.goto(10, 20);
    buffer.save_cursor();
    buffer.goto(5, 5);
    buffer.restore_cursor();

    let cursor = buffer.cursor();
    assert_eq!(cursor.row, 10);
    assert_eq!(cursor.col, 20);
}

#[test]
fn cell_default() {
    let cell = Cell::default();
    assert_eq!(cell.char, ' ');
    assert_eq!(cell.fg, Color::Default);
    assert_eq!(cell.bg, Color::Default);
}

#[test]
fn cell_new() {
    let cell = Cell::new('X');
    assert_eq!(cell.char, 'X');
}

#[test]
fn cell_with_colors() {
    let cell = Cell::new('A')
        .with_fg(Color::Red)
        .with_bg(Color::Blue);

    assert_eq!(cell.char, 'A');
    assert_eq!(cell.fg, Color::Red);
    assert_eq!(cell.bg, Color::Blue);
}

#[test]
fn cell_is_empty() {
    let empty = Cell::default();
    assert!(empty.is_empty());

    let not_empty = Cell::new('X');
    assert!(!not_empty.is_empty());
}

#[test]
fn attributes_empty() {
    let attrs = Attributes::empty();
    assert!(!attrs.contains(Attributes::BOLD));
    assert!(!attrs.contains(Attributes::ITALIC));
    assert!(!attrs.contains(Attributes::UNDERLINE));
}

#[test]
fn attributes_bold() {
    let attrs = Attributes::BOLD;
    assert!(attrs.contains(Attributes::BOLD));
    assert!(!attrs.contains(Attributes::ITALIC));
}

#[test]
fn attributes_combined() {
    let attrs = Attributes::BOLD | Attributes::UNDERLINE;
    assert!(attrs.contains(Attributes::BOLD));
    assert!(attrs.contains(Attributes::UNDERLINE));
    assert!(!attrs.contains(Attributes::ITALIC));
}

#[test]
fn color_from_ansi() {
    assert_eq!(Color::from_ansi(0), Color::Black);
    assert_eq!(Color::from_ansi(1), Color::Red);
    assert_eq!(Color::from_ansi(7), Color::White);
    assert_eq!(Color::from_ansi(8), Color::BrightBlack);
}

#[test]
fn color_rgb() {
    let color = Color::Rgb(255, 128, 0);
    assert!(!format!("{:?}", color).is_empty());
}

#[test]
fn cursor_new() {
    let cursor = Cursor::new();
    assert_eq!(cursor.row, 0);
    assert_eq!(cursor.col, 0);
    assert!(cursor.visible);
}

#[test]
fn cursor_goto() {
    let mut cursor = Cursor::new();
    cursor.goto(10, 20);

    assert_eq!(cursor.row, 10);
    assert_eq!(cursor.col, 20);
}

#[test]
fn cursor_move_by() {
    let mut cursor = Cursor::new();
    cursor.goto(10, 10);
    cursor.move_by(5, -3);

    assert_eq!(cursor.row, 15);
    assert_eq!(cursor.col, 7);
}

#[test]
fn cursor_move_by_clamps_to_zero() {
    let mut cursor = Cursor::new();
    cursor.goto(0, 0);
    cursor.move_by(-5, -5);

    // Should clamp to 0
    assert_eq!(cursor.row, 0);
    assert_eq!(cursor.col, 0);
}
