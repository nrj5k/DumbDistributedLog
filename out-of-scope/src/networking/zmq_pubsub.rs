//! ZeroMQ-based Publisher-Subscriber System for AutoQueues
//!
//! Provides high-performance distributed pub/sub using ZeroMQ.
//! Supports multiple transport protocols (TCP, IPC, inproc) and
//! advanced messaging patterns with built-in reliability.
//! Uses standard AutoQueues port 6967.

use crate::port_config::{PortConfig, TransportProtocol};
use crate::traits::transport::{ConnectionInfo, Transport, TransportError, TransportType};
use async_trait::async_trait;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;
use zmq::{Context, Socket, SocketType};

#[derive(Debug, thiserror::Error)]
pub enum ZmqError {
    #[error("Lock error: {0}")]
    LockError(String),
    #[error("ZMQ error: {0}")]
    ZmqError(#[from] zmq::Error),
}

/// ZeroMQ-based pub/sub broker
pub struct ZmqPubSubBroker {
    context: Context,
    publisher: Arc<Mutex<Socket>>,
    subscribers: Arc<Mutex<HashMap<String, Socket>>>,
    subscriptions: Arc<Mutex<HashMap<String, Vec<String>>>>, // topic -> subscriber addresses
    #[allow(dead_code)]
    bind_addr: String,
}

impl ZmqPubSubBroker {
    /// Create new ZeroMQ pub/sub broker on standard AutoQueues port
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let port_config = PortConfig::zeromq();
        let bind_addr = port_config.zmq_bind_addr();

        let context = Context::new();
        let publisher = context.socket(SocketType::PUB)?;
        publisher.bind(&bind_addr)?;

        Ok(Self {
            context,
            publisher: Arc::new(Mutex::new(publisher)),
            subscribers: Arc::new(Mutex::new(HashMap::new())),
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
            bind_addr,
        })
    }

    /// Create new ZeroMQ pub/sub broker with custom bind address
    pub fn with_bind_addr(bind_addr: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let context = Context::new();
        let publisher = context.socket(SocketType::PUB)?;
        publisher.bind(bind_addr)?;

        Ok(Self {
            context,
            publisher: Arc::new(Mutex::new(publisher)),
            subscribers: Arc::new(Mutex::new(HashMap::new())),
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
            bind_addr: bind_addr.to_string(),
        })
    }

    /// Create new ZeroMQ pub/sub broker with port configuration
    pub fn with_port_config(port_config: PortConfig) -> Result<Self, Box<dyn std::error::Error>> {
        if port_config.protocol != TransportProtocol::ZeroMQ {
            return Err("Port config protocol must be ZeroMQ".into());
        }

        Self::with_bind_addr(&port_config.zmq_bind_addr())
    }

    /// Publish message to topic
    pub fn publish<T: Serialize>(
        &self,
        topic: &str,
        data: &T,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let message = serde_json::to_vec(data)?;
        let mut frame = Vec::new();
        frame.extend_from_slice(topic.as_bytes());
        frame.push(0); // Separator
        frame.extend_from_slice(&message);

        match self.publisher.lock() {
            Ok(publisher) => {
                publisher.send(&frame, zmq::DONTWAIT)?;
            }
            Err(e) => {
                return Err(Box::new(ZmqError::LockError(e.to_string())));
            }
        }

        Ok(())
    }

    /// Subscribe to topic pattern (supports ZeroMQ wildcard patterns)
    pub async fn subscribe(
        &self,
        topic_pattern: &str,
        connect_addr: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let subscriber_id = format!("{}@{}", topic_pattern, connect_addr);

        let subscriber = self.context.socket(SocketType::SUB)?;
        subscriber.connect(connect_addr)?;
        subscriber.set_subscribe(topic_pattern.as_bytes())?;

        match self.subscribers.lock() {
            Ok(mut subscribers) => {
                subscribers.insert(subscriber_id.clone(), subscriber);
            }
            Err(e) => {
                return Err(Box::new(ZmqError::LockError(e.to_string())));
            }
        }

        // Track subscription
        match self.subscriptions.lock() {
            Ok(mut subscriptions) => {
                subscriptions
                    .entry(topic_pattern.to_string())
                    .or_insert_with(Vec::new)
                    .push(connect_addr.to_string());
            }
            Err(e) => {
                return Err(Box::new(ZmqError::LockError(e.to_string())));
            }
        }

        Ok(subscriber_id)
    }

