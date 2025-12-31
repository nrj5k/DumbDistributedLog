//! Queue Types Module
//!
//! Core types for the queue system

use std::sync::atomic::{AtomicU64, Ordering};

/// Timestamp type for queue data
pub type Timestamp = u64;

/// Queue data point with timestamp - tuple format for simplicity
pub type QueueData<T> = (Timestamp, T);

/// Queue identifier - u32 for performance
pub type QueueId = u32;

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

/// Atomic queue statistics - lock-free alternative to QueueStats
///
/// Uses AtomicU64 for both fields, eliminating mutex contention
/// on every publish operation.
#[derive(Debug)]
pub struct AtomicQueueStats {
    pub total_published: AtomicU64,
    pub last_updated: AtomicU64,
}

impl AtomicQueueStats {
    /// Create new atomic stats with zero values
    pub fn new() -> Self {
        Self {
            total_published: AtomicU64::new(0),
            last_updated: AtomicU64::new(0),
        }
    }

    /// Increment the published counter (lock-free)
    #[inline]
    pub fn increment_published(&self) {
        self.total_published.fetch_add(1, Ordering::Relaxed);
    }

    /// Update the last updated timestamp (lock-free)
    #[inline]
    pub fn update_timestamp(&self, timestamp: Timestamp) {
        self.last_updated.store(timestamp, Ordering::Relaxed);
    }

    /// Get current statistics snapshot (for reporting)
    #[inline]
    pub fn snapshot(&self) -> QueueStats {
        QueueStats {
            total_published: self.total_published.load(Ordering::Acquire),
            last_updated: self.last_updated.load(Ordering::Acquire),
        }
    }

    /// Get total published count
    #[inline]
    pub fn total_published(&self) -> u64 {
        self.total_published.load(Ordering::Acquire)
    }

    /// Get last updated timestamp
    #[inline]
    pub fn last_updated(&self) -> Timestamp {
        self.last_updated.load(Ordering::Acquire)
    }
}

impl Default for AtomicQueueStats {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_queue_stats_creation() {
        let stats = AtomicQueueStats::new();
        assert_eq!(stats.total_published(), 0);
        assert_eq!(stats.last_updated(), 0);
    }

    #[test]
    fn test_atomic_queue_stats_increment() {
        let stats = AtomicQueueStats::new();
        stats.increment_published();
        assert_eq!(stats.total_published(), 1);

        stats.increment_published();
        stats.increment_published();
        assert_eq!(stats.total_published(), 3);
    }

    #[test]
    fn test_atomic_queue_stats_timestamp() {
        let stats = AtomicQueueStats::new();
        stats.update_timestamp(12345);
        assert_eq!(stats.last_updated(), 12345);
    }

    #[test]
    fn test_atomic_queue_stats_snapshot() {
        let stats = AtomicQueueStats::new();
        stats.increment_published();
        stats.increment_published();
        stats.update_timestamp(54321);

        let snapshot = stats.snapshot();
        assert_eq!(snapshot.total_published, 2);
        assert_eq!(snapshot.last_updated, 54321);
    }
}
