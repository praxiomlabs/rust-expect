//! Integration tests for send utilities.

use rust_expect::{ControlChar, HumanTypingConfig};
use std::time::Duration;

#[test]
fn control_char_ctrl_c() {
    assert_eq!(ControlChar::CtrlC.as_byte(), 3);
}

#[test]
fn control_char_ctrl_d() {
    assert_eq!(ControlChar::CtrlD.as_byte(), 4);
}

#[test]
fn control_char_ctrl_z() {
    assert_eq!(ControlChar::CtrlZ.as_byte(), 26);
}

#[test]
fn control_char_escape() {
    let esc = ControlChar::Escape;
    assert_eq!(esc.as_byte(), 27);
}

#[test]
fn control_char_tab() {
    // Tab is Ctrl+I
    let tab = ControlChar::CtrlI;
    assert_eq!(tab.as_byte(), 9);
}

#[test]
fn control_char_enter() {
    // Enter/CR is Ctrl+M
    let enter = ControlChar::CtrlM;
    assert_eq!(enter.as_byte(), 13);
}

#[test]
fn control_char_backspace() {
    // Backspace is Ctrl+H
    let backspace = ControlChar::CtrlH;
    assert_eq!(backspace.as_byte(), 8);
}

#[test]
fn control_char_from_char() {
    assert_eq!(ControlChar::from_char('c'), Some(ControlChar::CtrlC));
    assert_eq!(ControlChar::from_char('C'), Some(ControlChar::CtrlC));
    assert_eq!(ControlChar::from_char('d'), Some(ControlChar::CtrlD));
    assert_eq!(ControlChar::from_char('z'), Some(ControlChar::CtrlZ));
    assert_eq!(ControlChar::from_char('?'), None);
}

#[test]
fn control_char_into_u8() {
    let byte: u8 = ControlChar::CtrlC.into();
    assert_eq!(byte, 3);
}

#[test]
fn basic_send_format_line() {
    // Test that line formatting includes line ending
    let line = "echo hello";
    let formatted = format!("{line}\n");
    assert!(formatted.ends_with('\n'));
}

#[test]
fn human_typing_config_default() {
    let config = HumanTypingConfig::default();
    assert_eq!(config.base_delay, Duration::from_millis(100));
    assert_eq!(config.variance, Duration::from_millis(50));
}

#[test]
fn human_typing_config_custom() {
    let config = HumanTypingConfig::new(Duration::from_millis(50), Duration::from_millis(25))
        .typo_chance(0.02)
        .correction_chance(0.9);

    assert_eq!(config.base_delay, Duration::from_millis(50));
    assert_eq!(config.variance, Duration::from_millis(25));
}

#[test]
fn control_char_all_variants() {
    // Test all control characters have valid byte values
    let chars = [
        ControlChar::CtrlA,
        ControlChar::CtrlB,
        ControlChar::CtrlC,
        ControlChar::CtrlD,
        ControlChar::CtrlE,
        ControlChar::CtrlF,
        ControlChar::CtrlG,
        ControlChar::CtrlH,
        ControlChar::CtrlI,
        ControlChar::CtrlJ,
        ControlChar::CtrlK,
        ControlChar::CtrlL,
        ControlChar::CtrlM,
        ControlChar::CtrlN,
        ControlChar::CtrlO,
        ControlChar::CtrlP,
        ControlChar::CtrlQ,
        ControlChar::CtrlR,
        ControlChar::CtrlS,
        ControlChar::CtrlT,
        ControlChar::CtrlU,
        ControlChar::CtrlV,
        ControlChar::CtrlW,
        ControlChar::CtrlX,
        ControlChar::CtrlY,
        ControlChar::CtrlZ,
        ControlChar::Escape,
    ];

    for (i, ctrl) in chars.iter().enumerate() {
        let expected = if i < 26 { (i + 1) as u8 } else { 27 };
        assert_eq!(ctrl.as_byte(), expected);
    }
}
