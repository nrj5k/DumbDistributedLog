//! TCP-based Raft network for production multi-node deployment.
//!
//! Provides real TCP networking for Raft RPCs in production deployments.
//!
//! NOTE: This is a placeholder implementation. Full TCP networking requires:
//! 1. Enable `serde` feature in openraft for RPC types
//! 2. Implement proper serialization/deserialization
//! 3. Server-side handling of incoming RPC requests
//!
//! For now, use InMemoryNetwork (in raft_cluster.rs) for development and testing.

use std::net::SocketAddr;

/// TCP network configuration for production deployments
///
/// This struct holds configuration for TCP-based Raft communication.
pub struct TcpNetworkConfig {
    /// Node ID of this node
    pub node_id: u64,
    /// Address to bind to
    pub bind_addr: SocketAddr,
    /// Map of node_id -> peer addresses
    pub peers: std::collections::HashMap<u64, SocketAddr>,
}

impl TcpNetworkConfig {
    /// Create a new TCP network configuration
    pub fn new(
        node_id: u64,
        bind_addr: SocketAddr,
        peers: std::collections::HashMap<u64, SocketAddr>,
    ) -> Self {
        Self {
            node_id,
            bind_addr,
            peers,
        }
    }
}

/// TCP-based Raft network placeholder
///
/// TODO: Implement actual TCP networking for production multi-node deployment.
/// For now, this is a placeholder. Use InMemoryNetwork for development.
pub struct TcpNetwork {
    /// Configuration
    pub config: TcpNetworkConfig,
}

impl TcpNetwork {
    /// Create a new TCP network instance
    pub fn new(config: TcpNetworkConfig) -> Self {
        Self { config }
    }
}

/// TCP server for receiving Raft RPCs (placeholder)
///
/// TODO: Implement actual server-side handling of incoming RPCs.
pub struct TcpRaftServer {
    /// Bind address
    pub bind_addr: SocketAddr,
}

impl TcpRaftServer {
    /// Create a new TCP server
    pub fn new(bind_addr: SocketAddr) -> Self {
        Self { bind_addr }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_tcp_network_config_creation() {
        let mut peers = HashMap::new();
        peers.insert(1, "127.0.0.1:8080".parse().unwrap());
        peers.insert(2, "127.0.0.1:8081".parse().unwrap());

        let config = TcpNetworkConfig::new(1, "127.0.0.1:9090".parse().unwrap(), peers);
        assert_eq!(config.node_id, 1);
        assert_eq!(config.peers.len(), 2);
    }

    #[test]
    fn test_tcp_network_server() {
        let server = TcpRaftServer::new("127.0.0.1:9090".parse().unwrap());
        assert_eq!(
            server.bind_addr,
            "127.0.0.1:9090".parse::<SocketAddr>().unwrap()
        );
    }

    #[test]
    fn test_tcp_network_creation() {
        let mut peers = HashMap::new();
        peers.insert(1, "127.0.0.1:8080".parse().unwrap());

        let config = TcpNetworkConfig::new(1, "127.0.0.1:9090".parse().unwrap(), peers);
        let network = TcpNetwork::new(config);
        assert_eq!(network.config.node_id, 1);
    }
}
