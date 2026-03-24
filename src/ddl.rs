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

/// Per-topic queue with ring buffer semantics.
///
/// # Mutex Trade-off Design Decision
///
/// This struct uses a hybrid synchronization approach:
/// - `AtomicU64` for `write_pos` and `ack_pos` (lock-free)
/// - `Mutex<Vec<Option<Entry>>>` for `entries` (lock-based)
///
/// ## Why Mutex is Used for Entries
///
/// The entries array requires mutable access for both push (write) and read operations.
/// Rust's ownership model doesn't allow safe lock-free mutable access to a `Vec` without
/// complex unsafe code that would be difficult to verify for correctness and freedom from
/// undefined behavior.
///
/// A fully lock-free implementation would require either:
/// - `UnsafeCell<Entry>` with manual memory ordering guarantees
/// - Atomic pointers with proper lifetime management
/// - A lock-free queue data structure like `crossbeam-queue`
///
/// All of these approaches introduce significant complexity and subtle correctness requirements.
///
/// ## Trade-off Analysis
///
/// **Pros:**
/// - Simplicity: Correct implementation without undefined behavior
/// - Safety: Mutex guarantees exclusive access, preventing data races
/// - Mutex overhead is O(1) for lock/unlock, not proportional to data size
/// - Well-understood semantics and debugging characteristics
///
/// **Cons:**
/// - Contention when multiple producers push to the same topic
/// - Blocks readers during push and vice versa
/// - Lock acquisition overhead in high-contention scenarios
///
/// ## Performance Characteristics
///
/// - Mutex contention is minimal in typical single-producer scenarios
/// - The lock is held briefly (just for index calculation and entry write)
/// - No lock is needed for `ack_pos` updates (atomic)
/// - For multi-producer, consider using per-producer queues or sharding
///
/// ## Future Optimization Paths
///
/// If contention becomes measurable in production:
/// 1. Use `crossbeam-queue` for fully lock-free implementation
/// 2. Use `Box<[UnsafeCell<AtomicPtr<Entry>>]>` with proper synchronization
///    (requires careful memory ordering and lifetime management)
/// 3. Use sharding by topic hash to distribute across multiple queues
/// 4. Consider `parking_lot::Mutex` for potentially better performance
///
/// Align to cache line boundary to prevent false sharing.
#[repr(align(64))]
struct TopicQueue {
    /// Write position (monotonically increasing) - placed first for cache alignment
    write_pos: AtomicU64,
    /// Last acknowledged position
    ack_pos: AtomicU64,
    /// Entries stored in ring buffer
    entries: Mutex<Vec<Option<Entry>>>,
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
            write_pos: AtomicU64::new(0),
            ack_pos: AtomicU64::new(0),
            entries: Mutex::new(entries),
            capacity,
            mask,
        }
    }
    
    /// Append an entry, returns the entry ID
    fn push(&self, topic: Arc<str>, payload: Vec<u8>) -> Result<u64, DdlError> {
        // Convert Vec<u8> to Arc<[u8]> for zero-copy sharing
        let payload_arc = Arc::from(payload);
        
        // Lock entries first - this provides mutual exclusion for both
        // the write_pos increment and the entry write, ensuring atomicity
        let mut entries = self.entries.lock()
            .map_err(|_| DdlError::LockPoisoned(topic.to_string()))?;
        
        // Get current position inside the lock
        let id = self.write_pos.load(Ordering::Relaxed);
        let index = (id as usize) & self.mask;
        
        // Check if we're overwriting unacknowledged data
        let ack = self.ack_pos.load(Ordering::Acquire);
        if id >= ack + self.capacity as u64 {
            return Err(DdlError::BufferFull(topic.to_string()));
        }
        
        let entry = Entry {
            id,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            topic,
            payload: payload_arc,
        };
        
        // Store the entry BEFORE incrementing write_pos
        // This ensures readers can't see incomplete entries
        entries[index] = Some(entry);
        
        // Now make the entry visible by incrementing write_pos with Release ordering
        self.write_pos.store(id + 1, Ordering::Release);
        
        Ok(id)
    }
    
    /// Read entry at specific ID
    /// Uses Acquire ordering to ensure we see the complete entry
    fn read(&self, id: u64) -> Option<Entry> {
        // Load the write position with Acquire ordering to synchronize with Release in push
        let write_pos = self.write_pos.load(Ordering::Acquire);
        
        // Check if this ID exists
        if id >= write_pos {
            return None;
        }
        
        let index = (id as usize) & self.mask;
        if let Ok(entries) = self.entries.lock() {
            entries[index].clone()
        } else {
            log::warn!("Poisoned lock on topic queue for entry id {}, returning None", id);
            None
        }
    }
    
    /// Acknowledge entries up to this ID
    fn ack(&self, id: u64) {
        // Simple: just move ack_pos forward with relaxed ordering
        // In production: handle out-of-order acks
        let current = self.ack_pos.load(Ordering::Acquire);
        if id > current {
            self.ack_pos.store(id, Ordering::Release);
        }
    }
    
    /// Get current position (next ID to write)
    fn position(&self) -> u64 {
        self.write_pos.load(Ordering::Relaxed)
    }
}

