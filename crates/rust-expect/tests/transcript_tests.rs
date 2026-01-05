//! Integration tests for transcript recording and playback.

use std::time::Duration;

use rust_expect::transcript::{
    EventType, PlaybackOptions, PlaybackSpeed, PlayerState, TranscriptMetadata,
};
use rust_expect::{Player, Recorder, Transcript, TranscriptEvent};

#[test]
fn transcript_metadata_new() {
    let metadata = TranscriptMetadata::new(80, 24);
    assert_eq!(metadata.width, 80);
    assert_eq!(metadata.height, 24);
}

#[test]
fn transcript_metadata_with_command() {
    let metadata = TranscriptMetadata::new(80, 24).with_command("bash -l");

    assert_eq!(metadata.command, Some("bash -l".to_string()));
}

#[test]
fn transcript_metadata_with_title() {
    let metadata = TranscriptMetadata::new(80, 24).with_title("Demo Session");

    assert_eq!(metadata.title, Some("Demo Session".to_string()));
}

#[test]
fn transcript_new() {
    let metadata = TranscriptMetadata::new(80, 24);
    let transcript = Transcript::new(metadata);

    assert!(transcript.events.is_empty());
}

#[test]
fn transcript_push_events() {
    let mut transcript = Transcript::new(TranscriptMetadata::new(80, 24));

    transcript.push(TranscriptEvent::output(Duration::ZERO, b"Hello"));
    transcript.push(TranscriptEvent::input(Duration::from_millis(100), b"world"));

    assert_eq!(transcript.events.len(), 2);
}

#[test]
fn transcript_event_output() {
    let event = TranscriptEvent::output(Duration::from_millis(50), b"test data");

    assert_eq!(event.event_type, EventType::Output);
    assert_eq!(event.timestamp, Duration::from_millis(50));
    assert_eq!(event.data, b"test data".to_vec());
}

#[test]
fn transcript_event_input() {
    let event = TranscriptEvent::input(Duration::from_millis(100), b"user input");

    assert_eq!(event.event_type, EventType::Input);
    assert_eq!(event.data, b"user input".to_vec());
}

#[test]
fn transcript_event_resize() {
    let event = TranscriptEvent::resize(Duration::from_millis(200), 120, 40);

    assert_eq!(event.event_type, EventType::Resize);
    assert_eq!(event.data, b"120x40".to_vec());
}

#[test]
fn transcript_event_marker() {
    let event = TranscriptEvent::marker(Duration::from_secs(1), "checkpoint");

    assert_eq!(event.event_type, EventType::Marker);
    assert_eq!(event.data, b"checkpoint".to_vec());
}

#[test]
fn transcript_duration() {
    let mut transcript = Transcript::new(TranscriptMetadata::new(80, 24));
    transcript.push(TranscriptEvent::output(Duration::ZERO, b"start"));
    transcript.push(TranscriptEvent::output(Duration::from_secs(5), b"end"));

    assert_eq!(transcript.duration(), Duration::from_secs(5));
}

#[test]
fn transcript_output_text() {
    let mut transcript = Transcript::new(TranscriptMetadata::new(80, 24));
    transcript.push(TranscriptEvent::output(Duration::ZERO, b"hello "));
    transcript.push(TranscriptEvent::output(
        Duration::from_millis(100),
        b"world",
    ));

    assert_eq!(transcript.output_text(), "hello world");
}

#[test]
fn transcript_input_text() {
    let mut transcript = Transcript::new(TranscriptMetadata::new(80, 24));
    transcript.push(TranscriptEvent::input(Duration::ZERO, b"command1\n"));
    transcript.push(TranscriptEvent::input(
        Duration::from_millis(100),
        b"command2\n",
    ));

    assert_eq!(transcript.input_text(), "command1\ncommand2\n");
}

#[test]
fn transcript_filter() {
    let mut transcript = Transcript::new(TranscriptMetadata::new(80, 24));
    transcript.push(TranscriptEvent::output(Duration::ZERO, b"prompt> "));
    transcript.push(TranscriptEvent::input(
        Duration::from_millis(100),
        b"command\n",
    ));
    transcript.push(TranscriptEvent::output(
        Duration::from_millis(200),
        b"output\n",
    ));

    let outputs = transcript.filter(EventType::Output);
    assert_eq!(outputs.len(), 2);

    let inputs = transcript.filter(EventType::Input);
    assert_eq!(inputs.len(), 1);
}

#[test]
fn recorder_new() {
    let recorder = Recorder::new(80, 24);
    assert!(recorder.is_recording());
    assert_eq!(recorder.event_count(), 0);
}

#[test]
fn recorder_record_events() {
    let recorder = Recorder::new(80, 24);

    recorder.record_output(b"prompt> ");
    recorder.record_input(b"command\n");
    recorder.record_output(b"output\n");

    assert_eq!(recorder.event_count(), 3);
}

