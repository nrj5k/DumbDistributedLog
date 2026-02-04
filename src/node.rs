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
mod aggregation {

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
        let other_nodes: Vec<SocketAddr> = config
            .other_nodes(node_id)
            .into_iter()
            .map(|(_, cfg)| cfg.pubsub_addr())
            .collect();

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
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_millis() as u64;

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
