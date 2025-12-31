//! Transport traits for AutoQueues - HPC-Optimized
//!
//! Minimal traits for high-performance networking with sub-ms latency targets.
//! Designed for RDMA compatibility and zero-copy operations.

use std::net::SocketAddr;
use std::time::Instant;

/// Transport errors for HPC use cases
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("Connection lost to {remote_addr}")]
    ConnectionLost { remote_addr: SocketAddr },

    #[error("Network unreachable")]
    NetworkUnreachable,

    #[error("Invalid message format")]
    InvalidMessage,

    #[error("Timeout")]
    Timeout,
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
