//! Leader - Derived metric aggregation and serving

use crate::cluster::leader_assignment::DerivedMetric;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::interval;
use zmq::{Context, Socket, SocketType};

#[derive(Debug, Clone)]
pub struct LeaderConfig {
    pub node_id: u64,
    pub data_port: u16,
    pub query_port: u16,
    pub bind_addr: String,
    pub aggregation_interval_ms: u64,
    pub peer_nodes: Vec<String>,
}

impl Default for LeaderConfig {
    fn default() -> Self {
        Self {
            node_id: 1,
            data_port: 6966,
            query_port: 6969,
            bind_addr: "0.0.0.0".to_string(),
            aggregation_interval_ms: 1000,
            peer_nodes: Vec::new(),
        }
    }
}

impl LeaderConfig {
    pub fn zmq_query_addr(&self) -> String {
        format!("tcp://{}:{}", self.bind_addr, self.query_port)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalMetricValue {
    pub metric_name: String,
    pub node_id: u64,
    pub value: f64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedMetricValue {
    pub metric_name: String,
    pub value: f64,
    pub aggregation_type: String,
    pub source_count: usize,
    pub timestamp: u64,
    pub leader_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AggregationType {
    Sum, Average, Max, Min,
}

impl AggregationType {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "sum" => AggregationType::Sum,
            "average" | "avg" => AggregationType::Average,
            "max" => AggregationType::Max,
            "min" => AggregationType::Min,
            _ => AggregationType::Sum,
        }
    }

    pub fn aggregate(&self, values: &[f64]) -> f64 {
        if values.is_empty() { return 0.0; }
        match self {
            AggregationType::Sum => values.iter().sum(),
            AggregationType::Average => values.iter().sum::<f64>() / values.len() as f64,
            AggregationType::Max => values.iter().fold(f64::MIN, |a, &b| a.max(b)),
            AggregationType::Min => values.iter().fold(f64::MAX, |a, &b| a.min(b)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LeaderState {
    pub derived_metrics: Vec<DerivedMetric>,
    pub local_values: HashMap<String, Vec<(u64, f64, Instant)>>,
    pub aggregated_values: HashMap<String, AggregatedMetricValue>,
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

pub struct Leader {
    config: LeaderConfig,
    state: Arc<RwLock<LeaderState>>,
    _context: Context,
    publish_socket: Socket,
    _subscribe_socket: Socket,  // Keep alive for peer connections
    shutdown_rx: mpsc::Receiver<()>,
}

impl Leader {
    pub fn new(config: LeaderConfig, derived_metrics: Vec<DerivedMetric>) -> Result<Self, zmq::Error> {
        let context = Context::new();

        let subscribe_socket = context.socket(SocketType::SUB)?;
        let _ = subscribe_socket.set_linger(0);
        let _ = subscribe_socket.set_rcvtimeo(1000);
        let _ = subscribe_socket.set_subscribe(b"atomic.");

        for peer in &config.peer_nodes {
            let peer_addr = format!("tcp://{}", peer);
            let _ = subscribe_socket.connect(&peer_addr);
        }

        let publish_socket = context.socket(SocketType::PUB)?;
        let _ = publish_socket.set_linger(0);
        let publish_addr = format!("tcp://{}:{}", config.bind_addr, config.data_port + 100);
        publish_socket.bind(&publish_addr)?;

        let (_shutdown_tx, shutdown_rx) = mpsc::channel(1);

        let state = Arc::new(RwLock::new(LeaderState {
            derived_metrics: derived_metrics.clone(),
            local_values: HashMap::new(),
            aggregated_values: HashMap::new(),
            last_aggregation: None,
        }));

        Ok(Self {
            config,
            state,
            _context: context,
            publish_socket,
            _subscribe_socket: subscribe_socket,
            shutdown_rx,
        })
    }

    pub async fn run(&mut self) {
        let mut aggregation_interval = interval(Duration::from_millis(self.config.aggregation_interval_ms));

        let query_addr = self.config.zmq_query_addr();
        let state = self.state.clone();
        let query_task = tokio::task::spawn_blocking(move || {
            let ctx = zmq::Context::new();
            let socket = ctx.socket(zmq::REP).expect("Failed to create query socket");
            let _ = socket.set_linger(0);
            if let Err(e) = socket.bind(&query_addr) {
                eprintln!("Failed to bind query socket: {}", e);
                return;
            }
            Self::run_query_handler(socket, state);
        });

        let sub_state = self.state.clone();
        let peers = self.config.peer_nodes.clone();
        let sub_task = tokio::task::spawn_blocking(move || {
            Self::run_subscription_handler(peers, sub_state);
        });

        loop {
            tokio::select! {
                _ = aggregation_interval.tick() => {
                    self.aggregate_all().await;
                }
                _ = self.shutdown_rx.recv() => {
                    query_task.abort();
                    sub_task.abort();
                    break;
                }
            }
        }
    }

    fn run_query_handler(mut socket: Socket, state: Arc<RwLock<LeaderState>>) {
        loop {
            match socket.recv_string(0) {
                Ok(Ok(request)) => {
                    let response = if request.starts_with("get_metric:") {
                        let metric = request.trim_start_matches("get_metric:");
                        let guard = state.read().unwrap();
                        match guard.aggregated_values.get(metric) {
                            Some(v) => format!("OK:{}", v.value),
                            None => "NOT_FOUND".to_string(),
                        }
                    } else {
                        "INVALID_REQUEST".to_string()
                    };
                    let _ = socket.send(&response, 0);
                }
                Ok(Err(_)) => {
                    // Received binary data
                    let _ = socket.send("BINARY_DATA", 0);
                }
                Err(_) => std::thread::sleep(Duration::from_millis(100)),
            }
        }
    }

    fn run_subscription_handler(peers: Vec<String>, state: Arc<RwLock<LeaderState>>) {
        // Create SUB socket for receiving atomic metrics
        let ctx = zmq::Context::new();
        let socket = ctx.socket(SocketType::SUB).unwrap();
        let _ = socket.set_linger(0);
        let _ = socket.set_rcvtimeo(100);
        let _ = socket.set_subscribe(b"atomic.");

        for peer in &peers {
            let peer_addr = format!("tcp://{}", peer);
            let _ = socket.connect(&peer_addr);
        }

        loop {
            match socket.recv_string(0) {
                Ok(Ok(msg)) => {
                    let parts: Vec<&str> = msg.split(':').collect();
                    if parts.len() == 3 {
                        let metric = parts[0].to_string();
                        if let Ok(node_id) = parts[1].parse::<u64>() {
                            if let Ok(value) = parts[2].parse::<f64>() {
                                let mut guard = state.write().unwrap();
                                let entry = guard.local_values.entry(metric).or_insert_with(Vec::new);
                                entry.push((node_id, value, Instant::now()));
                                if entry.len() > 100 {
                                    entry.drain(0..(entry.len() - 100));
                                }
                            }
                        }
                    }
                }
                Ok(Err(_)) => {}
                Err(zmq::Error::EAGAIN) => std::thread::sleep(Duration::from_millis(10)),
                Err(_) => std::thread::sleep(Duration::from_millis(100)),
            }
        }
    }

    async fn aggregate_all(&mut self) {
        let now = Instant::now();
        let derived_metrics: Vec<DerivedMetric> = {
            let state = self.state.read().unwrap();
            state.derived_metrics.clone()
        };

        {
            let mut state = self.state.write().unwrap();
            for metric in &derived_metrics {
                self.aggregate_metric(&metric, &mut state);
            }
            state.last_aggregation = Some(now);
        }

        self.publish_all();
    }

    fn aggregate_metric(&self, metric: &DerivedMetric, state: &mut LeaderState) {
        let mut all_values: Vec<f64> = Vec::new();
        for source in &metric.sources {
            if let Some(values) = state.local_values.get(source) {
                for &(_, value, _) in values {
                    all_values.push(value);
                }
            }
        }

        let agg_type = AggregationType::from_str(&metric.aggregation);
        let aggregated_value = agg_type.aggregate(&all_values);

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

    fn publish_all(&self) {
        let values: HashMap<String, AggregatedMetricValue> = {
            let state = self.state.read().unwrap();
            state.aggregated_values.clone()
        };

        for (_, value) in &values {
            let topic = format!("derived.{}", value.metric_name);
            let data = serde_json::to_string(value).unwrap();
            let mut msg = Vec::new();
            msg.extend_from_slice(topic.as_bytes());
            msg.push(0);
            msg.extend_from_slice(data.as_bytes());
            let _ = self.publish_socket.send(&msg, zmq::DONTWAIT);
        }
    }

    pub fn update_local_value(&mut self, value: LocalMetricValue) {
        let mut state = self.state.write().unwrap();
        let entry = state.local_values.entry(value.metric_name.clone()).or_insert_with(Vec::new);
        entry.push((value.node_id, value.value, Instant::now()));
        if entry.len() > 10 {
            entry.drain(0..(entry.len() - 10));
        }
    }

    pub fn get_aggregated_value(&self, metric_name: &str) -> Option<f64> {
        let state = self.state.read().unwrap();
        state.aggregated_values.get(metric_name).map(|v| v.value)
    }

    pub fn state(&self) -> &Arc<RwLock<LeaderState>> {
        &self.state
    }
}

pub async fn start_leader(
    node_id: u64,
    derived_metrics: Vec<DerivedMetric>,
    data_port: u16,
    query_port: u16,
    aggregation_interval_ms: u64,
    peer_nodes: Vec<String>,
) -> Result<Leader, zmq::Error> {
    let config = LeaderConfig {
        node_id, data_port, query_port,
        bind_addr: "0.0.0.0".to_string(),
        aggregation_interval_ms,
        peer_nodes,
    };
    Leader::new(config, derived_metrics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregation_sum() {
        assert_eq!(AggregationType::Sum.aggregate(&[1.0, 2.0, 3.0]), 6.0);
        assert_eq!(AggregationType::Sum.aggregate(&[]), 0.0);
    }

    #[test]
    fn test_aggregation_average() {
        assert_eq!(AggregationType::Average.aggregate(&[1.0, 2.0, 3.0]), 2.0);
        assert_eq!(AggregationType::Average.aggregate(&[]), 0.0);
    }

    #[test]
    fn test_leader_config() {
        let config = LeaderConfig::default();
        assert_eq!(config.node_id, 1);
        assert_eq!(config.data_port, 6966);
        assert_eq!(config.query_port, 6969);
    }
}
