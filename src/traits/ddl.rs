//! DDL Core Trait
//!
//! Defines the minimal interface for a Dumb Distributed Log.
//! All implementations (in-memory, with WAL, with gossip) must implement this.

use async_trait::async_trait;
use std::sync::Arc;

/// A single entry in the distributed log
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Entry {
    /// Monotonically increasing ID within a topic
    pub id: u64,
    /// When the entry was created (nanoseconds since epoch)
    pub timestamp: u64,
    /// Which topic this entry belongs to (Arc<str> for zero-copy sharing)
    pub topic: Arc<str>,
    /// The actual data (shared ownership for zero-copy)
    pub payload: Arc<[u8]>,
}

impl Entry {
    /// Create a new entry with auto-generated timestamp
    /// 
    /// # Example
    /// ```
    /// use ddl::Entry;
    /// use std::sync::Arc;
    /// 
    /// let entry = Entry::new(1, "my-topic", vec![1, 2, 3]);
    /// assert_eq!(entry.id, 1);
    /// assert_eq!(*entry.topic, "my-topic");
    /// ```
    pub fn new(id: u64, topic: impl Into<Arc<str>>, payload: impl Into<Arc<[u8]>>) -> Self {
        Self {
            id,
            timestamp: crate::types::now_nanos(),
            topic: topic.into(),
            payload: payload.into(),
        }
    }
    
    /// Create an entry with a specific timestamp (for testing)
    pub fn with_timestamp(id: u64, timestamp: u64, topic: impl Into<Arc<str>>, payload: impl Into<Arc<[u8]>>) -> Self {
        Self {
            id,
            timestamp,
            topic: topic.into(),
            payload: payload.into(),
        }
    }
}

/// Stream of entries from a subscription
pub struct EntryStream {
    // Internal receiver
    receiver: crossbeam::channel::Receiver<Entry>,
    // Backpressure mode (for informational purposes)
    backpressure_mode: BackpressureMode,
}

impl EntryStream {
    /// Create a new EntryStream from a receiver
    pub fn new(receiver: crossbeam::channel::Receiver<Entry>) -> Self {
        Self { 
            receiver,
            backpressure_mode: BackpressureMode::DropOldest,
        }
    }
    
    /// Create a new EntryStream with backpressure mode
    pub fn with_backpressure(
        receiver: crossbeam::channel::Receiver<Entry>, 
        backpressure_mode: BackpressureMode
    ) -> Self {
        Self { 
            receiver,
            backpressure_mode,
        }
    }
    
    /// Get the backpressure mode
    pub fn backpressure_mode(&self) -> BackpressureMode {
        self.backpressure_mode
    }
    
    /// Get the next entry (blocking)
    pub fn next(&mut self) -> Option<Entry> {
        self.receiver.recv().ok()
    }
    
    /// Try to get the next entry (non-blocking)
    pub fn try_next(&self) -> Option<Entry> {
        self.receiver.try_recv().ok()
    }
}

/// Core DDL trait - minimal distributed log interface
#[async_trait]
pub trait DDL: Send + Sync {
    /// Push data to a topic
    /// 
    /// Returns the entry ID if successful
    async fn push(&self, topic: &str, payload: Vec<u8>) -> Result<u64, DdlError>;
    
    /// Push multiple entries to a topic in a batch
    /// 
    /// Returns the last entry ID if successful
    async fn push_batch(&self, topic: &str, payloads: Vec<Vec<u8>>) -> Result<u64, DdlError> {
        // Default implementation for backward compatibility
        let mut last_id = 0;
        for payload in payloads {
            last_id = self.push(topic, payload).await?;
        }
        Ok(last_id)
    }
    
    /// Subscribe to a topic (exact match only)
    ///
    /// Returns a stream of entries. Must call `ack` on each entry.
    async fn subscribe(&self, topic: &str) -> Result<EntryStream, DdlError>;
    
    /// Acknowledge an entry has been processed
    ///
    /// This allows the log to advance and free space.
    async fn ack(&self, topic: &str, entry_id: u64) -> Result<(), DdlError>;
    
    /// Get current position (last entry ID) for a topic
    async fn position(&self, topic: &str) -> Result<u64, DdlError>;
    
    /// Check if this node owns a topic
    fn owns_topic(&self, topic: &str) -> bool;
}

/// Backpressure mode for subscription channels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackpressureMode {
    /// Block producers when buffer is full (reliable delivery)
    Block,
    /// Drop oldest messages when buffer is full (newest data more important)
    DropOldest,
    /// Drop newest messages when buffer is full
    DropNewest,
    /// Return error when buffer is full (explicit handling required)
    Error,
}

