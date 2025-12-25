//! Leader Assignment Module
//!
//! Implements weighted bag leader assignment for derived metrics.
//! Uses node scores to probabilistically assign leaders.

use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Derived metric configuration from config
#[derive(Debug, Clone)]
pub struct DerivedMetric {
    pub name: String,
    pub aggregation: String,
    pub sources: Vec<String>,
}

/// Weighted node entry for leader assignment
#[derive(Debug, Clone)]
pub struct WeightedNode {
    pub node_id: u64,
    pub hostname: String,
    pub score: f64,
}

/// Leader map: derived metric name → leader node_id
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderMap {
    pub assignments: HashMap<String, u64>, // metric_name → node_id
    pub version: u64,
}

impl LeaderMap {
    /// Create empty leader map
    pub fn new() -> Self {
        Self {
            assignments: HashMap::new(),
            version: 0,
        }
    }

    /// Get leader for a metric
    pub fn get_leader(&self, metric: &str) -> Option<u64> {
        self.assignments.get(metric).copied()
    }

    /// Check if a node is leader for a metric
    pub fn is_leader(&self, metric: &str, node_id: u64) -> bool {
        self.assignments.get(metric) == Some(&node_id)
    }

    /// Set leader for a metric
    pub fn set_leader(&mut self, metric: String, node_id: u64) {
        self.assignments.insert(metric, node_id);
        self.version += 1;
    }

    /// Remove leader for a metric
    pub fn remove_leader(&mut self, metric: &str) -> Option<u64> {
        let removed = self.assignments.remove(metric);
        if removed.is_some() {
            self.version += 1;
        }
        removed
    }

    /// Get all leader assignments
    pub fn assignments(&self) -> &HashMap<String, u64> {
        &self.assignments
    }

    /// Get version (incremented on changes)
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Get metrics assigned to a node
    pub fn metrics_for_node(&self, node_id: u64) -> Vec<String> {
        self.assignments
            .iter()
            .filter(|item| *item.1 == node_id)
            .map(|(metric, _)| metric.clone())
            .collect()
    }

    /// Merge another leader map (for Raft replication)
    pub fn merge(&mut self, other: LeaderMap) {
        if other.version > self.version {
            self.assignments = other.assignments;
            self.version = other.version;
        }
    }
}

impl Default for LeaderMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Weighted bag for probabilistic leader selection
#[derive(Debug, Clone)]
pub struct WeightedBag {
    nodes: Vec<WeightedNode>,
    total_weight: f64,
}

impl WeightedBag {
    /// Create new weighted bag from node scores and order
    pub fn new(node_scores: HashMap<String, f64>, node_order: &[String]) -> Self {
        let mut nodes = Vec::new();

        for (i, hostname) in node_order.iter().enumerate() {
            let score = node_scores.get(hostname).copied().unwrap_or(1.0);
            nodes.push(WeightedNode {
                node_id: (i + 1) as u64, // 1-indexed
                hostname: hostname.clone(),
                score,
            });
        }

        let total_weight: f64 = nodes.iter().map(|n| n.score).sum();

        Self {
            nodes,
            total_weight,
        }
    }

    /// Check if bag has nodes
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Get number of nodes
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Select a leader using weighted random selection
    pub fn select_leader(&self) -> Option<u64> {
        if self.nodes.is_empty() {
            return None;
        }

        let rng_val = rand::rng().random_range(0.0..self.total_weight);
        let mut cumulative = 0.0;

        for node in &self.nodes {
            cumulative += node.score;
            if rng_val <= cumulative {
                return Some(node.node_id);
            }
        }

        // Fallback to last node
        Some(self.nodes.last().unwrap().node_id)
    }

    /// Assign leaders for multiple metrics using round-robin within weighted bag
    pub fn assign_leaders(&self, metrics: &[DerivedMetric]) -> LeaderMap {
        let mut assignments = HashMap::new();
        let mut round_robin_index = 0;

        // Sort metrics for deterministic assignment
        let mut sorted_metrics: Vec<_> = metrics.iter().collect();
        sorted_metrics.sort_by_key(|m| &m.name);

        for metric in sorted_metrics {
            let node_id = if self.nodes.is_empty() {
                1 // fallback
            } else {
                self.nodes[round_robin_index % self.nodes.len()].node_id
            };

            assignments.insert(metric.name.clone(), node_id);
            round_robin_index += 1;
        }

        LeaderMap {
            assignments,
            version: 1,
        }
    }

    /// Get node info by node_id
    pub fn get_node(&self, node_id: u64) -> Option<&WeightedNode> {
        self.nodes.iter().find(|n| n.node_id == node_id)
    }

    /// Get all nodes
    pub fn nodes(&self) -> &[WeightedNode] {
        &self.nodes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weighted_bag_selection() {
        let node_scores: HashMap<String, f64> = [
            ("server1".to_string(), 10.0),
            ("server2".to_string(), 5.0),
            ("server3".to_string(), 1.0),
        ].iter().cloned().collect();

        let node_order: Vec<String> = vec!["server1".to_string(), "server2".to_string(), "server3".to_string()];
        let bag = WeightedBag::new(node_scores, &node_order);

        assert_eq!(bag.len(), 3);
        assert_eq!(bag.total_weight, 16.0);

        let metrics = vec![
            DerivedMetric {
                name: "m1".to_string(),
                aggregation: "sum".to_string(),
                sources: vec![],
            },
            DerivedMetric {
                name: "m2".to_string(),
                aggregation: "sum".to_string(),
                sources: vec![],
            },
            DerivedMetric {
                name: "m3".to_string(),
                aggregation: "sum".to_string(),
                sources: vec![],
            },
        ];

        let leader_map = bag.assign_leaders(&metrics);

        assert_eq!(leader_map.get_leader("m1"), Some(1));
        assert_eq!(leader_map.get_leader("m2"), Some(2));
        assert_eq!(leader_map.get_leader("m3"), Some(3));
    }

    #[test]
    fn test_leader_map_operations() {
        let mut map = LeaderMap::new();

        assert_eq!(map.get_leader("metric1"), None);

        map.set_leader("metric1".to_string(), 1);
        assert_eq!(map.get_leader("metric1"), Some(1));
        assert!(map.is_leader("metric1", 1));
        assert!(!map.is_leader("metric1", 2));

        let metrics = map.metrics_for_node(1);
        assert_eq!(metrics, vec!["metric1"]);

        map.remove_leader("metric1");
        assert_eq!(map.get_leader("metric1"), None);
    }

    #[test]
    fn test_weighted_bag_empty() {
        let bag = WeightedBag::new(HashMap::new(), &[]);
        assert!(bag.is_empty());
        assert_eq!(bag.select_leader(), None);

        let leader_map = bag.assign_leaders(&[]);
        assert!(leader_map.assignments.is_empty());
    }
}
