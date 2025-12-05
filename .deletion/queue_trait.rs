//! Core Queue Trait Definition
//!
//! This module defines the fundamental `Queue` trait that all queue implementations
//! in the autoqueues system must follow. This provides a unified interface for
//! different queue types while allowing specialized behavior through additional traits.

use crate::enums::QueueError;
use crate::types::{QueueConfig, QueueData, QueueStats, Timestamp};

/// Core queue trait that all queue implementations must follow
///
/// This trait provides the basic interface for queue operations.
/// All queue variants should implement this trait as their foundation.
pub trait QueueOperations<T> {
    /// Publish data to the queue
    fn publish(
        &mut self,
        value: T,
    ) -> impl std::future::Future<Output = Result<(), QueueError>> + Send;

    /// Get the latest data point from the queue
    fn get_latest(&self) -> impl std::future::Future<Output = Option<QueueData<T>>> + Send;

    /// Get queue statistics
    fn get_stats(&self) -> QueueStats;

    /// Get queue configuration
    fn get_config(&self) -> &QueueConfig<T>;
}

/// Extended queue trait for queues that support data retrieval by time range
pub trait TimeRangeQueue<T>: QueueOperations<T> {
    /// Get data points within a specific time range
    fn get_data_in_range(
        &self,
        start: Timestamp,
        end: Timestamp,
    ) -> impl std::future::Future<Output = Vec<QueueData<T>>> + Send;
}

/// AIMD-capable queue trait for queues that support adaptive interval management
pub trait AimdQueue<T>: QueueOperations<T> {
    /// Get AIMD statistics
    fn get_aimd_stats(&self) -> Option<crate::aimd::AimdStats>;

    /// Get current processing interval
    fn get_current_interval(&self) -> Timestamp;
}
