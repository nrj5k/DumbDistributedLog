//! Raft Node for cluster coordination
//!
//! Wrapper around OpenRaft that manages the Raft node lifecycle.
//! Currently minimal implementation - full OpenRaft integration to follow.

use std::time::Duration;
use thiserror::Error;
use zmq::{Context, Socket, SocketType};

use crate::cluster::config::ClusterConfig;
use crate::cluster::state_machine::{ClusterCommand, ClusterResponse, ClusterState};

/// Result type for cluster operations
pub type ClusterResult<T> = Result<T, ClusterError>;

/// Cluster errors
#[derive(Debug, Error)]
pub enum ClusterError {
    #[error("ZMQ error: {0}")]
    Zmq(#[from] zmq::Error),

    #[error("Not initialized")]
    NotInitialized,

    #[error("Not a leader")]
    NotLeader,

    #[error("Invalid state")]
    InvalidState,
}

/// Raft node wrapper
pub struct RaftNode {
    /// Node ID
    node_id: u64,
    /// ZMQ socket for sending
    send_socket: Socket,
    /// Current cluster state
    state: ClusterState,
    /// Configuration
    config: ClusterConfig,
}

impl RaftNode {
    /// Create new Raft node
    pub fn new(config: ClusterConfig) -> Result<Self, ClusterError> {
        let context = Context::new();

        // Create socket for sending (DEALER)
        let send_socket = context.socket(SocketType::DEALER)?;
        let _ = send_socket.set_linger(0);

        Ok(Self {
            node_id: config.node_id,
            send_socket,
            state: ClusterState::default(),
            config,
        })
    }

    /// Get the node ID
    pub fn node_id(&self) -> u64 {
        self.node_id
    }

    /// Get current cluster state
    pub fn state(&self) -> &ClusterState {
        &self.state
    }

    /// Submit a command to the cluster
    pub fn submit(&mut self, command: ClusterCommand) -> ClusterResult<ClusterResponse> {
        match command {
            ClusterCommand::AddNode { node_id, address } => {
                self.state.nodes.insert(
                    node_id,
                    crate::cluster::state_machine::NodeInfo {
                        node_id,
                        address,
                        is_active: true,
                        last_seen: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_else(|_| Duration::from_secs(0))
                            .as_secs(),
                    },
                );
                Ok(ClusterResponse {
                    success: true,
                    message: format!("Added node {}", node_id),
                })
            }
            ClusterCommand::RemoveNode { node_id } => {
                self.state.nodes.remove(&node_id);
                self.state.leader_assignments.remove(&node_id);
                if self.state.leader == Some(node_id) {
                    self.state.leader = None;
                }
                Ok(ClusterResponse {
                    success: true,
                    message: format!("Removed node {}", node_id),
                })
            }
            ClusterCommand::UpdateAddress { node_id, address } => {
                if let Some(node) = self.state.nodes.get_mut(&node_id) {
                    node.address = address.clone();
                    Ok(ClusterResponse {
                        success: true,
                        message: format!("Updated node {} address", node_id),
                    })
                } else {
                    Ok(ClusterResponse {
                        success: false,
                        message: format!("Node {} not found", node_id),
                    })
                }
            }
            ClusterCommand::SetLeaderAssignment { node_id, metrics } => {
                self.state.leader_assignments.insert(node_id, metrics);
                Ok(ClusterResponse {
                    success: true,
                    message: format!("Set leader assignment for node {}", node_id),
                })
            }
            ClusterCommand::ClearLeaderAssignment { node_id } => {
                self.state.leader_assignments.remove(&node_id);
                Ok(ClusterResponse {
                    success: true,
                    message: format!("Cleared leader assignment for node {}", node_id),
                })
            }
        }
    }

    /// Connect to a peer node
    pub fn connect(&self, _node_id: u64, address: &str) -> Result<(), ClusterError> {
        let addr_str = format!("tcp://{}", address);
        self.send_socket.connect(&addr_str)?;
        Ok(())
    }

    /// Get the coordination port
    pub fn port(&self) -> u16 {
        self.config.ports.coordination_port
    }

    /// Get the bind address
    pub fn bind_addr(&self) -> &str {
        &self.config.coordination_bind_addr
    }
}
