//! AutoQueues Programmatic API
//!
//! Building on AutoQueues programmatically, adding queues and auto-population functions.
//! Generic over type T for maximum flexibility.

use crate::autoqueues_config::Config;
use crate::pubsub::{PubSubBroker, SubscriptionId};
use crate::queue::registry::QueueRegistry;
use crate::queue::source::{AutoQueuesError, QueueSource};
use crate::traits::queue::QueueTrait;
use serde::{Serialize, de::DeserializeOwned};
use std::net::SocketAddr;
use std::sync::Arc;

/// Main AutoQueues struct for programmatic mode.
pub struct AutoQueues {
    registry: Arc<QueueRegistry>,
    config: Config,
    broker: Arc<PubSubBroker>,
}

impl AutoQueues {
    /// Create a new AutoQueues instance with the given configuration
    pub fn new(config: Config) -> Self {
        let registry = Arc::new(QueueRegistry::new(config.clone()));
        let broker = Arc::new(PubSubBroker::new(100));

        Self {
            registry,
            config,
            broker,
        }
    }

    /// Create a new AutoQueues instance with default configuration
    pub fn default() -> Self {
        Self::new(Config::default())
    }

    /// Configure AutoQueues
    pub fn configure() -> Config {
        Config::new()
    }

    /// Add a queue with a function source
    pub fn add_queue_fn<T, F>(&self, name: &str, func: F) -> Result<&Self, AutoQueuesError>
    where
        T: Clone + Send + Sync + 'static,
        F: QueueSource<T> + 'static,
    {
        self.registry.add_queue(name, func)?;
        Ok(self)
    }

    /// Add a queue with an expression source
    pub fn add_queue_expr(
        &self,
        name: &str,
        expression: &str,
        source_queue: &str,
        trigger_on_push: bool,
        trigger_interval_ms: Option<u64>,
    ) -> Result<&Self, AutoQueuesError> {
        self.registry.add_expression_queue(
            name,
            expression,
            source_queue,
            trigger_on_push,
            trigger_interval_ms,
        )?;
        Ok(self)
    }

    /// Add a distributed aggregation queue
    pub fn add_distributed_queue(
        &self,
        name: &str,
        operation: &str,
        source_queues: Vec<String>,
    ) -> Result<&Self, AutoQueuesError> {
        // Convert node strings to SocketAddr
        let nodes: Vec<SocketAddr> = self
            .config
            .nodes
            .iter()
            .filter_map(|node_str| node_str.parse().ok())
            .collect();

        let source = crate::queue::source::DistributedAggregationSource::new(
            name.to_string(),
            operation.to_string(),
            source_queues,
            nodes,
        );

        // Distributed aggregations always evaluate to f64 values
        self.registry.add_queue::<f64>(name, source)?;
        Ok(self)
    }

    /// Start all queues
    pub fn start(&self) {
        self.registry.start();
    }

    /// Start a specific queue
    pub fn start_queue(&self, name: &str) {
        // In a real implementation, this would start the specific queue
        println!("Starting queue: {}", name);
    }

    /// Pause a queue
    pub fn pause(&self, name: &str) {
        self.registry.pause(name);
    }

    /// Resume a queue
    pub fn resume(&self, name: &str) {
        self.registry.resume(name);
    }

    /// Remove a queue
    pub fn remove(&mut self, name: &str) {
        self.registry.remove(name);
    }

    /// Pop a value from a queue
    pub fn pop<T>(&self, name: &str) -> Result<Option<T>, AutoQueuesError>
    where
        T: Clone + Send + Sync + 'static,
    {
        let queue = self.registry.get_queue::<T>(name)?;
        let guard = queue.try_read().map_err(|_| {
            AutoQueuesError::QueueError(crate::queue::QueueError::Other(
                "Failed to acquire read lock".to_string(),
            ))
        })?;
        Ok(guard.get_latest().map(|(_, value)| value))
    }

    /// Try to pop a value from a queue (non-blocking)
    pub fn try_pop<T>(&self, name: &str) -> Result<Option<T>, AutoQueuesError>
    where
        T: Clone + Send + Sync + 'static,
    {
        let queue = self.registry.get_queue::<T>(name)?;
        let guard = queue.try_read().map_err(|_| {
            AutoQueuesError::QueueError(crate::queue::QueueError::Other(
                "Failed to acquire read lock".to_string(),
            ))
        })?;
        Ok(guard.get_latest().map(|(_, value)| value))
    }

    /// Subscribe to a topic pattern
    pub async fn subscribe<T>(&self, pattern: &str) -> Result<SubscriptionId, AutoQueuesError>
    where
        T: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
    {
        self.broker
            .subscribe_exact(pattern.to_string())
            .await
            .map_err(|e| AutoQueuesError::SourceError(e.to_string()))
    }
}

