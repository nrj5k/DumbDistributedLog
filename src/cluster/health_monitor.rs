//! Health monitoring for cluster nodes
//!
//! Detects node failures and triggers rebalancing of metric leadership.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::interval;
use log::{info, warn, error};

use crate::cluster::metric_leader_manager::MetricLeaderManager;

/// Health status of a node
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeHealth {
    Healthy,
    Suspicious,  // Missed a few heartbeats
    Unhealthy,   // Considered failed
}

/// Health information for a node
#[derive(Debug, Clone)]
pub struct NodeHealthInfo {
    pub node_id: u64,
    pub last_heartbeat: Instant,
    pub health_status: NodeHealth,
    pub consecutive_misses: u32,
}

/// Monitors health of all nodes in cluster
pub struct HealthMonitor {
    node_id: u64,
    /// Health info for all nodes
    node_health: Arc<RwLock<HashMap<u64, NodeHealthInfo>>>,
    /// Leader manager for rebalancing
    leader_manager: Arc<MetricLeaderManager>,
    /// Heartbeat timeout (default 5 seconds)
    heartbeat_timeout: Duration,
    /// Suspicious threshold (number of missed heartbeats)
    suspicious_threshold: u32,
    /// Failure threshold (number of missed heartbeats)
    failure_threshold: u32,
}

impl HealthMonitor {
    /// Create new HealthMonitor
    pub fn new(
        node_id: u64,
        leader_manager: Arc<MetricLeaderManager>,
    ) -> Self {
        Self {
            node_id,
            node_health: Arc::new(RwLock::new(HashMap::new())),
            leader_manager,
            heartbeat_timeout: Duration::from_secs(5),
            suspicious_threshold: 2,
            failure_threshold: 5,
        }
    }
    
    /// Register a node for monitoring
    pub async fn register_node(&self, node_id: u64) {
        let mut health_map = self.node_health.write().await;
        health_map.insert(node_id, NodeHealthInfo {
            node_id,
            last_heartbeat: Instant::now(),
            health_status: NodeHealth::Healthy,
            consecutive_misses: 0,
        });
        info!("HealthMonitor: Registered node {} for health monitoring", node_id);
    }
    
    /// Record heartbeat from a node
    pub async fn record_heartbeat(&self, node_id: u64) {
        let mut health_map = self.node_health.write().await;
        if let Some(info) = health_map.get_mut(&node_id) {
            info.last_heartbeat = Instant::now();
            info.consecutive_misses = 0;
            
            if info.health_status != NodeHealth::Healthy {
                info.health_status = NodeHealth::Healthy;
                info!("HealthMonitor: Node {} is healthy again", node_id);
            }
        }
    }
    
    /// Check health of all nodes
    pub async fn check_all_nodes(&self) {
        // Collect nodes that need to be checked for failure
        let mut failed_nodes = Vec::new();
        let mut suspicious_nodes = Vec::new();
        let mut recovered_nodes = Vec::new();
        
        {
            let mut health_map = self.node_health.write().await;
            let now = Instant::now();
            
            for (node_id, info) in health_map.iter_mut() {
                if *node_id == self.node_id {
                    continue; // Don't check self
                }
                
                let elapsed = now.duration_since(info.last_heartbeat);
                
                if elapsed > self.heartbeat_timeout {
                    info.consecutive_misses += 1;
                    
                    if info.consecutive_misses >= self.failure_threshold {
                        if info.health_status != NodeHealth::Unhealthy {
                            info.health_status = NodeHealth::Unhealthy;
                            error!("HealthMonitor: Node {} is UNHEALTHY (missed {} heartbeats)", 
                                   node_id, info.consecutive_misses);
                            failed_nodes.push(*node_id);
                        }
                    } else if info.consecutive_misses >= self.suspicious_threshold {
                        if info.health_status != NodeHealth::Suspicious {
                            info.health_status = NodeHealth::Suspicious;
                            warn!("HealthMonitor: Node {} is SUSPICIOUS (missed {} heartbeats)", 
                                  node_id, info.consecutive_misses);
                            suspicious_nodes.push(*node_id);
                        }
                    }
                } else {
                    // Node is healthy now
                    if info.consecutive_misses > 0 {
                        recovered_nodes.push(*node_id);
                    }
                }
            }
        } // Release the write lock here
        
        // Handle recovered nodes
        for node_id in recovered_nodes {
            let mut health_map = self.node_health.write().await;
            if let Some(info) = health_map.get_mut(&node_id) {
                info.consecutive_misses = 0;
                if info.health_status != NodeHealth::Healthy {
                    info.health_status = NodeHealth::Healthy;
                    info!("HealthMonitor: Node {} is healthy again", node_id);
                }
            }
        }
        
        // Handle failed nodes (outside of the lock to avoid deadlock issues)
        for node_id in failed_nodes {
            self.handle_node_failure(node_id).await;
        }
    }
    
