//! Basic send operations.
//!
//! This module provides fundamental send operations for writing data
//! to a session, including raw bytes, strings, lines, and control characters.

use crate::config::LineEnding;
use crate::error::Result;
use crate::types::ControlChar;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

/// Trait for basic send operations.
pub trait BasicSend: Send {
    /// Send raw bytes.
    fn send_bytes(&mut self, data: &[u8]) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Send a string.
    fn send_str(&mut self, s: &str) -> impl std::future::Future<Output = Result<()>> + Send
    where
        Self: Send,
    {
        async move { self.send_bytes(s.as_bytes()).await }
    }

    /// Send a line with the specified line ending.
    fn send_line_with(
        &mut self,
        line: &str,
        ending: LineEnding,
    ) -> impl std::future::Future<Output = Result<()>> + Send
    where
        Self: Send,
    {
        async move {
            self.send_str(line).await?;
            self.send_str(ending.as_str()).await
        }
    }

    /// Send a line with LF ending.
    fn send_line(&mut self, line: &str) -> impl std::future::Future<Output = Result<()>> + Send
    where
        Self: Send,
    {
        self.send_line_with(line, LineEnding::Lf)
    }

    /// Send a control character.
    fn send_control(
        &mut self,
        ctrl: ControlChar,
    ) -> impl std::future::Future<Output = Result<()>> + Send
    where
        Self: Send,
    {
        async move { self.send_bytes(&[ctrl.as_byte()]).await }
    }

    /// Send Ctrl+C (interrupt).
    fn send_interrupt(&mut self) -> impl std::future::Future<Output = Result<()>> + Send
    where
        Self: Send,
    {
        self.send_control(ControlChar::CtrlC)
    }

    /// Send Ctrl+D (EOF).
    fn send_eof(&mut self) -> impl std::future::Future<Output = Result<()>> + Send
    where
        Self: Send,
    {
        self.send_control(ControlChar::CtrlD)
    }

    /// Send Ctrl+Z (suspend).
    fn send_suspend(&mut self) -> impl std::future::Future<Output = Result<()>> + Send
    where
        Self: Send,
    {
        self.send_control(ControlChar::CtrlZ)
    }

    /// Send Escape.
    fn send_escape(&mut self) -> impl std::future::Future<Output = Result<()>> + Send
    where
        Self: Send,
    {
        self.send_control(ControlChar::Escape)
    }

    /// Send Tab (Ctrl+I).
    fn send_tab(&mut self) -> impl std::future::Future<Output = Result<()>> + Send
    where
        Self: Send,
    {
        self.send_control(ControlChar::CtrlI)
    }

    /// Send Backspace (Ctrl+H).
    fn send_backspace(&mut self) -> impl std::future::Future<Output = Result<()>> + Send
    where
        Self: Send,
    {
        self.send_control(ControlChar::CtrlH)
    }
}

/// A sender that wraps an async writer.
pub struct Sender<W> {
    writer: W,
    line_ending: LineEnding,
    /// Optional delay between characters.
    char_delay: Option<Duration>,
}

impl<W: AsyncWriteExt + Unpin + Send> Sender<W> {
    /// Create a new sender.
    pub const fn new(writer: W) -> Self {
        Self {
            writer,
            line_ending: LineEnding::Lf,
            char_delay: None,
        }
    }

    /// Set the line ending.
    pub fn set_line_ending(&mut self, ending: LineEnding) {
        self.line_ending = ending;
    }

    /// Set character delay for slow typing.
    pub fn set_char_delay(&mut self, delay: Option<Duration>) {
        self.char_delay = delay;
    }

    /// Get the line ending.
    #[must_use]
    pub const fn line_ending(&self) -> LineEnding {
        self.line_ending
    }

    /// Send bytes with optional character delay.
    pub async fn send_with_delay(&mut self, data: &[u8]) -> Result<()> {
        if let Some(delay) = self.char_delay {
            for byte in data {
                self.writer
                    .write_all(&[*byte])
                    .await
                    .map_err(crate::error::ExpectError::Io)?;
                self.writer
                    .flush()
                    .await
                    .map_err(crate::error::ExpectError::Io)?;
                tokio::time::sleep(delay).await;
            }
        } else {
            self.writer
                .write_all(data)
                .await
                .map_err(crate::error::ExpectError::Io)?;
            self.writer
                .flush()
                .await
                .map_err(crate::error::ExpectError::Io)?;
        }
        Ok(())
    }

    /// Get mutable access to the underlying writer.
    pub fn writer_mut(&mut self) -> &mut W {
        &mut self.writer
    }
}

impl<W: AsyncWriteExt + Unpin + Send> BasicSend for Sender<W> {
    async fn send_bytes(&mut self, data: &[u8]) -> Result<()> {
        self.send_with_delay(data).await
    }

    async fn send_line(&mut self, line: &str) -> Result<()> {
        self.send_line_with(line, self.line_ending).await
    }
}

/// ANSI escape sequence helpers.
pub struct AnsiSequences;

