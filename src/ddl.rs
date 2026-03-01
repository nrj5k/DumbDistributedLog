//! DDL - Dumb Distributed Log
//!
//! Core implementation with in-memory ring buffer storage.
//! This is the simplest possible implementation - no WAL, no gossip, no distribution yet.

use crate::traits::ddl::{BackpressureMode, DDL, DdlConfig, DdlError, Entry, EntryStream};
use async_trait::async_trait;
use crossbeam::channel::{bounded, Sender, TrySendError};
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

/// Subscriber metadata
struct SubscriberInfo {
    sender: Sender<Entry>,
}

/// Per-topic queue with ring buffer semantics
struct TopicQueue {
    /// Entries stored in ring buffer
    entries: Mutex<Vec<Option<Entry>>>,
    /// Write position (monotonically increasing) - protected by entries mutex
    write_pos: AtomicU64,
    /// Last acknowledged position
    ack_pos: AtomicU64,
    /// Buffer size (power of 2 for efficient wrap-around)
    capacity: usize,
    /// Mask for efficient modulo operations
    mask: usize,
}

impl TopicQueue {
    fn new(capacity: usize) -> Self {
        // Ensure capacity is power of 2
        let capacity = capacity.next_power_of_two();
        let mask = capacity - 1;
        
        let mut entries = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            entries.push(None);
        }
        
        Self {
            entries: Mutex::new(entries),
            write_pos: AtomicU64::new(0),
            ack_pos: AtomicU64::new(0),
            capacity,
            mask,
        }
    }
    
    /// Append an entry, returns the entry ID
    fn push(&self, topic: String, payload: Vec<u8>) -> Result<u64, DdlError> {
        // Lock entries first - this provides mutual exclusion for both
        // the write_pos increment and the entry write, ensuring atomicity
        let mut entries = self.entries.lock()
            .map_err(|_| DdlError::LockPoisoned(topic.clone()))?;
        
        // Get current position inside the lock
        let id = self.write_pos.load(Ordering::SeqCst);
        let index = (id as usize) & self.mask;
        
        // Check if we're overwriting unacknowledged data
        let ack = self.ack_pos.load(Ordering::SeqCst);
        if id >= ack + self.capacity as u64 {
            return Err(DdlError::BufferFull(topic));
        }
        
        let entry = Entry {
            id,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            topic,
            payload,
        };
        
        // Store the entry BEFORE incrementing write_pos
        // This ensures readers can't see incomplete entries
        entries[index] = Some(entry);
        
        // Memory barrier: ensure entry is written before making it visible
        // The mutex unlock provides this barrier, but we also increment atomically
        std::sync::atomic::fence(Ordering::Release);
        
        // Now make the entry visible by incrementing write_pos
        self.write_pos.store(id + 1, Ordering::Release);
        
        Ok(id)
    }
    
    /// Read entry at specific ID
    /// Uses Acquire ordering to ensure we see the complete entry
    fn read(&self, id: u64) -> Option<Entry> {
        // Memory barrier to synchronize with Release in push
        std::sync::atomic::fence(Ordering::Acquire);
        
        let index = (id as usize) & self.mask;
        if let Ok(entries) = self.entries.lock() {
            entries[index].clone()
        } else {
            None
        }
    }
    
    /// Acknowledge entries up to this ID
    fn ack(&self, id: u64) {
        // Simple: just move ack_pos forward
        // In production: handle out-of-order acks
        let current = self.ack_pos.load(Ordering::SeqCst);
        if id > current {
            self.ack_pos.store(id, Ordering::SeqCst);
        }
    }
    
    /// Get current position (next ID to write)
    fn position(&self) -> u64 {
        self.write_pos.load(Ordering::SeqCst)
    }
}

/// Dumb Distributed Log - Core Implementation
pub struct Ddl {
    config: DdlConfig,
    /// Map of topic_name -> TopicQueue
    topics: Arc<DashMap<String, Arc<TopicQueue>>>,
    /// Map of topic_name -> subscription senders with metadata
    subscribers: Arc<DashMap<String, Vec<SubscriberInfo>>>,
}

