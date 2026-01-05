//! SSH connection pooling.
//!
//! This module provides connection pooling for SSH sessions, allowing efficient
//! reuse of connections to the same host. The pool manages connection lifecycle,
//! idle timeouts, and provides thread-safe access to pooled connections.
//!
//! # Example
//!
//! ```ignore
//! use rust_expect::backend::ssh::{ConnectionPool, PoolConfig, SshConfig, SshCredentials};
//!
//! let pool = ConnectionPool::with_defaults();
//! let config = SshConfig::new("example.com")
//!     .username("user")
//!     .credentials(SshCredentials::password("user", "pass"));
//!
//! // Get a connection from the pool
//! let conn = pool.get_async(&config).await?;
//!
//! // Use the session
//! if let Some(session) = conn.session() {
//!     // ... use session
//! }
//!
//! // Connection is returned to pool when `conn` is dropped
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use super::session::{SshConfig, SshSession};

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
    /// Validate connections before reuse.
    pub validate_on_checkout: bool,
    /// Maximum connection age before forced refresh.
    pub max_connection_age: Option<Duration>,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_per_host: 5,
            max_total: 20,
            idle_timeout: Duration::from_secs(300),
            reuse_connections: true,
            validate_on_checkout: true,
            max_connection_age: Some(Duration::from_secs(3600)), // 1 hour
        }
    }
}

impl PoolConfig {
    /// Create a new pool config with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum connections per host.
    #[must_use]
    pub const fn max_per_host(mut self, max: usize) -> Self {
        self.max_per_host = max;
        self
    }

    /// Set maximum total connections.
    #[must_use]
    pub const fn max_total(mut self, max: usize) -> Self {
        self.max_total = max;
        self
    }

    /// Set idle timeout.
    #[must_use]
    pub const fn idle_timeout(mut self, timeout: Duration) -> Self {
        self.idle_timeout = timeout;
        self
    }

    /// Enable or disable connection reuse.
    #[must_use]
    pub const fn reuse_connections(mut self, reuse: bool) -> Self {
        self.reuse_connections = reuse;
        self
    }

    /// Enable or disable validation on checkout.
    #[must_use]
    pub const fn validate_on_checkout(mut self, validate: bool) -> Self {
        self.validate_on_checkout = validate;
        self
    }

    /// Set maximum connection age.
    #[must_use]
    pub const fn max_connection_age(mut self, age: Option<Duration>) -> Self {
        self.max_connection_age = age;
        self
    }
}

/// Shared session state for pooled connections.
#[derive(Debug)]
struct SharedSession {
    /// The SSH session.
    session: RwLock<SshSession>,
    /// Whether this session is currently in use.
    in_use: AtomicBool,
    /// When the session was created.
    created: Instant,
    /// When the session was last used (atomic for lock-free updates).
    last_used: AtomicU64,
    /// The configuration used to create this session.
    config: SshConfig,
}

impl SharedSession {
    fn new(session: SshSession, config: SshConfig) -> Self {
        Self {
            session: RwLock::new(session),
            in_use: AtomicBool::new(false),
            created: Instant::now(),
            last_used: AtomicU64::new(0),
            config,
        }
    }

    fn mark_used(&self) {
        let now = Instant::now().elapsed().as_millis() as u64;
        self.last_used.store(now, Ordering::Relaxed);
    }

    fn acquire(&self) -> bool {
        self.in_use
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    fn release(&self) {
        self.in_use.store(false, Ordering::Release);
        self.mark_used();
    }

    fn is_in_use(&self) -> bool {
        self.in_use.load(Ordering::Relaxed)
    }

    fn age(&self) -> Duration {
        self.created.elapsed()
    }

    fn is_connected(&self) -> bool {
        self.session
            .read()
            .map(|s| s.is_connected())
            .unwrap_or(false)
    }
}

/// A pooled connection entry.
type PoolEntry = Arc<SharedSession>;

/// SSH connection pool.
///
/// The connection pool manages a collection of SSH sessions, allowing efficient
/// reuse of connections to the same host. It handles connection lifecycle,
/// validates connections before reuse, and automatically cleans up idle connections.
#[derive(Debug)]
pub struct ConnectionPool {
    /// Pool configuration.
    config: PoolConfig,
    /// Connections by host key (user@host:port).
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

