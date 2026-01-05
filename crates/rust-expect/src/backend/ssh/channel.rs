//! SSH channel management.
//!
//! This module provides SSH channel handling with I/O support for
//! interactive sessions when the `ssh` feature is enabled.

#[cfg(feature = "ssh")]
use std::collections::VecDeque;
#[cfg(feature = "ssh")]
use std::io;
#[cfg(feature = "ssh")]
use std::pin::Pin;
#[cfg(feature = "ssh")]
use std::task::{Context, Poll};
use std::time::Duration;

#[cfg(feature = "ssh")]
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

#[cfg(feature = "ssh")]
use crate::error::SshError;
use crate::types::Dimensions;

/// SSH channel type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelType {
    /// Session channel (for shell/exec).
    Session,
    /// Direct TCP/IP forwarding.
    DirectTcpIp,
    /// Forwarded TCP/IP.
    ForwardedTcpIp,
    /// X11 forwarding.
    X11,
}

/// SSH channel request type.
#[derive(Debug, Clone)]
pub enum ChannelRequest {
    /// Request a PTY.
    Pty {
        /// Terminal type.
        term: String,
        /// Terminal dimensions.
        dimensions: Dimensions,
        /// Terminal modes.
        modes: Vec<u8>,
    },
    /// Request shell.
    Shell,
    /// Execute a command.
    Exec {
        /// Command to execute.
        command: String,
    },
    /// Request subsystem.
    Subsystem {
        /// Subsystem name.
        name: String,
    },
    /// Window size change.
    WindowChange {
        /// New dimensions.
        dimensions: Dimensions,
    },
    /// Send signal.
    Signal {
        /// Signal name.
        signal: String,
    },
    /// Request environment variable.
    Env {
        /// Variable name.
        name: String,
        /// Variable value.
        value: String,
    },
}

/// SSH channel state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelState {
    /// Opening.
    Opening,
    /// Open and ready.
    Open,
    /// End of file received.
    Eof,
    /// Closed.
    Closed,
}

/// SSH channel configuration.
#[derive(Debug, Clone)]
pub struct ChannelConfig {
    /// Channel type.
    pub channel_type: ChannelType,
    /// Read timeout.
    pub read_timeout: Duration,
    /// Write timeout.
    pub write_timeout: Duration,
    /// Buffer size.
    pub buffer_size: usize,
    /// Request PTY.
    pub pty: bool,
    /// Terminal type.
    pub term: String,
    /// Terminal dimensions.
    pub dimensions: Dimensions,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            channel_type: ChannelType::Session,
            read_timeout: Duration::from_secs(30),
            write_timeout: Duration::from_secs(30),
            buffer_size: 32768,
            pty: true,
            term: "xterm-256color".to_string(),
            dimensions: Dimensions::default(),
        }
    }
}

impl ChannelConfig {
    /// Create new config.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set terminal type.
    #[must_use]
    pub fn term(mut self, term: impl Into<String>) -> Self {
        self.term = term.into();
        self
    }

    /// Set dimensions.
    #[must_use]
    pub const fn dimensions(mut self, cols: u16, rows: u16) -> Self {
        self.dimensions = Dimensions { cols, rows };
        self
    }

    /// Disable PTY.
    #[must_use]
    pub const fn no_pty(mut self) -> Self {
        self.pty = false;
        self
    }

    /// Set buffer size.
    #[must_use]
    pub const fn buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }
}

// ============================================================================
// russh channel wrapper (when ssh feature is enabled)
// ============================================================================

/// SSH channel wrapper with AsyncRead/AsyncWrite support.
///
/// This wraps a russh `Channel` to provide a familiar async I/O interface
/// that can be used with the `Session` type.
#[cfg(feature = "ssh")]
pub struct SshChannelStream {
    /// The underlying russh channel.
    channel: russh::Channel<russh::client::Msg>,
    /// Configuration.
    config: ChannelConfig,
    /// Current state.
    state: ChannelState,
    /// Read buffer for data received from the channel.
    read_buffer: VecDeque<u8>,
    /// Exit status when received.
    exit_status: Option<u32>,
    /// Whether EOF has been received.
    eof_received: bool,
}

