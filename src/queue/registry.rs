//! Queue registry implementation
//!
//! Manages queues and their sources in a centralized registry.

use crate::config::Config;

use crate::queue::persistence::{PersistenceManager, QueuePersistence};
use crate::queue::source::{AutoQueuesError, ExpressionSource, QueueSource};
use crate::queue::spmc_lockfree_queue::SPMCLockFreeQueue as SimpleQueue;
use crate::queue::QueueError;
use crate::IntervalConfig;
use dashmap::DashMap;
use std::any::Any;
use std::sync::{Arc, RwLock};

/// Type-erased queue entry for the registry
pub trait AnyQueue: Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Clone + Send + Sync + 'static> AnyQueue for Arc<RwLock<SimpleQueue<T>>> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Queue registry for managing queues and sources
pub struct QueueRegistry {
    queues: DashMap<String, Box<dyn AnyQueue>>,
    sources: DashMap<String, Box<dyn Any>>,
    config: Config,
    persistence_manager: PersistenceManager,
}

impl QueueRegistry {
    /// Create a new queue registry
    pub fn new(config: Config) -> Self {
        let persistence_config = config.persistence.clone().unwrap_or_default();
        Self {
            queues: DashMap::new(),
            sources: DashMap::new(),
            config,
            persistence_manager: PersistenceManager::new(persistence_config),
        }
    }

    /// Add a queue with its source to the registry
    pub fn add_queue<T: Clone + Send + Sync + 'static>(
        &self,
        name: &str,
        source: impl QueueSource<T> + 'static,
    ) -> Result<(), AutoQueuesError> {
        self.add_queue_with_interval_and_persistence(name, source, IntervalConfig::default(), false)
    }

    /// Add a queue with its source to the registry and enable persistence
    pub fn add_queue_with_persistence<T: Clone + Send + Sync + 'static>(
        &self,
        name: &str,
        source: impl QueueSource<T> + 'static,
    ) -> Result<(), AutoQueuesError> {
        self.add_queue_with_interval_and_persistence(name, source, IntervalConfig::default(), true)
    }

    /// Add a queue with its source to the registry with specific interval configuration
    pub fn add_queue_with_interval<T: Clone + Send + Sync + 'static>(
        &self,
        name: &str,
        source: impl QueueSource<T> + 'static,
        interval: IntervalConfig,
    ) -> Result<(), AutoQueuesError> {
        self.add_queue_with_interval_and_persistence(name, source, interval, false)
    }

    /// Add a queue with its source to the registry with specific interval configuration and optional persistence
    pub fn add_queue_with_interval_and_persistence<T: Clone + Send + Sync + 'static>(
        &self,
        name: &str,
        source: impl QueueSource<T> + 'static,
        interval: IntervalConfig,
        enable_persistence: bool,
    ) -> Result<(), AutoQueuesError> {
        let queue: Arc<RwLock<SimpleQueue<T>>> = Arc::new(RwLock::new(SimpleQueue::new()));

        // Get persistence handle if enabled
        let persistence = if enable_persistence {
            Some(
                self.persistence_manager
                    .enable_for_queue(name)
                    .map_err(|e| {
                        AutoQueuesError::SourceError(format!("Persistence error: {}", e))
                    })?,
            )
        } else {
            None
        };

        // Start the source
        source.start(queue.clone(), interval, persistence);

        // Store the queue and source
        self.queues.insert(name.to_string(), Box::new(queue));
        self.sources.insert(name.to_string(), Box::new(source));

        Ok(())
    }

    /// Add an expression-based queue to the registry
    pub fn add_expression_queue(
        &self,
        name: &str,
        expression: &str,
        source_queue: &str,
        trigger_on_push: bool,
        trigger_interval_ms: Option<u64>,
    ) -> Result<(), AutoQueuesError> {
        self.add_expression_queue_with_persistence(
            name,
            expression,
            source_queue,
            trigger_on_push,
            trigger_interval_ms,
            false,
        )
    }

    /// Add an expression-based queue to the registry with optional persistence
    pub fn add_expression_queue_with_persistence(
        &self,
        name: &str,
        expression: &str,
        source_queue: &str,
        trigger_on_push: bool,
        trigger_interval_ms: Option<u64>,
        enable_persistence: bool,
    ) -> Result<(), AutoQueuesError> {
        // TODO: Validate the expression
        // For now, we'll skip validation since the expression module was removed
        let _ = expression;

        let source = ExpressionSource::new(
            expression.to_string(),
            source_queue.to_string(),
            trigger_on_push,
            trigger_interval_ms,
        );

        // Expressions always evaluate to f64 values
        self.add_queue_with_interval_and_persistence::<f64>(
            name,
            source,
            IntervalConfig::default(),
            enable_persistence,
        )
    }

    /// Check if a queue exists in the registry
    pub fn queue_exists(&self, name: &str) -> bool {
        self.queues.contains_key(name)
    }

    /// Get a queue from the registry
    pub fn get_queue<T: Clone + Send + Sync + 'static>(
        &self,
        name: &str,
    ) -> Result<Arc<RwLock<SimpleQueue<T>>>, AutoQueuesError> {
        if let Some(queue) = self.queues.get(name) {
            if let Some(typed_queue) = queue.as_any().downcast_ref::<Arc<RwLock<SimpleQueue<T>>>>()
            {
                Ok(typed_queue.clone())
            } else {
                Err(AutoQueuesError::QueueError(QueueError::Other(
                    "Queue type mismatch".to_string(),
                )))
            }
        } else {
            Err(AutoQueuesError::QueueNotFound(name.to_string()))
        }
    }

    /// Start all queues
    pub fn start(&self) {
        // In a real implementation, this would start all queue sources
        // For now, we'll just iterate through the queues
        for queue in self.queues.iter() {
            println!("Starting queue: {}", queue.key());
        }
    }

    /// Pause a queue
    pub fn pause(&self, name: &str) {
        if let Some(_source) = self.sources.get(name) {
            // In a real implementation, we would call source.pause()
            println!("Pausing queue: {}", name);
        }
    }

    /// Resume a queue
    pub fn resume(&self, name: &str) {
        if let Some(_source) = self.sources.get(name) {
            // In a real implementation, we would call source.resume()
            println!("Resuming queue: {}", name);
        }
    }

    /// Remove a queue
    pub fn remove(&self, name: &str) {
        self.queues.remove(name);
        self.sources.remove(name);
        println!("Removed queue: {}", name);
    }

    /// Enable persistence for a queue
    pub fn enable_persistence(&self, queue_name: &str) -> Result<(), AutoQueuesError> {
        self.persistence_manager
            .enable_for_queue(queue_name)
            .map_err(|e| AutoQueuesError::SourceError(format!("Persistence error: {}", e)))?;
        Ok(())
    }

    /// Get persistence handle for a queue
    pub fn get_persistence_handle(&self, queue_name: &str) -> Option<Arc<QueuePersistence>> {
        self.persistence_manager.get_handle(queue_name)
    }

    /// Shutdown all persistence handles gracefully
    pub fn shutdown_all_persistence(&self) {
        self.persistence_manager.shutdown_all();
    }
}
