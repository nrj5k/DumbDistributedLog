//! TCP transport for DDL with backpressure
//!
//! Provides reliable, ordered message delivery with flow control.
//! Unlike ZMQ, TCP will block rather than drop messages.

use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::time::{timeout, Duration};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, Semaphore};
use crate::traits::ddl::Entry;
use crate::network::transport_traits::TransportError;
use log::{info, debug, warn, error};

/// TCP transport server
pub struct TcpTransport {
    /// Bind address
    bind_addr: String,
    /// Connected peers (node_id -> connection)
    peers: Arc<RwLock<HashMap<String, PeerConnection>>>,
    /// Channel for receiving entries from network
    receiver: mpsc::Receiver<NetworkMessage>,
    /// Sender for the receiver channel (needed for cloning)
    sender: mpsc::Sender<NetworkMessage>,
    /// Connection semaphore (for backpressure)
    connection_sem: Arc<Semaphore>,
}

/// Peer connection handle
struct PeerConnection {
    sender: mpsc::Sender<NetworkMessage>,
    // Backpressure: limit in-flight messages
    semaphore: Arc<Semaphore>,
}

/// Message sent over network
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum NetworkMessage {
    /// Push entry to a topic
    Push {
        topic: String,
        entry: Entry,
    },
    /// Batch push entries to a topic
    BatchPush {
        topic: String,
        entries: Vec<Entry>,
    },
    /// Subscribe request
    Subscribe {
        topic: String,
        subscriber_id: String,
    },
    /// Acknowledge entry
    Ack {
        topic: String,
        entry_id: u64,
    },
    /// Heartbeat for connection keepalive
    Heartbeat,
}

impl TcpTransport {
    /// Create new TCP transport
    pub async fn new(bind_addr: &str, max_connections: usize) -> Result<Self, TransportError> {
        let (tx, rx) = mpsc::channel(1000);
        
        Ok(Self {
            bind_addr: bind_addr.to_string(),
            peers: Arc::new(RwLock::new(HashMap::new())),
            receiver: rx,
            sender: tx,
            connection_sem: Arc::new(Semaphore::new(max_connections)),
        })
    }
    
