//! AutoQueues Programmatic API
//!
//! Building on AutoQueues programmatically, adding queues and auto-population functions.
//! Generic over type T for maximum flexibility.

use crate::config::Config;
use crate::constants;
use crate::network::pubsub::zmq::ZmqPubSubBroker;
use crate::queue::interval::IntervalConfig;
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
    broker: Arc<ZmqPubSubBroker>,
}

impl AutoQueues {
    /// Create a new AutoQueues instance with the given configuration
    pub fn new(config: Config) -> Self {
        let registry = Arc::new(QueueRegistry::new(config.clone()));
        let broker = Arc::new(ZmqPubSubBroker::new().expect("Failed to create ZMQ pubsub broker"));

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
        // Check if queue already exists
        if self.registry.queue_exists(name) {
            return Err(AutoQueuesError::QueueAlreadyExists(name.to_string()));
        }
        
        self.registry.add_queue(name, func)?;
        Ok(self)
    }

    /// Add a queue with a function source and interval configuration
    pub fn add_queue_fn_with_interval<T, F>(
        &self,
        name: &str,
        func: F,
        interval: IntervalConfig,
    ) -> Result<&Self, AutoQueuesError>
    where
        T: Clone + Send + Sync + 'static,
        F: QueueSource<T> + 'static,
    {
        self.registry.add_queue_with_interval(name, func, interval)?;
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
        // Convert node configurations to SocketAddr
        let nodes: Vec<SocketAddr> = self
            .config
            .nodes
            .collect_nodes()
            .iter()
            .filter_map(|(_name, node_config)| {
                format!("{}:{}", node_config.host, node_config.communication_port)
                    .parse()
                    .ok()
            })
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
    pub async fn subscribe<T>(&self, pattern: &str) -> Result<String, AutoQueuesError>
    where
        T: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
    {
        // Using localhost as default connect address for broker
        let connect_addr = format!("tcp://localhost:{}", constants::network::DEFAULT_NODE_COMMUNICATION_PORT);
        
        self.broker
            .subscribe(pattern, &connect_addr)
            .await
            .map_err(|e| AutoQueuesError::SourceError(e.to_string()))
    }
    
    /// Enable persistence for a queue
    pub fn enable_persistence(&self, queue_name: &str) -> Result<&Self, AutoQueuesError> {
        // Check if queue exists
        if !self.registry.queue_exists(queue_name) {
            return Err(AutoQueuesError::QueueNotFound(queue_name.to_string()));
        }
        
        self.registry.enable_persistence(queue_name)?;
        Ok(self)
    }
    
    /// Add queue with function source and persistence enabled
    pub fn add_queue_fn_with_persistence<T, F>(
        &self,
        name: &str,
        func: F,
        interval: IntervalConfig,
    ) -> Result<&Self, AutoQueuesError>
    where
        T: Clone + Send + Sync + 'static,
        F: QueueSource<T> + 'static,
    {
        self.registry.add_queue_with_interval_and_persistence(name, func, interval, true)?;
        Ok(self)
    }
    
    /// Add expression queue with persistence enabled
    pub fn add_queue_expr_with_persistence(
        &self,
        name: &str,
        expression: &str,
        source_queue: &str,
        trigger_on_push: bool,
        trigger_interval_ms: Option<u64>,
    ) -> Result<&Self, AutoQueuesError> {
        self.registry.add_expression_queue_with_persistence(
            name,
            expression,
            source_queue,
            trigger_on_push,
            trigger_interval_ms,
            true,
        )?;
        Ok(self)
    }
    
    /// Graceful shutdown - flushes all persistence data
    pub fn shutdown(&self) {
        println!("Shutting down AutoQueues...");
        
        // Flush all persistence handles
        self.registry.shutdown_all_persistence();
        
        println!("All data flushed to disk.");
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_new_with_valid_config() {
        let config = Config::default();
        let autoqueues = AutoQueues::new(config);
        
        // Verify we can add a queue (proves broker and registry work)
        let result = autoqueues.add_queue_fn::<f64, _>("test", || 42.0);
        assert!(result.is_ok());
        
        // Verify queue exists
        assert!(autoqueues.registry.queue_exists("test"));
    }

    #[test]
    fn test_pop_nonexistent_queue_returns_error() {
        let autoqueues = AutoQueues::default();
        // Try to pop from queue that doesn't exist
        let result = autoqueues.pop::<f64>("nonexistent_queue");
        // Should return QueueNotFound error specifically
        assert!(matches!(result, Err(AutoQueuesError::QueueNotFound(_))));
    }

    // T-017: try_pop on non-existent queue
    #[test]
    fn test_try_pop_nonexistent_queue_returns_error() {
        let autoqueues = AutoQueues::default();
        let result = autoqueues.try_pop::<f64>("nonexistent_queue");
        assert!(matches!(result, Err(AutoQueuesError::QueueNotFound(_))));
    }

    // T-018: enable_persistence on non-existent queue
    #[test]
    fn test_enable_persistence_nonexistent_returns_error() {
        let autoqueues = AutoQueues::default();
        let result = autoqueues.enable_persistence("nonexistent_queue");
        assert!(matches!(result, Err(AutoQueuesError::QueueNotFound(_))));
    }

    // T-020: duplicate queue names
    #[test]
    fn test_add_duplicate_queue_returns_error() {
        let autoqueues = AutoQueues::default();
        autoqueues.add_queue_fn::<f64, _>("test_queue", || 42.0).unwrap();
        let result = autoqueues.add_queue_fn::<f64, _>("test_queue", || 100.0);
        assert!(matches!(result, Err(AutoQueuesError::QueueAlreadyExists(_))));
    }
}

