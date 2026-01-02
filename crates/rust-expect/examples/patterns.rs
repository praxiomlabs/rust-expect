//! Pattern matching examples.
//!
//! This example demonstrates various pattern matching capabilities.
//!
//! Run with: `cargo run --example patterns`

use rust_expect::expect::Pattern;

fn main() {
    println!("Pattern Matching Examples\n");

    // Literal patterns
    println!("=== Literal Patterns ===");
    let pattern = Pattern::literal("login:");
    println!("Pattern: 'login:'");
    let result = pattern.matches("Please enter login:");
    println!("  Matches 'Please enter login:': {}", result.is_some());
    if let Some(m) = result {
        println!("    Position: {}..{}", m.start, m.end);
    }

    let result = pattern.matches("username:");
    println!("  Matches 'username:': {}", result.is_some());

    // Regex patterns
    println!("\n=== Regex Patterns ===");
    let pattern = Pattern::regex(r"\d{3}-\d{4}").unwrap();
    println!("Pattern: r'\\d{{3}}-\\d{{4}}'");
    let result = pattern.matches("Call 555-1234");
    println!("  Matches '555-1234': {}", result.is_some());
    if let Some(m) = result {
        println!("    Position: {}..{}", m.start, m.end);
    }

    let result = pattern.matches("no number");
    println!("  Matches 'no number': {}", result.is_some());

    // Glob patterns
    println!("\n=== Glob Patterns ===");
    let pattern = Pattern::glob("*.txt");
    println!("Pattern: '*.txt'");
    println!("  Matches 'file.txt': {}", pattern.matches("file.txt").is_some());
    println!("  Matches 'file.rs': {}", pattern.matches("file.rs").is_some());

    // Special patterns
    println!("\n=== Special Patterns ===");
    let eof_pattern = Pattern::eof();
    println!("EOF pattern is_eof: {}", eof_pattern.is_eof());

    let timeout_pattern = Pattern::timeout(std::time::Duration::from_secs(5));
    println!("Timeout pattern is_timeout: {}", timeout_pattern.is_timeout());
    if let Some(d) = timeout_pattern.timeout_duration() {
        println!("  Duration: {d:?}");
    }

    // Extracting captures from regex
    println!("\n=== Extracting Captures ===");
    let pattern = Pattern::regex(r"user: (\w+)").unwrap();
    if let Some(m) = pattern.matches("user: alice") {
        println!("Found match at {}..{}", m.start, m.end);
        if !m.captures.is_empty() {
            println!("  Captures: {:?}", m.captures);
        }
    }

    println!("\nExamples completed!");
}
