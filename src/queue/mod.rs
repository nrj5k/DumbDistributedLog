//! Queue Module with Local Operations
//!
//! Provides queue operations using the established real networking foundation.

pub mod lockfree;
pub mod queue_server;
pub mod registry;
pub mod simple_queue;
pub mod source;

// Re-export main queue types
pub use crate::traits::queue::QueueTrait;
pub use lockfree::LockFreeQueue;
pub use queue_server::{QueueError, QueueServerHandle};
pub use simple_queue::SimpleQueue;