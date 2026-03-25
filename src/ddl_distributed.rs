//! Distributed DDL implementation using gossip for topic ownership

use crate::gossip::GossipCoordinator;
use crate::traits::ddl::{validate_topic, DDL, DdlConfig, DdlError, EntryStream};
use crate::topic_queue::{notify_subscribers, SubscriberInfo, TopicQueue};
use async_trait::async_trait;
use crossbeam::channel::bounded;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::task::JoinHandle;

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
    gossip_task_handle: Option<JoinHandle<()>>,
}

impl Drop for DdlDistributed {
    fn drop(&mut self) {
        // Cancel the gossip task if it's running to prevent resource leak
        if let Some(handle) = self.gossip_task_handle.take() {
            handle.abort();
        }
    }
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
            gossip_task_handle: Some(gossip_task_handle),
        })
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
    
    /// Shutdown the DDL instance and all associated tasks
    pub async fn shutdown(&self) {
        // Trigger gossip shutdown
        self.gossip.shutdown().await;
    }
}

#[async_trait]
impl DDL for DdlDistributed {
    async fn push(&self, topic: &str, payload: Vec<u8>) -> Result<u64, DdlError> {
        // Validate topic name (defense-in-depth)
        validate_topic(topic)?;

        // Check ownership using gossip
        if !self.owns_topic_async(topic).await {
            return Err(DdlError::NotOwner(topic.to_string()));
        }
        
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
    
    async fn subscribe(&self, topic: &str) -> Result<EntryStream, DdlError> {
        // Validate topic name (defense-in-depth)
        validate_topic(topic)?;

        // Ensure topic exists (create if needed)
        self.get_or_create_topic(topic)?;
        
        let buffer_size = self.config.subscription_buffer_size;
        let (sender, receiver) = bounded(buffer_size);
        
        // Add to subscribers with metadata
        self.subscribers
            .entry(topic.to_string())
            .or_insert_with(Vec::new)
            .push(SubscriberInfo {
                sender,
                created_at: Some(Instant::now()),
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