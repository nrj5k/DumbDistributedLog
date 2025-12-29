//! Unified Port Configuration for AutoQueues
//!
//! Provides standard port configuration across all transport implementations.
//! Default AutoQueues port: 69420

use crate::constants;
use std::net::SocketAddr;

/// Standard AutoQueues port configuration
pub const AUTOQUEUES_DEFAULT_PORT: u16 = constants::network::DEFAULT_QUIC_PORT;

/// Transport protocol types
#[derive(Debug, Clone, PartialEq)]
pub enum TransportProtocol {
    Quic,
    ZeroMQ,
    Tcp,
}

/// Unified port configuration for AutoQueues
#[derive(Debug, Clone)]
pub struct PortConfig {
    /// Port number (default: 69420)
    pub port: u16,
    /// Transport protocol
    pub protocol: TransportProtocol,
    /// Bind address (default: 0.0.0.0)
    pub bind_addr: String,
    /// Enable port reuse
    pub reuse_port: bool,
}

impl Default for PortConfig {
    fn default() -> Self {
        Self {
            port: AUTOQUEUES_DEFAULT_PORT,
            protocol: TransportProtocol::Quic,
            bind_addr: "0.0.0.0".to_string(),
            reuse_port: true,
        }
    }
}

impl PortConfig {
    /// Create new port configuration with default AutoQueues port
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with specific port
    pub fn with_port(port: u16) -> Self {
        Self {
            port,
            ..Default::default()
        }
    }

    /// Create with specific protocol
    pub fn with_protocol(protocol: TransportProtocol) -> Self {
        Self {
            protocol,
            ..Default::default()
        }
    }

    /// Create QUIC configuration on standard port
    pub fn quic() -> Self {
        Self {
            protocol: TransportProtocol::Quic,
            ..Default::default()
        }
    }

    /// Create ZeroMQ configuration on standard port
    pub fn zeromq() -> Self {
        Self {
            protocol: TransportProtocol::ZeroMQ,
            ..Default::default()
        }
    }

    /// Create TCP configuration on standard port
    pub fn tcp() -> Self {
        Self {
            protocol: TransportProtocol::Tcp,
            ..Default::default()
        }
    }

    /// Get full bind address string
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.bind_addr, self.port)
    }

    /// Get SocketAddr
    pub fn socket_addr(&self) -> Result<SocketAddr, std::net::AddrParseError> {
        self.bind_address().parse()
    }

    /// Get QUIC-specific bind address
    pub fn quic_bind_addr(&self) -> String {
        format!("{}:{}", self.bind_addr, self.port)
    }

    /// Get ZeroMQ-specific bind address
    pub fn zmq_bind_addr(&self) -> String {
        format!("tcp://{}:{}", self.bind_addr, self.port)
    }

    /// Get TCP-specific bind address
    pub fn tcp_bind_addr(&self) -> String {
        format!("{}:{}", self.bind_addr, self.port)
    }

    /// Set custom bind address
    pub fn with_bind_addr(mut self, addr: &str) -> Self {
        self.bind_addr = addr.to_string();
        self
    }

    /// Set port reuse
    pub fn with_reuse_port(mut self, reuse: bool) -> Self {
        self.reuse_port = reuse;
        self
    }
}

/// Helper functions for common port configurations
pub mod presets {
    use super::*;

    /// Development configuration with localhost binding
    pub fn development() -> PortConfig {
        PortConfig {
            bind_addr: "127.0.0.1".to_string(),
            ..Default::default()
        }
    }

    /// Production configuration with all interfaces binding
    pub fn production() -> PortConfig {
        PortConfig {
            bind_addr: "0.0.0.0".to_string(),
            ..Default::default()
        }
    }

    /// Local testing configuration
    pub fn local_test() -> PortConfig {
        PortConfig {
            port: AUTOQUEUES_DEFAULT_PORT,
            bind_addr: "127.0.0.1".to_string(),
            protocol: TransportProtocol::Quic,
            reuse_port: true,
        }
    }

    /// High-performance ZeroMQ configuration
    pub fn high_performance() -> PortConfig {
        PortConfig {
            protocol: TransportProtocol::ZeroMQ,
            bind_addr: "0.0.0.0".to_string(),
            reuse_port: true,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_port_config() {
        let config = PortConfig::default();
        assert_eq!(config.port, AUTOQUEUES_DEFAULT_PORT);
        assert_eq!(config.protocol, TransportProtocol::Quic);
        assert_eq!(config.bind_addr, "0.0.0.0");
        assert!(config.reuse_port);
    }

    #[test]
    fn test_port_config_with_custom_port() {
        let config = PortConfig::with_port(8080);
        assert_eq!(config.port, 8080);
        assert_eq!(config.bind_address(), "0.0.0.0:8080");
    }

    #[test]
    fn test_quic_configuration() {
        let config = PortConfig::quic();
        assert_eq!(config.protocol, TransportProtocol::Quic);
        assert_eq!(config.quic_bind_addr(), "0.0.0.0:6967");
    }

    #[test]
    fn test_zeromq_configuration() {
        let config = PortConfig::zeromq();
        assert_eq!(config.protocol, TransportProtocol::ZeroMQ);
        assert_eq!(config.zmq_bind_addr(), "tcp://0.0.0.0:6967");
    }

    #[test]
    fn test_presets() {
        let dev = presets::development();
        assert_eq!(dev.bind_addr, "127.0.0.1");

        let prod = presets::production();
        assert_eq!(prod.bind_addr, "0.0.0.0");

        let test = presets::local_test();
        assert_eq!(test.bind_addr, "127.0.0.1");
        assert_eq!(test.port, AUTOQUEUES_DEFAULT_PORT);
    }
}
