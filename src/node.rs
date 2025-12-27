//! AutoQueues - Fire-and-forget distributed node
//!
//! Usage: AutoQueuesNode::from_config("config.toml")?.run().await;

use crate::cluster::{DerivedMetric, Leader, LeaderConfig};
use crate::config::QueueConfig;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;
use zmq::{Context, SocketType};

pub struct AutoQueuesNode {
    config: QueueConfig,
    node_id: u64,
    context: Context,
    local_values: Arc<RwLock<HashMap<String, f64>>>,
    running: Arc<RwLock<bool>>,
}

impl AutoQueuesNode {
    pub fn from_config(path: &str) -> Result<Self, String> {
        let config = QueueConfig::from_file(path)
            .map_err(|e| format!("Failed to load {}: {}", path, e))?;

        let hostname = std::env::var("HOSTNAME")
            .unwrap_or_else(|_| "server1".to_string());
        
        let node_id = config.hostname_to_node_id(&hostname);
        let actual_id = if node_id > 0 { node_id } else { 1 };

        let context = Context::new();

        Ok(Self {
            config,
            node_id: actual_id,
            context,
            local_values: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
        })
    }

    pub async fn run(&self) {
        println!("[AutoQueues] Node {} starting...", self.node_id);
        *self.running.write().await = true;

        let derived = self.build_derived_metrics();
        let data_port = self.config.get_data_port();
        
        let running = self.running.clone();
        let values = self.local_values.clone();

        // Create publisher in this scope
        let ctx = self.context.clone();
        let publish_addr = format!("tcp://0.0.0.0:{}", data_port);
        
        // Metrics task with its own publisher
        tokio::spawn(async move {
            let publisher = ctx.socket(SocketType::PUB).unwrap();
            let _ = publisher.set_linger(0);
            let _ = publisher.bind(&publish_addr);
            
            let mut interval = interval(Duration::from_millis(1000));
            while *running.read().await {
                let cpu = 50.0 + (chrono::Utc::now().timestamp() as f64 * 0.1).sin() * 20.0;
                
                {
                    let mut guard = values.write().await;
                    guard.insert("cpu_percent".to_string(), cpu);
                }
                
                let msg = format!("atomic.cpu_percent:{}:{}", 1, cpu);
                let _ = publisher.send(&msg, zmq::DONTWAIT);
                
                interval.tick().await;
            }
        });

        // Leader tasks
        let running2 = self.running.clone();
        let values2 = self.local_values.clone();
        let query_port = self.config.get_query_port();
        let peer_nodes: Vec<String> = self.config.get_cluster_nodes().iter().map(|s| s.clone()).collect();

        for (i, dm) in derived.iter().enumerate() {
            let lc = LeaderConfig {
                node_id: self.node_id,
                data_port,
                query_port,
                bind_addr: "0.0.0.0".to_string(),
                aggregation_interval_ms: 500,
                peer_nodes: peer_nodes.clone(),
            };
            
            let leader = Leader::new(lc, vec![dm.clone()]).unwrap();
            let running3 = running2.clone();
            let values3 = values2.clone();
            let node_id = self.node_id;
            let metric_name = dm.name.clone();
            let ctx3 = self.context.clone();

            tokio::spawn(async move {
                // Create sockets for this leader
                let publisher = ctx3.socket(SocketType::PUB).unwrap();
                let _ = publisher.bind(&format!("tcp://0.0.0.0:{}", data_port + 100 + i as u16));
                
                let mut interval = interval(Duration::from_millis(500));
                let mut leader = leader;
                
                while *running3.read().await {
                    let guard = values3.read().await;
                    if let Some(&cpu) = guard.get("cpu_percent") {
                        leader.update_local_value(crate::cluster::leader::LocalMetricValue {
                            metric_name: "atomic.cpu_percent".to_string(),
                            node_id,
                            value: cpu,
                            timestamp: 0,
                        });
                    }
                    drop(guard);
                    interval.tick().await;
                }
                
                leader.run().await;
            });
            
            println!("[AutoQueues] Leader for: {}", dm.name);
        }

        println!("[AutoQueues] Node {} running", self.node_id);
        println!("  - Publish: tcp://0.0.0.0:{}", data_port);
        println!("  - Query:  tcp://0.0.0.0:{}", query_port);

        std::future::pending().await
    }

    pub async fn stop(&self) {
        *self.running.write().await = false;
    }

    pub async fn metrics(&self) -> HashMap<String, f64> {
        self.local_values.read().await.clone()
    }

    fn build_derived_metrics(&self) -> Vec<DerivedMetric> {
        let mut derived = Vec::new();
        if let Some(aq) = &self.config.autoqueues {
            for (name, dm) in &aq.derived_metrics {
                derived.push(DerivedMetric {
                    name: name.clone(),
                    aggregation: dm.aggregation.clone(),
                    sources: dm.sources.clone(),
                });
            }
        }
        derived
    }
}

pub async fn start_node(config_path: &str) -> Result<(), String> {
    let node = AutoQueuesNode::from_config(config_path)?;
    tokio::spawn(async move {
        node.run().await;
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_node_creation() {}
}