    /// Start listening for connections
    pub async fn start_listener(&self) -> Result<(), TransportError> {
        let listener = timeout(
            Duration::from_secs(30),
            TcpListener::bind(&self.bind_addr)
        )
        .await
        .map_err(|_| TransportError::Timeout(format!("Bind timeout to {}", self.bind_addr)))??;
        info!("TCP transport listening on {}", self.bind_addr);
        
        let peers = self.peers.clone();
        let receiver_sender = self.sender.clone();
        let sem = self.connection_sem.clone();
        
        tokio::spawn(async move {
            loop {
                // Accept connection (with backpressure)
                let permit = match sem.clone().acquire_owned().await {
                    Ok(p) => p,
                    Err(_) => {
                        error!("Failed to acquire connection permit");
                        continue;
                    }
                };
                
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        debug!("Accepted connection from {}", addr);
                        let peers = peers.clone();
                        let sender = receiver_sender.clone();
                        
                        tokio::spawn(async move {
                            // Handle connection (releases permit when done)
                            handle_connection(stream, peers, sender).await;
                            drop(permit); // Release permit
                        });
                    }
                    Err(e) => {
                        warn!("Failed to accept connection: {:?}", e);
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Connect to a peer
    pub async fn connect_to_peer(&self, node_id: &str, addr: &str) -> Result<(), TransportError> {
        let stream = timeout(
            Duration::from_secs(10),
            TcpStream::connect(addr)
        )
        .await
        .map_err(|_| TransportError::Timeout(format!("Connect timeout to {}", addr)))??;
        info!("Connected to peer {} at {}", node_id, addr);
        
        // Create channel for sending to this peer
        let (tx, mut rx) = mpsc::channel(100);
        let peer_sem = Arc::new(Semaphore::new(100)); // Max 100 in-flight to this peer
        
        // Spawn writer task
        let mut writer = BufWriter::new(stream);
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                // Serialize message
                let data = match serde_json::to_vec(&msg) {
                    Ok(d) => d,
                    Err(e) => {
                        error!("Failed to serialize message: {:?}", e);
                        continue;
                    }
                };
                
                // Write length prefix + data with timeout
                let len = data.len() as u32;
                
                // Write length with timeout
                match timeout(
                    Duration::from_secs(30),
                    writer.write_all(&len.to_be_bytes())
                ).await {
                    Ok(Ok(())) => {},
                    Ok(Err(e)) => {
                        error!("Failed to write message length: {:?}", e);
                        break;
                    }
                    Err(_) => {
                        error!("Write length timeout");
                        break;
                    }
                }
                
                // Write data with timeout
                match timeout(
                    Duration::from_secs(30),
                    writer.write_all(&data)
                ).await {
                    Ok(Ok(())) => {},
                    Ok(Err(e)) => {
                        error!("Failed to write message data: {:?}", e);
                        break;
                    }
                    Err(_) => {
                        error!("Write data timeout");
                        break;
                    }
                }
                
                // Flush with timeout
                match timeout(
                    Duration::from_secs(30),
                    writer.flush()
                ).await {
                    Ok(Ok(())) => {},
                    Ok(Err(e)) => {
                        error!("Failed to flush: {:?}", e);
                        break;
                    }
                    Err(_) => {
                        error!("Flush timeout");
                        break;
                    }
                }
            }
        });
        
        // Store peer connection
        let peer = PeerConnection {
            sender: tx,
            semaphore: peer_sem,
        };
        
        self.peers.write().await.insert(node_id.to_string(), peer);
        Ok(())
    }
    
    /// Send message to a specific peer (with backpressure)
    pub async fn send_to_peer(&self, node_id: &str, msg: NetworkMessage) -> Result<(), TransportError> {
        let peers = self.peers.read().await;
        
        if let Some(peer) = peers.get(node_id) {
            // Acquire permit for backpressure
            let _permit = peer.semaphore.acquire().await
                .map_err(|_| TransportError::Backpressure("Peer busy".to_string()))?;
            
            peer.sender.send(msg).await
                .map_err(|_| TransportError::SendFailed(node_id.to_string()))?;
            
            Ok(())
        } else {
            Err(TransportError::PeerNotFound(node_id.to_string()))
        }
    }
    
    /// Broadcast to all peers
    pub async fn broadcast(&self, msg: NetworkMessage) -> Result<(), TransportError> {
        let peers = self.peers.read().await;
        
        for (node_id, peer) in peers.iter() {
            if let Err(e) = peer.sender.send(msg.clone()).await {
                warn!("Failed to send to {}: {:?}", node_id, e);
            }
        }
        
        Ok(())
    }
    
    /// Receive next message from network
    pub async fn recv(&mut self) -> Option<NetworkMessage> {
        self.receiver.recv().await
    }
    
    /// Get connected peer count
    pub async fn peer_count(&self) -> usize {
        self.peers.read().await.len()
    }
}

/// Handle incoming connection
async fn handle_connection(
    stream: TcpStream,
    _peers: Arc<RwLock<HashMap<String, PeerConnection>>>,
    sender: mpsc::Sender<NetworkMessage>,
) {
    let mut reader = BufReader::new(stream);
    let mut len_buf = [0u8; 4];
    
    loop {
        // Read length prefix with timeout
        match timeout(
            Duration::from_secs(60),
            reader.read_exact(&mut len_buf)
        ).await {
            Ok(Ok(_)) => {
                let len = u32::from_be_bytes(len_buf) as usize;
                
                // Read message data with timeout
                let mut data = vec![0u8; len];
                match timeout(
                    Duration::from_secs(60),
                    reader.read_exact(&mut data)
                ).await {
                    Ok(Ok(_)) => {
                        // Deserialize
                        match serde_json::from_slice::<NetworkMessage>(&data) {
                            Ok(msg) => {
                                if let Err(e) = sender.send(msg).await {
                                    error!("Failed to forward message: {:?}", e);
                                    break;
                                }
                            }
                            Err(e) => {
                                error!("Failed to deserialize message: {:?}", e);
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        error!("Failed to read message: {:?}", e);
                        break;
                    }
                    Err(_) => {
                        error!("Read data timeout");
                        break;
                    }
                }
            }
            Ok(Err(e)) => {
                debug!("Connection closed: {:?}", e);
                break;
            }
            Err(_) => {
                debug!("Read timeout - connection inactive");
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_tcp_transport_creation() {
        let transport = TcpTransport::new("127.0.0.1:0", 100).await;
        assert!(transport.is_ok());
    }
}