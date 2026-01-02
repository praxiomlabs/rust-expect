//! SSH backend for remote terminal automation.
//!
//! This module provides SSH-based session management with features like:
//! - Multiple authentication methods (password, public key, agent)
//! - Connection pooling for efficient resource usage
//! - Retry policies with exponential backoff
//! - Keepalive management
//! - Resilient sessions with auto-reconnect

pub mod auth;
pub mod builder;
pub mod channel;
pub mod keepalive;
pub mod pool;
pub mod resilient;
pub mod retry;
pub mod session;

// Re-export commonly used types
pub use auth::{AuthMethod, HostKeyVerification, SshCredentials};
pub use builder::{parse_ssh_target, SshSessionBuilder};
pub use channel::{ChannelConfig, ChannelRequest, ChannelType, SshChannel};
pub use keepalive::{KeepaliveConfig, KeepaliveManager, KeepaliveState};
pub use pool::{ConnectionPool, PoolConfig, PoolStats, PooledConnection};
pub use resilient::{ResilientConfig, ResilientSession, ResilientState};
pub use retry::{RetryPolicy, RetryState, RetryStrategy};
pub use session::{SshConfig, SshSession, SshSessionState};
