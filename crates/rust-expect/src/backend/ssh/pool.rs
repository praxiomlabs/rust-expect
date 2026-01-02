//! SSH connection pooling.

use super::session::{SshConfig, SshSession};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Connection pool configuration.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum connections per host.
    pub max_per_host: usize,
    /// Maximum total connections.
    pub max_total: usize,
    /// Connection idle timeout.
    pub idle_timeout: Duration,
    /// Enable connection reuse.
    pub reuse_connections: bool,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_per_host: 5,
            max_total: 20,
            idle_timeout: Duration::from_secs(300),
            reuse_connections: true,
        }
    }
}

/// A pooled connection entry.
#[derive(Debug)]
struct PoolEntry {
    /// The session.
    session: SshSession,
    /// When the session was created.
    created: Instant,
    /// When the session was last used.
    last_used: Instant,
    /// Whether the session is in use.
    in_use: bool,
}

impl PoolEntry {
    fn new(session: SshSession) -> Self {
        let now = Instant::now();
        Self {
            session,
            created: now,
            last_used: now,
            in_use: false,
        }
    }

    fn is_idle(&self, timeout: Duration) -> bool {
        !self.in_use && self.last_used.elapsed() > timeout
    }
}

/// SSH connection pool.
#[derive(Debug)]
pub struct ConnectionPool {
    /// Pool configuration.
    config: PoolConfig,
    /// Connections by host key.
    connections: Arc<Mutex<HashMap<String, Vec<PoolEntry>>>>,
}

impl ConnectionPool {
    /// Create a new connection pool.
    #[must_use]
    pub fn new(config: PoolConfig) -> Self {
        Self {
            config,
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create with default config.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(PoolConfig::default())
    }

    /// Get a connection to a host.
    pub fn get(&self, ssh_config: &SshConfig) -> crate::error::Result<PooledConnection> {
        let key = format!("{}@{}:{}", ssh_config.credentials.username, ssh_config.host, ssh_config.port);

        let mut connections = self.connections.lock().unwrap_or_else(|e| e.into_inner());

        // Try to find an available connection
        if self.config.reuse_connections {
            if let Some(entries) = connections.get_mut(&key) {
                for entry in entries.iter_mut() {
                    if !entry.in_use && entry.session.is_connected() {
                        entry.in_use = true;
                        entry.last_used = Instant::now();
                        // Return a reference to the existing session
                        return Ok(PooledConnection {
                            pool: Arc::clone(&self.connections),
                            key: key.clone(),
                            index: entries.len() - 1,
                        });
                    }
                }
            }
        }

        // Create new connection
        let entries = connections.entry(key.clone()).or_default();

        if entries.len() >= self.config.max_per_host {
            return Err(crate::error::ExpectError::config(
                "Max connections per host exceeded",
            ));
        }

        let mut session = SshSession::new(ssh_config.clone());
        session.connect()?;

        let mut entry = PoolEntry::new(session);
        entry.in_use = true;
        let index = entries.len();
        entries.push(entry);

        Ok(PooledConnection {
            pool: Arc::clone(&self.connections),
            key,
            index,
        })
    }

    /// Clean up idle connections.
    pub fn cleanup(&self) {
        let mut connections = self.connections.lock().unwrap_or_else(|e| e.into_inner());

        for entries in connections.values_mut() {
            entries.retain(|entry| !entry.is_idle(self.config.idle_timeout));
        }

        connections.retain(|_, entries| !entries.is_empty());
    }

    /// Get pool statistics.
    #[must_use]
    pub fn stats(&self) -> PoolStats {
        let connections = self.connections.lock().unwrap_or_else(|e| e.into_inner());

        let mut total = 0;
        let mut active = 0;
        let mut idle = 0;

        for entries in connections.values() {
            for entry in entries {
                total += 1;
                if entry.in_use {
                    active += 1;
                } else {
                    idle += 1;
                }
            }
        }

        PoolStats {
            total,
            active,
            idle,
            hosts: connections.len(),
        }
    }

    /// Close all connections.
    pub fn close_all(&self) {
        let mut connections = self.connections.lock().unwrap_or_else(|e| e.into_inner());
        for entries in connections.values_mut() {
            for entry in entries.iter_mut() {
                entry.session.disconnect();
            }
        }
        connections.clear();
    }
}

impl Default for ConnectionPool {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Pool statistics.
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Total connections.
    pub total: usize,
    /// Active connections.
    pub active: usize,
    /// Idle connections.
    pub idle: usize,
    /// Number of unique hosts.
    pub hosts: usize,
}

/// A connection borrowed from the pool.
#[derive(Debug)]
pub struct PooledConnection {
    pool: Arc<Mutex<HashMap<String, Vec<PoolEntry>>>>,
    key: String,
    index: usize,
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        // Return the connection to the pool
        if let Ok(mut connections) = self.pool.lock() {
            if let Some(entries) = connections.get_mut(&self.key) {
                if let Some(entry) = entries.get_mut(self.index) {
                    entry.in_use = false;
                    entry.last_used = Instant::now();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_stats() {
        let pool = ConnectionPool::with_defaults();
        let stats = pool.stats();

        assert_eq!(stats.total, 0);
        assert_eq!(stats.active, 0);
        assert_eq!(stats.idle, 0);
    }

    #[test]
    fn pool_config() {
        let config = PoolConfig {
            max_per_host: 10,
            max_total: 50,
            idle_timeout: Duration::from_secs(600),
            reuse_connections: true,
        };

        let pool = ConnectionPool::new(config);
        assert_eq!(pool.config.max_per_host, 10);
    }
}