#[cfg(feature = "ssh")]
impl std::fmt::Debug for SshChannelStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SshChannelStream")
            .field("config", &self.config)
            .field("state", &self.state)
            .field("read_buffer_len", &self.read_buffer.len())
            .field("exit_status", &self.exit_status)
            .field("eof_received", &self.eof_received)
            .finish()
    }
}

#[cfg(feature = "ssh")]
impl SshChannelStream {
    /// Create a new channel stream from a russh channel.
    #[must_use]
    pub fn new(channel: russh::Channel<russh::client::Msg>, config: ChannelConfig) -> Self {
        Self {
            channel,
            config,
            state: ChannelState::Open,
            read_buffer: VecDeque::with_capacity(32768),
            exit_status: None,
            eof_received: false,
        }
    }

    /// Get the configuration.
    #[must_use]
    pub const fn config(&self) -> &ChannelConfig {
        &self.config
    }

    /// Get the current state.
    #[must_use]
    pub const fn state(&self) -> ChannelState {
        self.state
    }

    /// Check if the channel is open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.state == ChannelState::Open
    }

    /// Check if EOF has been received.
    #[must_use]
    pub const fn is_eof(&self) -> bool {
        self.eof_received
    }

    /// Get the exit status if the command has completed.
    #[must_use]
    pub const fn exit_status(&self) -> Option<u32> {
        self.exit_status
    }

    /// Request a PTY on this channel.
    ///
    /// # Errors
    ///
    /// Returns an error if the PTY request fails.
    pub async fn request_pty(&mut self) -> crate::error::Result<()> {
        self.channel
            .request_pty(
                false, // want_reply
                &self.config.term,
                self.config.dimensions.cols.into(),
                self.config.dimensions.rows.into(),
                0,   // pixel width
                0,   // pixel height
                &[], // terminal modes
            )
            .await
            .map_err(|e| {
                crate::error::ExpectError::Ssh(SshError::Channel {
                    reason: format!("PTY request failed: {e}"),
                })
            })
    }

    /// Request a shell on this channel.
    ///
    /// # Errors
    ///
    /// Returns an error if the shell request fails.
    pub async fn request_shell(&mut self) -> crate::error::Result<()> {
        self.channel.request_shell(false).await.map_err(|e| {
            crate::error::ExpectError::Ssh(SshError::Channel {
                reason: format!("Shell request failed: {e}"),
            })
        })
    }

    /// Execute a command on this channel.
    ///
    /// # Errors
    ///
    /// Returns an error if the exec request fails.
    pub async fn exec(&mut self, command: &str) -> crate::error::Result<()> {
        self.channel.exec(false, command).await.map_err(|e| {
            crate::error::ExpectError::Ssh(SshError::Channel {
                reason: format!("Exec request failed: {e}"),
            })
        })
    }

    /// Change the window size.
    ///
    /// # Errors
    ///
    /// Returns an error if the window change request fails.
    pub async fn window_change(&mut self, cols: u16, rows: u16) -> crate::error::Result<()> {
        self.channel
            .window_change(cols.into(), rows.into(), 0, 0)
            .await
            .map_err(|e| {
                crate::error::ExpectError::Ssh(SshError::Channel {
                    reason: format!("Window change failed: {e}"),
                })
            })
    }

    /// Send data to the channel.
    ///
    /// # Errors
    ///
    /// Returns an error if the data send fails.
    pub async fn send_data(&mut self, data: &[u8]) -> crate::error::Result<()> {
        self.channel.data(data).await.map_err(|e| {
            crate::error::ExpectError::Ssh(SshError::Channel {
                reason: format!("Data send failed: {e}"),
            })
        })
    }

    /// Send EOF to the channel.
    ///
    /// # Errors
    ///
    /// Returns an error if the EOF send fails.
    pub async fn send_eof(&mut self) -> crate::error::Result<()> {
        self.channel.eof().await.map_err(|e| {
            crate::error::ExpectError::Ssh(SshError::Channel {
                reason: format!("EOF send failed: {e}"),
            })
        })
    }

    /// Close the channel.
    pub async fn close(&mut self) -> crate::error::Result<()> {
        self.state = ChannelState::Closed;
        self.channel.close().await.map_err(|e| {
            crate::error::ExpectError::Ssh(SshError::Channel {
                reason: format!("Channel close failed: {e}"),
            })
        })
    }

    /// Wait for and process the next message from the channel.
    ///
    /// This should be called in a loop to receive data from the remote.
    /// Returns `None` when the channel is closed.
    pub async fn wait(&mut self) -> Option<russh::ChannelMsg> {
        let msg = self.channel.wait().await?;

        match &msg {
            russh::ChannelMsg::Data { data } => {
                self.read_buffer.extend(data.as_ref());
            }
            russh::ChannelMsg::ExtendedData { data, ext } => {
                // ext 1 is typically stderr
                if *ext == 1 {
                    // For now, treat stderr the same as stdout
                    self.read_buffer.extend(data.as_ref());
                }
            }
            russh::ChannelMsg::ExitStatus { exit_status } => {
                self.exit_status = Some(*exit_status);
            }
            russh::ChannelMsg::Eof => {
                self.eof_received = true;
                self.state = ChannelState::Eof;
            }
            russh::ChannelMsg::Close => {
                self.state = ChannelState::Closed;
            }
            _ => {}
        }

        Some(msg)
    }

    /// Read available data from the internal buffer.
    ///
    /// Returns the number of bytes read.
    pub fn read_buffered(&mut self, buf: &mut [u8]) -> usize {
        let len = std::cmp::min(buf.len(), self.read_buffer.len());
        for (i, byte) in self.read_buffer.drain(..len).enumerate() {
            buf[i] = byte;
        }
        len
    }

    /// Check if there's data available in the read buffer.
    #[must_use]
    pub fn has_data(&self) -> bool {
        !self.read_buffer.is_empty()
    }

    /// Get the number of bytes available in the read buffer.
    #[must_use]
    pub fn available(&self) -> usize {
        self.read_buffer.len()
    }
}

