//! Queue Manager - Central Orchestration System
//!
//! The QueueManager is the main orchestrator that:
//! - Loads configuration from TOML files
//! - Creates and manages all queue instances
//! - Parses and validates derived expressions
//! - Coordinates graceful shutdown
//! - Provides unified management interface

use crate::traits::queue::QueueTrait;
use crate::{
    config::{ConfigError, QueueConfig},
    expression::{Expression, ExpressionError, ExpressionF64},
    metrics::{MetricsCollector, SystemMetrics},
    queue::{QueueError, SimpleQueue},
    types::Timestamp,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::time::interval;

/// Queue manager configuration
#[derive(Debug, Clone)]
pub struct QueueManagerConfig {
    /// Path to TOML configuration file
    pub config_path: String,
    /// Update interval for metrics and expressions
    pub update_interval_ms: u64,
    /// Maximum number of queues to manage
    pub max_queues: usize,
    /// Enable real-time metrics collection
    pub enable_metrics: bool,
    /// Graceful shutdown timeout
    pub shutdown_timeout_ms: u64,
}

impl Default for QueueManagerConfig {
    fn default() -> Self {
        Self {
            config_path: "config.toml".to_string(),
            update_interval_ms: 1000,
            max_queues: 100,
            enable_metrics: true,
            shutdown_timeout_ms: 5000,
        }
    }
}

/// Queue instance with metadata
#[derive(Clone)]
pub struct ManagedQueue {
    /// Queue instance
    pub queue: Arc<Mutex<SimpleQueue<f64>>>,
    /// Expression for derived calculations
    pub expression: Option<ExpressionF64>,
    /// Last calculated value
    pub last_value: Option<f64>,
    /// Last update timestamp
    pub last_update: Option<Timestamp>,
    /// Queue name/identifier
    pub name: String,
    /// Whether this is a derived queue
    pub is_derived: bool,
}

/// Queue manager state
pub struct QueueManager {
    /// Configuration
    config: QueueManagerConfig,
    /// Loaded configuration
    queue_config: Arc<RwLock<QueueConfig>>,
    /// Managed queues by name
    queues: Arc<RwLock<HashMap<String, ManagedQueue>>>,
    /// Metrics collector
    metrics: Arc<Mutex<MetricsCollector>>,
    /// System metrics cache
    system_metrics: Arc<RwLock<Option<SystemMetrics>>>,
    /// Running flag for graceful shutdown
    running: Arc<RwLock<bool>>,
    /// Update task handle
    update_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

/// Queue manager errors
#[derive(Debug)]
pub enum QueueManagerError {
    ConfigError(ConfigError),
    ExpressionError(ExpressionError),
    QueueError(QueueError),
    IoError(std::io::Error),
    ValidationError(String),
    ShutdownError(String),
}

impl std::fmt::Display for QueueManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueueManagerError::ConfigError(e) => write!(f, "Configuration error: {}", e),
            QueueManagerError::ExpressionError(e) => write!(f, "Expression error: {}", e),
            QueueManagerError::QueueError(e) => write!(f, "Queue error: {}", e),
            QueueManagerError::IoError(e) => write!(f, "IO error: {}", e),
            QueueManagerError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            QueueManagerError::ShutdownError(msg) => write!(f, "Shutdown error: {}", msg),
        }
    }
}

impl std::error::Error for QueueManagerError {}

impl From<ConfigError> for QueueManagerError {
    fn from(error: ConfigError) -> Self {
        QueueManagerError::ConfigError(error)
    }
}

impl From<ExpressionError> for QueueManagerError {
    fn from(error: ExpressionError) -> Self {
        QueueManagerError::ExpressionError(error)
    }
}

impl From<QueueError> for QueueManagerError {
    fn from(error: QueueError) -> Self {
        QueueManagerError::QueueError(error)
    }
}

impl From<std::io::Error> for QueueManagerError {
    fn from(error: std::io::Error) -> Self {
        QueueManagerError::IoError(error)
    }
}

impl QueueManager {
    /// Create a new queue manager with default configuration
    pub fn new() -> Result<Self, QueueManagerError> {
        Self::with_config(QueueManagerConfig::default())
    }

