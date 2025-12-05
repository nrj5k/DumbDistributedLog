//! Queue Manager - Distributed Coordination Engine
//!
//! Implements Engine trait for cross-host variable resolution, health monitoring,
//! and distributed queue coordination. Queue instances call QueueManager as
//! just another Engine in their processing chain.

use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

use crate::engine::Engine;

/// Input types for QueueManager Engine
#[derive(Debug, Clone)]
pub enum ManagerInput {
    /// Request resolution of global variable (e.g., "global.cluster_cpu")
    VariableRequest(String),

    /// Health check request for specific queue
    HealthCheck(String),

    /// Load balancing request across hosts
    LoadBalancing(LoadBalancingRequest),
}

/// Output types from QueueManager Engine
#[derive(Debug, Clone)]
pub enum ManagerOutput {
    /// Resolved global variable value
    VariableValue(f64),

    /// Health status response
    HealthStatus(HealthStatus),

    /// Load balancing recommendation
    LoadRecommendation(LoadRecommendation),

    /// Error response with context
    Error(String),
}

/// Load balancing request data
#[derive(Debug, Clone)]
pub struct LoadBalancingRequest {
    pub queue_name: String,
    pub current_load: f64,
    pub target_hosts: Vec<String>,
}

/// Health status information
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub queue_name: String,
    pub is_healthy: bool,
    pub last_heartbeat: Instant,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub queue_depth: usize,
}

/// Load balancing recommendation
#[derive(Debug, Clone)]
pub struct LoadRecommendation {
    pub recommended_host: String,
    pub estimated_load: f64,
    pub confidence: f64,
}

/// Queue Manager implementing Engine trait for distributed coordination
pub struct QueueManager {
    name: String,
    global_variables: Arc<RwLock<HashMap<String, f64>>>,
    health_registry: Arc<RwLock<HashMap<String, HealthStatus>>>,
    host_registry: Arc<RwLock<Vec<String>>>,
}

impl QueueManager {
    /// Create new QueueManager with default configuration
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            global_variables: Arc::new(RwLock::new(HashMap::new())),
            health_registry: Arc::new(RwLock::new(HashMap::new())),
            host_registry: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Update global variable value
    pub async fn update_global_variable(&self, name: String, value: f64) {
        let mut globals = self.global_variables.write().await;
        globals.insert(name, value);
    }

    /// Update queue health status
    pub async fn update_health_status(&self, queue_name: String, status: HealthStatus) {
        let mut registry = self.health_registry.write().await;
        registry.insert(queue_name, status);
    }

    /// Register host for load balancing
    pub async fn register_host(&self, host: String) {
        let mut hosts = self.host_registry.write().await;
        if !hosts.contains(&host) {
            hosts.push(host);
        }
    }

    /// Resolve global variable with fallback to local approximation
    async fn resolve_global_variable(&self, var_name: &str) -> Result<f64, String> {
        // First try exact match in global registry
        let globals = self.global_variables.read().await;
        if let Some(&value) = globals.get(var_name) {
            return Ok(value);
        }
        drop(globals);

        // Fallback: compute from health registry if it's an aggregate
        if var_name.starts_with("global.cluster_") {
            return self.compute_cluster_aggregate(var_name).await;
        }

        // Final fallback: return safe default
        Ok(0.0)
    }

    /// Compute cluster-wide aggregates from health data
    async fn compute_cluster_aggregate(&self, var_name: &str) -> Result<f64, String> {
        let registry = self.health_registry.read().await;
        if registry.is_empty() {
            return Ok(0.0);
        }

        let mut total_cpu = 0.0;
        let mut total_memory = 0.0;
        let count = registry.len() as f64;

        for status in registry.values() {
            total_cpu += status.cpu_usage;
            total_memory += status.memory_usage;
        }

        match var_name {
            "global.cluster_cpu" => Ok(total_cpu / count),
            "global.cluster_memory" => Ok(total_memory / count),
            _ => Ok(0.0),
        }
    }

