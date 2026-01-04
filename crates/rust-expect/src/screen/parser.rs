//! ANSI escape sequence parser.
//!
//! This module provides parsing for ANSI escape sequences used in
//! terminal output, including cursor movement, colors, and text attributes.

use super::buffer::{Attributes, Color};

/// Parsed ANSI escape sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnsiSequence {
    /// Cursor up (CUU).
    CursorUp(u16),
    /// Cursor down (CUD).
    CursorDown(u16),
    /// Cursor forward (CUF).
    CursorForward(u16),
    /// Cursor backward (CUB).
    CursorBackward(u16),
    /// Cursor next line (CNL) - move to beginning of line n lines down.
    CursorNextLine(u16),
    /// Cursor previous line (CPL) - move to beginning of line n lines up.
    CursorPrevLine(u16),
    /// Cursor horizontal absolute (CHA) - move cursor to column n.
    CursorColumn(u16),
    /// Vertical position absolute (VPA) - move cursor to row n.
    CursorRow(u16),
    /// Cursor position (CUP).
    CursorPosition {
        /// Row position (1-based).
        row: u16,
        /// Column position (1-based).
        col: u16,
    },
    /// Erase in display (ED).
    EraseDisplay(EraseMode),
    /// Erase in line (EL).
    EraseLine(EraseMode),
    /// Erase characters (ECH) - erase n characters from cursor.
    EraseChars(u16),
    /// Select graphic rendition (SGR).
    SetGraphics(Vec<u16>),
    /// Scroll up (SU).
    ScrollUp(u16),
    /// Scroll down (SD).
    ScrollDown(u16),
    /// Reverse index (RI) - move cursor up, scroll down if at top.
    ReverseIndex,
    /// Index (IND) - move cursor down, scroll up if at bottom.
    Index,
    /// Next line (NEL) - move to start of next line, scroll if at bottom.
    NextLine,
    /// Save cursor position (DECSC).
    SaveCursor,
    /// Restore cursor position (DECRC).
    RestoreCursor,
    /// Set scroll region (DECSTBM).
    SetScrollRegion {
        /// Top row of scroll region (1-based).
        top: u16,
        /// Bottom row of scroll region (1-based).
        bottom: u16,
    },
    /// Show cursor (DECTCEM).
    ShowCursor,
    /// Hide cursor (DECTCEM).
    HideCursor,
    /// Insert lines (IL).
    InsertLines(u16),
    /// Delete lines (DL).
    DeleteLines(u16),
    /// Insert characters (ICH).
    InsertChars(u16),
    /// Delete characters (DCH).
    DeleteChars(u16),
    /// Repeat previous character (REP).
    RepeatChar(u16),
    /// Reset terminal.
    Reset,
    /// Unknown or unsupported sequence.
    Unknown(String),
}

/// Mode for erase operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EraseMode {
    /// Erase from cursor to end.
    ToEnd,
    /// Erase from start to cursor.
    ToStart,
    /// Erase entire area.
    All,
}

impl From<u16> for EraseMode {
    fn from(n: u16) -> Self {
        match n {
            0 => Self::ToEnd,
            1 => Self::ToStart,
            _ => Self::All,
        }
    }
}