impl AnsiSequences {
    /// Cursor up.
    pub const CURSOR_UP: &'static [u8] = b"\x1b[A";
    /// Cursor down.
    pub const CURSOR_DOWN: &'static [u8] = b"\x1b[B";
    /// Cursor right.
    pub const CURSOR_RIGHT: &'static [u8] = b"\x1b[C";
    /// Cursor left.
    pub const CURSOR_LEFT: &'static [u8] = b"\x1b[D";
    /// Home key.
    pub const HOME: &'static [u8] = b"\x1b[H";
    /// End key.
    pub const END: &'static [u8] = b"\x1b[F";
    /// Page up.
    pub const PAGE_UP: &'static [u8] = b"\x1b[5~";
    /// Page down.
    pub const PAGE_DOWN: &'static [u8] = b"\x1b[6~";
    /// Insert key.
    pub const INSERT: &'static [u8] = b"\x1b[2~";
    /// Delete key.
    pub const DELETE: &'static [u8] = b"\x1b[3~";
    /// F1 key.
    pub const F1: &'static [u8] = b"\x1bOP";
    /// F2 key.
    pub const F2: &'static [u8] = b"\x1bOQ";
    /// F3 key.
    pub const F3: &'static [u8] = b"\x1bOR";
    /// F4 key.
    pub const F4: &'static [u8] = b"\x1bOS";
    /// F5 key.
    pub const F5: &'static [u8] = b"\x1b[15~";
    /// F6 key.
    pub const F6: &'static [u8] = b"\x1b[17~";
    /// F7 key.
    pub const F7: &'static [u8] = b"\x1b[18~";
    /// F8 key.
    pub const F8: &'static [u8] = b"\x1b[19~";
    /// F9 key.
    pub const F9: &'static [u8] = b"\x1b[20~";
    /// F10 key.
    pub const F10: &'static [u8] = b"\x1b[21~";
    /// F11 key.
    pub const F11: &'static [u8] = b"\x1b[23~";
    /// F12 key.
    pub const F12: &'static [u8] = b"\x1b[24~";

    /// Generate cursor movement sequence.
    #[must_use]
    pub fn cursor_move(rows: i32, cols: i32) -> Vec<u8> {
        let mut result = Vec::new();

        if rows != 0 {
            let dir = if rows > 0 { 'B' } else { 'A' };
            let count = rows.unsigned_abs();
            result.extend(format!("\x1b[{count}{dir}").as_bytes());
        }

        if cols != 0 {
            let dir = if cols > 0 { 'C' } else { 'D' };
            let count = cols.unsigned_abs();
            result.extend(format!("\x1b[{count}{dir}").as_bytes());
        }

        result
    }

    /// Generate cursor position sequence.
    #[must_use]
    pub fn cursor_position(row: u32, col: u32) -> Vec<u8> {
        format!("\x1b[{row};{col}H").into_bytes()
    }
}

/// Extension trait for sending ANSI sequences.
pub trait AnsiSend: BasicSend {
    /// Send cursor up.
    fn send_cursor_up(&mut self) -> impl std::future::Future<Output = Result<()>> + Send
    where
        Self: Send,
    {
        async move { self.send_bytes(AnsiSequences::CURSOR_UP).await }
    }

    /// Send cursor down.
    fn send_cursor_down(&mut self) -> impl std::future::Future<Output = Result<()>> + Send
    where
        Self: Send,
    {
        async move { self.send_bytes(AnsiSequences::CURSOR_DOWN).await }
    }

    /// Send cursor right.
    fn send_cursor_right(&mut self) -> impl std::future::Future<Output = Result<()>> + Send
    where
        Self: Send,
    {
        async move { self.send_bytes(AnsiSequences::CURSOR_RIGHT).await }
    }

    /// Send cursor left.
    fn send_cursor_left(&mut self) -> impl std::future::Future<Output = Result<()>> + Send
    where
        Self: Send,
    {
        async move { self.send_bytes(AnsiSequences::CURSOR_LEFT).await }
    }

    /// Send home key.
    fn send_home(&mut self) -> impl std::future::Future<Output = Result<()>> + Send
    where
        Self: Send,
    {
        async move { self.send_bytes(AnsiSequences::HOME).await }
    }

    /// Send end key.
    fn send_end(&mut self) -> impl std::future::Future<Output = Result<()>> + Send
    where
        Self: Send,
    {
        async move { self.send_bytes(AnsiSequences::END).await }
    }

    /// Send delete key.
    fn send_delete(&mut self) -> impl std::future::Future<Output = Result<()>> + Send
    where
        Self: Send,
    {
        async move { self.send_bytes(AnsiSequences::DELETE).await }
    }

    /// Send a function key.
    fn send_function_key(&mut self, n: u8) -> impl std::future::Future<Output = Result<()>> + Send
    where
        Self: Send,
    {
        async move {
            let seq = match n {
                1 => AnsiSequences::F1,
                2 => AnsiSequences::F2,
                3 => AnsiSequences::F3,
                4 => AnsiSequences::F4,
                5 => AnsiSequences::F5,
                6 => AnsiSequences::F6,
                7 => AnsiSequences::F7,
                8 => AnsiSequences::F8,
                9 => AnsiSequences::F9,
                10 => AnsiSequences::F10,
                11 => AnsiSequences::F11,
                12 => AnsiSequences::F12,
                _ => return Ok(()),
            };
            self.send_bytes(seq).await
        }
    }
}

impl<T: BasicSend> AnsiSend for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_cursor_move() {
        assert_eq!(AnsiSequences::cursor_move(3, 0), b"\x1b[3B");
        assert_eq!(AnsiSequences::cursor_move(-2, 0), b"\x1b[2A");
        assert_eq!(AnsiSequences::cursor_move(0, 5), b"\x1b[5C");
        assert_eq!(AnsiSequences::cursor_move(0, -4), b"\x1b[4D");
    }

    #[test]
    fn ansi_cursor_position() {
        assert_eq!(AnsiSequences::cursor_position(1, 1), b"\x1b[1;1H");
        assert_eq!(AnsiSequences::cursor_position(10, 20), b"\x1b[10;20H");
    }
}
