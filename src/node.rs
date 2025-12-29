//! AutoQueues - Simplified distributed node for HPC
//!
//! HPC-optimized node with minimal overhead for ultra-fast operations.

use crate::config::QueueConfig;
use crate::constants;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;

pub struct AutoQueuesNode {
    metrics: Arc<RwLock<HashMap<String, f64>>>,
    running: Arc<RwLock<bool>>,
}

impl AutoQueuesNode {
    /// Create node from configuration file
    pub fn from_config(config_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let _config = QueueConfig::from_file(config_path)?;
        Ok(Self::new())
    }

    /// Create node with configuration
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Run the node
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Starting AutoQueues HPC node...");
        
        // Set running flag
        *self.running.write().await = true;

        // Start background tasks
        self.start_background_tasks().await?;

        // Wait for shutdown signal (simplified)
        tokio::signal::ctrl_c().await?;
        *self.running.write().await = false;
        
        println!("AutoQueues HPC node stopped");
        Ok(())
    }

    /// Start background tasks for HPC operations
    async fn start_background_tasks(&self) -> Result<(), Box<dyn std::error::Error>> {
        let running = self.running.clone();
        let metrics = self.metrics.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(constants::time::METRICS_INTERVAL_MS));
            
            while *running.read().await {
                interval.tick().await;
                
                // Update metrics (simplified)
                let mut metrics_guard = metrics.write().await;
                metrics_guard.insert("cpu_percent".to_string(), 42.0);
                metrics_guard.insert("memory_percent".to_string(), 65.0);
            }
        });

        Ok(())
    }

    /// Get latest metric value
    pub async fn get_metric(&self, name: &str) -> Option<f64> {
        self.metrics.read().await.get(name).copied()
    }
}

/// Convenience function to start a node
pub async fn start_node(config_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let node = AutoQueuesNode::from_config(config_path)?;
    node.run().await
}

impl Default for AutoQueuesNode {
    fn default() -> Self {
        Self::new()
    }
}