//! Queue Types Module - KISS Simplified
//!
//! Core types for the queue system following KISS principle.

/// Timestamp type for queue data
pub type Timestamp = u64;

/// Queue data point with timestamp - tuple format for simplicity
pub type QueueData<T> = (Timestamp, T);

/// Queue statistics for monitoring
#[derive(Debug, Clone)]
pub struct QueueStats {
    pub total_published: u64,
    pub last_updated: Timestamp,
}

impl QueueStats {
    pub fn new() -> Self {
        Self {
            total_published: 0,
            last_updated: 0,
        }
    }
}
