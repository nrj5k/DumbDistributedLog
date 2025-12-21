//! Transport trait definitions for AutoQueues
//!
//! Provides simple networking interface following KISS principle.
//! Ultra-minimal trait with 3 methods for future distributed features.

use async_trait::async_trait;
use std::net::SocketAddr;

/// Transport errors
#[derive(Debug)]
pub enum TransportError {
    ConnectionLost { remote_addr: SocketAddr },
    NetworkUnreachable,
    TransportShutdown,
    SerializationError { source: String },
    Timeout { duration: std::time::Duration },
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::ConnectionLost { remote_addr } => {
                write!(f, "Connection lost to {}", remote_addr)
            }
            TransportError::NetworkUnreachable => write!(f, "Network unreachable"),
            TransportError::TransportShutdown => write!(f, "Transport shutdown"),
            TransportError::SerializationError { source } => {
                write!(f, "Serialization error: {}", source)
            }
            TransportError::Timeout { duration } => {
                write!(f, "Timeout after {:?}", duration)
            }
        }
    }
}

impl std::error::Error for TransportError {}

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
