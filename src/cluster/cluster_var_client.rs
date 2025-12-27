//! ClusterVarClient - Query cluster variables from leaders
//!
//! Sends REQ/REP queries to leaders to get aggregated metric values.

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;
use zmq::{Context, Socket, SocketType};

/// Configuration for cluster variable queries
#[derive(Debug, Clone)]
pub struct ClusterVarClientConfig {
    /// Query timeout in milliseconds
    pub query_timeout_ms: u64,
    /// Default query port for leaders
    pub default_query_port: u16,
}

impl Default for ClusterVarClientConfig {
    fn default() -> Self {
        Self {
            query_timeout_ms: 1000,
            default_query_port: 6969,
        }
    }
}

/// Cluster variable query request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterVarRequest {
    pub metric: String,
}

/// Cluster variable query response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterVarResponse {
    pub metric: String,
    pub value: f64,
    pub found: bool,
    pub error: Option<String>,
}

/// ClusterVarClient - Queries cluster variables from leaders
pub struct ClusterVarClient {
    config: ClusterVarClientConfig,
    /// ZMQ context for creating REQ sockets
    context: Context,
}

impl ClusterVarClient {
    /// Create new client
    pub fn new(config: ClusterVarClientConfig) -> Self {
        let context = Context::new();
        Self { config, context }
    }

    /// Query a single cluster variable from a leader
    pub async fn query_metric(
        &self,
        leader_addr: SocketAddr,
        metric: &str,
    ) -> Result<f64, String> {
        let request = format!("get_metric:{}", metric);
        let timeout_duration = Duration::from_millis(self.config.query_timeout_ms);

        // Use tokio TcpStream for async networking
        match timeout(timeout_duration, TcpStream::connect(leader_addr)).await {
            Ok(Ok(mut stream)) => {
                use tokio::io::{AsyncWriteExt, AsyncReadExt};

                // Send request
                stream.write_all(request.as_bytes()).await
                    .map_err(|e| format!("Write error: {}", e))?;

                // Read response
                let mut response = Vec::new();
                stream.read_to_end(&mut response).await
                    .map_err(|e| format!("Read error: {}", e))?;

                let response_str = String::from_utf8(response)
                    .map_err(|e| format!("Invalid UTF-8: {}", e))?;

                // Parse response: "OK:<value>" or "NOT_FOUND"
                if response_str.starts_with("OK:") {
                    let value_str = response_str.trim_start_matches("OK:");
                    value_str.parse::<f64>()
                        .map_err(|e| format!("Invalid float: {}", e))
                } else {
                    Err(format!("Leader responded: {}", response_str))
                }
            }
            Ok(Err(e)) => Err(format!("Connection error: {}", e)),
            Err(_) => Err("Query timeout".to_string()),
        }
    }

    /// Query a single cluster variable using ZMQ REQ socket (blocking)
    pub fn query_metric_sync(&self, leader_addr: SocketAddr, metric: &str) -> Result<f64, String> {
        let request = format!("get_metric:{}", metric);

        // Create REQ socket
        let socket = self.context.socket(SocketType::REQ)
            .map_err(|e| format!("Failed to create socket: {}", e))?;
        let _ = socket.set_linger(0);

        // Connect to leader
        let addr_str = format!("tcp://{}", leader_addr);
        socket.connect(&addr_str)
            .map_err(|e| format!("Failed to connect: {}", e))?;

        // Send request
        socket.send(&request, 0)
            .map_err(|e| format!("Send error: {}", e))?;

        // Receive response
        let response = socket.recv_string(0)
            .map_err(|e| format!("Receive error: {}", e))?;

        // Parse response: "OK:<value>" or "NOT_FOUND"
        match response {
            Ok(response_str) => {
                if response_str.starts_with("OK:") {
                    let value_str = response_str.trim_start_matches("OK:");
                    value_str.parse::<f64>()
                        .map_err(|e| format!("Invalid float: {}", e))
                } else {
                    Err(format!("Leader responded: {}", response_str))
                }
            }
            Err(bytes) => {
                Err(format!("Received binary data ({} bytes): {:?}", bytes.len(), &bytes[..bytes.len().min(16)]))
            }
        }
    }

    /// Query multiple cluster variables from the same leader
    pub fn query_metrics_sync(
        &self,
        leader_addr: SocketAddr,
        metrics: &[&str],
    ) -> Result<Vec<(String, f64)>, String> {
        let mut results = Vec::new();
        for &metric in metrics {
            match self.query_metric_sync(leader_addr, metric) {
                Ok(value) => results.push((metric.to_string(), value)),
                Err(e) => return Err(format!("Failed to query {}: {}", metric, e)),
            }
        }
        Ok(results)
    }

    /// Build cluster_vars HashMap from multiple leaders
    /// Takes a mapping of metric -> (leader_id, leader_addr)
    pub fn query_cluster_vars(
        &self,
        metric_leaders: &[(String, SocketAddr)],
    ) -> Result<std::collections::HashMap<String, f64>, String> {
        let mut cluster_vars = std::collections::HashMap::new();

        // Group metrics by leader to minimize connections
        let mut metrics_by_leader: std::collections::HashMap<SocketAddr, Vec<String>> =
            std::collections::HashMap::new();

        for (metric, addr) in metric_leaders {
            metrics_by_leader.entry(*addr)
                .or_insert_with(Vec::new)
                .push(metric.clone());
        }

        // Query each leader
        for (addr, metrics) in metrics_by_leader {
            let metric_refs: Vec<&str> = metrics.iter().map(|s| s.as_str()).collect();
            match self.query_metrics_sync(addr, &metric_refs) {
                Ok(values) => {
                    for (metric, value) in values {
                        cluster_vars.insert(metric, value);
                    }
                }
                Err(e) => {
                    // Log but continue - partial results are OK
                    eprintln!("Warning: Failed to query leader {}: {}", addr, e);
                }
            }
        }

        Ok(cluster_vars)
    }
}

impl Default for ClusterVarClient {
    fn default() -> Self {
        Self::new(ClusterVarClientConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cluster_var_client_config_default() {
        let config = ClusterVarClientConfig::default();
        assert_eq!(config.query_timeout_ms, 1000);
        assert_eq!(config.default_query_port, 6969);
    }

    #[test]
    fn test_cluster_var_request_serialization() {
        let request = ClusterVarRequest {
            metric: "avg_cpu".to_string(),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("avg_cpu"));
    }

    #[test]
    fn test_cluster_var_response_serialization() {
        let response = ClusterVarResponse {
            metric: "avg_cpu".to_string(),
            value: 75.5,
            found: true,
            error: None,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("75.5"));
    }
}
