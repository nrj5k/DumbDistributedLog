//! Distributed aggregator for AutoQueues
//!
//! Provides aggregation operations across cluster nodes.

use crate::dist::node_client::NodeClient;
use crate::dist::sync::TimeSync;
use crate::cluster::RaftClusterNode;
use crate::cluster::types::EntryData;
use evalexpr::{ContextWithMutableVariables, HashMapContext, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

/// Aggregation type for global metrics
#[derive(Debug, Clone)]
pub enum AggregationType {
    Avg,
    Max,
    Min,
    Sum,
    Percentile(f64),
    Stddev,
    Count(String),  // predicate like "cpu > 80"
    Expression {
        expr: String,       // expression like "(100 - cpu) + memory_free"
        agg: Box<AggregationType>,  // how to aggregate the computed scores
    },
}

impl AggregationType {
    /// Parse aggregation string from config
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "avg" => Ok(Self::Avg),
            "max" => Ok(Self::Max),
            "min" => Ok(Self::Min),
            "sum" => Ok(Self::Sum),
            "stddev" => Ok(Self::Stddev),
            "median" => Ok(Self::Percentile(50.0)),
            s if s.starts_with("p") => {
                let pct: f64 = s[1..].parse().map_err(|_| "Invalid percentile")?;
                Ok(Self::Percentile(pct))
            }
            s if s.starts_with("count:") => {
                let predicate = s[6..].to_string();
                Ok(Self::Count(predicate))
            }
            _ => Err(format!("Unknown aggregation type: {}", s)),
        }
    }

    /// Create an expression aggregation
    pub fn expression(expr: &str, agg: AggregationType) -> Self {
        Self::Expression {
            expr: expr.to_string(),
            agg: Box::new(agg),
        }
    }
}

