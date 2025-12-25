//! Leader - Derived metric aggregation and serving
//!
//! Responsibilities:
//! - Subscribe to source topics on all nodes
//! - Aggregate local values into derived metrics
//! - Serve derived values on query (REQ/REP)
//! - Publish aggregated values to pub/sub

use crate::cluster::leader_assignment::DerivedMetric;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::interval;
use zmq::{Context, Socket, SocketType};

/// Leader configuration
#[derive(Debug, Clone)]
pub struct LeaderConfig {
    /// This node's ID
    pub node_id: u64,
    /// ZMQ data port (for receiving local metrics)
    pub data_port: u16,
    /// ZMQ bind address
    pub bind_addr: String,
    /// Aggregation interval
    pub aggregation_interval_ms: u64,
}

impl Default for LeaderConfig {
    fn default() -> Self {
        Self {
            node_id: 1,
            data_port: 6967,
            bind_addr: "0.0.0.0".to_string(),
            aggregation_interval_ms: 1000,
        }
    }
}

impl LeaderConfig {
    /// Get ZMQ bind address for data
    pub fn zmq_bind_addr(&self) -> String {
        format!("tcp://{}:{}", self.bind_addr, self.data_port)
    }
}

/// Local metric value with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalMetricValue {
    pub metric_name: String,
    pub node_id: u64,
    pub value: f64,
    pub timestamp: u64,
}

/// Aggregated derived metric value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedMetricValue {
    pub metric_name: String,
    pub value: f64,
    pub aggregation_type: String,
    pub source_count: usize,
    pub timestamp: u64,
    pub leader_id: u64,
}

/// Aggregation type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AggregationType {
    Sum,
    Average,
    Max,
    Min,
}

impl AggregationType {
    /// Parse from string
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "sum" => AggregationType::Sum,
            "average" | "avg" => AggregationType::Average,
            "max" => AggregationType::Max,
            "min" => AggregationType::Min,
            _ => AggregationType::Sum,  // Default
        }
    }

    /// Aggregate a list of values
    pub fn aggregate(&self, values: &[f64]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }

        match self {
            AggregationType::Sum => values.iter().sum(),
            AggregationType::Average => values.iter().sum::<f64>() / values.len() as f64,
            AggregationType::Max => values.iter().fold(f64::MIN, |a, &b| a.max(b)),
            AggregationType::Min => values.iter().fold(f64::MAX, |a, &b| a.min(b)),
        }
    }
}

/// Leader state
#[derive(Debug, Clone)]
pub struct LeaderState {
    /// Derived metrics this leader is responsible for
    pub derived_metrics: Vec<DerivedMetric>,
    /// Local values received from nodes: metric_name -> Vec<(node_id, value, timestamp)>
    pub local_values: HashMap<String, Vec<(u64, f64, Instant)>>,
    /// Aggregated values: metric_name -> AggregatedMetricValue
    pub aggregated_values: HashMap<String, AggregatedMetricValue>,
    /// Last aggregation time
    pub last_aggregation: Option<Instant>,
}

impl Default for LeaderState {
    fn default() -> Self {
        Self {
            derived_metrics: Vec::new(),
            local_values: HashMap::new(),
            aggregated_values: HashMap::new(),
            last_aggregation: None,
        }
    }
}

/// Leader for derived metric aggregation
pub struct Leader {
    /// Configuration
    config: LeaderConfig,
    /// Shared state
    state: Arc<RwLock<LeaderState>>,
    /// ZMQ context
    _context: Context,
    /// ZMQ SUB socket for receiving local metrics
    subscribe_socket: Socket,
    /// ZMQ REP socket for query service
    query_socket: Socket,
    /// ZMQ PUB socket for publishing aggregated values
    publish_socket: Socket,
    /// Channel for receiving local metric updates
    metric_rx: mpsc::Receiver<LocalMetricValue>,
    /// Shutdown signal
    shutdown_rx: mpsc::Receiver<()>,
}