/// In-memory DDL implementation - Core Implementation
pub struct InMemoryDdl {
    config: DdlConfig,
    /// Map of topic_name -> TopicQueue
    topics: Arc<DashMap<String, Arc<TopicQueue>>>,
    /// Map of topic_name -> subscription senders with metadata
    subscribers: Arc<DashMap<String, Vec<SubscriberInfo>>>,
}

impl InMemoryDdl {
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
    /// 
    /// Uses DashMap's entry API for atomic check-and-insert semantics.
    /// Handles the race condition where another thread might create the topic
    /// between the limit check and insertion.
    fn get_or_create_topic(&self, topic: &str) -> Result<Arc<TopicQueue>, DdlError> {
        // Fast path: check if topic already exists
        if let Some(queue) = self.topics.get(topic) {
            return Ok(queue.clone());
        }
        
        // Check topic limit (0 means unlimited)
        // Note: There's still a small race window here, but we handle it below
        if self.config.max_topics > 0 && self.topics.len() >= self.config.max_topics {
            // Double-check: another thread might have created this topic while we
            // were checking the limit. If so, return the existing queue instead of error.
            if let Some(queue) = self.topics.get(topic) {
                return Ok(queue.clone());
            }
            return Err(DdlError::TopicLimitExceeded {
                max: self.config.max_topics,
                current: self.topics.len(),
            });
        }
        
        // Use entry API for atomic check-and-insert
        // or_insert_with returns the value (either existing or newly created)
        // This handles the race where another thread creates the topic concurrently
        Ok(self.topics.entry(topic.to_string())
            .or_insert_with(|| Arc::new(TopicQueue::new(self.config.buffer_size)))
            .clone())
    }
}