/// Distributed aggregator for computing aggregations across cluster nodes
#[derive(Clone)]
pub struct DistributedAggregator {
    _node_id: String,
    local_values: HashMap<String, f64>,
    node_client: NodeClient,
    _time_sync: TimeSync,
    raft_node: Option<Arc<RaftClusterNode>>,
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
            raft_node: None,
        }
    }
    
    /// Create a new distributed aggregator with Raft cluster support
    pub fn with_raft(node_id: String, nodes: Vec<SocketAddr>, raft_node: Arc<RaftClusterNode>) -> Self {
        let node_client = NodeClient::new(node_id.clone(), nodes);
        let time_sync = TimeSync::new(node_id.clone());

        Self {
            _node_id: node_id,
            local_values: HashMap::new(),
            node_client,
            _time_sync: time_sync,
            raft_node: Some(raft_node),
        }
    }

    /// Set a local value for a queue
    pub fn set_local_value(&mut self, queue_name: String, value: f64) {
        self.local_values.insert(queue_name, value);
    }

    /// Get the global average across all nodes
    pub async fn global_avg(
        &mut self,
        queue_name: &str,
    ) -> Result<f64, Box<dyn std::error::Error>> {
        // Only leader can propose aggregations if Raft is enabled
        if let Some(ref raft_node) = self.raft_node {
            if raft_node.is_leader().await {
                let entry = EntryData {
                    metric_name: queue_name.to_string(),
                    aggregation_type: "avg".to_string(),
                    value: 0.0, // Will be calculated by the aggregation logic
                    timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs(),
                    sources: self.local_values.clone(),
                };
                if let Err(e) = raft_node.propose_aggregation(entry).await {
                    eprintln!("Failed to propose aggregation to Raft cluster: {}", e);
                }
            }
        }
        
        let all_values = self.collect_all(queue_name).await?;
        if all_values.is_empty() {
            return Ok(0.0);
        }

        let sum: f64 = all_values.iter().sum();
        Ok(sum / all_values.len() as f64)
    }

    /// Get the global maximum across all nodes
    pub async fn global_max(
        &mut self,
        queue_name: &str,
    ) -> Result<f64, Box<dyn std::error::Error>> {
        let all_values = self.collect_all(queue_name).await?;
        if all_values.is_empty() {
            return Ok(0.0);
        }

        Ok(all_values.into_iter().fold(f64::NEG_INFINITY, f64::max))
    }

    /// Get the global minimum across all nodes
    pub async fn global_min(
        &mut self,
        queue_name: &str,
    ) -> Result<f64, Box<dyn std::error::Error>> {
        let all_values = self.collect_all(queue_name).await?;
        if all_values.is_empty() {
            return Ok(0.0);
        }

        Ok(all_values.into_iter().fold(f64::INFINITY, f64::min))
    }

    /// Get the global sum across all nodes
    pub async fn global_sum(
        &mut self,
        queue_name: &str,
    ) -> Result<f64, Box<dyn std::error::Error>> {
        let all_values = self.collect_all(queue_name).await?;
        Ok(all_values.iter().sum())
    }

    /// Get the global count of values matching a predicate
    pub async fn global_count(
        &mut self,
        queue_name: &str,
        predicate: &str,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let all_values = self.collect_all(queue_name).await?;
        let predicate_fn = Self::parse_predicate(predicate)?;
        Ok(all_values.iter().filter(|&&v| predicate_fn(v)).count())
    }

    /// Get the global standard deviation across all nodes
    pub async fn global_stddev(
        &mut self,
        queue_name: &str,
    ) -> Result<f64, Box<dyn std::error::Error>> {
        let all_values = self.collect_all(queue_name).await?;
        if all_values.len() < 2 {
            return Ok(0.0);
        }

        let mean: f64 = all_values.iter().sum::<f64>() / all_values.len() as f64;
        let variance: f64 = all_values
            .iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>()
            / (all_values.len() - 1) as f64;
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
    pub async fn global_median(
        &mut self,
        queue_name: &str,
    ) -> Result<f64, Box<dyn std::error::Error>> {
        self.global_percentile(queue_name, 50.0).await
    }

    /// Compute expression-based aggregation
    ///
    /// Example: node_health_score = "(100 - cpu) + memory_free"
    /// Each node computes its local score, then we aggregate those scores.
    pub async fn global_expression(
        &mut self,
        expression: &str,
        sources: &[String],
        agg_type: &AggregationType,
    ) -> Result<f64, Box<dyn std::error::Error>> {
        // Collect values from all sources at each node
        let node_scores = self.collect_all_expression(expression, sources).await?;

        if node_scores.is_empty() {
            return Ok(0.0);
        }

        // Aggregate the per-node scores using the specified aggregation
        match agg_type {
            AggregationType::Avg => {
                let sum: f64 = node_scores.iter().sum();
                Ok(sum / node_scores.len() as f64)
            }
            AggregationType::Max => Ok(node_scores
                .into_iter()
                .fold(f64::NEG_INFINITY, f64::max)),
            AggregationType::Min => Ok(node_scores
                .into_iter()
                .fold(f64::INFINITY, f64::min)),
            AggregationType::Sum => Ok(node_scores.iter().sum()),
            AggregationType::Stddev => {
                if node_scores.len() < 2 {
                    return Ok(0.0);
                }
                let mean: f64 = node_scores.iter().sum::<f64>() / node_scores.len() as f64;
                let variance: f64 = node_scores
                    .iter()
                    .map(|&x| (x - mean).powi(2))
                    .sum::<f64>()
                    / (node_scores.len() - 1) as f64;
                Ok(variance.sqrt())
            }
            AggregationType::Percentile(pct) => {
                let mut sorted = node_scores;
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let index = (*pct / 100.0 * (sorted.len() - 1) as f64).round() as usize;
                Ok(sorted[index.min(sorted.len() - 1)])
            }
            AggregationType::Count(_) => Ok(node_scores.len() as f64),
            AggregationType::Expression { .. } => Err("Nested expression aggregation not supported".into()),
        }
    }

    /// Parse a predicate string into a closure
    fn parse_predicate(predicate: &str) -> Result<Box<dyn Fn(f64) -> bool>, String> {
        // Simple predicates like "cpu > 80"
        if let Some((var, rest)) = predicate.split_once(|c: char| !c.is_alphanumeric() && c != '_') {
            let _var = var.trim();
            let rest = rest.trim();

            if rest.starts_with(">=") {
                let threshold: f64 = rest[2..].trim().parse().map_err(|_| "Invalid threshold")?;
                Ok(Box::new(move |v: f64| v >= threshold))
            } else if rest.starts_with(">") {
                let threshold: f64 = rest[1..].trim().parse().map_err(|_| "Invalid threshold")?;
                Ok(Box::new(move |v: f64| v > threshold))
            } else if rest.starts_with("<=") {
                let threshold: f64 = rest[2..].trim().parse().map_err(|_| "Invalid threshold")?;
                Ok(Box::new(move |v: f64| v <= threshold))
            } else if rest.starts_with("<") {
                let threshold: f64 = rest[1..].trim().parse().map_err(|_| "Invalid threshold")?;
                Ok(Box::new(move |v: f64| v < threshold))
            } else if rest.starts_with("==") {
                let threshold: f64 = rest[2..].trim().parse().map_err(|_| "Invalid threshold")?;
                let threshold_copy = threshold;
                Ok(Box::new(move |v: f64| (v - threshold_copy).abs() < 0.001))
            } else {
                Err("Unknown predicate operator".to_string())
            }
        } else {
            Err("Invalid predicate format".to_string())
        }
    }

    /// Evaluate expression at a single node
    fn evaluate_expression(&self, expression: &str, sources: &[String]) -> Result<f64, String> {
        use evalexpr::DefaultNumericTypes;
        let mut context: HashMapContext<DefaultNumericTypes> = HashMapContext::new();

        for source in sources {
            if let Some(&value) = self.local_values.get(source) {
                context
                    .set_value(source.clone(), Value::Float(value))
                    .map_err(|e| e.to_string())?;
            }
        }

        evalexpr::eval_float_with_context(expression, &context)
            .map_err(|e| format!("Expression error: {}", e))
    }

    /// Collect all values for expression evaluation from all nodes
    async fn collect_all_expression(
        &mut self,
        expression: &str,
        sources: &[String],
    ) -> Result<Vec<f64>, Box<dyn std::error::Error>> {
        let mut node_scores = Vec::new();

        // Compute local score
        if let Ok(local_score) = self.evaluate_expression(expression, sources) {
            node_scores.push(local_score);
        }

        // Collect scores from remote nodes
        let remote_scores = self
            .node_client
            .collect_all_expression(expression, sources.to_vec())
            .await?;
        node_scores.extend(remote_scores);

        Ok(node_scores)
    }

    /// Helper to collect all values from all nodes including local
    async fn collect_all(
        &mut self,
        queue_name: &str,
    ) -> Result<Vec<f64>, Box<dyn std::error::Error>> {
        let mut values = Vec::new();

        // Get local value if available
        if let Some(&local_value) = self.local_values.get(queue_name) {
            values.push(local_value);
        }

        // Collect from remote nodes
        let remote_values = self.node_client.collect_all_values(queue_name).await?;
        values.extend(remote_values);

        Ok(values)
    }
}
