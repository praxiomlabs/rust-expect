//! Screen scrollback example.
//!
//! Demonstrates observing content that scrolls off the top of a virtual
//! terminal: a bounded scrollback history, a full-history text query, and the
//! lossless per-row callback.
//!
//! Run with: `cargo run --example scrollback --features screen`

#[cfg(feature = "screen")]
use rust_expect::screen::Screen;

fn main() {
    println!("rust-expect Screen Scrollback Example");
    println!("=====================================\n");

    #[cfg(not(feature = "screen"))]
    {
        println!("This example requires the 'screen' feature.");
        println!("Run with: cargo run --example scrollback --features screen");
    }

    #[cfg(feature = "screen")]
    run();
}

#[cfg(feature = "screen")]
fn run() {
    use std::sync::{Arc, Mutex};

    // A small 4-row viewport with room for 100 lines of scrollback.
    let mut screen = Screen::with_scrollback(4, 20, 100);

    // The lossless path: capture every row as it scrolls off, regardless of
    // the ring bound. Register before driving output.
    let evicted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = evicted.clone();
    screen.on_line_scrolled_out(move |row| sink.lock().unwrap().push(row.text()));

    // Emit 10 lines into the 4-row viewport, so 6 scroll off the top.
    for i in 1..=10 {
        screen.process_str(&format!("line {i}\r\n"));
    }

    // The viewport only shows the last few lines...
    println!("Viewport (text):");
    for line in screen.text().lines() {
        println!("  {line}");
    }

    // ...but scrollback retains what scrolled off, oldest first.
    println!("\nScrollback rows ({}):", screen.scrollback().count());
    for row in screen.scrollback() {
        println!("  {}", row.text());
    }

    // full_text() stitches history + viewport, identical to what text() would
    // have returned had nothing scrolled.
    println!("\nfull_text() (history + viewport):");
    for line in screen.full_text().lines() {
        println!("  {line}");
    }

    // The callback saw every evicted row, even ones a small ring might drop.
    println!("\nStreamed via on_line_scrolled_out:");
    for line in evicted.lock().unwrap().iter() {
        println!("  {line}");
    }
}
