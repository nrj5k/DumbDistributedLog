//! ZeroMQ-based Publisher-Subscriber System for AutoQueues
//!
//! Provides high-performance distributed pub/sub using ZeroMQ.
//! Supports multiple transport protocols (TCP, IPC, inproc) and
//! advanced messaging patterns with built-in reliability.

pub mod transport;

use crate::constants;

use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::mpsc;
use zmq::{Context, Socket, SocketType};

#[derive(Debug, thiserror::Error)]
pub enum ZmqError {
    #[error("ZMQ error: {0}")]
    ZmqError(#[from] zmq::Error),
}

/// Commands for the socket management task
pub enum SocketCommand {
    Publish { topic: String, data: Vec<u8> },
    Connect(String),
    Shutdown,
}

/// ZeroMQ-based pub/sub broker
pub struct ZmqPubSubBroker {
    context: Context,
    command_tx: mpsc::UnboundedSender<SocketCommand>,
    subscribers: Arc<Mutex<HashMap<String, Socket>>>,
    // topic -> subscriber addresses
    subscriptions: Arc<Mutex<HashMap<String, Vec<String>>>>,
    _bind_addr: String,
}

impl ZmqPubSubBroker {
    /// Create new ZeroMQ pub/sub broker on default port
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let bind_addr = format!(
            "tcp://*:{}",
            constants::network::DEFAULT_NODE_COMMUNICATION_PORT
        );
        Self::with_bind_addr(&bind_addr)
    }

    /// Create new ZeroMQ pub/sub broker with custom bind address
    pub fn with_bind_addr(bind_addr: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let context = Context::new();

        // Create channel for socket commands
        let (command_tx, mut command_rx) = mpsc::unbounded_channel::<SocketCommand>();

        // Spawn dedicated thread for socket operations
        let context_clone = context.clone();
        let bind_addr_clone = bind_addr.to_string();
        std::thread::spawn(move || {
            // Create and bind the publisher socket in the thread
            match context_clone.socket(SocketType::PUB) {
                Ok(publisher) => {
                    if let Err(e) = publisher.bind(&bind_addr_clone) {
                        eprintln!("Failed to bind publisher socket to {}: {}", bind_addr_clone, e);
                        return;
                    }
                    
                    println!("Publisher bound to {}", bind_addr_clone);
                    
                    while let Some(command) = command_rx.blocking_recv() {
                        match command {
                            SocketCommand::Publish { topic, data } => {
                                let mut frame = Vec::new();
                                frame.extend_from_slice(topic.as_bytes());
                                frame.push(0); // Separator
                                frame.extend_from_slice(&data);
                                
                                // Send without waiting (non-blocking)
                                if let Err(e) = publisher.send(&frame, zmq::DONTWAIT) {
                                    eprintln!("Failed to send message: {}", e);
                                }
                            }
                            SocketCommand::Connect(_) => {
                                // Connect is not applicable for broker as it binds, not connects
                                // but we keep this for interface consistency
                            }
                            SocketCommand::Shutdown => {
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to create publisher socket: {}", e);
                }
            }
        });

        Ok(Self {
            context,
            command_tx,
            subscribers: Arc::new(Mutex::new(HashMap::new())),
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
            _bind_addr: bind_addr.to_string(),
        })
    }

    /// Publish message to topic
    pub fn publish<T: Serialize>(
        &self,
        topic: &str,
        data: &T,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let message = serde_json::to_vec(data)?;
        
        self.command_tx.send(SocketCommand::Publish {
            topic: topic.to_string(),
            data: message,
        }).map_err(|e| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to send publish command: {}", e),
            ))
        })?;

        Ok(())
    }

