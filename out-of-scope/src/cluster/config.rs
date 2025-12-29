//! Cluster configuration for Raft-based coordination

use crate::constants;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Node ID type for Raft
pub type NodeId = u64;

/// Cluster ports configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClusterPorts {
    /// Data plane port (6966) - pub/sub for atomic metrics
    pub data_port: u16,
    /// Coordination port (6968) - leaderleader broadcasts, Raft
    pub coordination_port: u16,
    /// Query port (6969) - REQ/REP for leader queries
    pub query_port: u16,
}

impl Default for ClusterPorts {
    fn default() -> Self {
        Self {
            data_port: constants::network::DEFAULT_DATA_PORT,
            coordination_port: constants::network::DEFAULT_COORDINATION_PORT,
            query_port: constants::network::DEFAULT_QUERY_PORT,
        }
    }
}

/// Cluster configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClusterConfig {
    /// This node's ID
    pub node_id: NodeId,
    /// Cluster ports configuration
    pub ports: ClusterPorts,
    /// ZMQ bind address for coordination
    pub coordination_bind_addr: String,
    /// Initial cluster members
    pub initial_members: Vec<NodeId>,
    /// Tick interval in milliseconds
    pub tick_interval_ms: u64,
    /// Election timeout in milliseconds
    pub election_timeout_ms: u64,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            node_id: 1,
            ports: ClusterPorts::default(),
            coordination_bind_addr: "0.0.0.0".to_string(),
            initial_members: vec![1],
            tick_interval_ms: constants::time::TICK_INTERVAL_MS,
            election_timeout_ms: constants::time::ELECTION_TIMEOUT_MS,
        }
    }
}

impl ClusterConfig {
    /// Get the ZMQ bind address for coordination (leaderleader broadcasts)
    pub fn coordination_zmq_addr(&self) -> String {
        format!(
            "tcp://{}:{}",
            self.coordination_bind_addr, self.ports.coordination_port
        )
    }

    /// Get the ZMQ bind address for queries (REQ/REP)
    pub fn query_zmq_addr(&self) -> String {
        format!(
            "tcp://{}:{}",
            self.coordination_bind_addr, self.ports.query_port
        )
    }

    /// Get the ZMQ bind address for data (PUB/SUB)
    pub fn data_zmq_addr(&self) -> String {
        format!(
            "tcp://{}:{}",
            self.coordination_bind_addr, self.ports.data_port
        )
    }

    /// Get socket address for coordination
    pub fn coordination_socket_addr(&self) -> SocketAddr {
        format!(
            "{}:{}",
            self.coordination_bind_addr, self.ports.coordination_port
        )
        .parse()
        .unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], self.ports.coordination_port)))
    }

    /// Create config for a single node
    pub fn single_node(node_id: NodeId, base_port: u16) -> Self {
        let ports = ClusterPorts {
            data_port: base_port,
            coordination_port: base_port + 2,
            query_port: base_port + 3,
        };
        Self {
            node_id,
            ports,
            coordination_bind_addr: "0.0.0.0".to_string(),
            initial_members: vec![node_id],
            tick_interval_ms: constants::time::TICK_INTERVAL_MS,
            election_timeout_ms: constants::time::ELECTION_TIMEOUT_MS,
        }
    }

    /// Create config for multi-node cluster
    pub fn cluster(node_id: NodeId, base_port: u16, members: Vec<NodeId>, bind_addr: &str) -> Self {
        let ports = ClusterPorts {
            data_port: base_port,
            coordination_port: base_port + 2,
            query_port: base_port + 3,
        };
        Self {
            node_id,
            ports,
            coordination_bind_addr: bind_addr.to_string(),
            initial_members: members,
            tick_interval_ms: constants::time::TICK_INTERVAL_MS,
            election_timeout_ms: constants::time::ELECTION_TIMEOUT_MS,
        }
    }
}