    /// Get health status for queue
    async fn get_health_status(&self, queue_name: &str) -> HealthStatus {
        let registry = self.health_registry.read().await;

        if let Some(status) = registry.get(queue_name) {
            return status.clone();
        }

        // Default healthy status for unknown queues
        HealthStatus {
            queue_name: queue_name.to_string(),
            is_healthy: true,
            last_heartbeat: Instant::now(),
            cpu_usage: 0.0,
            memory_usage: 0.0,
            queue_depth: 0,
        }
    }

    /// Generate load balancing recommendation
    async fn generate_load_recommendation(
        &self,
        request: LoadBalancingRequest,
    ) -> LoadRecommendation {
        let hosts = self.host_registry.read().await;
        let health = self.health_registry.read().await;

        if hosts.is_empty() {
            return LoadRecommendation {
                recommended_host: "localhost".to_string(),
                estimated_load: request.current_load,
                confidence: 0.0,
            };
        }

        // Simple round-robin with health consideration
        let mut best_host = "localhost".to_string();
        let mut best_score = f64::MIN;

        for host in hosts.iter() {
            let score = if let Some(status) = health.get(host) {
                // Prefer healthy hosts with lower load
                if status.is_healthy {
                    100.0 - status.cpu_usage - status.memory_usage
                } else {
                    -100.0
                }
            } else {
                // Unknown hosts get neutral score
                0.0
            };

            if score > best_score {
                best_score = score;
                best_host = host.clone();
            }
        }

        LoadRecommendation {
            recommended_host: best_host,
            estimated_load: request.current_load * 0.8,
            confidence: (best_score + 100.0) / 200.0,
        }
    }
}

impl Engine<ManagerInput, ManagerOutput> for QueueManager {
    fn execute(&self, input: ManagerInput) -> Result<ManagerOutput, Box<dyn Error + Send + Sync>> {
        // Use tokio::task::block_in_place for async operations in sync context
        match input {
            ManagerInput::VariableRequest(var_name) => {
                let result = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current()
                        .block_on(self.resolve_global_variable(&var_name))
                });
                match result {
                    Ok(value) => Ok(ManagerOutput::VariableValue(value)),
                    Err(error) => Ok(ManagerOutput::Error(error)),
                }
            }

            ManagerInput::HealthCheck(queue_name) => {
                let status = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(self.get_health_status(&queue_name))
                });
                Ok(ManagerOutput::HealthStatus(status))
            }

            ManagerInput::LoadBalancing(request) => {
                let recommendation = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current()
                        .block_on(self.generate_load_recommendation(request))
                });
                Ok(ManagerOutput::LoadRecommendation(recommendation))
            }
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variable_resolution() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let manager = QueueManager::new("test_manager");

            // Set a global variable
            manager
                .update_global_variable("global.test_var".to_string(), 42.0)
                .await;

            // Test resolution
            let input = ManagerInput::VariableRequest("global.test_var".to_string());
            let result = manager.execute(input).unwrap();

            match result {
                ManagerOutput::VariableValue(value) => assert_eq!(value, 42.0),
                _ => panic!("Expected VariableValue"),
            }
        });
    }

    #[test]
    fn test_health_check() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let manager = QueueManager::new("test_manager");

            // Update health status
            let status = HealthStatus {
                queue_name: "test_queue".to_string(),
                is_healthy: true,
                last_heartbeat: Instant::now(),
                cpu_usage: 25.0,
                memory_usage: 40.0,
                queue_depth: 5,
            };
            manager
                .update_health_status("test_queue".to_string(), status)
                .await;

            // Test health check
            let input = ManagerInput::HealthCheck("test_queue".to_string());
            let result = manager.execute(input).unwrap();

            match result {
                ManagerOutput::HealthStatus(status) => {
                    assert_eq!(status.queue_name, "test_queue");
                    assert!(status.is_healthy);
                    assert_eq!(status.cpu_usage, 25.0);
                }
                _ => panic!("Expected HealthStatus"),
            }
        });
    }
}