    /// Subscribe to topic pattern (supports ZeroMQ wildcard patterns)
    pub async fn subscribe(
        &self,
        topic_pattern: &str,
        connect_addr: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let subscriber_id = format!("{}@{}", topic_pattern, connect_addr);

        let subscriber = self.context.socket(SocketType::SUB)?;
        subscriber.connect(connect_addr)?;
        subscriber.set_subscribe(topic_pattern.as_bytes())?;

        let mut subscribers = self.subscribers.lock().map_err(|e| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to lock subscribers: {}", e),
            ))
        })?;
        subscribers.insert(subscriber_id.clone(), subscriber);

        // Track subscription
        let mut subscriptions = self.subscriptions.lock().map_err(|e| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to lock subscriptions: {}", e),
            ))
        })?;
        subscriptions
            .entry(topic_pattern.to_string())
            .or_insert_with(Vec::new)
            .push(connect_addr.to_string());

        Ok(subscriber_id)
    }

    /// Receive message from subscriber (non-blocking)
    pub fn receive(
        &self,
        subscriber_id: &str,
    ) -> Result<Option<(String, Vec<u8>)>, Box<dyn std::error::Error + Send + Sync>> {
        let mut subscribers = self.subscribers.lock().map_err(|e| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to lock subscribers: {}", e),
            ))
        })?;
        if let Some(subscriber) = subscribers.get_mut(subscriber_id) {
            match subscriber.recv_bytes(zmq::DONTWAIT) {
                Ok(data) => {
                    // Extract topic and payload
                    if let Some(separator_pos) = data.iter().position(|&b| b == 0) {
                        let topic =
                            String::from_utf8_lossy(&data[..separator_pos]).to_string();
                        let payload = data[separator_pos + 1..].to_vec();
                        Ok(Some((topic, payload)))
                    } else {
                        Ok(None)
                    }
                }
                Err(zmq::Error::EAGAIN) => Ok(None), // No message available
                Err(e) => Err(e.into()),
            }
        } else {
            Ok(None)
        }
    }

    /// Get subscription statistics
    pub fn subscription_stats(&self) -> HashMap<String, usize> {
        match self.subscriptions.lock() {
            Ok(subscriptions) => {
                let mut stats = HashMap::new();

                for (topic, subscribers) in subscriptions.iter() {
                    stats.insert(topic.clone(), subscribers.len());
                }

                stats
            }
            Err(e) => {
                eprintln!("Lock error reading subscriptions: {}", e);
                HashMap::new()
            }
        }
    }

    /// Unsubscribe
    pub fn unsubscribe(&self, subscriber_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut subscribers = self.subscribers.lock().map_err(|e| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to lock subscribers: {}", e),
            ))
        })?;
        let mut subscriptions = self.subscriptions.lock().map_err(|e| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to lock subscriptions: {}", e),
            ))
        })?;

        // Extract topic and address from subscriber_id
        if let Some((topic_pattern, connect_addr)) = subscriber_id.split_once('@') {
            subscriptions
                .entry(topic_pattern.to_string())
                .and_modify(|addrs| {
                    addrs.retain(|addr| addr != connect_addr);
                });
        }

        subscribers.remove(subscriber_id);
        Ok(())
    }
}

/// ZeroMQ pub/sub client
pub struct ZmqPubSubClient {
    context: Context,
    command_tx: mpsc::UnboundedSender<SocketCommand>,
    subscribers: Arc<Mutex<HashMap<String, Socket>>>,
}

impl ZmqPubSubClient {
    /// Create new ZeroMQ pub/sub client
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let context = Context::new();

        // Create channel for socket commands
        let (command_tx, mut command_rx) = mpsc::unbounded_channel::<SocketCommand>();

        // Spawn dedicated thread for socket operations
        let context_clone = context.clone();
        std::thread::spawn(move || {
            let mut publisher: Option<Socket> = None;
            
            while let Some(command) = command_rx.blocking_recv() {
                match command {
                    SocketCommand::Connect(addr) => {
                        match context_clone.socket(SocketType::PUB) {
                            Ok(socket) => {
                                if let Err(e) = socket.connect(&addr) {
                                    eprintln!("Failed to connect publisher socket: {}", e);
                                } else {
                                    publisher = Some(socket);
                                    println!("Connected publisher to {}", addr);
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to create publisher socket: {}", e);
                            }
                        }
                    }
                    SocketCommand::Publish { topic, data } => {
                        if let Some(ref socket) = publisher {
                            let mut frame = Vec::new();
                            frame.extend_from_slice(topic.as_bytes());
                            frame.push(0); // Separator
                            frame.extend_from_slice(&data);
                            
                            // Send without waiting (non-blocking)
                            if let Err(e) = socket.send(&frame, zmq::DONTWAIT) {
                                eprintln!("Failed to send message: {}", e);
                            }
                        } else {
                            eprintln!("No publisher socket available for publishing");
                        }
                    }
                    SocketCommand::Shutdown => {
                        break;
                    }
                }
            }
        });

        Ok(Self {
            context,
            command_tx,
            subscribers: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Connect publisher to broker
    pub fn connect_publisher(&self, addr: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.command_tx.send(SocketCommand::Connect(addr.to_string())).map_err(|e| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to send connect command: {}", e),
            ))
        })?;
        Ok(())
    }

