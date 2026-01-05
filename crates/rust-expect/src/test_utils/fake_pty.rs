//! Fake PTY for unit testing.
//!
//! Provides a simulated PTY that can be used for testing without
//! spawning actual processes.

use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};

/// Shared buffer for fake PTY communication.
#[derive(Debug, Default)]
struct SharedBuffer {
    data: VecDeque<u8>,
    closed: bool,
}

/// A fake PTY for testing.
#[derive(Debug)]
pub struct FakePty {
    /// Read buffer (data to be read by the consumer).
    read_buf: Arc<Mutex<SharedBuffer>>,
    /// Write buffer (data written by the consumer).
    write_buf: Arc<Mutex<SharedBuffer>>,
}

impl FakePty {
    /// Create a new fake PTY.
    #[must_use]
    pub fn new() -> Self {
        Self {
            read_buf: Arc::new(Mutex::new(SharedBuffer::default())),
            write_buf: Arc::new(Mutex::new(SharedBuffer::default())),
        }
    }

    /// Queue data to be read.
    pub fn queue_input(&self, data: &[u8]) {
        let mut buf = self
            .read_buf
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        buf.data.extend(data);
    }

    /// Queue a string to be read.
    pub fn queue_input_str(&self, s: &str) {
        self.queue_input(s.as_bytes());
    }

    /// Get data that was written.
    #[must_use]
    pub fn take_output(&self) -> Vec<u8> {
        let mut buf = self
            .write_buf
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        buf.data.drain(..).collect()
    }

    /// Get data that was written as a string.
    #[must_use]
    pub fn take_output_str(&self) -> String {
        String::from_utf8_lossy(&self.take_output()).into_owned()
    }

    /// Check if there's pending output.
    #[must_use]
    pub fn has_output(&self) -> bool {
        !self
            .write_buf
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .data
            .is_empty()
    }

    /// Check if there's pending input.
    #[must_use]
    pub fn has_input(&self) -> bool {
        !self
            .read_buf
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .data
            .is_empty()
    }

    /// Close the PTY.
    pub fn close(&self) {
        self.read_buf
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .closed = true;
        self.write_buf
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .closed = true;
    }

    /// Check if closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.read_buf
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .closed
    }

    /// Simulate terminal output (what the "process" sends).
    pub fn send_output(&self, data: &[u8]) {
        self.queue_input(data);
    }

    /// Simulate terminal output with newline.
    pub fn send_line(&self, line: &str) {
        self.queue_input_str(line);
        self.queue_input(b"\r\n");
    }

    /// Simulate a prompt.
    pub fn send_prompt(&self, prompt: &str) {
        self.queue_input_str(prompt);
    }

    /// Get what was "typed" into the terminal.
    #[must_use]
    pub fn get_typed(&self) -> String {
        self.take_output_str()
    }
}

impl Default for FakePty {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::significant_drop_tightening)]
impl Read for FakePty {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut shared = self
            .read_buf
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if shared.closed && shared.data.is_empty() {
            return Ok(0);
        }
        if shared.data.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "No data available",
            ));
        }
        let len = buf.len().min(shared.data.len());
        for (i, byte) in shared.data.drain(..len).enumerate() {
            buf[i] = byte;
        }
        Ok(len)
    }
}

#[allow(clippy::significant_drop_tightening)]
impl Write for FakePty {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut shared = self
            .write_buf
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if shared.closed {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "PTY closed"));
        }
        shared.data.extend(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Clone for FakePty {
    fn clone(&self) -> Self {
        Self {
            read_buf: Arc::clone(&self.read_buf),
            write_buf: Arc::clone(&self.write_buf),
        }
    }
}

/// A pair of connected fake PTYs.
#[derive(Debug)]
pub struct FakePtyPair {
    /// The master end (what the test uses).
    pub master: FakePty,
    /// The slave end (what the "process" uses).
    pub slave: FakePty,
}

impl FakePtyPair {
    /// Create a new connected pair.
    #[must_use]
    pub fn new() -> Self {
        let master = FakePty::new();
        let slave = FakePty {
            read_buf: Arc::clone(&master.write_buf),
            write_buf: Arc::clone(&master.read_buf),
        };
        Self { master, slave }
    }
}

impl Default for FakePtyPair {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_pty_read_write() {
        let pty = FakePty::new();

        pty.queue_input(b"Hello");
        assert!(pty.has_input());

        let mut clone = pty;
        let mut buf = [0u8; 10];
        let n = clone.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"Hello");
    }

    #[test]
    fn fake_pty_pair() {
        let pair = FakePtyPair::new();

        // Write from master
        let mut master = pair.master.clone();
        master.write_all(b"command\n").unwrap();

        // Read from slave
        let mut slave = pair.slave;
        let mut buf = [0u8; 20];
        let n = slave.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"command\n");

        // Write from slave
        slave.write_all(b"response").unwrap();

        // Read from master
        let n = master.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"response");
    }

    #[test]
    fn send_helpers() {
        let pty = FakePty::new();
        pty.send_line("Hello, World!");
        pty.send_prompt("$ ");

        let mut clone = pty;
        let mut buf = [0u8; 50];
        let n = clone.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"Hello, World!\r\n$ ");
    }
}
