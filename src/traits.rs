//! Queue Traits Module
//!
//! This module defines the core traits that queue implementations must provide.
//! These traits establish the interface for different queue types in the system.

use crate::aimd::{AimdConfig, AimdStats};
use crate::enums::{Mode, Model, QueueValue};
use crate::types::QueueConfig;

/// Trait for sensor-based queue implementations
///
/// Sensor queues are designed to collect and process data from various sensors
/// or data sources. They focus on data ingestion and basic processing capabilities.
pub trait SensorQueue<T> {
    /// Get the current mode of the sensor queue
    fn get_mode(&self) -> Mode;

    /// Get the current data from the sensor
    fn get_data(&self) -> Option<T>;

    /// Process incoming data point
    fn process_data(&mut self, data: T) -> Result<(), crate::enums::QueueError>;

    /// Check if the queue is ready for new data
    fn is_ready(&self) -> bool;
}

/// Trait for insight-based queue implementations
///
/// Insight queues are designed to provide analysis and insights based on
/// collected data. They focus on data analysis, pattern recognition, and
/// generating actionable insights.
pub trait InsightQueue<T> {
    /// Get the current model used for analysis
    fn get_model(&self) -> Model;

    /// Get insights based on current data
    fn get_insights(&self) -> Vec<T>;

    /// Analyze data and generate insights
    fn analyze(&mut self, data: &[T]) -> Result<(), crate::enums::QueueError>;

    /// Get confidence score for current insights
    fn get_confidence(&self) -> f64;
}

/// Common queue operations that all queue types should support
pub trait QueueOperations<T> {
    /// Get queue configuration
    fn get_config(&self) -> &QueueConfig<T>;

    /// Get current queue statistics
    fn get_stats(&self) -> crate::types::QueueStats;

    /// Reset the queue to initial state
    fn reset(&mut self) -> Result<(), crate::enums::QueueError>;

    /// Check if queue is empty
    fn is_empty(&self) -> bool;

    /// Get current queue size
    fn size(&self) -> usize;
}

/// Trait for queues that support AIMD (Additive Increase Multiplicative Decrease)
pub trait AimdQueue {
    /// Get current AIMD statistics
    fn get_aimd_stats(&self) -> Option<AimdStats>;

    /// Update AIMD configuration
    fn update_aimd_config(&mut self, config: AimdConfig) -> Result<(), crate::enums::QueueError>;

    /// Get current processing interval
    fn get_current_interval(&self) -> u64;

    /// Force interval adjustment
    fn force_adjustment(&mut self) -> Result<(), crate::enums::QueueError>;
}
