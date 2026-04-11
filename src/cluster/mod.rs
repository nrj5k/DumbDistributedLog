//! Cluster coordination using Raft for topic ownership
//!
//! This module provides strongly consistent topic ownership via Raft consensus.
//! Gossip is used only for node discovery, while Raft handles all ownership operations.

pub mod gossip;
pub mod lock_utils;
pub mod membership;
pub mod ownership_machine;
pub mod raft_cluster;
pub mod raft_node;
pub mod raft_router;
pub mod storage;
pub mod types;

// Re-export ownership types from ownership_machine
pub use ownership_machine::{
    LeaseEntry, LeaseError, LeaseInfo, NodeId, OwnershipCommand, OwnershipQuery, OwnershipResponse,
    OwnershipState,
};

pub use gossip::{FailureDetector, FailureDetectorConfig};
pub use membership::{DdlMetrics, MembershipEvent, MembershipEventType, MembershipView, NodeInfo};
pub use raft_cluster::RaftClusterNode;
pub use raft_node::RaftNode;
pub use raft_router::RaftMessageRouter;
pub use storage::AutoqueuesRaftStorage;
pub use types::{NodeConfig, TypeConfig};
