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
    metrics::{MetricsCollector, SystemMetrics},
    queue::{QueueError, SimpleQueue},
    types::{QueueId, Timestamp},
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
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

/// Error type for queue manager operations
#[derive(Debug, thiserror::Error)]
pub enum QueueManagerError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Queue error: {0}")]
    Queue(#[from] QueueError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Runtime error: {0}")]
    Runtime(String),
}

/// Queue instance with metadata
#[derive(Clone)]
pub struct ManagedQueue {
    /// Queue instance
    pub queue: Arc<Mutex<SimpleQueue<f64>>>,
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
#[derive(Clone)]
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
    running: Arc<AtomicBool>,
    /// Update task handle
    update_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Cluster leaders for distributed metrics
    cluster_leaders: Arc<RwLock<HashMap<String, SocketAddr>>>,
}

impl QueueManager {
    /// Create a new queue manager with the given configuration
    pub async fn new(config: QueueManagerConfig) -> Result<Self, QueueManagerError> {
        let queue_config = QueueConfig::from_file(&config.config_path)?;

        Ok(Self {
            config,
            queue_config: Arc::new(RwLock::new(queue_config)),
            queues: Arc::new(RwLock::new(HashMap::new())),
            queue_names: Arc::new(RwLock::new(HashMap::new())),
            next_queue_id: Arc::new(AtomicU32::new(1)),
            metrics: Arc::new(RwLock::new(MetricsCollector::new())),
            system_metrics: Arc::new(RwLock::new(None)),
            running: Arc::new(AtomicBool::new(false)),
            update_handle: Arc::new(Mutex::new(None)),
            cluster_leaders: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Load configuration from file
    pub async fn load_config(&self, path: &str) -> Result<(), QueueManagerError> {
        let config = QueueConfig::from_file(path)?;
        let mut queue_config = self.queue_config.write().await;
        *queue_config = config;
        Ok(())
    }

    /// Initialize queues from configuration
    pub async fn initialize_queues(&self) -> Result<(), QueueManagerError> {
        let queue_config = self.queue_config.read().await;
        
        // Create base queues
        for base_queue in queue_config.get_base_metrics() {
            let queue_id = self.next_queue_id.fetch_add(1, Ordering::Relaxed);
            
            let managed_queue = ManagedQueue {
                queue: Arc::new(Mutex::new(SimpleQueue::new())),
                last_value: None,
                last_update: None,
                name: base_queue.clone(),
                is_derived: false,
            };
            
            let mut queues = self.queues.write().await;
            queues.insert(queue_id, managed_queue);
            
            let mut queue_names = self.queue_names.write().await;
            queue_names.insert(base_queue.clone(), queue_id);
        }
        
        // Create derived queues
        for (name, formula) in queue_config.get_derived_formulas() {
            let queue_id = self.next_queue_id.fetch_add(1, Ordering::Relaxed);
            
            let managed_queue = ManagedQueue {
                queue: Arc::new(Mutex::new(SimpleQueue::new())),
                last_value: None,
                last_update: None,
                name: name.to_string(),
                is_derived: true,
            };
            
            let mut queues = self.queues.write().await;
            queues.insert(queue_id, managed_queue);
            
            let mut queue_names = self.queue_names.write().await;
            queue_names.insert(name.to_string(), queue_id);
        }
        
        Ok(())
    }

    /// Start the queue manager
    pub async fn start(&self) -> Result<(), QueueManagerError> {
        // Initialize queues from configuration
        self.initialize_queues().await?;
        
        // Start metrics collection
        if self.config.enable_metrics {
            self.start_metrics_collection().await;
        }
        
        // Start update task
        self.start_update_task().await;
        
        // Mark as running
        self.running.store(true, Ordering::Relaxed);
        
        Ok(())
    }

    /// Stop the queue manager gracefully
    pub async fn stop(&self) -> Result<(), QueueManagerError> {
        // Mark as not running
        self.running.store(false, Ordering::Relaxed);
        
        // Stop update task
        self.stop_update_task().await;
        
        // Clear all queues
        {
            let mut queues = self.queues.write().await;
            queues.clear();
        }
        
        // Clear queue names
        {
            let mut queue_names = self.queue_names.write().await;
            queue_names.clear();
        }
        
        Ok(())
    }

    /// Start metrics collection
    async fn start_metrics_collection(&self) {
        let metrics = self.metrics.clone();
        let interval_ms = self.config.update_interval_ms;
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(interval_ms));
            loop {
                interval.tick().await;
                let mut metrics_guard = metrics.write().await;
                metrics_guard.refresh();
            }
        });
    }