    /// Create a new queue manager with custom configuration
    pub fn with_config(config: QueueManagerConfig) -> Result<Self, QueueManagerError> {
        // Try to load config file, but fall back to default if it doesn't exist
        let queue_config = match QueueConfig::from_file(&config.config_path) {
            Ok(config) => config,
            Err(ConfigError::FileNotFound(_)) => {
                println!(
                    "Config file '{}' not found, using default configuration",
                    config.config_path
                );
                QueueConfig::create_standard()
            }
            Err(e) => return Err(e.into()),
        };

        Ok(Self {
            config,
            queue_config: Arc::new(RwLock::new(queue_config)),
            queues: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(Mutex::new(MetricsCollector::new())),
            system_metrics: Arc::new(RwLock::new(None)),
            running: Arc::new(RwLock::new(false)),
            update_handle: Arc::new(Mutex::new(None)),
        })
    }

    /// Load configuration from string (useful for testing)
    pub async fn load_config_from_string(
        &self,
        toml_content: &str,
    ) -> Result<(), QueueManagerError> {
        let new_config = QueueConfig::from_str(toml_content)?;

        // Update configuration
        *self.queue_config.write().await = new_config;

        // Rebuild queues with new configuration
        self.rebuild_queues().await?;

        Ok(())
    }

    /// Start the queue manager
    pub async fn start(&self) -> Result<(), QueueManagerError> {
        println!("Starting Queue Manager...");

        // Set running flag
        *self.running.write().await = true;

        // Build initial queues from configuration
        self.build_queues().await?;

        // Start background update task
        self.start_update_task().await?;

        println!("Queue Manager started successfully!");
        println!("   Managing {} queues", self.get_queue_count().await);

        Ok(())
    }

    /// Stop the queue manager gracefully
    pub async fn stop(&self) -> Result<(), QueueManagerError> {
        println!("Stopping Queue Manager...");

        // Set running flag to false
        *self.running.write().await = false;

        // Stop update task
        self.stop_update_task().await?;

        // Clear all queues
        self.clear_all_queues().await?;

        println!("Queue Manager stopped successfully!");
        Ok(())
    }

    /// Build queues from current configuration
    async fn build_queues(&self) -> Result<(), QueueManagerError> {
        let config = self.queue_config.read().await;
        let mut queues = self.queues.write().await;

        // Build base metric queues
        for base_metric in config.get_base_metrics() {
            let queue_name = format!("base.{}", base_metric);
            let queue = SimpleQueue::new();

            let managed_queue = ManagedQueue {
                queue: Arc::new(Mutex::new(queue)),
                expression: None,
                last_value: None,
                last_update: None,
                name: queue_name.clone(),
                is_derived: false,
            };

            queues.insert(queue_name, managed_queue);
        }

        // Build derived queues with expressions
        for (derived_name, formula) in config.get_derived_formulas() {
            let queue_name = format!("derived.{}", derived_name);

            // Create and validate expression
            let expression = ExpressionF64::new(formula)?;

            let queue = SimpleQueue::new();

            let managed_queue = ManagedQueue {
                queue: Arc::new(Mutex::new(queue)),
                expression: Some(expression),
                last_value: None,
                last_update: None,
                name: queue_name.clone(),
                is_derived: true,
            };

            queues.insert(queue_name, managed_queue);
        }

        Ok(())
    }

    /// Rebuild queues when configuration changes
    async fn rebuild_queues(&self) -> Result<(), QueueManagerError> {
        // Clear existing queues
        self.clear_all_queues().await?;

        // Build new queues
        self.build_queues().await?;

        Ok(())
    }

    /// Clear all managed queues
    async fn clear_all_queues(&self) -> Result<(), QueueManagerError> {
        let mut queues = self.queues.write().await;
        queues.clear();
        Ok(())
    }

    /// Start background update task
    async fn start_update_task(&self) -> Result<(), QueueManagerError> {
        let running = self.running.clone();
        let queues = self.queues.clone();
        let metrics = self.metrics.clone();
        let system_metrics = self.system_metrics.clone();
        let update_interval = self.config.update_interval_ms;

        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(update_interval));

            while *running.read().await {
                tokio::select! {
                    _ = interval.tick() => {
                        // Update system metrics
                        if let Ok(mut metrics_guard) = metrics.try_lock() {
                            let new_metrics = metrics_guard.get_all_metrics();
                            *system_metrics.write().await = Some(new_metrics);
                        }

                        // Update derived queues
                        if let Ok(queues_guard) = queues.try_read() {
                            let system_data = system_metrics.read().await;
                            if let Some(metrics) = system_data.as_ref() {
                                Self::update_derived_queues(&queues_guard, metrics).await;
                            }
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {
                        // Allow graceful shutdown checking
                        continue;
                    }
                }
            }
        });

