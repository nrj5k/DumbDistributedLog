//! AutoQueues Server Mode - Redis-like API
//!
//! Provides a simple server interface where users can push data to topics
//! and subscribe to results. Includes built-in expression processing.

use crate::config::{QueueConfig, ConfigError};
use crate::expression::{ExpressionF64, SimpleExpression};
use crate::pubsub::{PubSubBroker, PubSubError, SubscriptionId};
use crate::queue::{SimpleQueue, QueueError};
use crate::traits::queue::QueueTrait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

/// AutoQueues server errors
#[derive(Debug, thiserror::Error)]
pub enum AutoQueuesServerError {
    #[error("Configuration error: {0}")]
    ConfigError(#[from] ConfigError),
    
    #[error("Queue error: {0}")]
    QueueError(#[from] QueueError),
    
    #[error("PubSub error: {0}")]
    PubSubError(#[from] PubSubError),
    
    #[error("Expression error: {0}")]
    ExpressionError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Server already running")]
    AlreadyRunning,
    
    #[error("Server not running")]
    NotRunning,
}

/// AutoQueues server configuration builder
pub struct AutoQueuesServerBuilder {
    config_path: Option<String>,
    port: Option<u16>,
    push_topics: Vec<String>,
    subscribe_topics: Vec<String>,
}

impl AutoQueuesServerBuilder {
    /// Create a new builder with default settings
    pub fn new() -> Self {
        Self {
            config_path: None,
            port: None,
            push_topics: Vec::new(),
            subscribe_topics: Vec::new(),
        }
    }

    /// Set configuration file path
    pub fn config_file(mut self, path: &str) -> Self {
        self.config_path = Some(path.to_string());
        self
    }

    /// Set server port
    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    /// Add a topic to accept pushes on
    pub fn push_topic(mut self, topic: &str) -> Self {
        self.push_topics.push(topic.to_string());
        self
    }

    /// Add a topic to allow subscriptions on
    pub fn subscribe_topic(mut self, topic: &str) -> Self {
        self.subscribe_topics.push(topic.to_string());
        self
    }

    /// Build the server instance
    pub fn build(self) -> Result<AutoQueuesServer, AutoQueuesServerError> {
        let config = if let Some(config_path) = self.config_path {
            QueueConfig::from_file(&config_path)?
        } else {
            QueueConfig::create_minimal()
        };

        Ok(AutoQueuesServer::new(config, self.port.unwrap_or(6969)))
    }
}

impl Default for AutoQueuesServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Main AutoQueues server struct for server mode
pub struct AutoQueuesServer {
    config: QueueConfig,
    port: u16,
    broker: Arc<PubSubBroker>,
    queues: Arc<RwLock<HashMap<String, Arc<RwLock<SimpleQueue<f64>>>>>>,
    expressions: Arc<RwLock<HashMap<String, ExpressionF64>>>,
    running: Arc<RwLock<bool>>,
}

impl AutoQueuesServer {
    /// Create server from configuration file
    pub async fn from_file(config_path: &str) -> Result<Self, AutoQueuesServerError> {
        let config = QueueConfig::from_file(config_path)?;
        Ok(Self::new(config, 6969))
    }

    /// Create minimal server configuration
    pub fn minimal() -> AutoQueuesServerBuilder {
        AutoQueuesServerBuilder::new()
    }

    /// Create a new server instance
    fn new(config: QueueConfig, port: u16) -> Self {
        Self {
            config,
            port,
            broker: Arc::new(PubSubBroker::new(100)),
            queues: Arc::new(RwLock::new(HashMap::new())),
            expressions: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the server
    pub async fn run(&self) -> Result<(), AutoQueuesServerError> {
        // Check if already running
        if *self.running.read().await {
            return Err(AutoQueuesServerError::AlreadyRunning);
        }

        // Set running flag
        *self.running.write().await = true;

        // Initialize queues from config
        self.initialize_queues().await?;

        // Start processing loop
        self.start_processing_loop().await;

        println!("AutoQueues server started on port {}", self.port);
        Ok(())
    }

    /// Stop the server
    pub async fn stop(&self) -> Result<(), AutoQueuesServerError> {
        if !*self.running.read().await {
            return Err(AutoQueuesServerError::NotRunning);
        }

        *self.running.write().await = false;
        println!("AutoQueues server stopped");
        Ok(())
    }

    /// Initialize queues based on configuration
    async fn initialize_queues(&self) -> Result<(), AutoQueuesServerError> {
        let mut queues = self.queues.write().await;
        let mut expressions = self.expressions.write().await;

        // Initialize base metric queues
        for base_metric in self.config.get_base_metrics() {
            let queue_name = format!("local.{}", base_metric);
            let queue = Arc::new(RwLock::new(SimpleQueue::new()));
            queues.insert(queue_name, queue);
        }

        // Initialize derived queues with expressions
        for (derived_name, formula) in self.config.get_derived_formulas() {
            let queue_name = format!("derived.{}", derived_name);
            let queue = Arc::new(RwLock::new(SimpleQueue::new()));
            queues.insert(queue_name.clone(), queue);

            // Parse and store expression
            let expression = SimpleExpression::<f64>::new(formula)
                .map_err(|e| AutoQueuesServerError::ExpressionError(e.to_string()))?;
            expressions.insert(queue_name, expression);
        }

        Ok(())
    }

    /// Start the background processing loop
    async fn start_processing_loop(&self) {
        let running = self.running.clone();
        let _broker = self.broker.clone();
        let _queues = self.queues.clone();
        let _expressions = self.expressions.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(100));

            while *running.read().await {
                interval.tick().await;

                // Process incoming messages
                // For now, we'll just demonstrate the structure
                // In a full implementation, we'd:
                // 1. Receive messages from subscribed topics
                // 2. Evaluate expressions
                // 3. Publish results to appropriate topics
            }
        });
    }

    /// Push data to a topic
    pub async fn push(&self, topic: &str, data: f64) -> Result<(), AutoQueuesServerError> {
        self.broker.publish(topic.to_string(), &data).await?;
        Ok(())
    }

    /// Subscribe to a topic
    pub async fn subscribe(&self, topic: &str) -> Result<SubscriptionId, AutoQueuesServerError> {
        let subscription_id = self.broker.subscribe_exact(topic.to_string()).await?;
        Ok(subscription_id)
    }

    /// Get health score (example endpoint)
    pub async fn get_health_score(&self) -> Result<f64, AutoQueuesServerError> {
        // This would calculate and return a health score
        // For now, return a dummy value
        Ok(95.5)
    }

    /// Get queue statistics
    pub async fn get_queue_stats(&self) -> Result<HashMap<String, usize>, AutoQueuesServerError> {
        let queues = self.queues.read().await;
        let mut stats = HashMap::new();

        for (name, queue) in queues.iter() {
            let size = queue.read().await.get_size();
            stats.insert(name.clone(), size);
        }

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_creation() {
        let server = AutoQueuesServer::minimal()
            .port(6969)
            .push_topic("metrics.cpu")
            .subscribe_topic("alerts.*")
            .build()
            .expect("Server should build successfully");

        assert_eq!(server.port, 6969);
    }
}