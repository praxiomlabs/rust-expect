//! Large output handling example.
//!
//! This example demonstrates handling large amounts of output,
//! buffer management, and backpressure handling.
//!
//! Run with: `cargo run --example large_output`

use std::time::Duration;

use rust_expect::prelude::*;
use rust_expect::util::Backpressure;

#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() -> Result<()> {
    println!("rust-expect Large Output Handling Example");
    println!("=========================================\n");

    // Example 1: Basic large output
    println!("1. Handling large output...");

    let mut session = Session::spawn("/bin/sh", &[]).await?;
    session
        .expect_timeout(Pattern::shell_prompt(), Duration::from_secs(2))
        .await?;

    // Generate a moderate amount of output
    session
        .send_line("for i in $(seq 1 100); do echo \"Line $i: Some sample output data\"; done")
        .await?;

    // Wait for all output and the next prompt
    session
        .expect_timeout(Pattern::shell_prompt(), Duration::from_secs(5))
        .await?;

    let buffer = session.buffer();
    println!("   Buffer size: {} bytes", buffer.len());
    println!("   Contains ~100 lines of output");

    // Example 2: Buffer management
    println!("\n2. Buffer management...");

    // Clear the buffer
    session.clear_buffer();
    println!("   Buffer cleared");
    println!(
        "   Buffer size after clear: {} bytes",
        session.buffer().len()
    );

    // Generate new output
    session.send_line("echo 'Fresh start'").await?;
    session.expect("Fresh start").await?;
    println!("   New output captured");

    // Example 3: Custom buffer configuration
    println!("\n3. Custom buffer configuration...");

    let config = SessionConfig {
        buffer: BufferConfig {
            max_size: 1024 * 1024, // 1 MB buffer
            search_window: Some(8192),
            ring_buffer: true,
        },
        ..Default::default()
    };

    let session2 = Session::spawn_with_config("/bin/sh", &[], config).await?;
    println!("   Created session with 1MB buffer");
    println!(
        "   Buffer max size: {} bytes",
        session2.config().buffer.max_size
    );
    println!(
        "   Search window: {:?}",
        session2.config().buffer.search_window
    );
    println!("   Ring buffer: {}", session2.config().buffer.ring_buffer);
    drop(session2);

    // Example 4: Backpressure handling
    println!("\n4. Backpressure handling...");

    let backpressure = Backpressure::new(1000); // 1000 bytes max buffer

    println!("   Created backpressure controller: 1000 bytes max");

    // Simulate acquiring and releasing buffer space
    for i in 1..=5 {
        let amount = 200; // Try to acquire 200 bytes
        if backpressure.try_acquire(amount) {
            println!("   Request {i}: acquired {amount} bytes");
        } else {
            println!("   Request {i}: buffer full");
        }
    }

    // Release some space
    backpressure.release(400);
    println!("   Released 400 bytes");

    // Now we can acquire more
    if backpressure.try_acquire(200) {
        println!("   After release: acquired 200 more bytes");
    }

    // Example 5: Processing large output in chunks
    println!("\n5. Processing output incrementally...");

    // Send a command that produces output over time
    session
        .send_line("for i in 1 2 3 4 5; do echo \"Chunk $i\"; sleep 0.1; done")
        .await?;

    // Process chunks as they arrive
    let mut chunks_received = 0;
    for _ in 0..5 {
        match session
            .expect_timeout("Chunk", Duration::from_millis(500))
            .await
        {
            Ok(m) => {
                chunks_received += 1;
                println!("   Received: {}", m.matched.trim());
            }
            Err(_) => break,
        }
    }
    println!("   Total chunks: {chunks_received}");

    // Wait for the final prompt
    session
        .expect_timeout(Pattern::shell_prompt(), Duration::from_secs(2))
        .await?;

    // Example 6: Ring buffer for memory efficiency
    println!("\n6. Ring buffer for memory efficiency...");

    let mut ring_buffer = RingBuffer::new(4096);
    println!(
        "   Ring buffer created with {} byte max size",
        ring_buffer.max_size()
    );

    // Append some data
    ring_buffer.append(b"Hello, ");
    ring_buffer.append(b"World!");
    println!("   Written data, buffer len: {}", ring_buffer.len());

    // Ring buffers prevent unbounded memory growth
    println!("   Ring buffers automatically discard old data when full");

    // Example 7: Buffer configuration options
    println!("\n7. Buffer configuration options...");

    let small_buffer = BufferConfig::new(1024).search_window(512).ring_buffer(true);

    println!(
        "   Small buffer: {} bytes, window: {:?}",
        small_buffer.max_size, small_buffer.search_window
    );

    let large_buffer = BufferConfig::new(10 * 1024 * 1024) // 10 MB
        .ring_buffer(false); // Keep all data

    println!(
        "   Large buffer: {} MB, ring: {}",
        large_buffer.max_size / 1024 / 1024,
        large_buffer.ring_buffer
    );

    // Clean up
    session.send_line("exit").await?;
    session.wait().await?;

    println!("\nLarge output handling examples completed successfully!");
    Ok(())
}
