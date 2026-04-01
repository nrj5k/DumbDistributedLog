//! AutoQueues Node
//!
//! Main node implementation that collects local metrics,
//! publishes them via ZMQ, and aggregates global metrics.

use crate::config::{Config, GlobalMetricConfig, NodeConfig};
use crate::constants;
use crate::network::pubsub::zmq::{ZmqPubSubBroker, ZmqPubSubClient};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;

/// Remote metric value with source information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteMetric {
    pub node_id: u64,
    pub metric_name: String,
    pub value: f64,
    pub timestamp: u64,
}

/// Message format for metric publication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricMessage {
    pub node_id: u64,
    pub metric_name: String,
    pub value: f64,
    pub timestamp: u64,
}

/// Aggregation helper functions
pub mod aggregation {

    /// Compute average of values
    pub fn avg(values: &[f64]) -> f64 {
        if values.is_empty() {
            0.0
        } else {
            values.iter().sum::<f64>() / values.len() as f64
        }
    }

    /// Compute maximum of values
    pub fn max(values: &[f64]) -> f64 {
        values.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    /// Compute minimum of values
    pub fn min(values: &[f64]) -> f64 {
        values.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    /// Compute sum of values
    pub fn sum(values: &[f64]) -> f64 {
        values.iter().sum()
    }

    /// Compute percentile (p95, p99, etc.)
    pub fn percentile(values: &mut Vec<f64>, p: f64) -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let index = (p / 100.0 * (values.len() - 1) as f64).round() as usize;
        values[index.min(values.len() - 1)]
    }

    /// Compute health score: (100 - cpu) + memory_free
    #[allow(dead_code)]
    pub fn health_score(cpu: f64, memory_free: f64) -> f64 {
        (100.0 - cpu) + memory_free
    }
}

pub struct AutoQueuesNode {
    pub node_id: u64,
    pub local_values: Arc<RwLock<HashMap<String, f64>>>,
    pub global_values: Arc<RwLock<HashMap<String, f64>>>,
    pub remote_values: Arc<RwLock<HashMap<String, HashMap<u64, f64>>>>, // metric -> node_id -> value
    pub running: Arc<RwLock<bool>>,
    pub config: Config,
    pub my_config: NodeConfig,
    pub publisher: Arc<RwLock<Option<ZmqPubSubBroker>>>,
    pub client: Arc<RwLock<Option<ZmqPubSubClient>>>,
}

impl AutoQueuesNode {
    pub async fn new(config: &Config) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let node_id = config.detect_node_id()?;
        let my_config = config.my_node(node_id)?;

