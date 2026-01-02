//! Integration tests for interactive mode.

use rust_expect::{InteractionMode, TerminalMode, TerminalState};
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
    let mode = InteractionMode::new()
        .with_exit_char(Some(0x1d)); // Ctrl+]

    assert!(mode.is_exit_char(0x1d));
    assert!(!mode.is_exit_char(0x03));
}

#[test]
fn interaction_mode_escape_char() {
    let mode = InteractionMode::new()
        .with_escape_char(Some(0x1e)); // Ctrl+^

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
