//! Hybrid transport for DDL - supporting both ZMQ (fast, unreliable) and TCP (reliable, backpressure)

use crate::traits::ddl::Entry;
use crate::network::pubsub::zmq::ZmqPubSubBroker;
use crate::network::tcp::{TcpTransport, NetworkMessage};
use async_trait::async_trait;
use log::info;

/// Transport trait - abstract over ZMQ and TCP
#[async_trait]
pub trait Transport: Send + Sync {
    /// Send entry to a topic
    async fn send(&self, topic: &str, entry: Entry) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    
    /// Receive entries
    async fn recv(&mut self) -> Option<Entry>;
    
    /// Subscribe to topic
    async fn subscribe(&self, topic: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Transport configuration
pub enum TransportConfig {
    /// ZMQ transport (fast, may drop)
    Zmq {
        bind_addr: String,
    },
    /// TCP transport (reliable, backpressure)
    Tcp {
        bind_addr: String,
        max_connections: usize,
    },
}

/// Hybrid transport that can use either ZMQ or TCP based on configuration
pub struct HybridTransport {
    zmq_broker: Option<ZmqPubSubBroker>,
    tcp_transport: Option<TcpTransport>,
    config: TransportConfig,
}

impl HybridTransport {
    /// Create new hybrid transport with specified configuration
    pub async fn new(config: TransportConfig) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut transport = Self {
            zmq_broker: None,
            tcp_transport: None,
            config,
        };
        
        match &transport.config {
            TransportConfig::Zmq { bind_addr } => {
                info!("Initializing ZMQ transport on {}", bind_addr);
                let broker = ZmqPubSubBroker::with_bind_addr(bind_addr)?;
                transport.zmq_broker = Some(broker);
            }
            TransportConfig::Tcp { bind_addr, max_connections } => {
                info!("Initializing TCP transport on {} with max connections {}", bind_addr, max_connections);
                let tcp_transport = TcpTransport::new(bind_addr, *max_connections).await?;
                tcp_transport.start_listener().await?;
                transport.tcp_transport = Some(tcp_transport);
            }
        }
        
        Ok(transport)
    }
    
    /// Connect to a peer node
    pub async fn connect_peer(&self, node_id: &str, addr: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match &self.config {
            TransportConfig::Zmq { .. } => {
                // For ZMQ, we establish connections through the client side
                // Peers will connect to our broker
                info!("Peer connection for ZMQ transport handled by clients connecting to broker");
                Ok(())
            }
            TransportConfig::Tcp { .. } => {
                if let Some(tcp) = &self.tcp_transport {
                    info!("Connecting to peer {} at {}", node_id, addr);
                    tcp.connect_to_peer(node_id, addr).await.map_err(|e| {
                        Box::new(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("Failed to connect to peer {}: {}", node_id, e),
                        ))
                    })?;
                }
                Ok(())
            }
        }
    }
    
    async fn recv(&mut self) -> Option<Entry> {
        match &self.config {
            TransportConfig::Zmq { .. } => {
                // This would require a subscriber implementation
                // For now, we'll return None
                None
            }
            TransportConfig::Tcp { .. } => {
                if let Some(tcp) = &mut self.tcp_transport {
                    if let Some(msg) = tcp.recv().await {
                        match msg {
                            NetworkMessage::Push { entry, .. } => {
                                return Some(entry);
                            }
                            _ => {
                                // Handle other message types if needed
                            }
                        }
                    }
                }
                None
            }
        }
    }
    
    async fn subscribe(&self, topic: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match &self.config {
            TransportConfig::Zmq { .. } => {
                if let Some(_broker) = &self.zmq_broker {
                    // In a real implementation, we'd create a subscriber
                    info!("Subscribe called for ZMQ transport on topic: {}", topic);
                    Ok(())
                } else {
                    Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::NotConnected,
                        "ZMQ broker not initialized",
                    )))
                }
            }
            TransportConfig::Tcp { .. } => {
                if let Some(tcp) = &self.tcp_transport {
                    let msg = NetworkMessage::Subscribe {
                        topic: topic.to_string(),
                        subscriber_id: "hybrid_transport_subscriber".to_string(),
                    };
                    tcp.broadcast(msg).await.map_err(|e| {
                        Box::new(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("Failed to subscribe via TCP: {}", e),
                        ))
                    })?;
                    Ok(())
                } else {
                    Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::NotConnected,
                        "TCP transport not initialized",
                    )))
                }
            }
        }
    }
}