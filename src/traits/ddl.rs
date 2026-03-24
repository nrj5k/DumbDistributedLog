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
    
    /// Acknowledge processing of an entry
    pub async fn ack(&self, _entry_id: u64) -> Result<(), DdlError> {
        // In this simple implementation, we don't do anything with acks
        Ok(())
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
}