/// ANSI sequence parser.
#[derive(Clone)]
pub struct AnsiParser {
    state: ParserState,
    params: Vec<u16>,
    intermediate: String,
    current_param: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParserState {
    Ground,
    Escape,
    CsiEntry,
    CsiParam,
    CsiIntermediate,
    OscString,
}

impl Default for AnsiParser {
    fn default() -> Self {
        Self::new()
    }
}

impl AnsiParser {
    /// Create a new parser.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: ParserState::Ground,
            params: Vec::new(),
            intermediate: String::new(),
            current_param: None,
        }
    }

    /// Reset the parser state.
    pub fn reset(&mut self) {
        self.state = ParserState::Ground;
        self.params.clear();
        self.intermediate.clear();
        self.current_param = None;
    }

    /// Parse a byte and return any completed sequences.
    pub fn parse(&mut self, byte: u8) -> Option<ParseResult> {
        match self.state {
            ParserState::Ground => self.ground(byte),
            ParserState::Escape => self.escape(byte),
            ParserState::CsiEntry => self.csi_entry(byte),
            ParserState::CsiParam => self.csi_param(byte),
            ParserState::CsiIntermediate => self.csi_intermediate(byte),
            ParserState::OscString => self.osc_string(byte),
        }
    }

    fn ground(&mut self, byte: u8) -> Option<ParseResult> {
        match byte {
            0x1b => {
                self.state = ParserState::Escape;
                None
            }
            0x00..=0x1a | 0x1c..=0x1f => {
                // Control characters
                Some(ParseResult::Control(byte))
            }
            _ => Some(ParseResult::Print(byte as char)),
        }
    }

    fn escape(&mut self, byte: u8) -> Option<ParseResult> {
        match byte {
            b'[' => {
                self.state = ParserState::CsiEntry;
                self.params.clear();
                self.intermediate.clear();
                self.current_param = None;
                None
            }
            b']' => {
                self.state = ParserState::OscString;
                None
            }
            b'7' => {
                self.reset();
                Some(ParseResult::Sequence(AnsiSequence::SaveCursor))
            }
            b'8' => {
                self.reset();
                Some(ParseResult::Sequence(AnsiSequence::RestoreCursor))
            }
            b'c' => {
                self.reset();
                Some(ParseResult::Sequence(AnsiSequence::Reset))
            }
            b'D' => {
                // IND - Index: move cursor down, scroll up if at bottom
                self.reset();
                Some(ParseResult::Sequence(AnsiSequence::Index))
            }
            b'E' => {
                // NEL - Next Line: move to start of next line
                self.reset();
                Some(ParseResult::Sequence(AnsiSequence::NextLine))
            }
            b'M' => {
                // RI - Reverse Index: move cursor up, scroll down if at top
                self.reset();
                Some(ParseResult::Sequence(AnsiSequence::ReverseIndex))
            }
            _ => {
                self.reset();
                Some(ParseResult::Sequence(AnsiSequence::Unknown(format!(
                    "ESC {}",
                    byte as char
                ))))
            }
        }
    }

    fn csi_entry(&mut self, byte: u8) -> Option<ParseResult> {
        match byte {
            b'0'..=b'9' => {
                self.current_param = Some(u16::from(byte - b'0'));
                self.state = ParserState::CsiParam;
                None
            }
            b';' => {
                self.params.push(0);
                self.state = ParserState::CsiParam;
                None
            }
            b'?' => {
                self.intermediate.push('?');
                None
            }
            b'>' => {
                self.intermediate.push('>');
                None
            }
            b' '..=b'/' => {
                self.intermediate.push(byte as char);
                self.state = ParserState::CsiIntermediate;
                None
            }
            b'@'..=b'~' => {
                self.finalize_csi(byte)
            }
            _ => {
                self.reset();
                None
            }
        }
    }

    fn csi_param(&mut self, byte: u8) -> Option<ParseResult> {
        match byte {
            b'0'..=b'9' => {
                let digit = u16::from(byte - b'0');
                self.current_param = Some(
                    self.current_param.unwrap_or(0).saturating_mul(10).saturating_add(digit)
                );
                None
            }
            b';' => {
                self.params.push(self.current_param.unwrap_or(0));
                self.current_param = None;
                None
            }
            b' '..=b'/' => {
                if let Some(p) = self.current_param.take() {
                    self.params.push(p);
                }
                self.intermediate.push(byte as char);
                self.state = ParserState::CsiIntermediate;
                None
            }
            b'@'..=b'~' => {
                if let Some(p) = self.current_param.take() {
                    self.params.push(p);
                }
                self.finalize_csi(byte)
            }
            _ => {
                self.reset();
                None
            }
        }
    }

    fn csi_intermediate(&mut self, byte: u8) -> Option<ParseResult> {
        match byte {
            b' '..=b'/' => {
                self.intermediate.push(byte as char);
                None
            }
            b'@'..=b'~' => self.finalize_csi(byte),
            _ => {
                self.reset();
                None
            }
        }
    }

    fn osc_string(&mut self, byte: u8) -> Option<ParseResult> {
        match byte {
            0x07 | 0x1b => {
                // BEL or ESC terminates OSC
                self.reset();
                None
            }
            _ => None, // Ignore OSC content for now
        }
    }

    fn finalize_csi(&mut self, final_byte: u8) -> Option<ParseResult> {
        let params = std::mem::take(&mut self.params);
        let intermediate = std::mem::take(&mut self.intermediate);
        self.reset();

        let seq = match (final_byte, intermediate.as_str()) {
            // Cursor movement
            (b'A', "") => AnsiSequence::CursorUp(params.first().copied().unwrap_or(1)),
            (b'B', "") => AnsiSequence::CursorDown(params.first().copied().unwrap_or(1)),
            (b'C', "") => AnsiSequence::CursorForward(params.first().copied().unwrap_or(1)),
            (b'D', "") => AnsiSequence::CursorBackward(params.first().copied().unwrap_or(1)),
            (b'E', "") => AnsiSequence::CursorNextLine(params.first().copied().unwrap_or(1)),
            (b'F', "") => AnsiSequence::CursorPrevLine(params.first().copied().unwrap_or(1)),
            (b'G', "") => AnsiSequence::CursorColumn(params.first().copied().unwrap_or(1)),
            (b'd', "") => AnsiSequence::CursorRow(params.first().copied().unwrap_or(1)),
            (b'H' | b'f', "") => AnsiSequence::CursorPosition {
                row: params.first().copied().unwrap_or(1),
                col: params.get(1).copied().unwrap_or(1),
            },
            // Erase operations
            (b'J', "") => AnsiSequence::EraseDisplay(
                params.first().copied().unwrap_or(0).into()
            ),
            (b'K', "") => AnsiSequence::EraseLine(
                params.first().copied().unwrap_or(0).into()
            ),
            (b'X', "") => AnsiSequence::EraseChars(params.first().copied().unwrap_or(1)),
            // Graphics
            (b'm', "") => AnsiSequence::SetGraphics(if params.is_empty() {
                vec![0]
            } else {
                params
            }),
            // Scrolling
            (b'S', "") => AnsiSequence::ScrollUp(params.first().copied().unwrap_or(1)),
            (b'T', "") => AnsiSequence::ScrollDown(params.first().copied().unwrap_or(1)),
            (b'r', "") => AnsiSequence::SetScrollRegion {
                top: params.first().copied().unwrap_or(1),
                bottom: params.get(1).copied().unwrap_or(0),
            },
            // Cursor save/restore
            (b's', "") => AnsiSequence::SaveCursor,
            (b'u', "") => AnsiSequence::RestoreCursor,
            // Line operations
            (b'L', "") => AnsiSequence::InsertLines(params.first().copied().unwrap_or(1)),
            (b'M', "") => AnsiSequence::DeleteLines(params.first().copied().unwrap_or(1)),
            // Character operations
            (b'@', "") => AnsiSequence::InsertChars(params.first().copied().unwrap_or(1)),
            (b'P', "") => AnsiSequence::DeleteChars(params.first().copied().unwrap_or(1)),
            (b'b', "") => AnsiSequence::RepeatChar(params.first().copied().unwrap_or(1)),
            // DEC private modes
            (b'h', "?") if params.first() == Some(&25) => AnsiSequence::ShowCursor,
            (b'l', "?") if params.first() == Some(&25) => AnsiSequence::HideCursor,
            _ => AnsiSequence::Unknown(format!(
                "CSI {}{}{}",
                params.iter().map(std::string::ToString::to_string).collect::<Vec<_>>().join(";"),
                intermediate,
                final_byte as char
            )),
        };

        Some(ParseResult::Sequence(seq))
    }
}

