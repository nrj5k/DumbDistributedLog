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

/// Maximum message size (10MB)
const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024;

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
                
                // Validate message size to prevent OOM attacks
                if len > MAX_MESSAGE_SIZE {
                    error!("Message too large: {} bytes (max: {})", len, MAX_MESSAGE_SIZE);
                    break;
                }
                if len == 0 {
                    error!("Empty message length");
                    break;
                }
                
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

    // === NetworkMessage Serialization Tests ===

    #[test]
    fn test_network_message_push_serialization() {
        let entry = Entry::with_timestamp(
            1,
            12345,
            "test.topic",
            vec![1, 2, 3],
        );
        let msg = NetworkMessage::Push {
            topic: "test.topic".to_string(),
            entry: entry.clone(),
        };

        // Serialize
        let json = serde_json::to_string(&msg).expect("Should serialize Push");
        assert!(json.contains("test.topic"));
        assert!(json.contains("\"Push\""));

        // Deserialize
        let decoded: NetworkMessage = serde_json::from_str(&json).expect("Should deserialize Push");
        match decoded {
            NetworkMessage::Push { topic, entry: decoded_entry } => {
                assert_eq!(topic, "test.topic");
                assert_eq!(decoded_entry.id, 1);
                assert_eq!(&*decoded_entry.payload, &[1, 2, 3]);
            }
            _ => panic!("Expected Push variant"),
        }
    }

    #[test]
    fn test_network_message_batch_push_serialization() {
        let entries = vec![
            Entry::with_timestamp(1, 100, "batch.topic", vec![1]),
            Entry::with_timestamp(2, 200, "batch.topic", vec![2]),
        ];
        let msg = NetworkMessage::BatchPush {
            topic: "batch.topic".to_string(),
            entries: entries.clone(),
        };

        let json = serde_json::to_string(&msg).expect("Should serialize BatchPush");
        assert!(json.contains("batch.topic"));
        assert!(json.contains("\"BatchPush\""));

        let decoded: NetworkMessage = serde_json::from_str(&json).expect("Should deserialize BatchPush");
        match decoded {
            NetworkMessage::BatchPush { topic, entries } => {
                assert_eq!(topic, "batch.topic");
                assert_eq!(entries.len(), 2);
                assert_eq!(entries[0].id, 1);
                assert_eq!(entries[1].id, 2);
            }
            _ => panic!("Expected BatchPush variant"),
        }
    }

    #[test]
    fn test_network_message_subscribe_serialization() {
        let msg = NetworkMessage::Subscribe {
            topic: "subscribe.topic".to_string(),
            subscriber_id: "node-123".to_string(),
        };

        let json = serde_json::to_string(&msg).expect("Should serialize Subscribe");
        assert!(json.contains("subscribe.topic"));
        assert!(json.contains("node-123"));
        assert!(json.contains("\"Subscribe\""));

        let decoded: NetworkMessage = serde_json::from_str(&json).expect("Should deserialize Subscribe");
        match decoded {
            NetworkMessage::Subscribe { topic, subscriber_id } => {
                assert_eq!(topic, "subscribe.topic");
                assert_eq!(subscriber_id, "node-123");
            }
            _ => panic!("Expected Subscribe variant"),
        }
    }

    #[test]
    fn test_network_message_ack_serialization() {
        let msg = NetworkMessage::Ack {
            topic: "ack.topic".to_string(),
            entry_id: 42,
        };

        let json = serde_json::to_string(&msg).expect("Should serialize Ack");
        assert!(json.contains("ack.topic"));
        assert!(json.contains("42"));
        assert!(json.contains("\"Ack\""));

        let decoded: NetworkMessage = serde_json::from_str(&json).expect("Should deserialize Ack");
        match decoded {
            NetworkMessage::Ack { topic, entry_id } => {
                assert_eq!(topic, "ack.topic");
                assert_eq!(entry_id, 42);
            }
            _ => panic!("Expected Ack variant"),
        }
    }

    #[test]
    fn test_network_message_heartbeat_serialization() {
        let msg = NetworkMessage::Heartbeat;

        let json = serde_json::to_string(&msg).expect("Should serialize Heartbeat");
        assert!(json.contains("\"Heartbeat\""));

        let decoded: NetworkMessage = serde_json::from_str(&json).expect("Should deserialize Heartbeat");
        match decoded {
            NetworkMessage::Heartbeat => {}
            _ => panic!("Expected Heartbeat variant"),
        }
    }

    #[test]
    fn test_network_message_all_variants_roundtrip() {
        let messages = vec![
            NetworkMessage::Push {
                topic: "t1".to_string(),
                entry: Entry::new(0, "t1", vec![]),
            },
            NetworkMessage::BatchPush {
                topic: "t2".to_string(),
                entries: vec![],
            },
            NetworkMessage::Subscribe {
                topic: "t3".to_string(),
                subscriber_id: "sub".to_string(),
            },
            NetworkMessage::Ack {
                topic: "t4".to_string(),
                entry_id: 99,
            },
            NetworkMessage::Heartbeat,
        ];

        for msg in messages {
            let json = serde_json::to_string(&msg).expect("Should serialize");
            let decoded: NetworkMessage = serde_json::from_str(&json).expect("Should deserialize");
            // Ensure roundtrip works - just verify no errors
            let _ = decoded;
        }
    }

    #[test]
    fn test_max_message_size() {
        // Verify the constant is 10MB as documented
        assert_eq!(MAX_MESSAGE_SIZE, 10 * 1024 * 1024);
        assert_eq!(MAX_MESSAGE_SIZE, 10_485_760);
    }

    #[test]
    fn test_entry_serialization() {
        let entry = Entry::with_timestamp(
            12345,
            999888777,
            "test.entry",
            vec![0, 255, 128, 64, 32],
        );

        let json = serde_json::to_string(&entry).expect("Should serialize Entry");
        assert!(json.contains("12345"));
        assert!(json.contains("999888777"));

        let decoded: Entry = serde_json::from_str(&json).expect("Should deserialize Entry");
        assert_eq!(decoded.id, entry.id);
        assert_eq!(&*decoded.payload, &*entry.payload);
        assert_eq!(decoded.timestamp, entry.timestamp);
    }

    #[test]
    fn test_network_message_empty_payload() {
        // Test with empty payload
        let msg = NetworkMessage::Push {
            topic: "empty".to_string(),
            entry: Entry::new(0, "empty", vec![]),
        };

        let json = serde_json::to_string(&msg).expect("Should serialize empty payload");
        let decoded: NetworkMessage = serde_json::from_str(&json).expect("Should deserialize");
        match decoded {
            NetworkMessage::Push { entry, .. } => {
                assert!(entry.payload.is_empty());
            }
            _ => panic!("Expected Push variant"),
        }
    }

    #[test]
    fn test_network_message_large_entry_id() {
        // Test with large entry ID
        let msg = NetworkMessage::Ack {
            topic: "test".to_string(),
            entry_id: u64::MAX,
        };

        let json = serde_json::to_string(&msg).expect("Should serialize");
        let decoded: NetworkMessage = serde_json::from_str(&json).expect("Should deserialize");
        match decoded {
            NetworkMessage::Ack { entry_id, .. } => {
                assert_eq!(entry_id, u64::MAX);
            }
            _ => panic!("Expected Ack variant"),
        }
    }

    #[test]
    fn test_network_message_unicode_topic() {
        // Test with unicode topic name
        let msg = NetworkMessage::Subscribe {
            topic: "topic-日本語-🚀".to_string(),
            subscriber_id: "sub".to_string(),
        };

        let json = serde_json::to_string(&msg).expect("Should serialize unicode");
        let decoded: NetworkMessage = serde_json::from_str(&json).expect("Should deserialize");
        match decoded {
            NetworkMessage::Subscribe { topic, .. } => {
                assert_eq!(topic, "topic-日本語-🚀");
            }
            _ => panic!("Expected Subscribe variant"),
        }
    }
}