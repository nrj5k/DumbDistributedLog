//! Hybrid transport configuration
//!
//! Provides configuration-based selection between TCP and ZMQ transports.

use crate::network::tcp::TcpTransport;

/// Transport configuration for selecting between TCP and ZMQ
#[derive(Debug, Clone)]
pub enum TransportConfig {
    /// TCP transport (reliable, backpressure-aware)
    Tcp {
        bind_addr: String,
        max_connections: usize,
    },
    /// ZMQ transport (fast, best-effort)
    Zmq {
        bind_addr: String,
    },
}

impl TransportConfig {
    /// Create a TCP transport instance
    pub async fn create_tcp(&self) -> Result<TcpTransport, String> {
        match self {
            TransportConfig::Tcp { bind_addr, max_connections } => {
                TcpTransport::new(bind_addr, *max_connections)
                    .await
                    .map_err(|e| format!("Failed to create TCP transport: {}", e))
            }
            _ => Err("TransportConfig is not TCP".to_string()),
        }
    }

    /// Create bind address string
    pub fn bind_addr(&self) -> &str {
        match self {
            TransportConfig::Tcp { bind_addr, .. } => bind_addr,
            TransportConfig::Zmq { bind_addr } => bind_addr,
        }
    }

    /// Check if this is a TCP configuration
    pub fn is_tcp(&self) -> bool {
        matches!(self, TransportConfig::Tcp { .. })
    }

    /// Check if this is a ZMQ configuration
    pub fn is_zmq(&self) -> bool {
        matches!(self, TransportConfig::Zmq { .. })
    }
}

impl Default for TransportConfig {
    fn default() -> Self {
        TransportConfig::Tcp {
            bind_addr: "0.0.0.0:0".to_string(),
            max_connections: 100,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TransportConfig::default();
        assert!(config.is_tcp());
        assert!(!config.is_zmq());
        assert_eq!(config.bind_addr(), "0.0.0.0:0");
    }

    #[test]
    fn test_tcp_config() {
        let config = TransportConfig::Tcp {
            bind_addr: "127.0.0.1:8080".to_string(),
            max_connections: 50,
        };
        assert!(config.is_tcp());
        assert_eq!(config.bind_addr(), "127.0.0.1:8080");
    }

    #[test]
    fn test_zmq_config() {
        let config = TransportConfig::Zmq {
            bind_addr: "tcp://*:5555".to_string(),
        };
        assert!(config.is_zmq());
        assert!(!config.is_tcp());
        assert_eq!(config.bind_addr(), "tcp://*:5555");
    }
}