/// DDL configuration
#[derive(Debug, Clone)]
pub struct DdlConfig {
    /// Unique ID for this node
    pub node_id: u64,
    /// List of peer nodes (for gossip)
    pub peers: Vec<String>, // "host:port" format
    /// Topics this node owns (static assignment for now)
    pub owned_topics: Vec<String>,
    /// Ring buffer size per topic
    pub buffer_size: usize,
    /// Maximum number of topics allowed (0 = unlimited)
    pub max_topics: usize,
    /// Whether to enable gossip discovery
    pub gossip_enabled: bool,
    /// Address to bind gossip to
    pub gossip_bind_addr: String,
    /// Bootstrap peers for gossip
    pub gossip_bootstrap: Vec<String>,
    /// Data directory for WAL and persistence
    pub data_dir: std::path::PathBuf,
    /// Whether to enable WAL durability
    pub wal_enabled: bool,
    /// Subscription channel buffer size
    pub subscription_buffer_size: usize,
    /// Backpressure mode for subscription channels
    pub subscription_backpressure: BackpressureMode,
    /// Heartbeat interval in seconds for gossip (default: 5)
    pub heartbeat_interval_secs: u64,
    /// Owner timeout in seconds for gossip (default: 30)
    pub owner_timeout_secs: u64,
    /// Enable Raft for topic ownership (strongly consistent)
    pub raft_enabled: bool,
    /// Is this the bootstrap node (first node in cluster)
    pub is_bootstrap: bool,
}

impl Default for DdlConfig {
    fn default() -> Self {
        Self {
            node_id: 1,
            peers: vec![],
            owned_topics: vec![],
            buffer_size: 1024 * 1024, // 1MB default
            max_topics: 10000, // Default limit of 10,000 topics
            gossip_enabled: false,
            gossip_bind_addr: "0.0.0.0:0".to_string(),
            gossip_bootstrap: vec![],
            data_dir: std::path::PathBuf::from("/tmp/ddl"),
            wal_enabled: true,
            subscription_buffer_size: 1000,
            subscription_backpressure: BackpressureMode::DropOldest,
            heartbeat_interval_secs: 5,  // Default: 5 seconds
            owner_timeout_secs: 30,        // Default: 30 seconds
            raft_enabled: false,           // Raft disabled by default
            is_bootstrap: false,           // Not a bootstrap node by default
        }
    }
}

/// DDL errors
#[derive(Debug, thiserror::Error)]
pub enum DdlError {
    #[error("Topic {0} not owned by this node")]
    NotOwner(String),
    
    #[error("Topic {0} not found")]
    TopicNotFound(String),
    
    #[error("Entry {0} not found in topic {1}")]
    EntryNotFound(u64, String),
    
    #[error("Buffer full for topic {0}")]  
    BufferFull(String),
    
    #[error("Topic limit exceeded: max {max}, current {current}")]
    TopicLimitExceeded { max: usize, current: usize },
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Subscriber buffer full for topic {0}")]
    SubscriberBufferFull(String),
    
    #[error("Lock poisoned for topic {0}")]
    LockPoisoned(String),
    
    #[error("WAL error: {0}")]
    Wal(String),
    
    #[error("Invalid topic name: {0}")]
    InvalidTopic(String),

    // ========================================================================
    // Lease Errors (TTL-based ownership for SCORE integration)
    // ========================================================================

    #[error("Lease for key {key} is held by node {owner}")]
    LeaseHeld { key: String, owner: u64 },

    #[error("Lease {0} has expired")]
    LeaseExpired(u64),

    #[error("Lease {0} not found")]
    LeaseNotFound(u64),
}

/// Validate topic name for defense-in-depth
pub fn validate_topic(topic: &str) -> Result<(), DdlError> {
    if topic.is_empty() {
        return Err(DdlError::InvalidTopic("topic name cannot be empty".to_string()));
    }
    if topic.len() > 255 {
        return Err(DdlError::InvalidTopic(format!("topic name too long: {} characters", topic.len())));
    }
    if topic.contains("..") {
        return Err(DdlError::InvalidTopic("topic name cannot contain '..'".to_string()));
    }
    if topic.chars().any(|c| c.is_control()) {
        return Err(DdlError::InvalidTopic("topic name cannot contain control characters".to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // P1 Fix #4: Test configurable heartbeat and timeout defaults
    #[test]
    fn test_ddl_config_defaults() {
        let config = DdlConfig::default();
        
        assert_eq!(config.heartbeat_interval_secs, 5);
        assert_eq!(config.owner_timeout_secs, 30);
        assert!(config.wal_enabled); // WAL should be enabled by default
    }

    #[test]
    fn test_validate_topic_valid() {
        assert!(validate_topic("valid.topic").is_ok());
        assert!(validate_topic("simple").is_ok());
        assert!(validate_topic("a.b.c").is_ok());
    }

    #[test]
    fn test_validate_topic_empty() {
        assert!(validate_topic("").is_err());
    }

    #[test]
    fn test_validate_topic_too_long() {
        let long_topic = "a".repeat(300);
        assert!(validate_topic(&long_topic).is_err());
    }

    #[test]
    fn test_validate_topic_path_traversal() {
        assert!(validate_topic("bad..topic").is_err());
    }

    #[test]
    fn test_validate_topic_control_chars() {
        assert!(validate_topic("bad\0topic").is_err());
        assert!(validate_topic("bad\ntopic").is_err());
    }
}