//! Cluster module for distributed coordination using OpenRaft
//!
//! Provides Raft-based cluster management with ZMQ transport.
//! Uses separate port from data channel for coordination.

pub mod config;
pub mod leader;
pub mod leader_assignment;
pub mod leader_map_client;
pub mod leaderleader;
pub mod node;
pub mod state_machine;

// Re-exports for convenient usage
pub use config::ClusterConfig;
pub use leader::{AggregationType, Leader, LeaderConfig, LeaderState, start_leader};
pub use leader_assignment::{DerivedMetric, LeaderMap, WeightedBag, WeightedNode};
pub use leader_map_client::{ConsensusConfig, LeaderMapClient};
pub use leaderleader::{LeaderLeader, LeaderLeaderConfig, LeaderLeaderState, start_leaderleader};
pub use node::RaftNode;
pub use state_machine::{ClusterState, ClusterCommand, ClusterResponse, NodeInfo};