    /// Generate a unique key for an SSH config.
    fn make_key(ssh_config: &SshConfig) -> String {
        format!(
            "{}@{}:{}",
            ssh_config.credentials.username, ssh_config.host, ssh_config.port
        )
    }

    /// Get a connection to a host synchronously.
    ///
    /// This method will try to reuse an existing connection if available,
    /// or create a new one. The connection is validated before being returned
    /// if `validate_on_checkout` is enabled.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The maximum connections per host is exceeded
    /// - The connection cannot be established
    pub fn get(&self, ssh_config: &SshConfig) -> crate::error::Result<PooledConnection> {
        let key = Self::make_key(ssh_config);

        // Try to get an existing connection
        if self.config.reuse_connections
            && let Some(conn) = self.try_acquire_existing(&key)
        {
            // Validate if configured
            if self.config.validate_on_checkout && !conn.is_connected() {
                // Connection is stale, release it and create new
                conn.release();
            } else {
                return Ok(PooledConnection::new(conn));
            }
        }

        // Check max connections per host
        {
            let connections = self
                .connections
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(entries) = connections.get(&key)
                && entries.len() >= self.config.max_per_host
            {
                return Err(crate::error::ExpectError::config(format!(
                    "Maximum connections per host ({}) exceeded for {}",
                    self.config.max_per_host, key
                )));
            }
        }

        // Create new connection
        let mut session = SshSession::new(ssh_config.clone());
        session.connect()?;

        let entry = Arc::new(SharedSession::new(session, ssh_config.clone()));
        entry.acquire(); // Mark as in use

        // Add to pool
        {
            let mut connections = self
                .connections
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            connections.entry(key).or_default().push(Arc::clone(&entry));
        }

        Ok(PooledConnection::new(entry))
    }

    /// Get a connection to a host asynchronously.
    ///
    /// This is the preferred method for async contexts. It will try to reuse
    /// an existing connection if available, or create a new one using async I/O.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The maximum connections per host is exceeded
    /// - The connection cannot be established
    #[cfg(feature = "ssh")]
    pub async fn get_async(
        &self,
        ssh_config: &SshConfig,
    ) -> crate::error::Result<PooledConnection> {
        let key = Self::make_key(ssh_config);

        // Try to get an existing connection
        if self.config.reuse_connections
            && let Some(conn) = self.try_acquire_existing(&key)
        {
            // Validate if configured
            if self.config.validate_on_checkout && !conn.is_connected() {
                // Connection is stale, release it and try next
                conn.release();
            } else {
                return Ok(PooledConnection::new(conn));
            }
        }

        // Check if we should reject due to connection limit
        {
            let connections = self
                .connections
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let total: usize = connections.values().map(std::vec::Vec::len).sum();
            if total >= self.config.max_total {
                return Err(crate::error::ExpectError::config(format!(
                    "Maximum total connections ({}) exceeded",
                    self.config.max_total
                )));
            }

            if let Some(entries) = connections.get(&key)
                && entries.len() >= self.config.max_per_host
            {
                return Err(crate::error::ExpectError::config(format!(
                    "Maximum connections per host ({}) exceeded for {}",
                    self.config.max_per_host, key
                )));
            }
        }

        // Create new connection asynchronously
        let mut session = SshSession::new(ssh_config.clone());
        session.connect_async().await?;

        let entry = Arc::new(SharedSession::new(session, ssh_config.clone()));
        entry.acquire(); // Mark as in use

        // Add to pool
        {
            let mut connections = self
                .connections
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            connections.entry(key).or_default().push(Arc::clone(&entry));
        }

        Ok(PooledConnection::new(entry))
    }

    /// Try to acquire an existing connection from the pool.
    #[allow(clippy::significant_drop_tightening)]
    fn try_acquire_existing(&self, key: &str) -> Option<PoolEntry> {
        let connections = self
            .connections
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if let Some(entries) = connections.get(key) {
            for entry in entries {
                // Check connection age
                if let Some(max_age) = self.config.max_connection_age
                    && entry.age() > max_age
                {
                    continue;
                }

                // Try to acquire (atomic compare-and-swap)
                if entry.acquire() {
                    return Some(Arc::clone(entry));
                }
            }
        }

        None
    }

