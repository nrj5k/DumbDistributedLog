//! Cluster coordination using Raft for topic ownership
//!
//! This module provides strongly consistent topic ownership via Raft consensus.
//! Gossip is used only for node discovery, while Raft handles all ownership operations.

pub mod types;
pub mod raft_node;
pub mod storage;
pub mod raft_cluster;
pub mod ownership_machine;
pub mod tcp_network;
pub mod raft_router;
pub mod membership;
pub mod gossip;
pub mod lock_utils;

// Re-export ownership types from ownership_machine
pub use ownership_machine::{
    LeaseEntry, LeaseError, LeaseInfo, NodeId, OwnershipCommand, OwnershipQuery,
    OwnershipResponse, OwnershipState,
};

pub use types::{NodeConfig, TypeConfig};
pub use raft_node::RaftNode;
pub use raft_cluster::RaftClusterNode;
pub use storage::AutoqueuesRaftStorage;
pub use tcp_network::{TcpNetwork, TcpNetworkConfig, TcpRaftServer};
pub use raft_router::RaftMessageRouter;
pub use membership::{MembershipEvent, MembershipEventType, MembershipView, DdlMetrics, NodeInfo};
pub use gossip::{FailureDetector, FailureDetectorConfig};