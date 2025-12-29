//! Cluster State Machine for OpenRaft
//!
//! Defines cluster state and commands for Raft consensus.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Cluster command types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ClusterCommand {
    /// Add a node to the cluster
    AddNode { node_id: u64, address: String },
    /// Remove a node from the cluster
    RemoveNode { node_id: u64 },
    /// Update node address
    UpdateAddress { node_id: u64, address: String },
    /// Set leader assignment
    SetLeaderAssignment { node_id: u64, metrics: Vec<String> },
    /// Clear leader assignment
    ClearLeaderAssignment { node_id: u64 },
}

/// Cluster state
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClusterState {
    /// All known nodes
    pub nodes: HashMap<u64, NodeInfo>,
    /// Current leader
    pub leader: Option<u64>,
    /// Node to assigned metrics mapping
    pub leader_assignments: HashMap<u64, Vec<String>>,
}

/// Node information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub node_id: u64,
    pub address: String,
    pub is_active: bool,
    pub last_seen: u64,
}

/// Cluster response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterResponse {
    pub success: bool,
    pub message: String,
}
