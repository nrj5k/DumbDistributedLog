//! Transport trait definitions for AutoQueues
//!
//! Provides simple networking interface following KISS principle.
//! Ultra-minimal trait with 3 methods for future distributed features.

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

/// Ultra-minimal transport trait for maximum flexibility
pub trait Transport: Send + Sync {
    /// Send data to remote endpoint
    fn send(&mut self, data: &[u8]) -> Result<(), TransportError>;

    /// Receive data from remote endpoint
    fn receive(&mut self) -> Result<Vec<u8>, TransportError>;

    /// Connect to remote address
    fn connect(&mut self, addr: &str) -> Result<(), TransportError>;
}