        // Create ZMQ pub/sub broker for publishing metrics
        let pubsub_addr = format!("tcp://*:{}", my_config.pubsub_port);
        let publisher = ZmqPubSubBroker::with_bind_addr(&pubsub_addr).map_err(|e| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;

        // Create pub/sub client for subscribing to other nodes
        let client = ZmqPubSubClient::new().map_err(|e| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;

        Ok(Self {
            node_id,
            local_values: Arc::new(RwLock::new(HashMap::new())),
            global_values: Arc::new(RwLock::new(HashMap::new())),
            remote_values: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
            config: config.clone(),
            my_config,
            publisher: Arc::new(RwLock::new(Some(publisher))),
            client: Arc::new(RwLock::new(Some(client))),
        })
    }

    pub fn node_id(&self) -> u64 {
        self.node_id
    }

    pub async fn run(
        &self,
        config: &Config,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!(
            "Starting AutoQueues node {} on pubsub port {}",
            self.node_id, self.my_config.pubsub_port
        );
        *self.running.write().await = true;

        // Clone for each background task
        let node_id = self.node_id;
        let publisher = self.publisher.clone();
        let client = self.client.clone();
        let running = self.running.clone();

        // Spawn metric collection and publishing task
        let local_values_pub = self.local_values.clone();
        tokio::spawn(async move {
            if let Err(e) =
                Self::collect_and_publish(node_id, local_values_pub, publisher, running).await
            {
                eprintln!("Publisher error: {}", e);
            }
        });

        // Spawn subscription and receive task
        let remote_values_sub = self.remote_values.clone();
        let running_sub = self.running.clone();
        let other_nodes: Result<Vec<SocketAddr>, _> = config
            .other_nodes(node_id)
            .into_iter()
            .map(|(_, cfg)| cfg.pubsub_addr())
            .collect();
        let other_nodes = other_nodes?;

        tokio::spawn(async move {
            if let Err(e) = Self::subscribe_and_receive(
                node_id,
                client,
                remote_values_sub,
                other_nodes,
                running_sub,
            )
            .await
            {
                eprintln!("Subscriber error: {}", e);
            }
        });

        // Spawn global aggregation task - uses config-defined aggregations
        let local_values_agg = self.local_values.clone();
        let remote_values_agg = self.remote_values.clone();
        let global_values_agg = self.global_values.clone();
        let running_agg = self.running.clone();
        let global_config = config.global.clone();

        tokio::spawn(async move {
            if let Err(e) = Self::aggregate_global(
                local_values_agg,
                remote_values_agg,
                global_values_agg,
                global_config,
                running_agg,
            )
            .await
            {
                eprintln!("Aggregator error: {}", e);
            }
        });

        println!("Node {} running. Press Ctrl+C to stop.", node_id);
        tokio::signal::ctrl_c().await?;
        *self.running.write().await = false;
        println!("Node {} stopped", node_id);
        Ok(())
    }

    /// Collect local metrics and publish them
    async fn collect_and_publish(
        node_id: u64,
        local_values: Arc<RwLock<HashMap<String, f64>>>,
        publisher: Arc<RwLock<Option<ZmqPubSubBroker>>>,
        running: Arc<RwLock<bool>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut interval = interval(Duration::from_millis(constants::time::METRICS_INTERVAL_MS));
        let mut tick_count = 0u64;

        while *running.read().await {
            interval.tick().await;
            tick_count += 1;

            // Simple deterministic pseudo-random based on time and node_id
            // This avoids Send+Sync issues with RNG
            let time_factor = (tick_count % 60) as f64 / 60.0;
            let noise = ((node_id * 7919 + tick_count % 100) as f64 / 100.0) - 0.5;

            // Simulate local metric collection (would use sysinfo in real code)
            let cpu = 40.0 + (node_id as f64 * 5.0) + (noise * 10.0);
            let memory = 50.0 + (node_id as f64 * 3.0) + (noise * 8.0);
            let memory_free = 100.0 - memory;
            let temperature = 60.0 + (node_id as f64 * 2.0) + (time_factor * 5.0);
            let vibration = 10.0 + (noise * 5.0);

            let mut l = local_values.write().await;
            l.insert("cpu".to_string(), cpu);
            l.insert("memory".to_string(), memory);
            l.insert("memory_free".to_string(), memory_free);
            l.insert("temperature".to_string(), temperature);
            l.insert("vibration".to_string(), vibration);

            // Publish metrics to cluster
            let publisher_guard = publisher.read().await;
            if let Some(broker) = &*publisher_guard {
                let timestamp = crate::types::now_millis();

                for (name, value) in &*l {
                    let msg = MetricMessage {
                        node_id,
                        metric_name: name.clone(),
                        value: *value,
                        timestamp,
                    };

                    let topic = format!("metric:{}", name);
                    if let Err(e) = broker.publish(&topic, &msg) {
                        eprintln!("Failed to publish {}: {}", topic, e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Subscribe to other nodes and receive their metrics
    async fn subscribe_and_receive(
        _node_id: u64,
        client: Arc<RwLock<Option<ZmqPubSubClient>>>,
        remote_values: Arc<RwLock<HashMap<String, HashMap<u64, f64>>>>,
        other_nodes: Vec<SocketAddr>,
        running: Arc<RwLock<bool>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Subscribe to all metric topics from each node
        let mut subscriber_ids = Vec::new();

        {
            let client_guard = client.read().await;
            if let Some(cl) = &*client_guard {
                for addr in &other_nodes {
                    let addr_str = addr.to_string();
                    // Subscribe to all metric topics
                    if let Ok(id) = cl.subscribe("metric:", &addr_str) {
                        subscriber_ids.push(id);
                    }
                }
            }
        }

        let mut interval = interval(Duration::from_millis(
            constants::time::SHORT_SLEEP_INTERVAL_MS,
        ));

        while *running.read().await {
            interval.tick().await;

            let client_guard = client.read().await;
            if let Some(cl) = &*client_guard {
                for sub_id in &subscriber_ids {
                    while let Ok(Some((topic, data))) = cl.receive(sub_id) {
                        if let Ok(msg) = serde_json::from_slice::<MetricMessage>(&data) {
                            // Parse topic to get metric name (format: "metric:cpu")
                            let metric_name = topic.strip_prefix("metric:").unwrap_or(&topic);

                            let mut r = remote_values.write().await;
                            r.entry(metric_name.to_string())
                                .or_insert_with(HashMap::new)
                                .insert(msg.node_id, msg.value);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Compute global aggregations from config-defined global metrics
    async fn aggregate_global(
        local_values: Arc<RwLock<HashMap<String, f64>>>,
        remote_values: Arc<RwLock<HashMap<String, HashMap<u64, f64>>>>,
        global_values: Arc<RwLock<HashMap<String, f64>>>,
        global_config: HashMap<String, GlobalMetricConfig>,
        running: Arc<RwLock<bool>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut interval = interval(Duration::from_millis(
            constants::time::AGGREGATION_INTERVAL_MS,
        ));

        while *running.read().await {
            interval.tick().await;

            let local = local_values.read().await;
            let remote = remote_values.read().await;

            // Process each global metric from config
            for (name, config) in &global_config {
                let values = Self::collect_all_values(&local, &remote, &config.sources);

                let result = match config.aggregation.as_str() {
                    "avg" => aggregation::avg(&values),
                    "max" => aggregation::max(&values),
                    "min" => aggregation::min(&values),
                    "sum" => aggregation::sum(&values),
                    "p95" => aggregation::percentile(&mut values.clone(), 95.0),
                    "p99" => aggregation::percentile(&mut values.clone(), 99.0),
                    "p50" | "median" => aggregation::percentile(&mut values.clone(), 50.0),
                    _ => {
                        // Check for expression-based aggregation
                        if let Some(ref expr) = config.expression {
                            Self::compute_expression(&local, &remote, expr, &config.sources)
                        } else {
                            0.0
                        }
                    }
                };

                let mut g = global_values.write().await;
                g.insert(name.clone(), result);
            }
        }

        Ok(())
    }

    /// Collect all values (local + remote) for given sources
    fn collect_all_values<'a>(
        local: &HashMap<String, f64>,
        remote: &HashMap<String, HashMap<u64, f64>>,
        sources: &[String],
    ) -> Vec<f64> {
        let mut values = Vec::new();

        for source in sources {
            // Add local value if present
            if let Some(&v) = local.get(source) {
                values.push(v);
            }

            // Add remote values from all nodes
            if let Some(nodes) = remote.get(source) {
                for &v in nodes.values() {
                    values.push(v);
                }
            }
        }

        values
    }

    /// Compute expression-based aggregation
    fn compute_expression(
        local: &HashMap<String, f64>,
        remote: &HashMap<String, HashMap<u64, f64>>,
        expression: &str,
        sources: &[String],
    ) -> f64 {
        // For expression aggregations, compute local first, then avg across nodes
        let local_value = Self::eval_expression(local, expression);

        // Collect expression values from all nodes
        let mut all_values = vec![local_value];

        // Get remote values for each source and compute
        for source in sources {
            if let Some(nodes) = remote.get(source) {
                for (&_node_id, &value) in nodes {
                    // For now, just collect the source values
                    // A full implementation would evaluate the expression per node
                    all_values.push(value);
                }
            }
        }

        // Return average of collected values as proxy
        // In a full implementation, each node would evaluate the expression
        aggregation::avg(&all_values)
    }

    /// Simple expression evaluator for health_score pattern
    fn eval_expression(local: &HashMap<String, f64>, expression: &str) -> f64 {
        // Handle common patterns like "(100 - cpu) + memory_free"
        if expression.contains("100 - cpu") && expression.contains("memory_free") {
            let cpu = local.get("cpu").copied().unwrap_or(0.0);
            let memory_free = local.get("memory_free").copied().unwrap_or(0.0);
            return (100.0 - cpu) + memory_free;
        }

        // Default: sum of referenced sources
        let mut sum = 0.0;
        for source in ["cpu", "memory", "temperature", "vibration"] {
            if expression.contains(source) {
                if let Some(&v) = local.get(source) {
                    sum += v;
                }
            }
        }
        sum
    }
}

pub async fn start_node(path: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::load(path)?;
    let node = AutoQueuesNode::new(&config).await?;
    node.run(&config).await
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Aggregation function tests (in addition to coverage_tests.rs) ---

    #[test]
    fn test_aggregation_avg_zeros() {
        let values = vec![0.0, 0.0, 0.0];
        assert_eq!(aggregation::avg(&values), 0.0);
    }

    #[test]
    fn test_aggregation_percentile_p0() {
        let mut values = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let p0 = aggregation::percentile(&mut values, 0.0);
        assert_eq!(p0, 10.0);
    }

    #[test]
    fn test_aggregation_percentile_p100() {
        let mut values = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let p100 = aggregation::percentile(&mut values, 100.0);
        assert_eq!(p100, 50.0);
    }

    #[test]
    fn test_aggregation_percentile_with_negative() {
        let mut values = vec![-10.0, -20.0, -30.0, -40.0, -50.0];
        let p50 = aggregation::percentile(&mut values, 50.0);
        assert_eq!(p50, -30.0);
    }

    #[test]
    fn test_aggregation_health_score_various_cpu() {
        // Test various combinations
        assert_eq!(aggregation::health_score(25.0, 75.0), 150.0); // (100-25) + 75 = 150
        assert_eq!(aggregation::health_score(75.0, 25.0), 50.0);  // (100-75) + 25 = 50
        assert_eq!(aggregation::health_score(50.0, 50.0), 100.0); // (100-50) + 50 = 100
    }

    // --- collect_all_values tests ---

    #[test]
    fn test_collect_all_values_empty() {
        let local = HashMap::new();
        let remote = HashMap::new();
        let sources: Vec<String> = vec![];

        let values = AutoQueuesNode::collect_all_values(&local, &remote, &sources);
        assert!(values.is_empty());
    }

    #[test]
    fn test_collect_all_values_local_only() {
        let mut local = HashMap::new();
        local.insert("cpu".to_string(), 50.0);
        local.insert("memory".to_string(), 75.0);

        let remote = HashMap::new();
        let sources = vec!["cpu".to_string()];

        let values = AutoQueuesNode::collect_all_values(&local, &remote, &sources);
        assert_eq!(values.len(), 1);
        assert!(values.contains(&50.0));
    }

    #[test]
    fn test_collect_all_values_multiple_sources() {
        let mut local = HashMap::new();
        local.insert("cpu".to_string(), 40.0);
        local.insert("memory".to_string(), 60.0);

        let remote = HashMap::new();
        let sources = vec!["cpu".to_string(), "memory".to_string()];

        let values = AutoQueuesNode::collect_all_values(&local, &remote, &sources);
        assert_eq!(values.len(), 2);
        assert!(values.contains(&40.0));
        assert!(values.contains(&60.0));
    }

    #[test]
    fn test_collect_all_values_remote_only() {
        let local = HashMap::new();

        let mut remote: HashMap<String, HashMap<u64, f64>> = HashMap::new();
        let mut nodes: HashMap<u64, f64> = HashMap::new();
        nodes.insert(1, 30.0);
        nodes.insert(2, 40.0);
        remote.insert("cpu".to_string(), nodes);

        let sources = vec!["cpu".to_string()];

        let values = AutoQueuesNode::collect_all_values(&local, &remote, &sources);
        assert_eq!(values.len(), 2);
        assert!(values.contains(&30.0));
        assert!(values.contains(&40.0));
    }

    #[test]
    fn test_collect_all_values_local_and_remote() {
        let mut local = HashMap::new();
        local.insert("cpu".to_string(), 50.0);

        let mut remote: HashMap<String, HashMap<u64, f64>> = HashMap::new();
        let mut nodes: HashMap<u64, f64> = HashMap::new();
        nodes.insert(1, 30.0);
        nodes.insert(2, 40.0);
        remote.insert("cpu".to_string(), nodes);

        let sources = vec!["cpu".to_string()];

        let values = AutoQueuesNode::collect_all_values(&local, &remote, &sources);
        assert_eq!(values.len(), 3);
        assert!(values.contains(&50.0));
        assert!(values.contains(&30.0));
        assert!(values.contains(&40.0));
    }

    #[test]
    fn test_collect_all_values_multiple_metrics() {
        let mut local = HashMap::new();
        local.insert("cpu".to_string(), 50.0);
        local.insert("temperature".to_string(), 65.0);

        let mut remote: HashMap<String, HashMap<u64, f64>> = HashMap::new();
        let mut cpu_nodes: HashMap<u64, f64> = HashMap::new();
        cpu_nodes.insert(1, 30.0);
        cpu_nodes.insert(2, 35.0);
        remote.insert("cpu".to_string(), cpu_nodes);

        // Temperature also has node 2's value, which we're including
        let mut temp_nodes: HashMap<u64, f64> = HashMap::new();
        temp_nodes.insert(1, 60.0);
        remote.insert("temperature".to_string(), temp_nodes);

        // Using only cpu as source to get exactly 3 values
        let sources = vec!["cpu".to_string()];

        let values = AutoQueuesNode::collect_all_values(&local, &remote, &sources);
        assert_eq!(values.len(), 3);
        assert!(values.contains(&50.0));
        assert!(values.contains(&30.0));
        assert!(values.contains(&35.0));
    }

    #[test]
    fn test_collect_all_values_missing_source() {
        let mut local = HashMap::new();
        local.insert("cpu".to_string(), 50.0);

        let remote = HashMap::new();
        let sources = vec!["cpu".to_string(), "nonexistent".to_string()];

        let values = AutoQueuesNode::collect_all_values(&local, &remote, &sources);
        assert_eq!(values.len(), 1);
        assert!(values.contains(&50.0));
    }

    // --- compute_expression tests ---

    #[test]
    fn test_compute_expression_basic() {
        let mut local = HashMap::new();
        local.insert("cpu".to_string(), 40.0);
        local.insert("memory_free".to_string(), 60.0);

        let remote = HashMap::new();
        let expression = "(100 - cpu) + memory_free";  // This matches the health_score pattern
        let sources: Vec<String> = vec![];

        let result = AutoQueuesNode::compute_expression(&local, &remote, expression, &sources);

        // compute_expression calls eval_expression locally and averages with remote values
        // local_value = (100 - 40) + 60 = 120
        // all_values = [120]
        // avg([120]) = 120
        assert_eq!(result, 120.0);
    }

    #[test]
    fn test_compute_expression_with_remote_values() {
        let mut local = HashMap::new();
        local.insert("cpu".to_string(), 40.0);

        let mut remote: HashMap<String, HashMap<u64, f64>> = HashMap::new();
        let mut nodes: HashMap<u64, f64> = HashMap::new();
        nodes.insert(1, 30.0);
        nodes.insert(2, 35.0);
        remote.insert("cpu".to_string(), nodes);

        let expression = "cpu + memory";  // Sum pattern
        let sources = vec!["cpu".to_string()];

        let result = AutoQueuesNode::compute_expression(&local, &remote, expression, &sources);

        // Local: only cpu found (40), memory doesn't exist so sum is 40
        // Remote: adds 30 and 35 to values
        // all_values = [40, 30, 35]
        // avg([40, 30, 35]) = 35
        let expected = (40.0 + 30.0 + 35.0) / 3.0;
        assert!((result - expected).abs() < 0.001);
    }

    #[test]
    fn test_compute_expression_empty_sources() {
        let mut local = HashMap::new();
        local.insert("cpu".to_string(), 50.0);

        let remote = HashMap::new();
        let expression = "(100 - cpu) + memory_free";
        let sources: Vec<String> = vec![];

        let result = AutoQueuesNode::compute_expression(&local, &remote, expression, &sources);

        // eval_expression: matches pattern, cpu=50, memory_free=0 (not found)
        // local_value = (100 - 50) + 0 = 50
        // all_values = [50]
        // avg([50]) = 50
        assert_eq!(result, 50.0);
    }

    // --- eval_expression tests ---

    #[test]
    fn test_eval_expression_health_score_pattern() {
        let mut local = HashMap::new();
        local.insert("cpu".to_string(), 40.0);
        local.insert("memory_free".to_string(), 60.0);

        let expression = "(100 - cpu) + memory_free";
        let result = AutoQueuesNode::eval_expression(&local, expression);
        assert_eq!(result, 120.0);
    }

    #[test]
    fn test_eval_expression_health_score_no_cpu() {
        let mut local = HashMap::new();
        local.insert("memory_free".to_string(), 50.0);

        let expression = "(100 - cpu) + memory_free";
        let result = AutoQueuesNode::eval_expression(&local, expression);
        // cpu defaults to 0.0, so (100 - 0) + 50 = 150
        assert_eq!(result, 150.0);
    }

    #[test]
    fn test_eval_expression_sum_sources() {
        let mut local = HashMap::new();
        local.insert("cpu".to_string(), 40.0);
        local.insert("memory".to_string(), 60.0);
        local.insert("temperature".to_string(), 70.0);

        let expression = "cpu + memory + temperature";
        let result = AutoQueuesNode::eval_expression(&local, expression);
        assert_eq!(result, 170.0);
    }

    #[test]
    fn test_eval_expression_partial_sources() {
        let mut local = HashMap::new();
        local.insert("cpu".to_string(), 30.0);

        let expression = "cpu + missing_metric";
        let result = AutoQueuesNode::eval_expression(&local, expression);
        // Only cpu found, missing_metric defaults to 0
        assert_eq!(result, 30.0);
    }

    #[test]
    fn test_eval_expression_empty_local() {
        let local = HashMap::new();
        let expression = "cpu + memory";
        let result = AutoQueuesNode::eval_expression(&local, expression);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_eval_expression_all_sources() {
        let mut local = HashMap::new();
        local.insert("cpu".to_string(), 10.0);
        local.insert("memory".to_string(), 20.0);
        local.insert("temperature".to_string(), 30.0);
        local.insert("vibration".to_string(), 40.0);

        let expression = "cpu + memory + temperature + vibration";
        let result = AutoQueuesNode::eval_expression(&local, expression);
        assert_eq!(result, 100.0);
    }

    #[test]
    fn test_eval_expression_vibration_pattern() {
        let mut local = HashMap::new();
        local.insert("vibration".to_string(), 15.0);

        let expression = "vibration";
        let result = AutoQueuesNode::eval_expression(&local, expression);
        assert_eq!(result, 15.0);
    }

    #[test]
    fn test_eval_expression_temperature_pattern() {
        let mut local = HashMap::new();
        local.insert("temperature".to_string(), 75.0);

        let expression = "temperature";
        let result = AutoQueuesNode::eval_expression(&local, expression);
        assert_eq!(result, 75.0);
    }

    // --- Integration-style tests ---

    #[test]
    fn test_collect_all_values_with_typical_cluster_setup() {
        // Simulate a 3-node cluster
        let mut local = HashMap::new();
        local.insert("cpu".to_string(), 45.0);
        local.insert("memory".to_string(), 60.0);
        local.insert("temperature".to_string(), 65.0);

        let mut remote: HashMap<String, HashMap<u64, f64>> = HashMap::new();

        // Node 1 (local) is already in local
        // Node 2
        let mut node2_values: HashMap<u64, f64> = HashMap::new();
        node2_values.insert(2, 50.0); // cpu from node 2
        remote.insert("cpu".to_string(), node2_values);

        let mut node2_mem: HashMap<u64, f64> = HashMap::new();
        node2_mem.insert(2, 55.0); // memory from node 2
        remote.insert("memory".to_string(), node2_mem);

        let mut node2_temp: HashMap<u64, f64> = HashMap::new();
        node2_temp.insert(2, 70.0); // temperature from node 2
        remote.insert("temperature".to_string(), node2_temp);

        let sources = vec!["cpu".to_string()];

        let values = AutoQueuesNode::collect_all_values(&local, &remote, &sources);
        assert_eq!(values.len(), 2);
        assert!(values.contains(&45.0));
        assert!(values.contains(&50.0));
    }

    #[test]
    fn test_aggregation_integration_with_collected_values() {
        // Test that aggregations work correctly with collected values
        let values = vec![10.0, 20.0, 30.0, 40.0, 50.0];

        assert_eq!(aggregation::avg(&values), 30.0);
        assert_eq!(aggregation::max(&values), 50.0);
        assert_eq!(aggregation::min(&values), 10.0);
        assert_eq!(aggregation::sum(&values), 150.0);

        let mut sorted_values = values.clone();
        let p50 = aggregation::percentile(&mut sorted_values, 50.0);
        assert_eq!(p50, 30.0);
    }
}
