//! Transport trait definitions for AutoQueues
//!
//! Provides simple networking interface following KISS principle.
//! Ultra-minimal trait with 3 methods for future distributed features.

use async_trait::async_trait;
use std::net::SocketAddr;

/// Transport errors
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("Connection lost to {remote_addr}")]
    ConnectionLost { remote_addr: SocketAddr },

    #[error("Network unreachable")]
    NetworkUnreachable,

    #[error("Transport shutdown")]
    TransportShutdown,

    #[error("Serialization error: {source}")]
    SerializationError {
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Timeout after {duration:?}")]
    Timeout { duration: std::time::Duration },
}

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
