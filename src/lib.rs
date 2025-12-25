//! AutoQueues - High-Performance Distributed Queue System

// NOTE: the cluster module is for all the distributed stuff
pub mod cluster;
pub mod config;
pub mod expression;
pub mod metrics;
pub mod networking;
pub mod port_config;
pub mod pubsub;
pub mod queue;
pub mod queue_manager;
pub mod traits;
pub mod types;

pub use crate::cluster::{ClusterConfig, RaftNode};
pub use crate::config::{AssignmentStrategy, AutoQueuesConfig, QueueConfig};
pub use crate::expression::{Expression, ExpressionF64, SimpleExpression};
pub use crate::metrics::MetricsCollector;
pub use crate::networking::{ZmqPubSubBroker, ZmqPubSubClient, ZmqTransport};
pub use crate::port_config::PortConfig;
pub use crate::pubsub::PubSubBroker;
pub use crate::queue::{QueueTrait, SimpleQueue};
pub use crate::queue_manager::{QueueManager, QueueManagerConfig, QueueManagerError};
pub use crate::traits::transport::{ConnectionInfo, Transport, TransportError, TransportType};
pub use crate::types::{QueueData, QueueStats, Timestamp};