impl Leader {
    /// Create new leader
    pub fn new(
        config: LeaderConfig,
        derived_metrics: Vec<DerivedMetric>,
    ) -> Result<Self, zmq::Error> {
        let context = Context::new();

        // Create SUB socket for receiving local metrics
        let subscribe_socket = context.socket(SocketType::SUB)?;
        subscribe_socket.set_linger(0);
        subscribe_socket.set_rcvtimeo(1000);

        // Subscribe to all source topics
        for metric in &derived_metrics {
            for source in &metric.sources {
                // Convert "local.nvme_size" -> "local.nvme_size"
                let topic = source.clone();
                subscribe_socket.set_subscribe(topic.as_bytes())?;
            }
        }

        // Create REP socket for query service
        let query_socket = context.socket(SocketType::REP)?;
        query_socket.set_linger(0);
        let query_addr = format!("tcp://{}:{}", config.bind_addr, config.data_port + 1000);
        query_socket.bind(&query_addr)?;

        // Create PUB socket for publishing aggregated values
        let publish_socket = context.socket(SocketType::PUB)?;
        publish_socket.set_linger(0);
        let publish_addr = format!("tcp://{}:{}", config.bind_addr, config.data_port + 2000);
        publish_socket.bind(&publish_addr)?;

        // Create channels
        let (_metric_tx, metric_rx) = mpsc::channel(100);
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        let state = Arc::new(RwLock::new(LeaderState {
            derived_metrics: derived_metrics.clone(),
            local_values: HashMap::new(),
            aggregated_values: HashMap::new(),
            last_aggregation: None,
        }));

        let _ = shutdown_tx;  // Drop the sender

        Ok(Self {
            config,
            state,
            _context: context,
            subscribe_socket,
            query_socket,
            publish_socket,
            metric_rx,
            shutdown_rx,
        })
    }

    /// Run the leader
    pub async fn run(&mut self) {
        let mut aggregation_interval = interval(Duration::from_millis(self.config.aggregation_interval_ms));

        loop {
            tokio::select! {
                _ = aggregation_interval.tick() => {
                    self.aggregate_all().await;
                }
                _ = self.shutdown_rx.recv() => {
                    println!("Leader shutting down");
                    break;
                }
            }
        }
    }

    /// Aggregate all derived metrics
    async fn aggregate_all(&mut self) {
        let now = Instant::now();

        // Clone derived metrics to avoid borrow issues
        let derived_metrics: Vec<DerivedMetric> = {
            let state = self.state.read().unwrap();
            state.derived_metrics.clone()
        };

        // Aggregate each metric
        {
            let mut state = self.state.write().unwrap();
            for metric in &derived_metrics {
                self.aggregate_metric(&metric, &mut state, now);
            }
            state.last_aggregation = Some(now);
        }

        // Publish aggregated values
        self.publish_all();
    }

    /// Aggregate a single metric
    fn aggregate_metric(
        &self,
        metric: &DerivedMetric,
        state: &mut LeaderState,
        now: Instant,
    ) {
        // Get local values for this metric's sources
        let mut all_values: Vec<f64> = Vec::new();

        for source in &metric.sources {
            if let Some(values) = state.local_values.get(source) {
                for &(_node_id, value, _timestamp) in values {
                    all_values.push(value);
                }
            }
        }

        // Aggregate
        let agg_type = AggregationType::from_str(&metric.aggregation);
        let aggregated_value = agg_type.aggregate(&all_values);

        // Store aggregated value
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let aggregated = AggregatedMetricValue {
            metric_name: metric.name.clone(),
            value: aggregated_value,
            aggregation_type: metric.aggregation.clone(),
            source_count: all_values.len(),
            timestamp,
            leader_id: self.config.node_id,
        };

        state.aggregated_values.insert(metric.name.clone(), aggregated);
    }

    /// Publish all aggregated values
    fn publish_all(&self) {
        let aggregated_values: HashMap<String, AggregatedMetricValue> = {
            let state = self.state.read().unwrap();
            state.aggregated_values.clone()
        };

        for (_name, value) in &aggregated_values {
            let topic = format!("derived.{}", value.metric_name);
            let data = serde_json::to_string(value).unwrap();

            let mut message = Vec::new();
            message.extend_from_slice(topic.as_bytes());
            message.push(0);
            message.extend_from_slice(data.as_bytes());

            if let Err(e) = self.publish_socket.send(&message, zmq::DONTWAIT) {
                println!("Failed to publish aggregated value: {}", e);
            }
        }
    }

    /// Update local value
    pub fn update_local_value(&mut self, value: LocalMetricValue) {
        let mut state = self.state.write().unwrap();

        // Add to local values
        let entry = state
            .local_values
            .entry(value.metric_name.clone())
            .or_insert_with(Vec::new);

        entry.push((value.node_id, value.value, Instant::now()));

        // Keep only recent values (last 10 per metric)
        if entry.len() > 10 {
            entry.drain(0..(entry.len() - 10));
        }
    }

