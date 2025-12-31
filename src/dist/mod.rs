//! Distributed computing module for AutoQueues
//!
//! Provides distributed aggregation capabilities across cluster nodes.

pub mod node_client;
pub mod aggregator;
pub mod sync;
pub mod discovery;

pub use node_client::NodeClient;
pub use aggregator::DistributedAggregator;
pub use sync::TimeSync;
pub use discovery::{ClusterDiscovery, DiscoveryMethod};