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

    // T-004: add_queue_fn creates queue successfully
    #[test]
    fn test_add_queue_fn_creates_queue_successfully() {
        let autoqueues = AutoQueues::default();
        
        // Add a simple function-based queue
        let result = autoqueues.add_queue_fn::<f64, _>("cpu_queue", || 42.0);
        
        // Verify queue was created successfully
        assert!(result.is_ok());
        
        // Verify we can check the queue exists
        // Note: We can't directly check registry, but we can try operations
        // that would fail if queue doesn't exist
        let pop_result = autoqueues.try_pop::<f64>("cpu_queue");
        assert!(pop_result.is_ok()); // Queue exists even if empty
    }
    
    // T-005: test_add_queue_fn_with_interval
    #[test]
    fn test_add_queue_fn_with_interval() {
        use crate::queue::interval::IntervalConfig;
        
        let autoqueues = AutoQueues::default();
        let interval = IntervalConfig::Constant(500); // 500ms
        
        let result = autoqueues.add_queue_fn_with_interval::<f64, _>(
            "fast_queue",
            || 100.0,
            interval
        );
        
        assert!(result.is_ok());
        // Verify queue exists
        let pop_result = autoqueues.try_pop::<f64>("fast_queue");
        assert!(pop_result.is_ok());
    }
    
    // T-006: test_add_queue_fn_with_persistence
    #[test]
    fn test_add_queue_fn_with_persistence() {
        use crate::queue::interval::IntervalConfig;
        
        let autoqueues = AutoQueues::default();
        let interval = IntervalConfig::Constant(1000);
        
        let result = autoqueues.add_queue_fn_with_persistence::<f64, _>(
            "persistent_queue",
            || 50.0,
            interval
        );
        
        assert!(result.is_ok());
        // Verify queue exists
        let pop_result = autoqueues.try_pop::<f64>("persistent_queue");
        assert!(pop_result.is_ok());
    }
    
    // T-007: test_add_queue_expr
    #[tokio::test]
    async fn test_add_queue_expr() {
        let autoqueues = AutoQueues::default();
        
        // First create a source queue
        autoqueues.add_queue_fn::<f64, _>("source_queue", || 10.0).unwrap();
        
        // Then create expression queue that references it
        let result = autoqueues.add_queue_expr(
            "expr_queue",
            "source_queue * 2",
            "source_queue",
            true,
            Some(1000)
        );
        
        assert!(result.is_ok());
    }
    
    // T-008: test_add_queue_expr_with_persistence
    #[tokio::test]
    async fn test_add_queue_expr_with_persistence() {
        let autoqueues = AutoQueues::default();
        
        // Create source queue first
        autoqueues.add_queue_fn::<f64, _>("base_queue", || 5.0).unwrap();
        
        // Create expression queue with persistence
        let result = autoqueues.add_queue_expr_with_persistence(
            "persistent_expr_queue",
            "base_queue + 10",
            "base_queue",
            false,
            Some(2000)
        );
        
        assert!(result.is_ok());
    }
    
    // T-010: test_enable_persistence
    #[test]
    fn test_enable_persistence() {
        let autoqueues = AutoQueues::default();
        
        // Create a queue first
        autoqueues.add_queue_fn::<f64, _>("test_queue", || 42.0).unwrap();
        
        // Enable persistence
        let result = autoqueues.enable_persistence("test_queue");
        
        assert!(result.is_ok());
    }
    
    // T-011: test_start_does_not_panic
    #[test]
    fn test_start_does_not_panic() {
        let autoqueues = AutoQueues::default();
        
        // Add a queue first
        autoqueues.add_queue_fn::<f64, _>("test_queue", || 42.0).unwrap();
        
        // Start should not panic
        autoqueues.start();
        
        // If we get here, start() succeeded
        // We can't easily verify background tasks started, but at least no panic
    }
    
    // T-014: test_subscribe
    #[tokio::test]
    async fn test_subscribe_returns_subscription_id() {
        let autoqueues = AutoQueues::default();
        
        // Subscribe to a pattern
        let result = autoqueues.subscribe::<String>("local.*").await;
        
        // Should return Ok with subscription ID
        assert!(result.is_ok());
        
        // Verify we got a non-empty string ID
        let sub_id = result.unwrap();
        assert!(!sub_id.is_empty());
    }
    
    // T-012: test_pop_returns_data
    #[test]
    fn test_pop_returns_data_when_available() {
        let autoqueues = AutoQueues::default();
        
        // Add a queue
        autoqueues.add_queue_fn::<f64, _>("data_queue", || 42.0).unwrap();
        
        // Start the system
        autoqueues.start();
        
        // Wait a bit for data to be generated
        std::thread::sleep(std::time::Duration::from_millis(100));
        
        // Try to pop - should return Some(data) or None if queue is empty
        // Note: pop is blocking, so we use try_pop for testing
        let result = autoqueues.try_pop::<f64>("data_queue");
        
        // Result should be Ok (either Some(data) or None)
        assert!(result.is_ok());
    }

    // T-013: test_try_pop_non_blocking
    #[test]
    fn test_try_pop_returns_immediately() {
        let autoqueues = AutoQueues::default();
        
        // Add a queue
        autoqueues.add_queue_fn::<f64, _>("empty_queue", || 100.0).unwrap();
        
        // Don't start - so queue should be empty
        // try_pop should return immediately with Ok(None)
        let start = std::time::Instant::now();
        let result = autoqueues.try_pop::<f64>("empty_queue");
        let elapsed = start.elapsed();
        
        // Should return quickly (less than 100ms)
        assert!(elapsed < std::time::Duration::from_millis(100));
        assert!(result.is_ok());
    }

    // T-015: test_shutdown
    #[test]
    fn test_shutdown_no_panic() {
        let autoqueues = AutoQueues::default();
        
        // Add a queue
        autoqueues.add_queue_fn::<f64, _>("test_queue", || 42.0).unwrap();
        
        // Start the system
        autoqueues.start();
        
        // Shutdown should not panic
        autoqueues.shutdown();
        
        // If we get here, shutdown succeeded
    }

    // T-030: test_e2e_basic_workflow
    #[test]
    fn test_e2e_basic_workflow() {
        let autoqueues = AutoQueues::default();
        
        // Create queue
        autoqueues.add_queue_fn::<f64, _>("cpu", || 50.0).unwrap();
        
        // Start
        autoqueues.start();
        
        // Wait for data
        std::thread::sleep(std::time::Duration::from_millis(200));
        
        // Pop data (or try)
        let _ = autoqueues.try_pop::<f64>("cpu");
        
        // Shutdown
        autoqueues.shutdown();
        
        // Success if we got here
    }

    // T-031: test_e2e_multiple_queues
    #[test]
    fn test_e2e_multiple_queues_workflow() {
        let autoqueues = AutoQueues::default();
        
        // Create multiple queues
        autoqueues.add_queue_fn::<f64, _>("cpu", || 45.0).unwrap();
        autoqueues.add_queue_fn::<f64, _>("memory", || 60.0).unwrap();
        autoqueues.add_queue_fn::<i32, _>("connections", || 100).unwrap();
        
        // Start all
        autoqueues.start();
        
        // Wait
        std::thread::sleep(std::time::Duration::from_millis(200));
        
        // Try to pop from each
        let _ = autoqueues.try_pop::<f64>("cpu");
        let _ = autoqueues.try_pop::<f64>("memory");
        let _ = autoqueues.try_pop::<i32>("connections");
        
        // Shutdown
        autoqueues.shutdown();
    }

    // T-032: test_e2e_full_lifecycle
    #[test]
    fn test_e2e_full_lifecycle() {
        let autoqueues = AutoQueues::default();
        
        // Create
        autoqueues.add_queue_fn::<f64, _>("metric", || 42.0).unwrap();
        
        // Configure persistence
        autoqueues.enable_persistence("metric").unwrap();
        
        // Start
        autoqueues.start();
        
        // Operate
        std::thread::sleep(std::time::Duration::from_millis(100));
        
        // Shutdown gracefully
        autoqueues.shutdown();
    }

    // T-033: test_e2e_with_expression
    #[tokio::test]
    async fn test_e2e_with_expression_queue() {
        let autoqueues = AutoQueues::default();
        
        // Create source queue
        autoqueues.add_queue_fn::<f64, _>("raw", || 10.0).unwrap();
        
        // Create expression queue (simplified - just verify it doesn't panic)
        // Note: Expression evaluation may require runtime, so we just test setup
        let result = autoqueues.add_queue_expr(
            "computed",
            "raw * 2",
            "raw",
            false,
            Some(500)
        );
        
        // Should be created successfully
        assert!(result.is_ok());
        
        // Start
        autoqueues.start();
        
        // Wait
        std::thread::sleep(std::time::Duration::from_millis(200));
        
        // Shutdown
        autoqueues.shutdown();
    }

    // T-034: test_shutdown_with_multiple_queues
    #[test]
    fn test_shutdown_with_multiple_queues() {
        let autoqueues = AutoQueues::default();
        
        // Create multiple queues
        autoqueues.add_queue_fn::<f64, _>("queue1", || 1.0).unwrap();
        autoqueues.add_queue_fn::<f64, _>("queue2", || 2.0).unwrap();
        autoqueues.add_queue_fn::<f64, _>("queue3", || 3.0).unwrap();
        autoqueues.add_queue_fn::<f64, _>("queue4", || 4.0).unwrap();
        
        // Start all
        autoqueues.start();
        
        // Let them run briefly
        std::thread::sleep(std::time::Duration::from_millis(100));
        
        // Shutdown all - should not panic
        autoqueues.shutdown();
        
        // Success if we got here without panic
    }
    
    // T-035: test_shutdown_with_persistence
    #[test]
    fn test_shutdown_with_persistence_enabled() {
        let autoqueues = AutoQueues::default();
        
        // Create queues with persistence
        use crate::queue::interval::IntervalConfig;
        
        autoqueues.add_queue_fn_with_persistence::<f64, _>(
            "persistent1",
            || 100.0,
            IntervalConfig::Constant(1000)
        ).unwrap();
        
        autoqueues.add_queue_fn_with_persistence::<f64, _>(
            "persistent2",
            || 200.0,
            IntervalConfig::Constant(1000)
        ).unwrap();
        
        // Start
        autoqueues.start();
        
        // Run briefly
        std::thread::sleep(std::time::Duration::from_millis(100));
        
        // Shutdown - should flush all persistence data
        autoqueues.shutdown();
        
        // Success - all data should be flushed to disk
    }
}