/// Result of parsing a byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseResult {
    /// A printable character.
    Print(char),
    /// A control character.
    Control(u8),
    /// A complete ANSI sequence.
    Sequence(AnsiSequence),
}

/// Apply SGR (Select Graphic Rendition) parameters.
pub fn apply_sgr(params: &[u16], fg: &mut Color, bg: &mut Color, attrs: &mut Attributes) {
    let mut i = 0;
    while i < params.len() {
        match params[i] {
            0 => {
                *fg = Color::Default;
                *bg = Color::Default;
                *attrs = Attributes::empty();
            }
            1 => *attrs |= Attributes::BOLD,
            2 => *attrs |= Attributes::DIM,
            3 => *attrs |= Attributes::ITALIC,
            4 => *attrs |= Attributes::UNDERLINE,
            5 => *attrs |= Attributes::BLINK,
            7 => *attrs |= Attributes::INVERSE,
            8 => *attrs |= Attributes::HIDDEN,
            9 => *attrs |= Attributes::STRIKETHROUGH,
            22 => *attrs &= !(Attributes::BOLD | Attributes::DIM),
            23 => *attrs &= !Attributes::ITALIC,
            24 => *attrs &= !Attributes::UNDERLINE,
            25 => *attrs &= !Attributes::BLINK,
            27 => *attrs &= !Attributes::INVERSE,
            28 => *attrs &= !Attributes::HIDDEN,
            29 => *attrs &= !Attributes::STRIKETHROUGH,
            30..=37 => *fg = Color::from_ansi((params[i] - 30) as u8),
            38 => {
                if i + 2 < params.len() && params[i + 1] == 5 {
                    *fg = Color::Indexed(params[i + 2] as u8);
                    i += 2;
                } else if i + 4 < params.len() && params[i + 1] == 2 {
                    *fg = Color::Rgb(
                        params[i + 2] as u8,
                        params[i + 3] as u8,
                        params[i + 4] as u8,
                    );
                    i += 4;
                }
            }
            39 => *fg = Color::Default,
            40..=47 => *bg = Color::from_ansi((params[i] - 40) as u8),
            48 => {
                if i + 2 < params.len() && params[i + 1] == 5 {
                    *bg = Color::Indexed(params[i + 2] as u8);
                    i += 2;
                } else if i + 4 < params.len() && params[i + 1] == 2 {
                    *bg = Color::Rgb(
                        params[i + 2] as u8,
                        params[i + 3] as u8,
                        params[i + 4] as u8,
                    );
                    i += 4;
                }
            }
            49 => *bg = Color::Default,
            90..=97 => *fg = Color::from_ansi((params[i] - 90 + 8) as u8),
            100..=107 => *bg = Color::from_ansi((params[i] - 100 + 8) as u8),
            _ => {}
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cursor_up() {
        let mut parser = AnsiParser::new();
        let result = "\x1b[5A".bytes().filter_map(|b| parser.parse(b)).last();
        assert_eq!(result, Some(ParseResult::Sequence(AnsiSequence::CursorUp(5))));
    }

    #[test]
    fn parse_cursor_position() {
        let mut parser = AnsiParser::new();
        let result = "\x1b[10;20H".bytes().filter_map(|b| parser.parse(b)).last();
        assert_eq!(
            result,
            Some(ParseResult::Sequence(AnsiSequence::CursorPosition {
                row: 10,
                col: 20
            }))
        );
    }

    #[test]
    fn parse_sgr() {
        let mut parser = AnsiParser::new();
        let result = "\x1b[1;31m".bytes().filter_map(|b| parser.parse(b)).last();
        assert_eq!(
            result,
            Some(ParseResult::Sequence(AnsiSequence::SetGraphics(vec![1, 31])))
        );
    }

    #[test]
    fn parse_printable() {
        let mut parser = AnsiParser::new();
        let result = parser.parse(b'A');
        assert_eq!(result, Some(ParseResult::Print('A')));
    }

    #[test]
    fn apply_sgr_colors() {
        let mut fg = Color::Default;
        let mut bg = Color::Default;
        let mut attrs = Attributes::empty();

        apply_sgr(&[1, 31, 42], &mut fg, &mut bg, &mut attrs);

        assert!(attrs.contains(Attributes::BOLD));
        assert_eq!(fg, Color::Red);
        assert_eq!(bg, Color::Green);
    }
}
