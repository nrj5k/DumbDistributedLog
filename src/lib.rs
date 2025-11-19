//! # Autoqueues Library
//!
//! A Rust library for autonomous queue systems with AIMD (Additive Increase Multiplicative Decrease)
//! support. This library provides queue implementations that can automatically adjust their
//! processing intervals based on data variance patterns.
//!
//! ## Key Features
//!
//! - **Autonomous Servers**: Each queue runs its own async task
//! - **Circular Buffer**: Fixed-capacity time-series storage  
//! - **AIMD Integration**: Intelligent interval adaptation based on data variance
//! - **Function Hooks**: Custom data processing via closures
//! - **Trait System**: SensorQueue and InsightQueue implementations
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use autoqueues::{Queue, QueueConfig, QueueType, FunctionHook};
//! use std::sync::Arc;
//!
//! let hook: FunctionHook<i32> = Arc::new(|_| Ok(42));
//! let queue_type = QueueType::new(
//!     autoqueues::QueueValue::NodeAvailability,
//!     1000,  // base_interval
//!     2,     // increase_factor
//!     "trace.txt".to_string(),
//!     "var".to_string(),
//! );
//! let config = QueueConfig::new(
//!     autoqueues::Mode::Sensor,
//!     hook,
//!     autoqueues::Model::Linear,
//!     queue_type,
//! );
//! let queue = Queue::new(config);
//! let server_handle = queue.start_server()?;
//! ```

pub mod aimd;
pub mod core;
pub mod enums;
pub mod server;
pub mod traits;
pub mod types;

// Re-export main types for convenience
pub use aimd::{AimdConfig, AimdController, AimdStats};
pub use core::Queue;
pub use enums::{Mode, Model, QueueError, QueueValue};
pub use server::QueueServerHandle;
pub use traits::{AimdQueue, InsightQueue, QueueOperations, SensorQueue};
pub use types::{FunctionHook, QueueConfig, QueueStats, QueueType};

// Re-export the timestamp type
pub use core::QueueData;
pub use core::Timestamp;
