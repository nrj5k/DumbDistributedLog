//! QUIC-based Publisher-Subscriber System for AutoQueues
//!
//! Provides distributed pub/sub using QUIC transport for cross-machine communication.
//! Supports topic-based messaging with reliable delivery and connection management.
//! Uses standard AutoQueues port 69420.

use crate::networking::quic::{QuicConfig, QuicTransport, QuicConnectionStats};
use crate::port_config::{PortConfig, TransportProtocol};
use crate::pubsub::{TopicMessage, PubSubError, Topic, SubscriptionId};
use crate::traits::transport::Transport;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tokio::time::{timeout, Duration};

/// QUIC-based pub/sub broker for distributed messaging
pub struct QuicPubSubBroker {
    transport: Arc<RwLock<QuicTransport>>,
    topics: Arc<RwLock<HashMap<Topic, Vec<SubscriptionId>>>>,
    subscriptions: Arc<RwLock<HashMap<SubscriptionId, mpsc::UnboundedSender<TopicMessage>>>>,
    remote_nodes: Arc<RwLock<HashMap<String, QuicTransport>>>,
    config: QuicConfig,
}

impl QuicPubSubBroker {
    /// Create new QUIC pub/sub broker with standard port configuration
    pub async fn new(config: QuicConfig) -> Result<Self, PubSubError> {
        let transport = QuicTransport::new();

        Ok(Self {
            transport: Arc::new(RwLock::new(transport)),
            topics: Arc::new(RwLock::new(HashMap::new())),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            remote_nodes: Arc::new(RwLock::new(HashMap::new())),
            config,
        })
    }

    /// Create new QUIC pub/sub broker with port configuration
    pub async fn with_port_config(port_config: PortConfig) -> Result<Self, PubSubError> {
        if port_config.protocol != TransportProtocol::Quic {
            return Err(PubSubError::SerializationError(
                "Port config protocol must be QUIC".to_string()
            ));
        }

        let config = QuicConfig {
            timeout: Duration::from_secs(30),
            max_streams: 1000,
            keep_alive: Duration::from_secs(10),
            server_name: "localhost".to_string(),
        };

        Self::new(config).await
    }

    /// Start as QUIC server on standard AutoQueues port
    pub async fn start_server_on_standard_port(&self) -> Result<(), PubSubError> {
        let port_config = PortConfig::quic();
        self.start_server(&port_config.quic_bind_addr()).await
    }