        *self.update_handle.lock().await = Some(handle);
        Ok(())
    }

    /// Stop background update task
    async fn stop_update_task(&self) -> Result<(), QueueManagerError> {
        if let Some(handle) = self.update_handle.lock().await.take() {
            handle.abort();
        }
        Ok(())
    }

    /// Update derived queues with current system metrics
    async fn update_derived_queues(
        queues: &HashMap<String, ManagedQueue>,
        metrics: &SystemMetrics,
    ) {
        let mut atomic_vars = HashMap::new();
        let cluster_vars = HashMap::new();

        // Populate variables from system metrics
        atomic_vars.insert("cpu_usage".to_string(), metrics.cpu_usage_percent as f64);
        atomic_vars.insert(
            "memory_usage".to_string(),
            metrics.memory_usage.usage_percent as f64,
        );
        atomic_vars.insert(
            "disk_usage".to_string(),
            metrics.disk_usage.usage_percent as f64,
        );

        // Update each derived queue
        for (_, managed_queue) in queues.iter() {
            if managed_queue.is_derived {
                if let Some(expression) = &managed_queue.expression {
                    match expression.evaluate(&atomic_vars, &cluster_vars) {
                        Ok(value) => {
                            if let Ok(mut queue_guard) = managed_queue.queue.try_lock() {
                                // Use the Queue trait method
                                drop(queue_guard.publish(value));
                            }
                        }
                        Err(e) => {
                            eprintln!(
                                "Error evaluating expression for {}: {}",
                                managed_queue.name, e
                            );
                        }
                    }
                }
            }
        }
    }

    /// Get queue count
    pub async fn get_queue_count(&self) -> usize {
        self.queues.read().await.len()
    }

    /// Get queue by name
    pub async fn get_queue(&self, name: &str) -> Option<ManagedQueue> {
        self.queues.read().await.get(name).cloned()
    }

    /// Get all queue names
    pub async fn get_queue_names(&self) -> Vec<String> {
        self.queues.read().await.keys().cloned().collect()
    }

    /// Get current system metrics
    pub async fn get_system_metrics(&self) -> Option<SystemMetrics> {
        self.system_metrics.read().await.clone()
    }

    /// Publish data to a specific queue
    pub async fn publish(&self, queue_name: &str, data: f64) -> Result<(), QueueManagerError> {
        let queues = self.queues.read().await;

        if let Some(managed_queue) = queues.get(queue_name) {
            if let Ok(mut queue_guard) = managed_queue.queue.try_lock() {
                // Use the Queue trait method
                queue_guard.publish(data)?;
                Ok(())
            } else {
                Err(QueueManagerError::QueueError(QueueError::Other(
                    "Failed to acquire queue lock".to_string(),
                )))
            }
        } else {
            Err(QueueManagerError::ValidationError(format!(
                "Queue '{}' not found",
                queue_name
            )))
        }
    }

    /// Get latest data from a specific queue
    pub async fn get_latest(
        &self,
        queue_name: &str,
    ) -> Result<Option<(Timestamp, f64)>, QueueManagerError> {
        let queues = self.queues.read().await;

        if let Some(managed_queue) = queues.get(queue_name) {
            if let Ok(queue_guard) = managed_queue.queue.try_lock() {
                // Use the Queue trait method
                Ok(queue_guard.get_latest())
            } else {
                Err(QueueManagerError::QueueError(QueueError::Other(
                    "Failed to acquire queue lock".to_string(),
                )))
            }
        } else {
            Err(QueueManagerError::ValidationError(format!(
                "Queue '{}' not found",
                queue_name
            )))
        }
    }

    /// Get queue statistics
    pub async fn get_queue_stats(&self) -> Result<HashMap<String, QueueStats>, QueueManagerError> {
        let queues = self.queues.read().await;
        let mut stats = HashMap::new();

        for (name, managed_queue) in queues.iter() {
            if let Ok(queue_guard) = managed_queue.queue.try_lock() {
                let queue_stats = QueueStats {
                    name: name.clone(),
                    size: queue_guard.get_size(),
                    is_derived: managed_queue.is_derived,
                    has_expression: managed_queue.expression.is_some(),
                    last_value: managed_queue.last_value,
                    last_update: managed_queue.last_update,
                };
                stats.insert(name.clone(), queue_stats);
            }
        }

        Ok(stats)
    }

    /// Check if manager is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

/// Queue statistics for monitoring
#[derive(Debug, Clone)]
pub struct QueueStats {
    pub name: String,
    pub size: usize,
    pub is_derived: bool,
    pub has_expression: bool,
    pub last_value: Option<f64>,
    pub last_update: Option<Timestamp>,
}

impl Default for QueueManager {
    fn default() -> Self {
        Self::new().expect("Default queue manager creation should succeed")
    }
}