#[test]
fn recorder_stop() {
    let mut recorder = Recorder::new(80, 24);
    recorder.record_output(b"before");
    recorder.stop();
    recorder.record_output(b"after"); // Should be ignored

    assert_eq!(recorder.event_count(), 1);
    assert!(!recorder.is_recording());
}

#[test]
fn recorder_with_max_events() {
    let recorder = Recorder::new(80, 24).with_max_events(2);

    recorder.record_output(b"one");
    recorder.record_output(b"two");
    recorder.record_output(b"three"); // Should be ignored

    assert_eq!(recorder.event_count(), 2);
}

#[test]
fn recorder_elapsed() {
    let recorder = Recorder::new(80, 24);
    std::thread::sleep(Duration::from_millis(10));

    // Elapsed should be at least the sleep duration
    assert!(recorder.elapsed() >= Duration::from_millis(10));
}

#[test]
fn recorder_into_transcript() {
    let recorder = Recorder::new(80, 24);
    recorder.record_output(b"hello");
    recorder.record_input(b"world");

    let transcript = recorder.into_transcript();
    assert_eq!(transcript.events.len(), 2);
}

#[test]
fn player_new() {
    let mut transcript = Transcript::new(TranscriptMetadata::new(80, 24));
    transcript.push(TranscriptEvent::output(Duration::ZERO, b"Hello"));
    transcript.push(TranscriptEvent::output(
        Duration::from_millis(100),
        b"World",
    ));

    let player = Player::new(&transcript);

    assert_eq!(player.state(), PlayerState::Stopped);
    assert_eq!(player.total_events(), 2);
    assert_eq!(player.position(), 0);
}

#[test]
fn player_play_and_next() {
    let mut transcript = Transcript::new(TranscriptMetadata::new(80, 24));
    transcript.push(TranscriptEvent::output(Duration::ZERO, b"event1"));
    transcript.push(TranscriptEvent::output(
        Duration::from_millis(100),
        b"event2",
    ));

    let mut player = Player::new(&transcript);
    player.play();

    assert_eq!(player.state(), PlayerState::Playing);

    let event1 = player.next_event();
    assert!(event1.is_some());
    assert_eq!(event1.unwrap().data, b"event1".to_vec());

    let event2 = player.next_event();
    assert!(event2.is_some());
    assert_eq!(event2.unwrap().data, b"event2".to_vec());

    let event3 = player.next_event();
    assert!(event3.is_none());
    assert_eq!(player.state(), PlayerState::Finished);
}

#[test]
fn player_pause_and_stop() {
    let mut transcript = Transcript::new(TranscriptMetadata::new(80, 24));
    transcript.push(TranscriptEvent::output(Duration::ZERO, b"test"));

    let mut player = Player::new(&transcript);

    player.play();
    assert_eq!(player.state(), PlayerState::Playing);

    player.pause();
    assert_eq!(player.state(), PlayerState::Paused);

    player.stop();
    assert_eq!(player.state(), PlayerState::Stopped);
    assert_eq!(player.position(), 0);
}

#[test]
fn player_seek() {
    let mut transcript = Transcript::new(TranscriptMetadata::new(80, 24));
    for i in 0..5 {
        transcript.push(TranscriptEvent::output(
            Duration::from_millis(i * 100),
            format!("event{i}").as_bytes(),
        ));
    }

    let mut player = Player::new(&transcript);
    player.seek(3);

    assert_eq!(player.position(), 3);
}

#[test]
fn playback_speed_realtime() {
    let speed = PlaybackSpeed::Realtime;
    assert_eq!(speed, PlaybackSpeed::default());
}

#[test]
fn playback_speed_instant() {
    let speed = PlaybackSpeed::Instant;
    assert!(!format!("{speed:?}").is_empty());
}

#[test]
fn playback_options_default() {
    let options = PlaybackOptions::default();
    assert_eq!(options.speed, PlaybackSpeed::Realtime);
    assert!(!options.show_input);
}

#[test]
fn playback_options_builder() {
    let options = PlaybackOptions::new()
        .with_speed(PlaybackSpeed::Speed(2.0))
        .with_max_idle(Duration::from_secs(1))
        .with_show_input(true);

    assert_eq!(options.speed, PlaybackSpeed::Speed(2.0));
    assert_eq!(options.max_idle, Duration::from_secs(1));
    assert!(options.show_input);
}

#[test]
fn player_with_options() {
    let mut transcript = Transcript::new(TranscriptMetadata::new(80, 24));
    transcript.push(TranscriptEvent::output(Duration::ZERO, b"test"));

    let options = PlaybackOptions::new().with_speed(PlaybackSpeed::Instant);
    let player = Player::new(&transcript).with_options(options);

    assert_eq!(player.delay_to_next(), Duration::ZERO);
}
