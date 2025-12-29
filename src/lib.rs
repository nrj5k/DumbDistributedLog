//! AutoQueues - High-Performance Distributed Queue System
//!
//! HPC-optimized architecture with clean separation of concerns:
//! - Core: Ultra-fast queue operations (<100μs target)
//! - Networking: RDMA-ready transport abstractions
//! - Coordination: Simple node management without consensus overhead

// Core modules for HPC performance
pub mod autoqueues;
pub mod cluster;
pub mod config;
pub mod constants;
pub mod expression;
pub mod metrics;
pub mod networking;
pub mod node;
pub mod port_config;
pub mod pubsub;
pub mod queue;
pub mod queue_manager;
pub mod server;
pub mod traits;
pub mod types;

// Re-exports for programmatic API
pub use crate::autoqueues::{AutoQueues, AutoQueuesError};
pub use crate::cluster::{ClusterConfig, RaftNode};
pub use crate::config::{AssignmentStrategy, AutoQueuesConfig, QueueConfig};
pub use crate::expression::{Expression, ExpressionF64, SimpleExpression};
pub use crate::metrics::MetricsCollector;
pub use crate::networking::{ConnectionInfo, Transport, TransportError, TransportType};
pub use crate::node::{AutoQueuesNode, start_node};
pub use crate::port_config::PortConfig;
pub use crate::pubsub::PubSubBroker;
pub use crate::queue::{QueueTrait, SimpleQueue};
pub use crate::queue_manager::{QueueManager, QueueManagerConfig, QueueManagerError};
pub use crate::server::{AutoQueuesServer, AutoQueuesServerError};
pub use crate::types::{QueueData, QueueStats, Timestamp};