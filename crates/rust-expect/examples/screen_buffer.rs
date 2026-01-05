//! Screen buffer example.
//!
//! This example demonstrates the virtual terminal screen buffer
//! for processing ANSI escape sequences and querying screen content.
//!
//! Run with: `cargo run --example screen_buffer --features screen`

#[cfg(feature = "screen")]
use rust_expect::screen::{Screen, ScreenBuffer};

fn main() {
    println!("rust-expect Screen Buffer Example");
    println!("==================================\n");

    #[cfg(not(feature = "screen"))]
    {
        println!("This example requires the 'screen' feature.");
        println!("Run with: cargo run --example screen_buffer --features screen");
    }

    #[cfg(feature = "screen")]
    run_examples();
}

#[cfg(feature = "screen")]
fn run_examples() {
    // Example 1: Basic screen creation
    println!("1. Creating a virtual screen...");

    let mut screen = Screen::new(24, 80);
    println!("   Created screen: {}x{}", screen.rows(), screen.cols());

    // VT100 standard size
    let vt100 = Screen::vt100();
    println!("   VT100 screen: {}x{}", vt100.rows(), vt100.cols());

    // Example 2: Processing text
    println!("\n2. Processing text output...");

    screen.process_str("Hello, World!");
    println!("   Wrote: 'Hello, World!'");
    println!(
        "   Cursor position: ({}, {})",
        screen.cursor().row,
        screen.cursor().col
    );

    // Example 3: ANSI escape sequences
    println!("\n3. Processing ANSI sequences...");

    let mut screen = Screen::new(10, 40);

    // Cursor movement
    screen.process_str("Line 1\n");
    screen.process_str("Line 2\n");
    screen.process_str("\x1b[1;1H"); // Move cursor to row 1, col 1
    screen.process_str("Modified Line 1");

    println!("   Processed cursor movement");
    println!("   First line now starts with 'Modified'");

    // Example 4: Colors and attributes
    println!("\n4. Colors and text attributes...");

    let mut screen = Screen::new(5, 40);

    // Red text
    screen.process_str("\x1b[31mRed Text\x1b[0m ");

    // Bold green text
    screen.process_str("\x1b[1;32mBold Green\x1b[0m ");

    // Underlined blue text
    screen.process_str("\x1b[4;34mUnderlined Blue\x1b[0m");

    // Check the first cell
    if let Some(cell) = screen.buffer().get(0, 0) {
        println!("   First char: '{}', color: {:?}", cell.char, cell.fg);
    }

    // Example 5: Screen clearing
    println!("\n5. Screen clearing...");

    let mut screen = Screen::new(5, 20);
    screen.process_str("Content to clear");
    println!("   Before clear: '{}'", screen.text().trim());

    screen.process_str("\x1b[2J\x1b[H"); // Clear screen and home cursor
    screen.process_str("New content");
    println!("   After clear: '{}'", screen.text().trim());

    // Example 6: Scrolling
    println!("\n6. Screen scrolling...");

    let mut screen = Screen::new(3, 20);
    screen.process_str("Line 1\n");
    screen.process_str("Line 2\n");
    screen.process_str("Line 3\n");
    screen.process_str("Line 4"); // This should scroll

    let text = screen.text();
    let has_line1 = text.contains("Line 1");
    let has_line4 = text.contains("Line 4");
    println!("   Contains 'Line 1': {has_line1}");
    println!("   Contains 'Line 4': {has_line4}");

    // Example 7: Screen queries
    println!("\n7. Querying screen content...");

    let mut screen = Screen::new(10, 40);
    screen.process_str("Username: admin\n");
    screen.process_str("Password: ****\n");
    screen.process_str("Status: Connected\n");

    let query = screen.query();
    println!("   Contains 'admin': {}", query.contains("admin"));
    println!("   Contains 'Status': {}", query.contains("Status"));
    println!("   Contains 'error': {}", query.contains("error"));

    // Example 8: Raw buffer access
    println!("\n8. Direct buffer access...");

    let mut buffer = ScreenBuffer::new(5, 20);
    buffer.write_char('H');
    buffer.write_char('i');
    buffer.write_char('!');

    println!("   Wrote 3 characters directly to buffer");
    println!(
        "   Cursor at: ({}, {})",
        buffer.cursor().row,
        buffer.cursor().col
    );

    // Example 9: Resize
    println!("\n9. Screen resizing...");

    let mut screen = Screen::new(24, 80);
    screen.process_str("Original content");
    println!("   Original size: {}x{}", screen.rows(), screen.cols());

    screen.resize(40, 120);
    println!("   New size: {}x{}", screen.rows(), screen.cols());

    // Example 10: Practical example - parsing program output
    println!("\n10. Parsing program output...");

    let mut screen = Screen::new(24, 80);

    // Simulate a program that uses cursor positioning
    let program_output = concat!(
        "\x1b[2J\x1b[H", // Clear screen, home cursor
        "┌────────────────────┐\n",
        "│  System Status     │\n",
        "├────────────────────┤\n",
        "│ CPU: 45%           │\n",
        "│ MEM: 62%           │\n",
        "│ DISK: 78%          │\n",
        "└────────────────────┘\n",
    );

    screen.process_str(program_output);

    let text = screen.text();
    println!("   Parsed TUI output:");
    println!("   Contains 'CPU': {}", text.contains("CPU"));
    println!("   Contains 'MEM': {}", text.contains("MEM"));
    println!("   Contains 'DISK': {}", text.contains("DISK"));

    println!("\nScreen buffer examples completed successfully!");
}
