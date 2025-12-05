//! # Autoqueues Library - KISS Simplified
//!
//! A Rust library for autonomous queue systems following KISS principle.
//! Provides trait-based architecture with minimal complexity and maximum flexibility.
//!
//! ## Key Features
//!
//! - **Trait-Based Design**: Ultra-minimal traits for maximum flexibility.
//! - **Simple Expression Engine**: Basic arithmetic with local/global variables
//! - **Unified Configuration**: Single file-based configuration system
//! - **Queue Operations**: Clean, focused API for queue management
//! - **System Metrics**: Real-time system monitoring
//! - **Pub/Sub System**: Topic-based messaging within machine
//!
//! ## Quick Start
//!
//! See the examples directory for complete usage examples.

// KISS Simplified modules
pub mod config; // Unified configuration system
pub mod expression; // New simplified expression system
pub mod metrics; // System metrics (keep)
pub mod networking; // Simple networking layer
pub mod pubsub; // Pub/sub system (keep)
pub mod queue;
pub mod traits; // Trait definitions
pub mod types; // Core types and utilities // New queue module structure

// Re-export main types for convenience (KISS simplified)
pub use crate::config::{ConfigError, QueueConfig}; // Unified configuration
pub use crate::expression::{Expression, ExpressionError, SimpleExpression}; // Expression engine
pub use crate::metrics::{DiskMetrics, MemoryMetrics, MetricsCollector, SystemInfo, SystemMetrics};
pub use crate::networking::{DistributedQueueManager, TcpTransport}; // Simple networking
pub use crate::pubsub::{PubSubBroker, PubSubError, TopicMessage};
pub use crate::queue::{Queue, QueueError, QueueServerHandle, SimpleQueue}; // New queue system
pub use crate::traits::transport::{Transport, TransportError}; // Transport traits
pub use crate::types::{QueueData, QueueStats, Timestamp};
