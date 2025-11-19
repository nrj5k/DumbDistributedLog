//! Queue System Module
//!
//! This module provides the core queue system extracted from SCORE.
//! It offers autonomous data processing with configurable intervals and AIMD support.
//!
//! ## Key Features
//! - **Autonomous Servers**: Each queue runs its own async task
//! - **Circular Buffer**: Fixed-capacity time-series storage
//! - **AIMD Integration**: Intelligent interval adaptation based on data variance
//! - **Function Hooks**: Custom data processing via closures
//! - **Trait System**: SensorQueue and InsightQueue implementations
//!
//! ## Usage
//! ```rust
//! use score_queue::{Queue, QueueConfig, QueueType};
//!
//! let queue = Queue::new(queue_config);
//! let server_handle = queue.start_server()?;
//! ```

pub mod aimd;
pub mod core;
pub mod server;
pub mod traits;
pub mod types;

// Re-export main types for convenience
pub use aimd::{AimdConfig, AimdController, AimdStats};
pub use core::Queue;
pub use server::QueueServerHandle;
pub use traits::{InsightQueue, SensorQueue};
pub use types::{QueueConfig, QueueStats, QueueType};
