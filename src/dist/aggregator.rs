//! Distributed aggregator for AutoQueues
//!
//! Provides aggregation operations across cluster nodes.

use crate::dist::node_client::NodeClient;
use crate::dist::sync::TimeSync;
use std::collections::HashMap;
use std::net::SocketAddr;

/// Distributed aggregator for computing aggregations across cluster nodes
pub struct DistributedAggregator {
    _node_id: String,
    local_values: HashMap<String, f64>,
    node_client: NodeClient,
    _time_sync: TimeSync,
}

impl DistributedAggregator {
    /// Create a new distributed aggregator
    pub fn new(node_id: String, nodes: Vec<SocketAddr>) -> Self {
        let node_client = NodeClient::new(node_id.clone(), nodes);
        let time_sync = TimeSync::new(node_id.clone());
        
        Self {
            _node_id: node_id,
            local_values: HashMap::new(),
            node_client,
            _time_sync: time_sync,
        }
    }
    
    /// Set a local value for a queue
    pub fn set_local_value(&mut self, queue_name: String, value: f64) {
        self.local_values.insert(queue_name, value);
    }
    
    /// Get the global average across all nodes
    pub async fn global_avg(&mut self, queue_name: &str) -> Result<f64, Box<dyn std::error::Error>> {
        let all_values = self.collect_all(queue_name).await?;
        if all_values.is_empty() {
            return Ok(0.0);
        }
        
        let sum: f64 = all_values.iter().sum();
        Ok(sum / all_values.len() as f64)
    }
    
    /// Get the global maximum across all nodes
    pub async fn global_max(&mut self, queue_name: &str) -> Result<f64, Box<dyn std::error::Error>> {
        let all_values = self.collect_all(queue_name).await?;
        if all_values.is_empty() {
            return Ok(0.0);
        }
        
        Ok(all_values.into_iter().fold(f64::NEG_INFINITY, f64::max))
    }
    
    /// Get the global minimum across all nodes
    pub async fn global_min(&mut self, queue_name: &str) -> Result<f64, Box<dyn std::error::Error>> {
        let all_values = self.collect_all(queue_name).await?;
        if all_values.is_empty() {
            return Ok(0.0);
        }
        
        Ok(all_values.into_iter().fold(f64::INFINITY, f64::min))
    }
    
    /// Get the global sum across all nodes
    pub async fn global_sum(&mut self, queue_name: &str) -> Result<f64, Box<dyn std::error::Error>> {
        let all_values = self.collect_all(queue_name).await?;
        Ok(all_values.iter().sum())
    }
    
    /// Get the global count of values matching a predicate
    pub async fn global_count<F>(
        &mut self,
        queue_name: &str,
        predicate: F,
    ) -> Result<usize, Box<dyn std::error::Error>>
    where
        F: Fn(f64) -> bool,
    {
        let all_values = self.collect_all(queue_name).await?;
        Ok(all_values.iter().filter(|&&v| predicate(v)).count())
    }
    
    /// Get the global standard deviation across all nodes
    pub async fn global_stddev(&mut self, queue_name: &str) -> Result<f64, Box<dyn std::error::Error>> {
        let all_values = self.collect_all(queue_name).await?;
        if all_values.len() < 2 {
            return Ok(0.0);
        }
        
        let mean: f64 = all_values.iter().sum::<f64>() / all_values.len() as f64;
        let variance: f64 = all_values.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (all_values.len() - 1) as f64;
        Ok(variance.sqrt())
    }
    
    /// Get the global percentile across all nodes
    pub async fn global_percentile(
        &mut self,
        queue_name: &str,
        percentile: f64,
    ) -> Result<f64, Box<dyn std::error::Error>> {
        let mut all_values = self.collect_all(queue_name).await?;
        if all_values.is_empty() {
            return Ok(0.0);
        }
        
        all_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let index = (percentile / 100.0 * (all_values.len() - 1) as f64).round() as usize;
        Ok(all_values[index.min(all_values.len() - 1)])
    }
    
    /// Get the global median across all nodes
    pub async fn global_median(&mut self, queue_name: &str) -> Result<f64, Box<dyn std::error::Error>> {
        self.global_percentile(queue_name, 50.0).await
    }
    
    /// Helper to collect all values from all nodes including local
    async fn collect_all(&mut self, queue_name: &str) -> Result<Vec<f64>, Box<dyn std::error::Error>> {
        // Get local value if available
        let mut values = Vec::new();
        if let Some(&local_value) = self.local_values.get(queue_name) {
            values.push(local_value);
        }
        
        // Collect from remote nodes
        let remote_values = self.node_client.collect_all_values(queue_name).await?;
        values.extend(remote_values);
        
        Ok(values)
    }
}