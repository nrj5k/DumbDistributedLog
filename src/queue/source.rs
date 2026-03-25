//! Queue source implementations
//!
//! Provides the QueueSource trait and implementations for different queue data sources.

use crate::queue::interval::IntervalConfig;
use crate::queue::persistence::QueuePersistence;
use crate::queue::spmc_lockfree_queue::SPMCLockFreeQueue as SimpleQueue;
use crate::queue::QueueError;
use crate::traits::queue::QueueTrait;
use crate::types::convert_to_f64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::time::interval;

/// Error type for AutoQueues operations
#[derive(Debug, thiserror::Error)]
pub enum AutoQueuesError {
    #[error("Queue error: {0}")]
    QueueError(#[from] QueueError),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Source error: {0}")]
    SourceError(String),
    
    #[error("Queue already exists: {0}")]
    QueueAlreadyExists(String),
    
    #[error("Queue not found: {0}")]
    QueueNotFound(String),
}

/// Function source with lifecycle control
///
/// This source wraps a function and provides pause/resume/remove functionality
/// using atomic flags for thread-safe control.
pub struct FunctionSource<F, T>
where
    F: Fn() -> T + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    /// The function to call for generating queue data
    func: F,
    /// Flag indicating if the source is paused
    paused: Arc<AtomicBool>,
    /// Flag indicating if the source should stop
    should_stop: Arc<AtomicBool>,
}

impl<F, T> FunctionSource<F, T>
where
    F: Fn() -> T + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    /// Create a new function source
    pub fn new(func: F) -> Self {
        Self {
            func,
            paused: Arc::new(AtomicBool::new(false)),
            should_stop: Arc::new(AtomicBool::new(false)),
        }
    }
    
    /// Check if the source is paused
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }
    
    /// Check if the source should stop
    pub fn should_stop(&self) -> bool {
        self.should_stop.load(Ordering::Relaxed)
    }
}

impl<F, T> QueueSource<T> for FunctionSource<F, T>
where
    F: Fn() -> T + Send + Sync + Clone + 'static,
    T: Clone + Send + Sync + 'static,
{
    fn start(
        &self,
        queue: Arc<RwLock<SimpleQueue<T>>>,
        interval_config: IntervalConfig,
        persistence: Option<Arc<QueuePersistence>>,
    ) {
        let f = self.func.clone();
        let paused = self.paused.clone();
        let should_stop = self.should_stop.clone();
        let queue = queue.clone();

        tokio::spawn(async move {
            // Get the interval duration from the config
            let interval_duration = match &interval_config {
                IntervalConfig::Constant(ms) => Duration::from_millis(*ms),
                IntervalConfig::Adaptive { initial_ms, .. } => Duration::from_millis(*initial_ms),
            };
            
            let mut interval = interval(interval_duration);
            
            // Store persistence reference for use in the loop
            let persistence_ref = persistence.clone();
            
            loop {
                // Check if we should stop
                if should_stop.load(Ordering::Relaxed) {
                    break;
                }
                
                // Wait for the next interval tick
                interval.tick().await;
                
                // Skip if paused
                if paused.load(Ordering::Relaxed) {
                    continue;
                }
                
                // Call the function to get data
                let result = f();
                
                // Publish to queue
                if let Ok(mut queue_guard) = queue.write() {
                    QueueTrait::publish(&mut *queue_guard, result.clone()).ok();
                }
                
                // Persist data if persistence is enabled
                if let Some(ref persister) = persistence_ref {
                    // Convert result to f64 for persistence (this assumes T can be converted to f64)
                    // In a real implementation, you'd want to handle this more gracefully
                    if let Some(value) = convert_to_f64(&result) {
                        persister.persist(value);
                    }
                }
            }
        });
    }

    fn pause(&self) {
        self.paused.store(true, Ordering::Relaxed);
    }

    fn resume(&self) {
        self.paused.store(false, Ordering::Relaxed);
    }

    fn remove(&self) {
        self.should_stop.store(true, Ordering::Relaxed);
    }
}

/// Trait for queue data sources
pub trait QueueSource<T: Clone + Send + Sync + 'static>: Send {
    /// Start the queue source with interval configuration and optional persistence
    fn start(
        &self,
        queue: Arc<RwLock<SimpleQueue<T>>>,
        interval: IntervalConfig,
        persistence: Option<Arc<QueuePersistence>>,
    );

    /// Pause the queue source
    fn pause(&self);

    /// Resume the queue source
    fn resume(&self);

    /// Remove the queue source
    fn remove(&self);
}