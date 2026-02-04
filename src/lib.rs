//! AutoQueues - High-Performance Distributed Queue System
//!
//! HPC-optimized architecture with clean separation of concerns:
//! - Core: Ultra-fast queue operations (<100μs target)
//! - Networking: RDMA-ready transport abstractions
//! - Coordination: Simple node management without consensus overhead

// Core modules for HPC performance
pub mod autoqueues;
pub mod benchmarks;
pub mod cluster;
pub mod config;
pub mod constants;
pub mod dist;
pub mod expression;
pub mod metrics;
pub mod network;
pub mod node;
pub mod queue;
pub mod traits;
pub mod types;

// Re-exports for programmatic API
pub use crate::autoqueues::AutoQueues;
pub use crate::config::{Config as AutoQueuesConfig, QueueConfig, ConfigError};
pub use crate::queue::source::AutoQueuesError;
pub use crate::cluster::raft_node::{ClusterConfig, RaftNode};
pub use crate::metrics::MetricsCollector;
pub use crate::network::transport_traits::{ConnectionInfo, Transport, TransportError, TransportType};
pub use crate::node::{AutoQueuesNode, start_node};
pub use crate::queue::QueueTrait;
pub use crate::queue::interval::IntervalConfig;
pub use crate::queue::persistence::{PersistenceConfig, QueuePersistence};
pub use crate::network::pubsub::zmq::{ZmqPubSubBroker, ZmqPubSubClient};
pub use crate::types::{QueueData, QueueStats, Timestamp};