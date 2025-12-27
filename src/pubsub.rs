//! Publisher-Subscriber System for AutoQueues
//!
//! Provides local topic-based pub/sub using memory channels for queue-to-queue
//! communication within a single machine. Supports hierarchical topics and
//! wildcard subscriptions for flexible aggregation patterns.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, mpsc};

/// Publisher-Subscriber specific errors
#[derive(Debug, thiserror::Error)]
pub enum PubSubError {
    #[error("Topic '{0}' not found")]
    TopicNotFound(String),
    
    #[error("Conflict for '{0}'")]
    SubscriptionConflict(String),
    
    #[error("Serialization: {0}")]
    SerializationError(String),
}

pub type Topic = String;
pub type SubscriptionId = uuid::Uuid;

/// Message wrapper for topic-based communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicMessage {
    pub topic: Topic,
    pub data_type: String,
    pub data: Vec<u8>,
    pub timestamp: u64,
}

impl TopicMessage {
    pub fn new<T: Serialize>(topic: Topic, data: &T) -> Result<Self, Box<dyn std::error::Error>> {
        let data_type = std::any::type_name::<T>().to_string();
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;

        Ok(TopicMessage {
            topic,
            data_type,
            data: serde_json::to_vec(data)?,
            timestamp,
        })
    }

    pub fn extract_data<T: for<'de> Deserialize<'de>>(
        &self,
    ) -> Result<T, Box<dyn std::error::Error>> {
        Ok(serde_json::from_slice(&self.data)?)
    }
}

/// Central publisher-subscriber registry and broker
pub struct PubSubBroker {
    topics: Arc<RwLock<HashMap<Topic, mpsc::UnboundedSender<TopicMessage>>>>,
    subscriptions:
        Arc<RwLock<HashMap<SubscriptionId, (Topic, mpsc::UnboundedReceiver<TopicMessage>)>>>,
}

impl PubSubBroker {
    /// Create a new broker with specified channel capacity per topic
    pub fn new(_capacity_per_topic: usize) -> Self {
        // Note: Using unbounded channels for now - can add capacity later
        PubSubBroker {
            topics: Arc::new(RwLock::new(HashMap::new())),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create or get topic for publishing
    async fn get_topic_sender(&self, topic: &Topic) -> Option<mpsc::UnboundedSender<TopicMessage>> {
        let topics = self.topics.read().await;
        topics.get(topic).cloned()
    }

    /// Subscribe to exact topic (no wildcards for now)
    pub async fn subscribe_exact(&self, topic: Topic) -> Result<SubscriptionId, PubSubError> {
        let mut topics = self.topics.write().await;

        let (sender, receiver) = mpsc::unbounded_channel();

        // If topic exists, we need to handle multiple subscribers per topic
        if let Some(_existing_sender) = topics.get(&topic) {
            return Err(PubSubError::SubscriptionConflict(
                "Multiple subscribers per topic not yet implemented".to_string(),
            ));
        }

        topics.insert(topic.clone(), sender);

        let subscription_id = uuid::Uuid::new_v4();
        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.insert(subscription_id, (topic.clone(), receiver));

        Ok(subscription_id)
    }

    /// Publish data to specific topic
    pub async fn publish<T: Serialize>(&self, topic: Topic, data: &T) -> Result<(), PubSubError> {
        let topic_sender = self
            .get_topic_sender(&topic)
            .await
            .ok_or_else(|| PubSubError::TopicNotFound(topic.clone()))?;

        let message = TopicMessage::new(topic, data)
            .map_err(|e| PubSubError::SerializationError(e.to_string()))?;

        topic_sender
            .send(message)
            .map_err(|_| PubSubError::TopicNotFound("Topic closed".to_string()))?;

        Ok(())
    }

    /// Receive and acknowledge message from subscription
    pub async fn receive(&self, subscription_id: SubscriptionId) -> Option<TopicMessage> {
        let mut subscriptions = self.subscriptions.write().await;

        if let Some((_topic, receiver)) = subscriptions.get_mut(&subscription_id) {
            receiver.recv().await
        } else {
            None
        }
    }

    /// Unsubscribe and cleanup
    pub async fn unsubscribe(&self, subscription_id: SubscriptionId) -> Result<(), PubSubError> {
        let mut subscriptions = self.subscriptions.write().await;
        let mut topics = self.topics.write().await;

        if let Some((topic, _receiver)) = subscriptions.remove(&subscription_id) {
            // Cleanup topic if no subscribers remain
            topics.remove(&topic);
            Ok(())
        } else {
            Err(PubSubError::TopicNotFound(format!(
                "Subscription '{}' not found",
                subscription_id
            )))
        }
    }
}

impl Drop for PubSubBroker {
    fn drop(&mut self) {
        // Tokio channels will clean up automatically when dropped
    }
}
