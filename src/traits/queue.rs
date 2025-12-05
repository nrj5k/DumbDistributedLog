//! Queue trait definitions for AutoQueues
//!
//! Provides ultra-minimal trait interfaces following KISS principle.
//! Each trait has 3-4 methods maximum for maximum flexibility.

use crate::types::Timestamp;

/// Ultra-minimal queue trait for maximum flexibility
pub trait Queue: Send + Sync {
    type Data: Clone + Send + 'static;

    /// Publish data to queue
    fn publish(&mut self, data: Self::Data) -> Result<(), crate::QueueError>;

    /// Get most recent data
    fn get_latest(&self) -> Option<(Timestamp, Self::Data)>;

    /// Get N most recent data items
    fn get_latest_n(&self, n: usize) -> Vec<Self::Data>;

    /// Start autonomous server
    fn start_server(self) -> Result<crate::QueueServerHandle, crate::QueueError>;
}