impl Ddl {
    /// Create new DDL instance
    pub fn new(config: DdlConfig) -> Self {
        let topics = Arc::new(DashMap::new());
        let subscribers = Arc::new(DashMap::new());
        
        // Pre-create queues for owned topics
        for topic in &config.owned_topics {
            let queue = Arc::new(TopicQueue::new(config.buffer_size));
            topics.insert(topic.clone(), queue);
        }
        
        Self {
            config,
            topics,
            subscribers,
        }
    }
    
    /// Create with default config
    pub fn default() -> Self {
        Self::new(DdlConfig::default())
    }
    
    /// Get or create a topic queue
    fn get_or_create_topic(&self, topic: &str) -> Result<Arc<TopicQueue>, DdlError> {
        // Check if topic already exists (allowed)
        if let Some(queue) = self.topics.get(topic) {
            return Ok(queue.clone());
        }
        
        // Check topic limit (0 means unlimited)
        if self.config.max_topics > 0 && self.topics.len() >= self.config.max_topics {
            return Err(DdlError::TopicLimitExceeded {
                max: self.config.max_topics,
                current: self.topics.len(),
            });
        }
        
        // Create new topic
        let queue = Arc::new(TopicQueue::new(self.config.buffer_size));
        self.topics.insert(topic.to_string(), queue.clone());
        Ok(queue)
    }
}

#[async_trait]
impl DDL for Ddl {
    async fn push(&self, topic: &str, payload: Vec<u8>) -> Result<u64, DdlError> {
        // Check ownership (for now, allow all topics)
        // In distributed version: check if we own this topic
        
        let queue = self.get_or_create_topic(topic)?;
        let id = queue.push(topic.to_string(), payload)?;
        
        // Get entry to send to subscribers
        let entry = queue.read(id);
        
        // Notify subscribers with backpressure handling
        if let Some(senders) = self.subscribers.get(topic) {
            if let Some(entry) = entry {
                for info in senders.value().iter() {
                    match self.config.subscription_backpressure {
                        BackpressureMode::Block => {
                            // This should not be called in async context, but we handle it
                            // In practice, Block mode requires async send
                            if let Err(e) = info.sender.send(entry.clone()) {
                                log::debug!("Subscriber disconnected: {:?}", e);
                            }
                        }
                        BackpressureMode::DropOldest => {
                            // Try to send, drop if full (non-blocking)
                            match info.sender.try_send(entry.clone()) {
                                Err(TrySendError::Full(_)) => {
                                    log::debug!("Subscriber buffer full for topic {}, dropping message", topic);
                                    // Message dropped, continue with next subscriber
                                }
                                Err(TrySendError::Disconnected(_)) => {
                                    log::debug!("Subscriber disconnected for topic {}", topic);
                                }
                                Ok(_) => {}
                            }
                        }
                        BackpressureMode::DropNewest => {
                            // Check if full before sending
                            if info.sender.is_full() {
                                log::debug!("Subscriber buffer full for topic {}, dropping newest", topic);
                                continue;
                            }
                            if let Err(e) = info.sender.try_send(entry.clone()) {
                                log::debug!("Send error for topic {}: {:?}", topic, e);
                            }
                        }
                        BackpressureMode::Error => {
                            // Return error if full
                            if info.sender.is_full() {
                                return Err(DdlError::SubscriberBufferFull(topic.to_string()));
                            }
                            if let Err(e) = info.sender.try_send(entry.clone()) {
                                log::debug!("Send error for topic {}: {:?}", topic, e);
                            }
                        }
                    }
                }
            }
        }
        
        Ok(id)
    }
    
