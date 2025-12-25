//! Queue Module with Local Operations
//!
//! Provides queue operations using the established real networking foundation.

pub mod implementation;
pub mod queue_server;

// Re-export main queue types
pub use crate::traits::queue::QueueTrait;
pub use implementation::SimpleQueue;
pub use queue_server::{QueueError, QueueServerHandle};