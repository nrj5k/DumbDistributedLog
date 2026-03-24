//! Transport trait definitions for AutoQueues
//!

use async_trait::async_trait;

// Re-export TransportError from transport_traits (the trait module)
pub use crate::network::transport_traits::TransportError;

/// Connection information
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub remote_addr: String,
    pub transport_type: TransportType,
    pub connected_at: std::time::Instant,
}

/// Transport types
#[derive(Debug, Clone, PartialEq)]
pub enum TransportType {
    ZeroMQ,
    QUIC,
    TCP,
}

/// Ultra-minimal async transport trait for maximum flexibility
#[async_trait]
pub trait Transport: Send + Sync + std::fmt::Debug {
    /// Connect to remote address
    async fn connect(&self, addr: &str) -> Result<ConnectionInfo, TransportError>;

    /// Send data to remote endpoint
    async fn send(&self, data: &[u8]) -> Result<(), TransportError>;

    /// Receive data from remote endpoint
    async fn receive(&self) -> Result<Vec<u8>, TransportError>;

    /// Check if connected
    fn is_connected(&self) -> bool;

    /// Get connection info
    fn connection_info(&self) -> Option<ConnectionInfo>;
}
