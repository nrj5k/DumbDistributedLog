//! Distributed DDL implementation using gossip for topic ownership

use crate::gossip::GossipCoordinator;
use crate::traits::ddl::{BackpressureMode, DDL, DdlConfig, DdlError, Entry, EntryStream};
use async_trait::async_trait;
use crossbeam::channel::{bounded, Sender, TrySendError};
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;
use tokio::task::JoinHandle;

/// Subscriber metadata
struct SubscriberInfo {
    sender: Sender<Entry>,
    created_at: Instant,
}

/// Per-topic queue with ring buffer semantics
struct TopicQueue {
    /// Entries stored in ring buffer
    entries: Mutex<Vec<Option<Entry>>>,
    /// Write position (monotonically increasing)
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
        let id = self.write_pos.fetch_add(1, Ordering::SeqCst);
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
                .unwrap()
                .as_nanos() as u64,
            topic,
            payload,
        };
        
        // Store the entry
        if let Ok(mut entries) = self.entries.lock() {
            entries[index] = Some(entry);
        }
        
        Ok(id)
    }
    
    /// Read entry at specific ID
    fn read(&self, id: u64) -> Option<Entry> {
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

/// Distributed DDL implementation using gossip for topic ownership
pub struct DdlDistributed {
    config: DdlConfig,
    /// Map of topic_name -> TopicQueue
    topics: Arc<DashMap<String, Arc<TopicQueue>>>,
    /// Map of topic_name -> subscription senders with metadata
    subscribers: Arc<DashMap<String, Vec<SubscriberInfo>>>,
    /// Gossip coordinator for topic ownership discovery
    gossip: Arc<GossipCoordinator>,
    /// Handle to the gossip message handler task
    _gossip_task_handle: Option<JoinHandle<()>>,
}

impl DdlDistributed {
    /// Create new distributed DDL instance
    pub async fn new(config: DdlConfig) -> Result<Self, DdlError> {
        let topics = Arc::new(DashMap::new());
        let subscribers = Arc::new(DashMap::new());
        
        // Create gossip coordinator
        let gossip = Arc::new(
            GossipCoordinator::new(
                format!("node-{}", config.node_id),
                &config.gossip_bind_addr,
                config.gossip_bootstrap.clone(),
            )
            .await
            .map_err(|e| DdlError::Network(format!("Failed to create gossip: {:?}", e)))?,
        );
        
        // Pre-create queues for owned topics
        for topic in &config.owned_topics {
            let queue = Arc::new(TopicQueue::new(config.buffer_size));
            topics.insert(topic.clone(), queue);
            
            // Join gossip for this topic
            gossip.join_topic(topic).await.map_err(|e| {
                DdlError::Network(format!("Failed to join gossip for topic {}: {:?}", topic, e))
            })?;
        }
        
        // Set owned topics in gossip
        gossip.set_owned_topics(config.owned_topics.clone()).await;
        
        // Start gossip message handler (in background)
        let gossip_clone = gossip.clone();
        let gossip_task_handle = tokio::spawn(async move {
            gossip_clone.handle_messages().await;
        });
        
        Ok(Self {
            config,
            topics,
            subscribers,
            gossip,
            _gossip_task_handle: Some(gossip_task_handle),
        })
    }
    
    /// Get or create a topic queue
    fn get_or_create_topic(&self, topic: &str) -> Arc<TopicQueue> {
        self.topics
            .entry(topic.to_string())
            .or_insert_with(|| Arc::new(TopicQueue::new(self.config.buffer_size)))
            .clone()
    }
    
    /// Shutdown the DDL instance and all associated tasks
    pub async fn shutdown(&self) {
        // Trigger gossip shutdown
        self.gossip.shutdown().await;
    }
}

#[async_trait]
impl DDL for DdlDistributed {
    async fn push(&self, topic: &str, payload: Vec<u8>) -> Result<u64, DdlError> {
        // Check ownership using gossip
        if !self.owns_topic_async(topic).await {
            return Err(DdlError::NotOwner(topic.to_string()));
        }
        
        let queue = self.get_or_create_topic(topic);
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
        // In a real implementation, we would check if the topic exists or can be created
        // For now, we'll just create a channel and return a stream
        
        let buffer_size = self.config.subscription_buffer_size;
        let (sender, receiver) = bounded(buffer_size);
        
        // Add to subscribers with metadata
        self.subscribers
            .entry(topic.to_string())
            .or_insert_with(Vec::new)
            .push(SubscriberInfo {
                sender,
                created_at: Instant::now(),
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
    
    fn owns_topic(&self, topic: &str) -> bool {
        // In a real implementation, we would check with gossip to see if we own the topic
        // For now, we'll check our config
        self.config.owned_topics.contains(&topic.to_string())
    }
}

// Add async version of owns_topic
impl DdlDistributed {
    /// Check if this node owns a topic (using gossip)
    pub async fn owns_topic_async(&self, topic: &str) -> bool {
        // First check our local config
        if self.config.owned_topics.contains(&topic.to_string()) {
            return true;
        }
        
        // Then check gossip
        if let Some(owner) = self.gossip.get_topic_owner(topic).await {
            owner == format!("node-{}", self.config.node_id)
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ddl::DdlConfig;
    
    #[tokio::test]
    async fn test_distributed_ddl_creation() {
        let mut config = DdlConfig::default();
        config.gossip_enabled = true;
        config.gossip_bind_addr = "127.0.0.1:0".to_string();
        config.owned_topics = vec!["test".to_string()];
        
        let ddl = DdlDistributed::new(config)
            .await
            .expect("Failed to create distributed DDL");
        
        assert!(ddl.owns_topic("test"));
        assert!(!ddl.owns_topic("other"));
    }
}