    /// Clean up idle and stale connections.
    ///
    /// This method removes connections that:
    /// - Have been idle longer than `idle_timeout`
    /// - Are older than `max_connection_age`
    /// - Are no longer connected
    ///
    /// Call this periodically to prevent connection buildup.
    pub fn cleanup(&self) {
        let mut connections = self
            .connections
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let idle_timeout = self.config.idle_timeout;
        let max_age = self.config.max_connection_age;

        for entries in connections.values_mut() {
            entries.retain(|entry| {
                // Don't remove entries that are in use
                if entry.is_in_use() {
                    return true;
                }

                // Remove if not connected
                if !entry.is_connected() {
                    return false;
                }

                // Remove if too old
                if let Some(max) = max_age
                    && entry.age() > max
                {
                    return false;
                }

                // Remove if idle too long (approximation based on created time)
                if entry.age() > idle_timeout {
                    return false;
                }

                true
            });
        }

        // Remove empty host entries
        connections.retain(|_, entries| !entries.is_empty());
    }

    /// Get pool statistics.
    #[must_use]
    pub fn stats(&self) -> PoolStats {
        let connections = self
            .connections
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let mut total = 0;
        let mut active = 0;
        let mut idle = 0;
        let mut connected = 0;

        for entries in connections.values() {
            for entry in entries {
                total += 1;
                if entry.is_in_use() {
                    active += 1;
                } else {
                    idle += 1;
                }
                if entry.is_connected() {
                    connected += 1;
                }
            }
        }

        PoolStats {
            total,
            active,
            idle,
            connected,
            hosts: connections.len(),
        }
    }

    /// Close all connections in the pool.
    ///
    /// This disconnects all sessions and clears the pool. Any outstanding
    /// `PooledConnection` handles will no longer be able to use their sessions.
    pub fn close_all(&self) {
        let mut connections = self
            .connections
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        for entries in connections.values() {
            for entry in entries {
                if let Ok(mut session) = entry.session.write() {
                    session.disconnect();
                }
            }
        }

        connections.clear();
    }

    /// Get the pool configuration.
    #[must_use]
    pub const fn config(&self) -> &PoolConfig {
        &self.config
    }
}

impl Default for ConnectionPool {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Pool statistics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolStats {
    /// Total connections in the pool.
    pub total: usize,
    /// Connections currently in use.
    pub active: usize,
    /// Connections available for reuse.
    pub idle: usize,
    /// Connections that are still connected.
    pub connected: usize,
    /// Number of unique hosts.
    pub hosts: usize,
}

impl PoolStats {
    /// Check if the pool is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.total == 0
    }

    /// Get the utilization percentage (active/total).
    #[must_use]
    pub fn utilization(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.active as f64 / self.total as f64) * 100.0
        }
    }
}

/// A connection borrowed from the pool.
///
/// This handle provides access to the underlying SSH session while the connection
/// is checked out from the pool. When dropped, the connection is automatically
/// returned to the pool for reuse.
///
/// # Example
///
/// ```ignore
/// let conn = pool.get(&config)?;
///
/// // Access the session
/// conn.with_session(|session| {
///     // Use the session
/// });
///
/// // Or get mutable access
/// conn.with_session_mut(|session| {
///     session.disconnect();
/// });
/// ```
pub struct PooledConnection {
    entry: PoolEntry,
}

impl std::fmt::Debug for PooledConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PooledConnection")
            .field("connected", &self.is_connected())
            .field("age", &self.age())
            .finish()
    }
}

impl PooledConnection {
    fn new(entry: PoolEntry) -> Self {
        entry.mark_used();
        Self { entry }
    }