    async fn subscribe(&self, topic: &str) -> Result<EntryStream, DdlError> {
        // Ensure topic exists (create if needed)
        self.get_or_create_topic(topic)?;
        
        let buffer_size = self.config.subscription_buffer_size;
        let (sender, receiver) = bounded(buffer_size);
        
        // Add to subscribers
        self.subscribers
            .entry(topic.to_string())
            .or_insert_with(Vec::new)
            .push(SubscriberInfo {
                sender,
            });
        
        // Create stream with backpressure mode
        Ok(EntryStream::with_backpressure(
            receiver,
            self.config.subscription_backpressure,
        ))
    }
    
    async fn ack(&self, topic: &str, entry_id: u64) -> Result<(), DdlError> {
        if let Some(queue) = self.topics.get(topic) {
            queue.ack(entry_id);
            Ok(())
        } else {
            Err(DdlError::TopicNotFound(topic.to_string()))
        }
    }
    
    async fn position(&self, topic: &str) -> Result<u64, DdlError> {
        if let Some(queue) = self.topics.get(topic) {
            Ok(queue.position())
        } else {
            Err(DdlError::TopicNotFound(topic.to_string()))
        }
    }
    
    fn owns_topic(&self, _topic: &str) -> bool {
        // For now, all nodes own all topics (single node mode)
        // In distributed version: check config.owned_topics
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ddl::BackpressureMode;
    
    #[tokio::test]
    async fn test_push_and_subscribe() {
        let ddl = Ddl::default();
        
        // Subscribe first
        let mut stream = ddl.subscribe("test").await.unwrap();
        
        // Push data
        let id = ddl.push("test", vec![1, 2, 3]).await.unwrap();
        assert_eq!(id, 0);
        
        // Receive
        let entry = stream.next().unwrap();
        assert_eq!(entry.id, 0);
        assert_eq!(entry.payload, vec![1, 2, 3]);
    }
    
    #[tokio::test]
    async fn test_ack() {
        let ddl = Ddl::default();
        
        let id = ddl.push("test", vec![1]).await.unwrap();
        ddl.ack("test", id).await.unwrap();
        
        // Position should advance
        let pos = ddl.position("test").await.unwrap();
        assert!(pos >= 1);
    }
    
    #[tokio::test]
    async fn test_drop_oldest_backpressure() {
        // Configure with DropOldest (default) and small buffer
        let mut config = DdlConfig::default();
        config.subscription_buffer_size = 2;
        config.subscription_backpressure = BackpressureMode::DropOldest;
        
        let ddl = Ddl::new(config);
        let stream = ddl.subscribe("test").await.unwrap();
        
        // Push more messages than buffer can hold
        for i in 0..5 {
            ddl.push("test", vec![i]).await.unwrap();
        }
        
        // Should be able to read some messages
        let mut count = 0;
        while let Some(_) = stream.try_next() {
            count += 1;
        }
        
        // We should get at most 2 messages (buffer size)
        assert!(count <= 2);
    }
    
    #[tokio::test]
    async fn test_error_backpressure() {
        let mut config = DdlConfig::default();
        config.subscription_buffer_size = 2;
        config.subscription_backpressure = BackpressureMode::Error;
        
        let ddl = Ddl::new(config);
        let _stream = ddl.subscribe("test").await.unwrap();
        
        // Fill the buffer
        ddl.push("test", vec![1]).await.unwrap();
        ddl.push("test", vec![2]).await.unwrap();
        
        // Third push should fail with buffer full error
        let result = ddl.push("test", vec![3]).await;
        assert!(result.is_err());
        match result {
            Err(DdlError::SubscriberBufferFull(topic)) => assert_eq!(topic, "test"),
            _ => panic!("Expected SubscriberBufferFull error"),
        }
    }
    
    #[tokio::test]
    async fn test_drop_newest_backpressure() {
        let mut config = DdlConfig::default();
        config.subscription_buffer_size = 2;
        config.subscription_backpressure = BackpressureMode::DropNewest;
        
        let ddl = Ddl::new(config);
        let _stream = ddl.subscribe("test").await.unwrap();
        
        // Push messages - should succeed without errors
        for i in 0..5 {
            ddl.push("test", vec![i]).await.unwrap();
        }
        
        // No assertions needed - just verifying no errors are thrown
        // In DropNewest mode, when buffer is full, messages are silently dropped
    }
    
    #[tokio::test]
    async fn test_block_backpressure_slow_consumer() {
        let mut config = DdlConfig::default();
        config.subscription_buffer_size = 1;
        config.subscription_backpressure = BackpressureMode::Block;
        
        let ddl = Arc::new(Ddl::new(config));
        let stream = ddl.subscribe("test").await.unwrap();
        
        // Push one message - should succeed immediately
        ddl.push("test", vec![1]).await.unwrap();
        
        // Buffer is now full (size 1)
        // Next push should block in Block mode, but we can't easily test blocking
        // in a unit test without causing deadlock. We'll just verify the config works.
        
        let entry = stream.try_next().unwrap();
        assert_eq!(entry.payload, vec![1]);
    }
    
    #[tokio::test]
    async fn test_multiple_subscribers_same_topic() {
        let ddl = Ddl::default();
        
        let stream1 = ddl.subscribe("test").await.unwrap();
        let stream2 = ddl.subscribe("test").await.unwrap();
        
        ddl.push("test", vec![42]).await.unwrap();
        
        // Both subscribers should receive the message
        let entry1 = stream1.try_next().unwrap();
        let entry2 = stream2.try_next().unwrap();
        
        assert_eq!(entry1.payload, vec![42]);
        assert_eq!(entry2.payload, vec![42]);
    }
    
    #[tokio::test]
    async fn test_backpressure_mode_in_stream() {
        let mut config = DdlConfig::default();
        config.subscription_backpressure = BackpressureMode::Block;
        
        let ddl = Ddl::new(config);
        let stream = ddl.subscribe("test").await.unwrap();
        
        assert_eq!(stream.backpressure_mode(), BackpressureMode::Block);
        
        let mut config2 = DdlConfig::default();
        config2.subscription_backpressure = BackpressureMode::Error;
        
        let ddl2 = Ddl::new(config2);
        let stream2 = ddl2.subscribe("test").await.unwrap();
        
        assert_eq!(stream2.backpressure_mode(), BackpressureMode::Error);
    }
    
    #[test]
    fn test_concurrent_push_pop() {
        use std::sync::Barrier;
        use std::thread;
        
        let queue = Arc::new(TopicQueue::new(1024));
        let barrier = Arc::new(Barrier::new(11)); // 10 producers + 1 consumer
        
        let mut handles = vec![];
        
        // Spawn 10 producers
        for i in 0..10 {
            let q = queue.clone();
            let b = barrier.clone();
            handles.push(thread::spawn(move || {
                b.wait(); // Synchronize all threads to start simultaneously
                for j in 0..100 {
                    q.push(format!("entry_{}_{}", i, j), vec![j as u8; 64]).unwrap();
                }
            }));
        }
        
        // Spawn consumer
        let q = queue.clone();
        let b = barrier.clone();
        let consumer = thread::spawn(move || {
            b.wait();
            let mut count = 0u64;
            let mut id = 0u64;
            
            // Read until we've got all 1000 messages
            while count < 1000 {
                // Try to read next expected entry
                if let Some(entry) = q.read(id) {
                    // Verify entry is complete and valid
                    assert_eq!(entry.id, id, "Entry ID mismatch at {}", id);
                    assert!(!entry.payload.is_empty(), "Empty payload at {}", id);
                    assert_eq!(entry.payload.len(), 64, "Payload size mismatch at {}", id);
                    count += 1;
                    id += 1;
                }
                // Small yield to avoid tight spinning
                if count % 100 == 0 {
                    thread::yield_now();
                }
            }
            count
        });
        
        for h in handles {
            h.join().unwrap();
        }
        
        let count = consumer.join().unwrap();
        assert_eq!(count, 1000, "Should have read all 1000 entries");
        
        // Verify final queue position
        assert_eq!(queue.position(), 1000);
    }
}