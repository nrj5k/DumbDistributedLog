//! ZeroMQ Transport Implementation for AutoQueues
//!
//! Implements the Transport trait using ZMQ for distributed node communication.

use crate::network::transport_traits::{ConnectionInfo, Transport, TransportError, TransportType};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use zmq::{Context, Message, Socket, SocketType};

/// ZeroMQ Transport Implementation
///
/// Uses ZMQ REQ-REP pattern for request-response communication between nodes.
pub struct ZmqTransport {
    /// ZMQ context (not Debug, so we skip it)
    _context: Context,
    /// The socket (using Arc<Mutex> for async compatibility)
    socket: Arc<Mutex<Socket>>,
    /// Connection info
    connection_info: Option<ConnectionInfo>,
    /// Remote address
    remote_addr: String,
}

impl std::fmt::Debug for ZmqTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ZmqTransport")
            .field("connection_info", &self.connection_info)
            .field("remote_addr", &self.remote_addr)
            .finish()
    }
}

impl ZmqTransport {
    /// Create a new ZMQ transport
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let context = Context::new();
        let socket = context.socket(SocketType::REQ)?;

        Ok(Self {
            _context: context,
            socket: Arc::new(Mutex::new(socket)),
            connection_info: None,
            remote_addr: String::new(),
        })
    }

    /// Get the underlying socket
    pub fn socket(&self) -> &Arc<Mutex<Socket>> {
        &self.socket
    }
}

#[async_trait]
impl Transport for ZmqTransport {
    /// Connect to remote address
    async fn connect(&mut self, addr: &str) -> Result<ConnectionInfo, TransportError> {
        let socket = self.socket.lock().await;
        
        // Connect to the address
        socket.connect(addr).map_err(|_e| TransportError::ConnectionLost {
            remote_addr: addr.parse().unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], 0))),
        })?;

        let connection_info = ConnectionInfo {
            remote_addr: addr.to_string(),
            transport_type: TransportType::ZeroMQ,
            connected_at: Instant::now(),
        };

        drop(socket);

        // Store connection info
        // Note: In a truly concurrent-safe design, we'd use interior mutability
        // For now, this is set after construction
        Ok(connection_info)
    }

    /// Send data to remote endpoint
    async fn send(&mut self, data: &[u8]) -> Result<(), TransportError> {
        let socket = self.socket.lock().await;
        
        let msg = Message::from(data);
        socket.send(msg, 0).map_err(|_e| TransportError::ConnectionLost {
            remote_addr: self.remote_addr.parse().unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], 0))),
        })?;

        Ok(())
    }

    /// Receive data from remote endpoint
    async fn receive(&mut self) -> Result<Vec<u8>, TransportError> {
        let socket = self.socket.lock().await;
        
        let msg = socket.recv_bytes(0).map_err(|_e| TransportError::ConnectionLost {
            remote_addr: self.remote_addr.parse().unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], 0))),
        })?;

        Ok(msg)
    }

    /// Check if connected
    fn is_connected(&self) -> bool {
        self.connection_info.is_some()
    }

    /// Get connection info
    fn connection_info(&self) -> Option<ConnectionInfo> {
        self.connection_info.clone()
    }
}

/// Message types for node-to-node communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeMessage {
    /// Request a value from a specific queue
    RequestValue { queue_name: String },

    /// Response with a value from a queue
    ValueResponse {
        queue_name: String,
        value: Option<f64>,
    },

    /// Request an expression evaluation
    RequestExpression { sources: Vec<String> },

    /// Response with expression evaluation result
    ExpressionResponse {
        sources: Vec<String>,
        score: Option<f64>,
    },

    /// Request an aggregation operation
    RequestAggregation {
        queue_name: String,
        operation: String,
        args: Vec<String>,
    },

    /// Response with aggregation result
    AggregationResponse { result: f64 },

    /// Broadcast a value to all nodes
    BroadcastValue { queue_name: String, value: f64 },
}

/// Send a message using ZMQ transport
pub async fn send_message(
    transport: &mut ZmqTransport,
    addr: &str,
    message: &NodeMessage,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let data = serde_json::to_vec(message)?;

    // Connect if not already connected
    if !transport.is_connected() {
        transport.connect(addr).await?;
    }

    Transport::send(transport, &data).await?;

    Ok(())
}

/// Request a value from a node
pub async fn request_value(
    addr: &SocketAddr,
    queue_name: &str,
) -> Result<Option<f64>, Box<dyn std::error::Error + Send + Sync>> {
    let mut transport = ZmqTransport::new()?;
    let addr_str = format!("tcp://{}", addr);

    let message = NodeMessage::RequestValue {
        queue_name: queue_name.to_string(),
    };

    send_message(&mut transport, &addr_str, &message).await?;

    let response_data = transport.receive().await?;
    let response_msg: NodeMessage = serde_json::from_slice(&response_data)?;

    match response_msg {
        NodeMessage::ValueResponse { value, .. } => Ok(value),
        _ => Err("Unexpected response type".into()),
    }
}

/// Request an expression score from a node
pub async fn request_expression_score(
    addr: &SocketAddr,
    sources: &[String],
) -> Result<Option<f64>, Box<dyn std::error::Error + Send + Sync>> {
    let mut transport = ZmqTransport::new()?;
    let addr_str = format!("tcp://{}", addr);

    let message = NodeMessage::RequestExpression {
        sources: sources.to_vec(),
    };

    send_message(&mut transport, &addr_str, &message).await?;

    let response_data = transport.receive().await?;
    let response_msg: NodeMessage = serde_json::from_slice(&response_data)?;

    match response_msg {
        NodeMessage::ExpressionResponse { score, .. } => Ok(score),
        _ => Err("Unexpected response type".into()),
    }
}

/// Request an aggregation operation from a node
pub async fn request_aggregation(
    addr: &SocketAddr,
    queue_name: &str,
    operation: &str,
    args: Vec<String>,
) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
    let mut transport = ZmqTransport::new()?;
    let addr_str = format!("tcp://{}", addr);

    let message = NodeMessage::RequestAggregation {
        queue_name: queue_name.to_string(),
        operation: operation.to_string(),
        args,
    };

    send_message(&mut transport, &addr_str, &message).await?;

    let response_data = transport.receive().await?;
    let response_msg: NodeMessage = serde_json::from_slice(&response_data)?;

    match response_msg {
        NodeMessage::AggregationResponse { result } => Ok(result),
        _ => Err("Unexpected response type".into()),
    }
}