#[cfg(feature = "ssh")]
impl AsyncRead for SshChannelStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // First, try to read from the internal buffer
        if !self.read_buffer.is_empty() {
            let len = std::cmp::min(buf.remaining(), self.read_buffer.len());
            let data: Vec<u8> = self.read_buffer.drain(..len).collect();
            buf.put_slice(&data);
            return Poll::Ready(Ok(()));
        }

        // If EOF, return 0 bytes (signals EOF to caller)
        if self.eof_received || self.state == ChannelState::Closed {
            return Poll::Ready(Ok(()));
        }

        // We need to poll for more data from the channel.
        // Use a boxed future to avoid borrow issues with self.
        let this = self.get_mut();

        // Create and poll the wait future
        let wait_future = this.channel.wait();
        tokio::pin!(wait_future);

        match wait_future.poll(cx) {
            Poll::Ready(Some(msg)) => {
                // Future is done, safe to modify self now
                match msg {
                    russh::ChannelMsg::Data { data } => {
                        let len = std::cmp::min(buf.remaining(), data.len());
                        buf.put_slice(&data[..len]);
                        // Store remaining data in buffer
                        if len < data.len() {
                            this.read_buffer.extend(&data[len..]);
                        }
                        Poll::Ready(Ok(()))
                    }
                    russh::ChannelMsg::ExtendedData { data, ext } => {
                        if ext == 1 {
                            let len = std::cmp::min(buf.remaining(), data.len());
                            buf.put_slice(&data[..len]);
                            if len < data.len() {
                                this.read_buffer.extend(&data[len..]);
                            }
                        }
                        Poll::Ready(Ok(()))
                    }
                    russh::ChannelMsg::Eof => {
                        this.eof_received = true;
                        this.state = ChannelState::Eof;
                        Poll::Ready(Ok(()))
                    }
                    russh::ChannelMsg::Close => {
                        this.state = ChannelState::Closed;
                        Poll::Ready(Ok(()))
                    }
                    russh::ChannelMsg::ExitStatus { exit_status } => {
                        this.exit_status = Some(exit_status);
                        // Continue waiting for more data
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    }
                    _ => {
                        // Other messages, continue waiting
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    }
                }
            }
            Poll::Ready(None) => {
                // Channel closed
                this.state = ChannelState::Closed;
                Poll::Ready(Ok(()))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(feature = "ssh")]
impl AsyncWrite for SshChannelStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();

        if this.state == ChannelState::Closed {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Channel is closed",
            )));
        }

        // Create and poll the data future
        let data_future = this.channel.data(buf);
        tokio::pin!(data_future);

        match data_future.poll(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(buf.len())),
            Poll::Ready(Err(e)) => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::Other,
                format!("SSH write error: {e}"),
            ))),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // SSH channels don't have an explicit flush
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();

        let eof_future = this.channel.eof();
        tokio::pin!(eof_future);

        match eof_future.poll(cx) {
            Poll::Ready(Ok(())) => {
                this.state = ChannelState::Eof;
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::Other,
                format!("SSH shutdown error: {e}"),
            ))),
            Poll::Pending => Poll::Pending,
        }
    }
}

