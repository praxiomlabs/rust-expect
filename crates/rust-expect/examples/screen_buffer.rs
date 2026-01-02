//! Screen buffer example.
//!
//! This example demonstrates screen buffer functionality for
//! tracking terminal screen state.
//!
//! Run with: `cargo run --example screen_buffer --features screen`

#[cfg(feature = "screen")]
fn main() {
    use rust_expect::screen::{ScreenBuffer, ScreenQueryExt};

    println!("Screen Buffer Examples\n");

    // Create a screen buffer
    let mut buffer = ScreenBuffer::new(24, 80);
    println!("Created 80x24 screen buffer");

    // Write some text character by character
    buffer.goto(0, 0);
    for c in "Hello, World!".chars() {
        buffer.write_char(c);
    }
    buffer.goto(1, 0);
    for c in "This is line 2".chars() {
        buffer.write_char(c);
    }
    buffer.goto(2, 0);
    for c in "Login: admin".chars() {
        buffer.write_char(c);
    }

    // Query the buffer
    println!("\n=== Buffer Contents ===");
    let text = buffer.query().text();
    for (i, line) in text.lines().take(5).enumerate() {
        if !line.trim().is_empty() {
            println!("Line {i}: {line}");
        }
    }

    // Find text
    println!("\n=== Finding Text ===");
    if let Some((row, col)) = buffer.query().find("World") {
        println!("Found 'World' at row {row}, col {col}");
    }

    if buffer.query().contains("Login:") {
        println!("Buffer contains 'Login:'");
    }

    // Cursor position
    println!("\n=== Cursor Position ===");
    let cursor = buffer.cursor();
    println!("Cursor at row {}, col {}", cursor.row, cursor.col);

    // Screen dimensions
    println!("\n=== Dimensions ===");
    println!("Rows: {}, Cols: {}", buffer.rows(), buffer.cols());

    // Scrolling
    println!("\n=== Scrolling ===");
    buffer.scroll_up(1);
    println!("Scrolled up 1 line");

    println!("\nScreen buffer examples completed!");
}

#[cfg(not(feature = "screen"))]
fn main() {
    println!("This example requires the 'screen' feature.");
    println!("Run with: cargo run --example screen_buffer --features screen");
}