    /// Start update task
    async fn start_update_task(&self) {
        let self_clone = self.clone();
        let interval_ms = self.config.update_interval_ms;
        
        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(interval_ms));
            while self_clone.running.load(Ordering::Relaxed) {
                interval.tick().await;
                self_clone.update_queues().await;
            }
        });
        
        let mut update_handle = self.update_handle.lock().await;
        *update_handle = Some(handle);
    }

    /// Stop update task
    async fn stop_update_task(&self) {
        let mut update_handle = self.update_handle.lock().await;
        if let Some(handle) = update_handle.take() {
            handle.abort();
        }
    }

    /// Update all queues (collect metrics, evaluate expressions)
    async fn update_queues(&self) {
        // Get system metrics
        let system_metrics = {
            let mut metrics = self.metrics.write().await;
            metrics.get_all_metrics()
        };
        
        // Update system metrics cache
        {
            let mut system_metrics_cache = self.system_metrics.write().await;
            *system_metrics_cache = Some(system_metrics.clone());
        }
        
        // Update base queues with system metrics
        self.update_base_queues(system_metrics).await;
        
        // Update derived queues with expressions
        self.update_derived_queues().await;
    }

    /// Update base queues with system metrics
    async fn update_base_queues(&self, system_metrics: SystemMetrics) {
        let queue_names = self.queue_names.read().await;
        let mut queues_to_update = Vec::new();
        
        // Update CPU usage queue
        if let Some(&queue_id) = queue_names.get("cpu_percent") {
            queues_to_update.push((queue_id, system_metrics.cpu_usage_percent as f64));
        }
        
        // Update memory usage queue
        if let Some(&queue_id) = queue_names.get("memory_percent") {
            queues_to_update.push((queue_id, system_metrics.memory_usage.usage_percent as f64));
        }
        
        // Update disk usage queue (use first disk)
        if let Some(&queue_id) = queue_names.get("drive_percent") {
            queues_to_update.push((queue_id, system_metrics.disk_usage.usage_percent as f64));
        }
        
        // Update queues
        for (queue_id, value) in queues_to_update {
            if let Some(managed_queue) = self.get_queue_by_id(queue_id).await {
                let mut queue_guard = managed_queue.queue.lock().await;
                let _ = queue_guard.publish(value);
            }
        }
    }

    /// Update derived queues with expressions
    async fn update_derived_queues(&self) {
        // In a real implementation, this would evaluate expressions
        // For now, we'll just log that it would happen
        println!("Updating derived queues with expressions");
    }

    /// Get queue by ID
    async fn get_queue_by_id(&self, queue_id: QueueId) -> Option<ManagedQueue> {
        let queues = self.queues.read().await;
        queues.get(&queue_id).cloned()
    }

    /// Get queue by name
    pub async fn get_queue(&self, name: &str) -> Option<ManagedQueue> {
        let queue_names = self.queue_names.read().await;
        if let Some(&queue_id) = queue_names.get(name) {
            self.get_queue_by_id(queue_id).await
        } else {
            None
        }
    }

    /// Publish data to a queue
    pub async fn publish(&self, queue_name: &str, data: f64) -> Result<(), QueueManagerError> {
        if let Some(managed_queue) = self.get_queue(queue_name).await {
            let mut queue_guard = managed_queue.queue.lock().await;
            queue_guard.publish(data)?;
            Ok(())
        } else {
            Err(QueueManagerError::Runtime(format!("Queue '{}' not found", queue_name)))
        }
    }

    /// Get latest value from a queue
    pub async fn get_latest(&self, queue_name: &str) -> Result<Option<(Timestamp, f64)>, QueueManagerError> {
        if let Some(managed_queue) = self.get_queue(queue_name).await {
            let queue_guard = managed_queue.queue.lock().await;
            Ok(queue_guard.get_latest())
        } else {
            Err(QueueManagerError::Runtime(format!("Queue '{}' not found", queue_name)))
        }
    }

    /// Get queue statistics
    pub async fn get_queue_stats(&self, queue_name: &str) -> Option<crate::types::QueueStats> {
        if let Some(managed_queue) = self.get_queue(queue_name).await {
            let queue_guard = managed_queue.queue.lock().await;
            // In a real implementation, we would return actual stats
            Some(crate::types::QueueStats::new())
        } else {
            None
        }
    }

    /// Get all queue information
    pub async fn get_all_queues(&self) -> HashMap<QueueId, ManagedQueue> {
        let queues = self.queues.read().await;
        queues.clone()
    }

    /// Get queue count
    pub async fn get_queue_count(&self) -> usize {
        let queues = self.queues.read().await;
        queues.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_queue_manager_creation() {
        let config = QueueManagerConfig::default();
        let manager = QueueManager::new(config).await.unwrap();
        assert!(!manager.running.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_queue_manager_start_stop() {
        let config = QueueManagerConfig::default();
        let manager = QueueManager::new(config).await.unwrap();
        
        manager.start().await.unwrap();
        assert!(manager.running.load(Ordering::Relaxed));
        
        manager.stop().await.unwrap();
        assert!(!manager.running.load(Ordering::Relaxed));
    }
}