// ============================================================================
// Stub implementation (when ssh feature is disabled)
// ============================================================================

/// SSH channel stub for when the `ssh` feature is disabled.
#[cfg(not(feature = "ssh"))]
#[derive(Debug)]
pub struct SshChannel {
    /// Configuration.
    config: ChannelConfig,
    /// Current state.
    state: ChannelState,
    /// Exit status (if command completed).
    exit_status: Option<i32>,
}

#[cfg(not(feature = "ssh"))]
impl SshChannel {
    /// Create a new channel.
    #[must_use]
    pub const fn new(config: ChannelConfig) -> Self {
        Self {
            config,
            state: ChannelState::Opening,
            exit_status: None,
        }
    }

    /// Get configuration.
    #[must_use]
    pub const fn config(&self) -> &ChannelConfig {
        &self.config
    }

    /// Get current state.
    #[must_use]
    pub const fn state(&self) -> ChannelState {
        self.state
    }

    /// Check if open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.state == ChannelState::Open
    }

    /// Check if closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.state == ChannelState::Closed
    }

    /// Get exit status.
    #[must_use]
    pub const fn exit_status(&self) -> Option<i32> {
        self.exit_status
    }

    /// Open the channel (stub).
    pub fn open(&mut self) -> crate::error::Result<()> {
        self.state = ChannelState::Open;
        Ok(())
    }

    /// Close the channel.
    pub fn close(&mut self) {
        self.state = ChannelState::Closed;
    }

    /// Set exit status.
    pub fn set_exit_status(&mut self, status: i32) {
        self.exit_status = Some(status);
    }
}

/// Alias for backward compatibility when ssh feature is enabled.
#[cfg(feature = "ssh")]
pub type SshChannel = SshChannelStream;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_config() {
        let config = ChannelConfig::new()
            .term("vt100")
            .dimensions(120, 40)
            .buffer_size(65536);

        assert_eq!(config.term, "vt100");
        assert_eq!(config.dimensions.cols, 120);
        assert_eq!(config.buffer_size, 65536);
    }

    #[cfg(not(feature = "ssh"))]
    #[test]
    fn channel_state() {
        let mut channel = SshChannel::new(ChannelConfig::default());
        assert_eq!(channel.state(), ChannelState::Opening);

        channel.open().unwrap();
        assert!(channel.is_open());

        channel.close();
        assert!(channel.is_closed());
    }
}