    /// Publish message
    pub fn publish<T: Serialize>(
        &self,
        topic: &str,
        data: &T,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let message = serde_json::to_vec(data)?;
        
        self.command_tx.send(SocketCommand::Publish {
            topic: topic.to_string(),
            data: message,
        }).map_err(|e| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to send publish command: {}", e),
            ))
        })?;

        Ok(())
    }

    /// Subscribe to topic
    pub fn subscribe(
        &self,
        topic_pattern: &str,
        connect_addr: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let subscriber_id = format!("{}@{}", topic_pattern, connect_addr);

        let subscriber = self.context.socket(SocketType::SUB)?;
        subscriber.connect(connect_addr)?;
        subscriber.set_subscribe(topic_pattern.as_bytes())?;

        let mut subscribers = self.subscribers.lock().map_err(|e| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to lock subscribers: {}", e),
            ))
        })?;
        subscribers.insert(subscriber_id.clone(), subscriber);

        Ok(subscriber_id)
    }

    /// Receive message (non-blocking)
    pub fn receive(
        &self,
        subscriber_id: &str,
    ) -> Result<Option<(String, Vec<u8>)>, Box<dyn std::error::Error + Send + Sync>> {
        let mut subscribers = self.subscribers.lock().map_err(|e| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to lock subscribers: {}", e),
            ))
        })?;
        if let Some(subscriber) = subscribers.get_mut(subscriber_id) {
            match subscriber.recv_bytes(zmq::DONTWAIT) {
                Ok(data) => {
                    // Extract topic and payload
                    if let Some(separator_pos) = data.iter().position(|&b| b == 0) {
                        let topic =
                            String::from_utf8_lossy(&data[..separator_pos]).to_string();
                        let payload = data[separator_pos + 1..].to_vec();
                        Ok(Some((topic, payload)))
                    } else {
                        Ok(None)
                    }
                }
                Err(zmq::Error::EAGAIN) => Ok(None), // No message available
                Err(e) => Err(e.into()),
            }
        } else {
            Ok(None)
        }
    }

    /// Unsubscribe from topic
    pub fn unsubscribe(&self, subscriber_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut subscribers = self.subscribers.lock().map_err(|e| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to lock subscribers: {}", e),
            ))
        })?;
        subscribers.remove(subscriber_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct TestMessage {
        content: String,
        value: i32,
    }

    #[tokio::test]
    async fn test_broker_create() {
        let broker = ZmqPubSubBroker::new();
        assert!(broker.is_ok());
    }

    #[tokio::test]
    async fn test_client_create() {
        let client = ZmqPubSubClient::new();
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_broker_publish() {
        // Use dynamic port (0) to avoid port conflicts
        let broker = ZmqPubSubBroker::with_bind_addr("tcp://*:0")
            .expect("Failed to create broker");
        
        let message = TestMessage {
            content: "test".to_string(),
            value: 42,
        };
        
        let result = broker.publish("test.topic", &message);
        // We can't verify delivery without a subscriber, but we can check it doesn't error
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_client_publish() {
        let client = ZmqPubSubClient::new().expect("Failed to create client");
        
        let message = TestMessage {
            content: "test".to_string(),
            value: 42,
        };
        
        let result = client.publish("test.topic", &message);
        // We can't verify delivery without a broker, but we can check it doesn't error
        assert!(result.is_ok());
    }

    #[test]
    fn test_zmq_error_properties() {
        let error: Box<dyn std::error::Error> = Box::new(ZmqError::ZmqError(zmq::Error::EINVAL));
        let _ = error.to_string();
    }
}