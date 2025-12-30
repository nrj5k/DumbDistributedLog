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
    constants,
    expression::{Expression, ExpressionError, ExpressionF64},
    metrics::{MetricsCollector, SystemMetrics},
    queue::{QueueError, SimpleQueue},
    types::{Timestamp, QueueId},
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
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
            update_interval_ms: constants::time::METRICS_INTERVAL_MS,
            max_queues: constants::system::MAX_QUEUES,
            enable_metrics: true,
            shutdown_timeout_ms: constants::system::SHUTDOWN_TIMEOUT_MS,
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
    /// Managed queues by ID
    queues: Arc<RwLock<HashMap<QueueId, ManagedQueue>>>,
    /// Queue name to ID mapping
    queue_names: Arc<RwLock<HashMap<String, QueueId>>>,
    /// Next queue ID to assign
    next_queue_id: Arc<AtomicU32>,
    /// Metrics collector (RwLock for concurrent read-heavy access)
    metrics: Arc<RwLock<MetricsCollector>>,
    /// System metrics cache
    system_metrics: Arc<RwLock<Option<SystemMetrics>>>,
    /// Running flag for graceful shutdown
    running: Arc<RwLock<bool>>,
    /// Update task handle
    update_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Cluster leaders for distributed metrics
    cluster_leaders: Arc<RwLock<HashMap<String, SocketAddr>>>,
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
            queue_names: Arc::new(RwLock::new(HashMap::new())),
            next_queue_id: Arc::new(AtomicU32::new(1)),
            metrics: Arc::new(RwLock::new(MetricsCollector::new())),
            system_metrics: Arc::new(RwLock::new(None)),
            running: Arc::new(RwLock::new(false)),
            update_handle: Arc::new(Mutex::new(None)),
            cluster_leaders: Arc::new(RwLock::new(HashMap::new())),
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
        let mut queue_names = self.queue_names.write().await;

        // Build base metric queues
        for base_metric in config.get_base_metrics() {
            let queue_name = format!("base.{}", base_metric);
            let queue_id = self.next_queue_id.fetch_add(1, Ordering::AcqRel);
            
            let queue = SimpleQueue::new();

            let managed_queue = ManagedQueue {
                queue: Arc::new(Mutex::new(queue)),
                expression: None,
                last_value: None,
                last_update: None,
                name: queue_name.clone(),
                is_derived: false,
            };

            queues.insert(queue_id, managed_queue);
            queue_names.insert(queue_name, queue_id);
        }

        // Build derived queues with expressions
        for (derived_name, formula) in config.get_derived_formulas() {
            let queue_name = format!("derived.{}", derived_name);
            let queue_id = self.next_queue_id.fetch_add(1, Ordering::AcqRel);

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

            queues.insert(queue_id, managed_queue);
            queue_names.insert(queue_name, queue_id);
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
        let cluster_leaders = self.cluster_leaders.clone();
        let update_interval = self.config.update_interval_ms;

        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(update_interval));

            while *running.read().await {
                tokio::select! {
                    _ = interval.tick() => {
                        // Update system metrics
                        if let Ok(mut metrics_guard) = metrics.try_write() {
                            let new_metrics = metrics_guard.get_all_metrics();
                            *system_metrics.write().await = Some(new_metrics);
                        }

                        // Update derived queues
                        if let Ok(queues_guard) = queues.try_read() {
                            let system_data = system_metrics.read().await;
                            if let Some(metrics) = system_data.as_ref() {
                                // Simplified cluster variables handling for HPC core
                                let cluster_leaders_data = cluster_leaders.read().await.clone();
                                let cluster_vars = Self::query_cluster_variables(&cluster_leaders_data)
                                    .unwrap_or_else(|_| HashMap::new());

                                Self::update_derived_queues(&queues_guard, metrics, &cluster_vars).await;
                            }
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(constants::time::SHORT_SLEEP_INTERVAL_MS)) => {
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
        queues: &HashMap<QueueId, ManagedQueue>,
        metrics: &SystemMetrics,
        cluster_vars: &HashMap<String, f64>,
    ) {
        let mut atomic_vars = HashMap::new();

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
                    // Merge atomic and cluster variables for simplified expression engine
                    let mut all_vars = atomic_vars.clone();
                    for (key, value) in cluster_vars {
                        all_vars.insert(key.clone(), *value);
                    }
                    
                    match expression.evaluate(&all_vars) {
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
        let queue_names = self.queue_names.read().await;
        let queue_id = queue_names.get(name)?;
        let queues = self.queues.read().await;
        queues.get(queue_id).cloned()
    }

    /// Get all queue names
    pub async fn get_queue_names(&self) -> Vec<String> {
        let queue_names = self.queue_names.read().await;
        queue_names.keys().cloned().collect()
    }

    /// Get current system metrics
    pub async fn get_system_metrics(&self) -> Option<SystemMetrics> {
        self.system_metrics.read().await.clone()
    }

    /// Publish data to a specific queue
    pub async fn publish(&self, queue_name: &str, data: f64) -> Result<(), QueueManagerError> {
        // Get the queue ID from the name mapping
        let queue_names = self.queue_names.read().await;
        let queue_id = match queue_names.get(queue_name) {
            Some(id) => *id,
            None => return Err(QueueManagerError::ValidationError(format!(
                "Queue '{}' not found",
                queue_name
            ))),
        };
        
        // Get the queue using the ID
        let queues = self.queues.read().await;
        if let Some(managed_queue) = queues.get(&queue_id) {
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
        // Get the queue ID from the name mapping
        let queue_names = self.queue_names.read().await;
        let queue_id = match queue_names.get(queue_name) {
            Some(id) => *id,
            None => return Err(QueueManagerError::ValidationError(format!(
                "Queue '{}' not found",
                queue_name
            ))),
        };
        
        // Get the queue using the ID
        let queues = self.queues.read().await;
        if let Some(managed_queue) = queues.get(&queue_id) {
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
        let queue_names = self.queue_names.read().await;
        let mut stats = HashMap::new();

        // Create a reverse mapping from ID to name for lookup
        let id_to_name: HashMap<QueueId, String> = queue_names
            .iter()
            .map(|(name, id)| (*id, name.clone()))
            .collect();

        for (queue_id, managed_queue) in queues.iter() {
            if let Ok(queue_guard) = managed_queue.queue.try_lock() {
                let queue_stats = QueueStats {
                    name: managed_queue.name.clone(),
                    size: queue_guard.get_size(),
                    is_derived: managed_queue.is_derived,
                    has_expression: managed_queue.expression.is_some(),
                    last_value: managed_queue.last_value,
                    last_update: managed_queue.last_update,
                };
                // Get the queue name from the ID
                if let Some(queue_name) = id_to_name.get(queue_id) {
                    stats.insert(queue_name.clone(), queue_stats);
                }
            }
        }

        Ok(stats)
    }

    /// Check if manager is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Query cluster variables from leaders (simplified for HPC core)
    fn query_cluster_variables(
        _cluster_leaders: &HashMap<String, SocketAddr>,
    ) -> Result<HashMap<String, f64>, String> {
        // Simplified implementation for HPC core - return empty map
        // Full implementation moved to out-of-scope components
        Ok(HashMap::new())
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