    /// Handle node failure - trigger metric rebalancing
    async fn handle_node_failure(&self, failed_node: u64) {
        warn!("HealthMonitor: Handling failure of node {}", failed_node);
        
        // Use leader manager to redistribute metrics
        self.leader_manager.handle_node_failure(failed_node).await;
        
        // Remove node from health monitoring
        let mut health_map = self.node_health.write().await;
        health_map.remove(&failed_node);
        
        info!("HealthMonitor: Completed handling failure of node {}", failed_node);
    }
    
    /// Start background health checking
    pub fn start_health_checks(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut check_interval = interval(Duration::from_secs(1));
            
            loop {
                check_interval.tick().await;
                self.check_all_nodes().await;
            }
        });
    }
    
    /// Get health status of a specific node
    pub async fn get_node_health(&self, node_id: u64) -> Option<NodeHealth> {
        let health_map = self.node_health.read().await;
        health_map.get(&node_id).map(|info| info.health_status)
    }
    
    /// Get all unhealthy nodes
    pub async fn get_unhealthy_nodes(&self) -> Vec<u64> {
        let health_map = self.node_health.read().await;
        health_map
            .iter()
            .filter(|(_, info)| info.health_status == NodeHealth::Unhealthy)
            .map(|(node_id, _)| *node_id)
            .collect()
    }
    
    /// Get cluster health summary
    pub async fn get_health_summary(&self) -> (usize, usize, usize) {
        let health_map = self.node_health.read().await;
        let mut healthy = 0;
        let mut suspicious = 0;
        let mut unhealthy = 0;
        
        for (_, info) in health_map.iter() {
            match info.health_status {
                NodeHealth::Healthy => healthy += 1,
                NodeHealth::Suspicious => suspicious += 1,
                NodeHealth::Unhealthy => unhealthy += 1,
            }
        }
        
        (healthy, suspicious, unhealthy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_register_and_record_heartbeat() {
        let leader_manager = Arc::new(MetricLeaderManager::with_defaults(1));
        let monitor = HealthMonitor::new(1, leader_manager);
        
        monitor.register_node(2).await;
        monitor.record_heartbeat(2).await;
        
        let health = monitor.get_node_health(2).await;
        assert_eq!(health, Some(NodeHealth::Healthy));
    }
    
    #[tokio::test]
    async fn test_node_becomes_suspicious() {
        let leader_manager = Arc::new(MetricLeaderManager::with_defaults(1));
        let monitor = Arc::new(HealthMonitor::new(1, leader_manager));
        
        monitor.register_node(2).await;
        // Manually modify the health info to simulate missed heartbeats
        {
            let mut health_map = monitor.node_health.write().await;
            if let Some(info) = health_map.get_mut(&2) {
                // Set last heartbeat to a time far in the past to simulate missed heartbeats
                info.last_heartbeat = Instant::now() - Duration::from_secs(10);
                info.consecutive_misses = 2; // Above suspicious threshold
            }
        }
        
        monitor.check_all_nodes().await;
        
        // Should become suspicious
        let health = monitor.get_node_health(2).await;
        assert_eq!(health, Some(NodeHealth::Suspicious));
    }
    
    #[tokio::test]
    async fn test_get_unhealthy_nodes() {
        let leader_manager = Arc::new(MetricLeaderManager::with_defaults(1));
        let monitor = Arc::new(HealthMonitor::new(1, leader_manager));
        
        monitor.register_node(2).await;
        // Manually modify the health info to simulate missed heartbeats
        {
            let mut health_map = monitor.node_health.write().await;
            if let Some(info) = health_map.get_mut(&2) {
                // Set last heartbeat to a time far in the past to simulate missed heartbeats
                info.last_heartbeat = Instant::now() - Duration::from_secs(10);
                info.consecutive_misses = 5; // Above failure threshold
                info.health_status = NodeHealth::Suspicious; // Previous state
            }
        }
        
        // Check that we have the node in unhealthy list before check
        let unhealthy_before = monitor.get_unhealthy_nodes().await;
        assert_eq!(unhealthy_before.len(), 0); // Still suspicious
        
        monitor.check_all_nodes().await;
        
        // After check, node should be removed (handled as failure) so list should be empty
        let unhealthy_after = monitor.get_unhealthy_nodes().await;
        assert_eq!(unhealthy_after.len(), 0);
    }
    
    #[tokio::test]
    async fn test_get_health_summary() {
        let leader_manager = Arc::new(MetricLeaderManager::with_defaults(1));
        let monitor = HealthMonitor::new(1, leader_manager);
        
        monitor.register_node(2).await;
        monitor.register_node(3).await;
        
        monitor.record_heartbeat(2).await;
        monitor.record_heartbeat(3).await;
        
        let (healthy, suspicious, unhealthy) = monitor.get_health_summary().await;
        assert_eq!(healthy, 2);
        assert_eq!(suspicious, 0);
        assert_eq!(unhealthy, 0);
    }
}