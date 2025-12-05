//! Queue Module - KISS Simplified
//!
//! Clean, focused queue operations following KISS principle.
//! Implements ultra-minimal Queue trait with 4 essential methods.

pub mod implementation;
pub mod queue_server;

// Re-export main queue types
pub use implementation::SimpleQueue;
pub use queue_server::{Queue, QueueError, QueueServerHandle};
