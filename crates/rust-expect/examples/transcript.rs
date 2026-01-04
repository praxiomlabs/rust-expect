//! Session transcript example.
//!
//! This example demonstrates recording and playing back
//! terminal sessions using the transcript module.
//!
//! Run with: `cargo run --example transcript`

use rust_expect::prelude::*;
use rust_expect::transcript::{
    EventType, PlaybackOptions, PlaybackSpeed, Player, PlayerState, RecorderBuilder,
    Transcript, TranscriptEvent, TranscriptMetadata,
};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    println!("rust-expect Session Transcript Example");
    println!("======================================\n");

    // Example 1: Creating a transcript manually
    println!("1. Creating a transcript...");

    let metadata = TranscriptMetadata::new(80, 24)
        .with_title("Demo Session")
        .with_command("/bin/sh");

    let mut transcript = Transcript::new(metadata);

    // Add events (note: Duration is first argument, then data)
    transcript.push(TranscriptEvent::output(Duration::from_millis(0), b"$ "));
    transcript.push(TranscriptEvent::input(Duration::from_millis(100), b"echo hello\n"));
    transcript.push(TranscriptEvent::output(Duration::from_millis(150), b"echo hello\n"));
    transcript.push(TranscriptEvent::output(Duration::from_millis(200), b"hello\n"));
    transcript.push(TranscriptEvent::output(Duration::from_millis(250), b"$ "));

    println!("   Created transcript with {} events", transcript.events.len());
    println!("   Duration: {:?}", transcript.duration());

    // Example 2: Recording a session
    println!("\n2. Recording a live session...");

    let recorder = RecorderBuilder::new()
        .title("Live Recording")
        .command("/bin/sh")
        .build();

    println!("   Recorder created");
    println!("   Recording: {}", recorder.is_recording());

    // Example 3: Playback configuration
    println!("\n3. Playback options...");

    let _options = PlaybackOptions::new()
        .with_speed(PlaybackSpeed::Speed(2.0)); // 2x speed

    println!("   Speed: 2.0x (double)");
    println!("   Available speeds:");
    println!("   - Speed(0.5) - half speed");
    println!("   - Realtime - normal speed");
    println!("   - Speed(2.0) - double speed");
    println!("   - Instant - no delays");

    // Example 4: Player states
    println!("\n4. Player states...");

    let states = [
        PlayerState::Stopped,
        PlayerState::Playing,
        PlayerState::Paused,
        PlayerState::Finished,
    ];

    for state in states {
        println!("   State: {state:?}");
    }

    // Example 5: Event types
    println!("\n5. Event types...");

    let event_types = [
        (EventType::Input, "User input to the terminal"),
        (EventType::Output, "Output from the terminal"),
        (EventType::Resize, "Terminal resize event"),
    ];

    for (event_type, description) in event_types {
        println!("   {event_type:?}: {description}");
    }

    // Example 6: Real session recording
    println!("\n6. Recording a real session...");

    let recorder = RecorderBuilder::new()
        .title("Real Session")
        .size(80, 24)
        .build();

    // Spawn and record a session
    let mut session = Session::spawn("/bin/sh", &[]).await?;
    session.expect_timeout(Pattern::regex(r"[$#>]").unwrap(), Duration::from_secs(2)).await?;

    // Record the initial output
    recorder.record_output(b"$ ");

    // Send a command and record
    recorder.record_input(b"echo 'Recorded!'\n");
    session.send_line("echo 'Recorded!'").await?;

    let m = session.expect("Recorded!").await?;
    recorder.record_output(m.matched.as_bytes());

    session.expect_timeout(Pattern::regex(r"[$#>]").unwrap(), Duration::from_secs(2)).await?;
    recorder.record_output(b"$ ");

    // Finish recording
    let transcript = recorder.into_transcript();
    println!("   Recorded {} events", transcript.events.len());
    println!("   Total duration: {:?}", transcript.duration());

    // Clean up session
    session.send_line("exit").await?;
    session.wait().await?;

    // Example 7: Transcript analysis
    println!("\n7. Analyzing transcript...");

    let input_count = transcript.events
        .iter()
        .filter(|e| matches!(e.event_type, EventType::Input))
        .count();

    let output_count = transcript.events
        .iter()
        .filter(|e| matches!(e.event_type, EventType::Output))
        .count();

    println!("   Input events: {input_count}");
    println!("   Output events: {output_count}");

    // Example 8: Transcript serialization
    println!("\n8. Transcript formats...");

    println!("   Supported formats:");
    println!("   - Native JSON format");
    println!("   - asciicast v2 (asciinema compatible)");

    // Example showing how to serialize (conceptual)
    println!("\n   // Save as JSON:");
    println!("   let json = serde_json::to_string(&transcript)?;");
    println!("   ");
    println!("   // Save as asciicast:");
    println!("   write_asciicast(&mut file, &transcript)?;");

    // Example 9: Use cases
    println!("\n9. Use cases for transcripts...");

    let use_cases = [
        "Debugging - Replay failed automation",
        "Documentation - Generate animated demos",
        "Testing - Compare expected vs actual output",
        "Training - Create interactive tutorials",
        "Auditing - Record admin sessions",
    ];

    for use_case in use_cases {
        println!("   - {use_case}");
    }

    // Example 10: Player for playback
    println!("\n10. Session playback...");

    // Create a player for the transcript
    let player = Player::new(&transcript);
    println!("   Player created");
    println!("   State: {:?}", player.state());

    // In a real application, you would:
    // player.play();
    // player.play_to(&mut stdout)?;

    println!("\nTranscript examples completed successfully!");
    Ok(())
}
