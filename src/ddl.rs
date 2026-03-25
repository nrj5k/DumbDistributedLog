//! DDL - Dumb Distributed Log
//!
//! Core implementation with in-memory ring buffer storage.
//! This is the simplest possible implementation - no WAL, no gossip, no distribution yet.

use crate::traits::ddl::{validate_topic, DDL, DdlConfig, DdlError, EntryStream};
use crate::topic_queue::{notify_subscribers, SubscriberInfo, TopicQueue};
use async_trait::async_trait;
use crossbeam::channel::bounded;
use dashmap::DashMap;
use std::sync::Arc;

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
        // Validate topic name (defense-in-depth)
        validate_topic(topic)?;
        
        // Check ownership (for now, allow all topics)
        // In distributed version: check if we own this topic
        
        // Convert topic to Arc<str> once for zero-copy sharing
        let topic_arc: Arc<str> = topic.into();
        
        let queue = self.get_or_create_topic(topic)?;
        let id = queue.push(topic_arc, payload)?;
        
        // Get entry to send to subscribers
        if let Some(entry) = queue.read(id) {
            // Use shared notification function
            notify_subscribers(
                &self.subscribers,
                topic,
                &entry,
                self.config.subscription_backpressure,
            )?;
        }
        
        Ok(id)
    }
    
    async fn push_batch(&self, topic: &str, payloads: Vec<Vec<u8>>) -> Result<u64, DdlError> {
        // Validate topic name (defense-in-depth)
        validate_topic(topic)?;
        
        if payloads.is_empty() {
            return Err(DdlError::BufferFull(topic.to_string()));
        }
        
        // Convert topic to Arc<str> once for zero-copy sharing
        let topic_arc: Arc<str> = topic.into();
        
        let queue = self.get_or_create_topic(topic)?;
        let mut last_id = 0;
        
        // Push all entries and notify subscribers
        for payload in payloads {
            let id = queue.push(topic_arc.clone(), payload)?;
            last_id = id;
            
            // Get entry and send to subscribers
            if let Some(entry) = queue.read(id) {
                notify_subscribers(
                    &self.subscribers,
                    topic,
                    &entry,
                    self.config.subscription_backpressure,
                )?;
            }
        }
        
        Ok(last_id)
    }
    
    async fn subscribe(&self, topic: &str) -> Result<EntryStream, DdlError> {
        // Validate topic name (defense-in-depth)
        validate_topic(topic)?;

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
                created_at: None,
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
}