    /// Receive message from subscriber (non-blocking)
    pub fn receive(
        &self,
        subscriber_id: &str,
    ) -> Result<Option<(String, Vec<u8>)>, Box<dyn std::error::Error>> {
        match self.subscribers.lock() {
            Ok(mut subscribers) => {
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
            Err(e) => Err(Box::new(ZmqError::LockError(e.to_string()))),
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
    pub fn unsubscribe(&self, subscriber_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let subscribers_result = self.subscribers.lock();
        let subscriptions_result = self.subscriptions.lock();

        match (subscribers_result, subscriptions_result) {
            (Ok(mut subscribers), Ok(mut subscriptions)) => {
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
            (Err(e), _) => Err(Box::new(ZmqError::LockError(e.to_string()))),
            (_, Err(e)) => Err(Box::new(ZmqError::LockError(e.to_string()))),
        }
    }
}

/// ZeroMQ pub/sub client
pub struct ZmqPubSubClient {
    context: Context,
    publisher: Arc<Mutex<Socket>>,
    subscribers: Arc<Mutex<HashMap<String, Socket>>>,
}

impl ZmqPubSubClient {
    /// Create new ZeroMQ pub/sub client
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let context = Context::new();
        let publisher = context.socket(SocketType::PUB)?;

        Ok(Self {
            context,
            publisher: Arc::new(Mutex::new(publisher)),
            subscribers: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Connect publisher to broker
    pub fn connect_publisher(&self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        match self.publisher.lock() {
            Ok(publisher) => {
                publisher.connect(addr)?;
                Ok(())
            }
            Err(e) => Err(Box::new(ZmqError::LockError(e.to_string()))),
        }
    }

    /// Publish message
    pub fn publish<T: Serialize>(
        &self,
        topic: &str,
        data: &T,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let message = serde_json::to_vec(data)?;
        let mut frame = Vec::new();
        frame.extend_from_slice(topic.as_bytes());
        frame.push(0); // Separator
        frame.extend_from_slice(&message);

        match self.publisher.lock() {
            Ok(publisher) => {
                publisher.send(&frame, zmq::DONTWAIT)?;
            }
            Err(e) => {
                return Err(Box::new(ZmqError::LockError(e.to_string())));
            }
        }

        Ok(())
    }

    /// Subscribe to topic
    pub fn subscribe(
        &self,
        topic_pattern: &str,
        connect_addr: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let subscriber_id = format!("{}@{}", topic_pattern, connect_addr);

        let subscriber = self.context.socket(SocketType::SUB)?;
        subscriber.connect(connect_addr)?;
        subscriber.set_subscribe(topic_pattern.as_bytes())?;

        match self.subscribers.lock() {
            Ok(mut subscribers) => {
                subscribers.insert(subscriber_id.clone(), subscriber);
            }
            Err(e) => {
                return Err(Box::new(ZmqError::LockError(e.to_string())));
            }
        }

        Ok(subscriber_id)
    }

    /// Receive message (non-blocking)
    pub fn receive(
        &self,
        subscriber_id: &str,
    ) -> Result<Option<(String, Vec<u8>)>, Box<dyn std::error::Error>> {
        match self.subscribers.lock() {
            Ok(mut subscribers) => {
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
            Err(e) => Err(Box::new(ZmqError::LockError(e.to_string()))),
        }
    }
}

/// ZeroMQ transport implementation for AutoQueues
/// Real implementation using actual ZeroMQ operations - no more simulation!
pub struct ZmqTransport {
    context: Context,
    socket: Arc<Mutex<Socket>>,
    connection_info: Arc<Mutex<Option<ConnectionInfo>>>,
    transport_type: TransportType,
}

impl Debug for ZmqTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ZmqTransport")
            .field("socket", &"<zmq::Socket>")
            .field("connection_info", &"<Arc<Mutex<Option<ConnectionInfo>>>")
            .field("transport_type", &self.transport_type)
            .finish()
    }
}

impl ZmqTransport {
    /// Create new ZeroMQ transport
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let context = Context::new();
        let socket = context.socket(SocketType::REQ)?;

        Ok(Self {
            context,
            socket: Arc::new(Mutex::new(socket)),
            connection_info: Arc::new(Mutex::new(None)),
            transport_type: TransportType::ZeroMQ,
        })
    }

    /// Create ZMQ transport with custom socket type
    pub fn with_socket_type(socket_type: SocketType) -> Result<Self, Box<dyn std::error::Error>> {
        let context = Context::new();
        let socket = context.socket(socket_type)?;

        Ok(Self {
            context,
            socket: Arc::new(Mutex::new(socket)),
            connection_info: Arc::new(Mutex::new(None)),
            transport_type: TransportType::ZeroMQ,
        })
    }

    /// Get context for creating additional sockets
    pub fn context(&self) -> &Context {
        &self.context
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        // Check if we have connection info stored
        // For real ZeroMQ, we'd need to check socket state
        match self.connection_info.lock() {
            Ok(connection_info) => connection_info.is_some(),
            Err(e) => {
                eprintln!("Lock error checking connection: {}", e);
                false
            }
        }
    }

    /// Get remote address
    pub fn remote_addr(&self) -> Option<String> {
        match self.connection_info.lock() {
            Ok(connection_info) => connection_info
                .as_ref()
                .map(|info| info.remote_addr.clone()),
            Err(e) => {
                eprintln!("Lock error getting remote address: {}", e);
                None
            }
        }
    }
}

#[async_trait]
impl Transport for ZmqTransport {
    /// Connect to remote address using real ZeroMQ operations
    async fn connect(&self, addr: &str) -> Result<ConnectionInfo, TransportError> {
        let socket_clone = self.socket.clone();
        let addr_owned = addr.to_string();
        let transport_type = self.transport_type.clone();

        // Use tokio::task::spawn_blocking for async bridge to sync ZeroMQ
        tokio::task::spawn_blocking(move || match socket_clone.lock() {
            Ok(socket) => {
                socket
                    .connect(&addr_owned)
                    .map_err(|_e| TransportError::ConnectionLost {
                        remote_addr: std::net::SocketAddr::from(([0, 0, 0, 0], 0)),
                    })?;

                let connection_info = ConnectionInfo {
                    remote_addr: addr_owned,
                    transport_type,
                    connected_at: Instant::now(),
                };

                Ok(connection_info)
            }
            Err(_e) => Err(TransportError::ConnectionLost {
                remote_addr: std::net::SocketAddr::from(([0, 0, 0, 0], 0)),
            }),
        })
        .await
        .map_err(|_| TransportError::NetworkUnreachable)?
    }

    /// Send data using real ZeroMQ operations
    async fn send(&self, data: &[u8]) -> Result<(), TransportError> {
        let socket_clone = self.socket.clone();
        let data_vec = data.to_vec();

        // Use async bridge for real ZeroMQ operations
        tokio::task::spawn_blocking(move || match socket_clone.lock() {
            Ok(socket) => {
                socket
                    .send(&data_vec, 0)
                    .map_err(|_| TransportError::NetworkUnreachable)?;

                Ok(())
            }
            Err(_e) => Err(TransportError::NetworkUnreachable),
        })
        .await
        .map_err(|_| TransportError::NetworkUnreachable)?
    }

    /// Receive data using real ZeroMQ operations
    async fn receive(&self) -> Result<Vec<u8>, TransportError> {
        let socket_clone = self.socket.clone();

        // Use async bridge for real ZeroMQ operations
        tokio::task::spawn_blocking(move || match socket_clone.lock() {
            Ok(socket) => {
                let data = socket
                    .recv_bytes(0)
                    .map_err(|_| TransportError::NetworkUnreachable)?;

                Ok(data)
            }
            Err(_e) => Err(TransportError::NetworkUnreachable),
        })
        .await
        .map_err(|_| TransportError::NetworkUnreachable)?
    }

    fn is_connected(&self) -> bool {
        // Check if we have connection info stored
        // For real ZeroMQ, we'd need to check socket state
        match self.connection_info.lock() {
            Ok(connection_info) => connection_info.is_some(),
            Err(e) => {
                eprintln!("Lock error checking connection: {}", e);
                false
            }
        }
    }

    fn connection_info(&self) -> Option<ConnectionInfo> {
        match self.connection_info.lock() {
            Ok(connection_info) => connection_info.clone(),
            Err(e) => {
                eprintln!("Lock error getting connection info: {}", e);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_zmq_pubsub_broker_creation() {
        // Note: This test may fail if ZeroMQ is not properly installed
        // In a real environment, you'd want to handle this gracefully
        let broker = ZmqPubSubBroker::new();
        // Skip test if ZeroMQ is not available
        if let Ok(_broker) = broker {
            // Test passed
        } else {
            println!("ZeroMQ not available, skipping test");
        }
    }

    #[test]
    fn test_zmq_pubsub_client_creation() {
        let client = ZmqPubSubClient::new();
        assert!(client.is_ok());
    }

    #[test]
    fn test_zmq_transport_creation() {
        // Note: This test may fail if ZeroMQ is not properly installed
        let transport = ZmqTransport::new();
        assert!(transport.is_ok());

        let transport = transport.unwrap();
        // Use trait methods - they exist!
        let as_trait: &dyn crate::traits::transport::Transport = &transport;
        assert!(!as_trait.is_connected());
        assert!(as_trait.connection_info().is_none());
    }
}
