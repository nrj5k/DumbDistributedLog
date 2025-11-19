//! Queue Types Module
//!
//! This module provides core types for the queue system.

use crate::enums::{Mode, Model, QueueValue};
use std::sync::Arc;

/// Function hook type for queue data processing
pub type FunctionHook<T> = Arc<dyn Fn(Vec<T>) -> Result<T, crate::enums::QueueError> + Send + Sync>;

/// Queue configuration with function hook
#[derive(Clone)]
pub struct QueueConfig<T> {
    pub mode: Mode,
    pub hook: FunctionHook<T>,
    pub model: Model,
    pub queue_type: QueueType,
}

impl<T> QueueConfig<T> {
    pub fn new(mode: Mode, hook: FunctionHook<T>, model: Model, queue_type: QueueType) -> Self {
        Self {
            mode,
            hook,
            model,
            queue_type,
        }
    }
}

/// Queue type definition
#[derive(Debug, Clone)]
pub struct QueueType {
    pub value: QueueValue,
    pub base_interval: u64,
    pub increase_factor: u64,
    pub tracefile: String,
    pub tracevar: String,
}

impl QueueType {
    pub fn new(
        value: QueueValue,
        base_interval: u64,
        increase_factor: u64,
        tracefile: String,
        tracevar: String,
    ) -> Self {
        Self {
            value,
            base_interval,
            increase_factor,
            tracefile,
            tracevar,
        }
    }
}

/// Queue statistics for monitoring
#[derive(Debug, Clone)]
pub struct QueueStats {
    pub size: usize,
    pub capacity: usize,
    pub oldest_timestamp: Option<u64>,
    pub newest_timestamp: Option<u64>,
    pub mode: Mode,
    pub model: Model,
    pub interval_ms: u64,
}
