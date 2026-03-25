//! Transport traits for AutoQueues - HPC-Optimized
//!
//! Minimal traits for high-performance networking with sub-ms latency targets.
//! Designed for RDMA compatibility and zero-copy operations.

use std::time::Instant;

/// Transport errors
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Generic serialization error: {source}")]
    SerializationError {
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Peer not found: {0}")]
    PeerNotFound(String),

    #[error("Send failed to peer: {0}")]
    SendFailed(String),

    #[error("Backpressure: {0}")]
    Backpressure(String),

    #[error("Connection limit reached")]
    ConnectionLimit,

    #[error("Operation timeout: {0}")]
    Timeout(String),

    #[error("Connection lost to {remote_addr}")]
    ConnectionLost { remote_addr: std::net::SocketAddr },

    #[error("Network unreachable")]
    NetworkUnreachable,

    #[error("Transport shutdown")]
    TransportShutdown,

    #[error("Invalid message format")]
    InvalidMessage,

    #[error("Message too large: {0} bytes")]
    MessageTooLarge(usize),
}

/// Transport type enumeration
#[derive(Debug, Clone, PartialEq)]
pub enum TransportType {
    /// RDMA transport for ultra-low latency
    Rdma,

    /// TCP transport for compatibility
    Tcp,

    /// In-memory transport for local testing
    InMemory,

    /// ZeroMQ transport for pub/sub messaging
    ZeroMQ,
}

/// Connection information
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// Remote address
    pub remote_addr: String,

    /// Transport type
    pub transport_type: TransportType,

    /// Connection timestamp
    pub connected_at: Instant,
}

/// Minimal transport trait for HPC use cases
#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    /// Connect to remote address
    async fn connect(&mut self, addr: &str) -> Result<ConnectionInfo, TransportError>;

    /// Send data with zero-copy semantics
    async fn send(&mut self, data: &[u8]) -> Result<(), TransportError>;

    /// Receive data with zero-copy semantics
    async fn receive(&mut self) -> Result<Vec<u8>, TransportError>;

    /// Check if transport is connected
    fn is_connected(&self) -> bool;

    /// Get connection information
    fn connection_info(&self) -> Option<ConnectionInfo>;
}