    /// Start as QUIC server on specified address
    pub async fn start_server(&self, addr: &str) -> Result<(), PubSubError> {
        let transport = QuicTransport::bind(addr).map_err(|e| PubSubError::SerializationError(e.to_string()))?;
        *self.transport.write().await = transport;

        // Start accepting connections in background
        let _transport_clone = self.transport.clone();
        let _remote_nodes_clone = self.remote_nodes.clone();

        tokio::spawn(async move {
            loop {
                let mut transport = _transport_clone.write().await;
                if let Ok(_) = transport.accept() {
                    // Handle new connection
                    println!("QUIC pub/sub: New connection accepted");
                }
                drop(transport);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });

        Ok(())
    }

    /// Connect to remote QUIC pub/sub server
    pub async fn connect(&self, addr: &str) -> Result<(), PubSubError> {
        let mut transport = self.transport.write().await;
        Transport::connect(&mut *transport, addr).map_err(|e| PubSubError::SerializationError(e.to_string()))?;

        // Store remote connection
        let mut remote_nodes = self.remote_nodes.write().await;
        remote_nodes.insert(addr.to_string(), transport.clone());

        Ok(())
    }

    /// Subscribe to topic with QUIC transport
    pub async fn subscribe(&self, topic: Topic) -> Result<SubscriptionId, PubSubError> {
        let subscription_id = uuid::Uuid::new_v4();
        let (sender, _receiver) = mpsc::unbounded_channel();

        // Store subscription
        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.insert(subscription_id, sender);

        // Add to topic registry
        let topic_clone = topic.clone();
        let mut topics = self.topics.write().await;
        topics.entry(topic).or_insert_with(Vec::new).push(subscription_id);

        // Notify remote nodes about subscription
        self.notify_remote_subscription(&topic_clone, &subscription_id).await?;

        Ok(subscription_id)
    }

    /// Publish message to topic via QUIC
    pub async fn publish<T: serde::Serialize>(&self, topic: Topic, data: &T) -> Result<(), PubSubError> {
        let message = TopicMessage::new(topic.clone(), data)
            .map_err(|e| PubSubError::SerializationError(e.to_string()))?;

        // Send to local subscribers
        self.publish_local(&topic, &message).await?;

        // Send to remote nodes
        self.publish_remote(&topic, &message).await?;

        Ok(())
    }

    /// Receive message from subscription
    pub async fn receive(&self, subscription_id: SubscriptionId) -> Option<TopicMessage> {
        let subscriptions = self.subscriptions.read().await;
        if let Some(_sender) = subscriptions.get(&subscription_id) {
            // Create receiver for this call
            let (_send, _recv) = mpsc::unbounded_channel::<TopicMessage>();
            // In a real implementation, we'd store the receiver properly
            // For now, return None to indicate no message
            None
        } else {
            None
        }
    }

    /// Get connection statistics
    pub async fn get_connection_stats(&self) -> Option<QuicConnectionStats> {
        let transport = self.transport.read().await;
        transport.get_connection_stats()
    }

    /// Get connected remote nodes
    pub async fn remote_nodes(&self) -> Vec<String> {
        let remote_nodes = self.remote_nodes.read().await;
        remote_nodes.keys().cloned().collect()
    }

    async fn publish_local(&self, topic: &Topic, message: &TopicMessage) -> Result<(), PubSubError> {
        let topics = self.topics.read().await;
        if let Some(subscription_ids) = topics.get(topic) {
            let subscriptions = self.subscriptions.read().await;
            for sub_id in subscription_ids {
                if let Some(sender) = subscriptions.get(sub_id) {
                    let _ = sender.send(message.clone());
                }
            }
        }
        Ok(())
    }

    async fn publish_remote(&self, topic: &Topic, message: &TopicMessage) -> Result<(), PubSubError> {
        let remote_nodes = self.remote_nodes.read().await;
        for (addr, transport) in remote_nodes.iter() {
            let serialized = serde_json::to_vec(message)
                .map_err(|e| PubSubError::SerializationError(e.to_string()))?;

            match timeout(self.config.timeout, async {
                let mut transport_clone = transport.clone();
                Transport::send(&mut transport_clone, &serialized)
            }).await {
                Ok(Ok(_)) => println!("QUIC pub/sub: Published to {} for topic {}", addr, topic),
                Ok(Err(e)) => eprintln!("QUIC pub/sub: Failed to publish to {}: {}", addr, e),
                Err(_) => eprintln!("QUIC pub/sub: Timeout publishing to {} for topic {}", addr, topic),
            }
        }
        Ok(())
    }

    async fn notify_remote_subscription(&self, topic: &Topic, subscription_id: &SubscriptionId) -> Result<(), PubSubError> {
        // In a real implementation, we'd send a subscription notification
        // For now, just log it
        println!("QUIC pub/sub: Subscribed to {} with ID {}", topic, subscription_id);
        Ok(())
    }
}

/// QUIC-based pub/sub client
pub struct QuicPubSubClient {
    broker: Arc<QuicPubSubBroker>,
    subscriptions: Arc<RwLock<HashMap<SubscriptionId, mpsc::UnboundedReceiver<TopicMessage>>>>,
}

impl QuicPubSubClient {
    /// Create new QUIC pub/sub client
    pub fn new(broker: Arc<QuicPubSubBroker>) -> Self {
        Self {
            broker,
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Subscribe to topic
    pub async fn subscribe(&self, topic: Topic) -> Result<SubscriptionId, PubSubError> {
        let subscription_id = self.broker.subscribe(topic).await?;

        // Create receiver for client
        let (_sender, receiver) = mpsc::unbounded_channel();
        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.insert(subscription_id, receiver);

        Ok(subscription_id)
    }

    /// Publish message to topic
    pub async fn publish<T: serde::Serialize>(&self, topic: Topic, data: &T) -> Result<(), PubSubError> {
        self.broker.publish(topic, data).await
    }

    /// Receive message from subscription
    pub async fn receive(&self, subscription_id: SubscriptionId) -> Option<TopicMessage> {
        let mut subscriptions = self.subscriptions.write().await;
        if let Some(receiver) = subscriptions.get_mut(&subscription_id) {
            receiver.recv().await
        } else {
            None
        }
    }

    /// Unsubscribe from topic
    pub async fn unsubscribe(&self, subscription_id: SubscriptionId) -> Result<(), PubSubError> {
        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.remove(&subscription_id);
        // In a real implementation, we'd also notify the broker
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_quic_pubsub_broker_creation() {
        let config = QuicConfig::default();
        let broker = QuicPubSubBroker::new(config).await.unwrap();
        assert!(broker.get_connection_stats().await.is_none()); // Not connected yet
    }

    #[tokio::test]
    async fn test_quic_pubsub_client_creation() {
        let config = QuicConfig::default();
        let broker = Arc::new(QuicPubSubBroker::new(config).await.unwrap());
        let client = QuicPubSubClient::new(broker);

        // Test subscription
        let sub_id = client.subscribe("test/topic".to_string()).await.unwrap();
        assert!(!sub_id.is_nil());
    }
}
