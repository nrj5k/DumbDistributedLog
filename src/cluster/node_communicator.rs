//! Inter-node communication for distributed data sharing
//!
//! Uses ZMQ for efficient data replication between cluster nodes.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use serde::{Serialize, Deserialize};
use log::{info, warn, debug, error};

/// Data message sent between nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataMessage {
    pub source_node: u64,
    pub queue_name: String,
    pub timestamp: u64,
    pub value: f64,
}

/// Handles communication with other cluster nodes
pub struct NodeCommunicator {
    node_id: u64,
    bind_addr: String,
    /// Receiver channel for incoming data
    data_receiver: mpsc::Receiver<DataMessage>,
    /// Known peer nodes
    peers: Arc<RwLock<HashMap<u64, String>>>, // node_id -> address
}

impl NodeCommunicator {
    /// Create new NodeCommunicator
    pub fn new(node_id: u64, bind_addr: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let (tx, rx) = mpsc::channel(1000);
        
        let communicator = Self {
            node_id,
            bind_addr: bind_addr.to_string(),
            data_receiver: rx,
            peers: Arc::new(RwLock::new(HashMap::new())),
        };
        
        // Start background receiver task
        communicator.start_receiver_task(tx, bind_addr.to_string());
        
        Ok(communicator)
    }
    
    /// Add a peer node to communicate with
    pub async fn add_peer(&self, peer_id: u64, peer_addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut peers = self.peers.write().await;
        peers.insert(peer_id, peer_addr.to_string());
        
        info!("Node {} added peer {} at {}", self.node_id, peer_id, peer_addr);
        Ok(())
    }
    
    /// Publish data to all peers
    pub fn publish_data(&self, queue_name: &str, value: f64) -> Result<(), Box<dyn std::error::Error>> {
        let message = DataMessage {
            source_node: self.node_id,
            queue_name: queue_name.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis() as u64,
            value,
        };
        
        let serialized = serde_json::to_vec(&message)?;
        
        // Create publisher socket for sending to all peers
        let context = zmq::Context::new();
        let publisher = context.socket(zmq::PUB)?;
        publisher.set_linger(0)?; // Don't wait for messages to be sent
        publisher.set_sndhwm(1000)?; // Set high water mark
        
        // Connect to all known peers
        let peers = self.peers.try_read().map_err(|_| "Failed to acquire peers lock")?;
        for (_, peer_addr) in peers.iter() {
            // Ignore connection errors - peers might not be online yet
            if let Err(e) = publisher.connect(peer_addr) {
                debug!("Failed to connect to peer {}: {}", peer_addr, e);
            }
        }
        drop(peers); // Release the lock
        
        publisher.send(&serialized, 0)?;
        
        debug!("Node {} published data to {}: {}", self.node_id, queue_name, value);
        Ok(())
    }
    
    /// Receive data from peers (non-blocking)
    pub async fn receive_data(&mut self) -> Option<DataMessage> {
        self.data_receiver.recv().await
    }
    
    /// Start background task to receive data from peers
    fn start_receiver_task(&self, sender: mpsc::Sender<DataMessage>, bind_addr: String) {
        let node_id = self.node_id;
        
        std::thread::spawn(move || {
            let context = zmq::Context::new();
            let subscriber = context.socket(zmq::SUB).expect("Failed to create subscriber socket");
            subscriber.set_rcvhwm(1000).expect("Failed to set receive high water mark");
            
            // Bind to listen for incoming connections
            if let Err(e) = subscriber.bind(&bind_addr) {
                error!("Failed to bind subscriber on {}: {}", bind_addr, e);
                return;
            }
            
            // Subscribe to all messages
            subscriber.set_subscribe(b"").expect("Failed to set subscription");
            
            info!("Node {} listening on {}", node_id, bind_addr);
            
            loop {
                match subscriber.recv_bytes(0) {
                    Ok(bytes) => {
                        match serde_json::from_slice::<DataMessage>(&bytes) {
                            Ok(message) => {
                                if message.source_node != node_id {  // Don't process our own messages
                                    debug!("Node {} received data from node {}: {:?}", 
                                           node_id, message.source_node, message);
                                    
                                    // Send to async channel
                                    let sender_clone = sender.clone();
                                    std::thread::spawn(move || {
                                        let rt = tokio::runtime::Builder::new_current_thread()
                                            .enable_all()
                                            .build()
                                            .expect("Failed to create runtime");
                                        
                                        rt.block_on(async {
                                            if sender_clone.send(message).await.is_err() {
                                                error!("Failed to send received data to channel");
                                            }
                                        });
                                    });
                                }
                            }
                            Err(e) => {
                                warn!("Failed to deserialize message: {:?}", e);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Error receiving data on node {}: {:?}", node_id, e);
                    }
                }
            }
        });
    }
    
    /// Get list of connected peers
    pub async fn get_peers(&self) -> Vec<u64> {
        let peers = self.peers.read().await;
        peers.keys().copied().collect()
    }
    
    /// Check if a peer is connected
    pub async fn is_peer_connected(&self, peer_id: u64) -> bool {
        let peers = self.peers.read().await;
        peers.contains_key(&peer_id)
    }
    
    /// Get the bind address for this node
    pub fn get_bind_address(&self) -> &str {
        &self.bind_addr
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_node_communicator_creation() {
        let communicator = NodeCommunicator::new(1, "tcp://127.0.0.1:5555");
        assert!(communicator.is_ok());
    }
    
    #[tokio::test]
    async fn test_add_peer() {
        let comm = NodeCommunicator::new(1, "tcp://127.0.0.1:5556").unwrap();
        let result = comm.add_peer(2, "tcp://127.0.0.1:5557").await;
        assert!(result.is_ok());
        assert!(comm.is_peer_connected(2).await);
    }
    
    // Note: Integration tests that require actual network communication
    // are not included here to avoid port conflicts during testing
}