    /// Get aggregated value (simple getter for queries)
    /// Returns the latest aggregated value for a metric, or None if not available
    pub fn get_aggregated_value(&self, metric_name: &str) -> Option<f64> {
        let state = self.state.read().unwrap();
        state.aggregated_values.get(metric_name).map(|v| v.value)
    }

    /// Query derived metric value (full info)
    pub fn query_derived(&self, metric_name: &str) -> Option<AggregatedMetricValue> {
        let state = self.state.read().unwrap();
        state.aggregated_values.get(metric_name).cloned()
    }

    /// Get all aggregated values
    pub fn all_aggregated(&self) -> HashMap<String, AggregatedMetricValue> {
        let state = self.state.read().unwrap();
        state.aggregated_values.clone()
    }

    /// Check if this leader handles a metric
    pub fn handles_metric(&self, metric_name: &str) -> bool {
        let state = self.state.read().unwrap();
        state.derived_metrics.iter().any(|m| &m.name == metric_name)
    }
}

/// Start leader with config
pub async fn start_leader(
    node_id: u64,
    derived_metrics: Vec<DerivedMetric>,
    data_port: u16,
    aggregation_interval_ms: u64,
) -> Result<Leader, zmq::Error> {
    let config = LeaderConfig {
        node_id,
        data_port,
        bind_addr: "0.0.0.0".to_string(),
        aggregation_interval_ms,
    };

    Leader::new(config, derived_metrics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregation_type_from_str() {
        assert_eq!(AggregationType::from_str("sum"), AggregationType::Sum);
        assert_eq!(AggregationType::from_str("average"), AggregationType::Average);
        assert_eq!(AggregationType::from_str("avg"), AggregationType::Average);
        assert_eq!(AggregationType::from_str("max"), AggregationType::Max);
        assert_eq!(AggregationType::from_str("min"), AggregationType::Min);
        assert_eq!(AggregationType::from_str("unknown"), AggregationType::Sum);  // Default
    }

    #[test]
    fn test_aggregation_sum() {
        let agg = AggregationType::Sum;
        assert_eq!(agg.aggregate(&[1.0, 2.0, 3.0]), 6.0);
        assert_eq!(agg.aggregate(&[5.0]), 5.0);
        assert_eq!(agg.aggregate(&[]), 0.0);
    }

    #[test]
    fn test_aggregation_average() {
        let agg = AggregationType::Average;
        assert_eq!(agg.aggregate(&[1.0, 2.0, 3.0]), 2.0);
        assert_eq!(agg.aggregate(&[5.0]), 5.0);
        assert_eq!(agg.aggregate(&[]), 0.0);
    }

    #[test]
    fn test_aggregation_max() {
        let agg = AggregationType::Max;
        assert_eq!(agg.aggregate(&[1.0, 5.0, 3.0]), 5.0);
        assert_eq!(agg.aggregate(&[10.0]), 10.0);
        assert_eq!(agg.aggregate(&[]), 0.0);
    }

    #[test]
    fn test_aggregation_min() {
        let agg = AggregationType::Min;
        assert_eq!(agg.aggregate(&[1.0, 5.0, 3.0]), 1.0);
        assert_eq!(agg.aggregate(&[10.0]), 10.0);
        assert_eq!(agg.aggregate(&[]), 0.0);
    }

    #[test]
    fn test_leader_config() {
        let config = LeaderConfig::default();
        assert_eq!(config.node_id, 1);
        assert_eq!(config.data_port, 6967);
        assert_eq!(config.aggregation_interval_ms, 1000);
    }

    #[test]
    fn test_local_metric_value() {
        let value = LocalMetricValue {
            metric_name: "local.nvme_size".to_string(),
            node_id: 2,
            value: 512.0,
            timestamp: 1234567890,
        };

        assert_eq!(value.metric_name, "local.nvme_size");
        assert_eq!(value.node_id, 2);
        assert_eq!(value.value, 512.0);
    }

    #[test]
    fn test_aggregated_metric_value() {
        let value = AggregatedMetricValue {
            metric_name: "derived.nvme_drive_capacity".to_string(),
            value: 1792.0,
            aggregation_type: "sum".to_string(),
            source_count: 3,
            timestamp: 1234567890,
            leader_id: 1,
        };

        assert_eq!(value.metric_name, "derived.nvme_drive_capacity");
        assert_eq!(value.value, 1792.0);
        assert_eq!(value.source_count, 3);
    }
}