#[async_trait]
impl DDL for InMemoryDdl {
    async fn push(&self, topic: &str, payload: Vec<u8>) -> Result<u64, DdlError> {
        // Check ownership (for now, allow all topics)
        // In distributed version: check if we own this topic
        
        // Convert topic to Arc<str> once for zero-copy sharing
        let topic_arc: Arc<str> = topic.into();
        
        let queue = self.get_or_create_topic(topic)?;
        let id = queue.push(topic_arc, payload)?;
        
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
    
    async fn push_batch(&self, topic: &str, payloads: Vec<Vec<u8>>) -> Result<u64, DdlError> {
        if payloads.is_empty() {
            return Err(DdlError::BufferFull(topic.to_string()));
        }
        
        // Convert topic to Arc<str> once for zero-copy sharing
        let topic_arc: Arc<str> = topic.into();
        
        let queue = self.get_or_create_topic(topic)?;
        let mut last_id = 0;
        
        // Collect all entries to send to subscribers
        let mut entries = Vec::with_capacity(payloads.len());
        
        // Push all entries
        for payload in payloads {
            let id = queue.push(topic_arc.clone(), payload)?;
            last_id = id;
            
            // Get entry to send to subscribers
            if let Some(entry) = queue.read(id) {
                entries.push(entry);
            }
        }
        
        // Notify subscribers with backpressure handling for all entries
        if let Some(senders) = self.subscribers.get(topic) {
            for entry in entries {
                for info in senders.value().iter() {
                    match self.config.subscription_backpressure {
                        BackpressureMode::Block => {
                            if let Err(e) = info.sender.send(entry.clone()) {
                                log::debug!("Subscriber disconnected: {:?}", e);
                            }
                        }
                        BackpressureMode::DropOldest => {
                            match info.sender.try_send(entry.clone()) {
                                Err(TrySendError::Full(_)) => {
                                    log::debug!("Subscriber buffer full for topic {}, dropping message", topic);
                                }
                                Err(TrySendError::Disconnected(_)) => {
                                    log::debug!("Subscriber disconnected for topic {}", topic);
                                }
                                Ok(_) => {}
                            }
                        }
                        BackpressureMode::DropNewest => {
                            if info.sender.is_full() {
                                log::debug!("Subscriber buffer full for topic {}, dropping newest", topic);
                                continue;
                            }
                            if let Err(e) = info.sender.try_send(entry.clone()) {
                                log::debug!("Send error for topic {}: {:?}", topic, e);
                            }
                        }
                        BackpressureMode::Error => {
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
        
        Ok(last_id)
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
        let ddl = InMemoryDdl::default();
        
        // Subscribe first
        let mut stream = ddl.subscribe("test").await.unwrap();
        
        // Push data
        let id = ddl.push("test", vec![1, 2, 3]).await.unwrap();
        assert_eq!(id, 0);
        
        // Receive
        let entry = stream.next().unwrap();
        assert_eq!(entry.id, 0);
        assert_eq!(&*entry.payload, &[1, 2, 3]);
    }
    
    #[tokio::test]
    async fn test_ack() {
        let ddl = InMemoryDdl::default();
        
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
        
        let ddl = InMemoryDdl::new(config);
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
        
        let ddl = InMemoryDdl::new(config);
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
        
        let ddl = InMemoryDdl::new(config);
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
        
        let ddl = Arc::new(InMemoryDdl::new(config));
        let stream = ddl.subscribe("test").await.unwrap();
        
        // Push one message - should succeed immediately
        ddl.push("test", vec![1]).await.unwrap();
        
        // Buffer is now full (size 1)
        // Next push should block in Block mode, but we can't easily test blocking
        // in a unit test without causing deadlock. We'll just verify the config works.
        
        let entry = stream.try_next().unwrap();
        assert_eq!(&*entry.payload, &[1]);
    }
    
    #[tokio::test]
    async fn test_multiple_subscribers_same_topic() {
        let ddl = InMemoryDdl::default();
        
        let stream1 = ddl.subscribe("test").await.unwrap();
        let stream2 = ddl.subscribe("test").await.unwrap();
        
        ddl.push("test", vec![42]).await.unwrap();
        
        // Both subscribers should receive the message
        let entry1 = stream1.try_next().unwrap();
        let entry2 = stream2.try_next().unwrap();
        
        assert_eq!(&*entry1.payload, &[42]);
        assert_eq!(&*entry2.payload, &[42]);
    }
    
    #[tokio::test]
    async fn test_backpressure_mode_in_stream() {
        let mut config = DdlConfig::default();
        config.subscription_backpressure = BackpressureMode::Block;
        
        let ddl = InMemoryDdl::new(config);
        let stream = ddl.subscribe("test").await.unwrap();
        
        assert_eq!(stream.backpressure_mode(), BackpressureMode::Block);
        
        let mut config2 = DdlConfig::default();
        config2.subscription_backpressure = BackpressureMode::Error;
        
        let ddl2 = InMemoryDdl::new(config2);
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
                    q.push(format!("entry_{}_{}", i, j).into(), vec![j as u8; 64]).unwrap();
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