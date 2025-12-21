//! AutoQueues - High-Performance Distributed Queue System
//!
//! Focused on ZMQ-based real networking

// pub mod cluster; // Temporarily disabled for ZMQ focus
pub mod config;
pub mod expression;
pub mod metrics;
pub mod networking;
pub mod queue;
pub mod traits;
pub mod types;
pub mod pubsub;
pub mod queue_manager;
pub mod port_config;

// Error handling exports
pub use crate::queue_manager::{QueueManager, QueueManagerConfig, QueueManagerError};
pub use crate::traits::transport::{Transport, TransportError, ConnectionInfo, TransportType};
pub use crate::types::{QueueData, QueueStats, Timestamp};