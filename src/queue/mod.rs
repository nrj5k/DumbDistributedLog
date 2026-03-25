//! Queue Module with Local Operations
//!
//! Provides queue operations using the established real networking foundation.

pub mod interval;
pub mod persistence;
pub mod queue_server;
pub mod registry;
pub mod source;
pub mod spmc_lockfree_queue;

// Re-export main queue types
pub use interval::{AimdController, IntervalConfig};
pub use persistence::{PersistenceConfig, PersistenceManager, QueuePersistence};
pub use queue_server::{QueueError, QueueServerHandle};
pub use spmc_lockfree_queue::{SPMCConsumer, SPMCLockFreeQueue};