    /// Check if the connection is still connected.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.entry.is_connected()
    }

    /// Get the age of the connection.
    #[must_use]
    pub fn age(&self) -> Duration {
        self.entry.age()
    }

    /// Get the configuration used to create this connection.
    #[must_use]
    pub fn config(&self) -> &SshConfig {
        &self.entry.config
    }

    /// Access the session with a closure (read-only).
    ///
    /// This provides safe access to the underlying session without exposing
    /// the locking mechanism.
    ///
    /// # Example
    ///
    /// ```ignore
    /// conn.with_session(|session| {
    ///     println!("Connected: {}", session.is_connected());
    /// });
    /// ```
    pub fn with_session<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&SshSession) -> R,
    {
        self.entry.session.read().ok().map(|s| f(&s))
    }

    /// Access the session with a closure (mutable).
    ///
    /// This provides safe mutable access to the underlying session.
    ///
    /// # Example
    ///
    /// ```ignore
    /// conn.with_session_mut(|session| {
    ///     session.disconnect();
    /// });
    /// ```
    pub fn with_session_mut<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut SshSession) -> R,
    {
        self.entry.session.write().ok().map(|mut s| f(&mut s))
    }

    /// Get a read lock on the session.
    ///
    /// For most use cases, prefer `with_session` or `with_session_mut` which
    /// provide safer access patterns.
    #[must_use]
    pub fn session(&self) -> Option<std::sync::RwLockReadGuard<'_, SshSession>> {
        self.entry.session.read().ok()
    }

    /// Get a write lock on the session.
    ///
    /// For most use cases, prefer `with_session` or `with_session_mut` which
    /// provide safer access patterns.
    #[must_use]
    pub fn session_mut(&self) -> Option<std::sync::RwLockWriteGuard<'_, SshSession>> {
        self.entry.session.write().ok()
    }

    /// Explicitly release the connection back to the pool.
    ///
    /// This is equivalent to dropping the connection, but makes the intent
    /// more explicit. After calling this, the connection handle should not
    /// be used.
    pub fn release(self) {
        // Drop will handle the release
        drop(self);
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        self.entry.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_stats_empty() {
        let pool = ConnectionPool::with_defaults();
        let stats = pool.stats();

        assert_eq!(stats.total, 0);
        assert_eq!(stats.active, 0);
        assert_eq!(stats.idle, 0);
        assert_eq!(stats.connected, 0);
        assert!(stats.is_empty());
        assert!(stats.utilization().abs() < f64::EPSILON);
    }

    #[test]
    fn pool_config_builder() {
        let config = PoolConfig::new()
            .max_per_host(10)
            .max_total(50)
            .idle_timeout(Duration::from_secs(600))
            .reuse_connections(true)
            .validate_on_checkout(false)
            .max_connection_age(Some(Duration::from_secs(7200)));

        let pool = ConnectionPool::new(config);
        assert_eq!(pool.config().max_per_host, 10);
        assert_eq!(pool.config().max_total, 50);
        assert_eq!(pool.config().idle_timeout, Duration::from_secs(600));
        assert!(pool.config().reuse_connections);
        assert!(!pool.config().validate_on_checkout);
        assert_eq!(
            pool.config().max_connection_age,
            Some(Duration::from_secs(7200))
        );
    }

    #[test]
    fn pool_config_defaults() {
        let config = PoolConfig::default();

        assert_eq!(config.max_per_host, 5);
        assert_eq!(config.max_total, 20);
        assert_eq!(config.idle_timeout, Duration::from_secs(300));
        assert!(config.reuse_connections);
        assert!(config.validate_on_checkout);
        assert_eq!(config.max_connection_age, Some(Duration::from_secs(3600)));
    }

    #[test]
    fn pool_cleanup_empty() {
        let pool = ConnectionPool::with_defaults();
        pool.cleanup();
        assert!(pool.stats().is_empty());
    }

    #[test]
    fn pool_close_all_empty() {
        let pool = ConnectionPool::with_defaults();
        pool.close_all();
        assert!(pool.stats().is_empty());
    }

    #[test]
    fn pool_stats_utilization() {
        let stats = PoolStats {
            total: 10,
            active: 5,
            idle: 5,
            connected: 8,
            hosts: 2,
        };
        assert!((stats.utilization() - 50.0).abs() < f64::EPSILON);
    }
}
