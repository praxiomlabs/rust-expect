//! Integration tests for interactive mode.

use rust_expect::{
    InteractAction, InteractEndReason, InteractResult, InteractionMode, TerminalMode, TerminalState,
};
use std::time::Duration;

#[test]
fn interaction_mode_new() {
    let mode = InteractionMode::new();
    assert!(!mode.local_echo);
    assert!(mode.crlf);
}

#[test]
fn interaction_mode_default() {
    let mode = InteractionMode::default();
    assert!(!mode.local_echo);
    assert!(mode.crlf);
    assert_eq!(mode.buffer_size, 4096);
}

#[test]
fn interaction_mode_builder() {
    let mode = InteractionMode::new()
        .with_local_echo(true)
        .with_crlf(false)
        .with_buffer_size(8192)
        .with_read_timeout(Duration::from_millis(50));

    assert!(mode.local_echo);
    assert!(!mode.crlf);
    assert_eq!(mode.buffer_size, 8192);
    assert_eq!(mode.read_timeout, Duration::from_millis(50));
}

#[test]
fn interaction_mode_exit_char() {
    let mode = InteractionMode::new().with_exit_char(Some(0x1d)); // Ctrl+]

    assert!(mode.is_exit_char(0x1d));
    assert!(!mode.is_exit_char(0x03));
}

#[test]
fn interaction_mode_escape_char() {
    let mode = InteractionMode::new().with_escape_char(Some(0x1e)); // Ctrl+^

    assert!(mode.is_escape_char(0x1e));
    assert!(!mode.is_escape_char(0x03));
}

#[test]
fn interaction_mode_display() {
    let mode = InteractionMode::new();
    assert!(!format!("{:?}", mode).is_empty());
}

#[test]
fn terminal_mode_raw() {
    let mode = TerminalMode::Raw;
    assert!(!format!("{:?}", mode).is_empty());
}

#[test]
fn terminal_mode_cooked() {
    let mode = TerminalMode::Cooked;
    assert!(!format!("{:?}", mode).is_empty());
}

#[test]
fn terminal_mode_cbreak() {
    let mode = TerminalMode::Cbreak;
    assert!(!format!("{:?}", mode).is_empty());
}

#[test]
fn terminal_mode_equality() {
    assert_eq!(TerminalMode::Raw, TerminalMode::Raw);
    assert_ne!(TerminalMode::Raw, TerminalMode::Cooked);
}

#[test]
fn terminal_state_default() {
    let state = TerminalState::default();
    assert_eq!(state.mode, TerminalMode::Cooked);
    assert!(state.echo);
    assert!(state.canonical);
}

#[test]
fn terminal_state_clone() {
    let state1 = TerminalState::default();
    let state2 = state1.clone();
    assert_eq!(format!("{:?}", state1), format!("{:?}", state2));
}

// Tests for new pattern hook types

#[test]
fn interact_action_continue() {
    let action = InteractAction::Continue;
    assert!(!format!("{:?}", action).is_empty());
}

#[test]
fn interact_action_send_string() {
    let action = InteractAction::send("hello");
    match action {
        InteractAction::Send(data) => assert_eq!(data, b"hello"),
        _ => panic!("Expected Send action"),
    }
}

#[test]
fn interact_action_send_bytes() {
    let action = InteractAction::send_bytes(vec![1, 2, 3]);
    match action {
        InteractAction::Send(data) => assert_eq!(data, vec![1, 2, 3]),
        _ => panic!("Expected Send action"),
    }
}

#[test]
fn interact_action_stop() {
    let action = InteractAction::Stop;
    assert!(!format!("{:?}", action).is_empty());
}

#[test]
fn interact_action_error() {
    let action = InteractAction::Error("test error".to_string());
    match action {
        InteractAction::Error(msg) => assert_eq!(msg, "test error"),
        _ => panic!("Expected Error action"),
    }
}

#[test]
fn interact_end_reason_pattern_stop() {
    let reason = InteractEndReason::PatternStop { pattern_index: 5 };
    match reason {
        InteractEndReason::PatternStop { pattern_index } => assert_eq!(pattern_index, 5),
        _ => panic!("Expected PatternStop"),
    }
}

#[test]
fn interact_end_reason_escape() {
    let reason = InteractEndReason::Escape;
    assert!(!format!("{:?}", reason).is_empty());
}

#[test]
fn interact_end_reason_timeout() {
    let reason = InteractEndReason::Timeout;
    assert!(!format!("{:?}", reason).is_empty());
}

#[test]
fn interact_end_reason_eof() {
    let reason = InteractEndReason::Eof;
    assert!(!format!("{:?}", reason).is_empty());
}

#[test]
fn interact_end_reason_error() {
    let reason = InteractEndReason::Error("connection lost".to_string());
    match reason {
        InteractEndReason::Error(msg) => assert_eq!(msg, "connection lost"),
        _ => panic!("Expected Error"),
    }
}

#[test]
fn interact_result_struct() {
    let result = InteractResult {
        reason: InteractEndReason::Timeout,
        buffer: "test output".to_string(),
    };
    assert_eq!(result.buffer, "test output");
    assert!(!format!("{:?}", result.reason).is_empty());
}
