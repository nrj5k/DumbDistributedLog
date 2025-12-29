//! LeaderMapClient - Get leader map via consensus
//!
//! Queries multiple nodes and returns leader map based on majority consensus.

use crate::cluster::leader_assignment::LeaderMap;
use crate::constants;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio::time::{Duration, timeout};

/// Configuration for consensus
#[derive(Debug, Clone)]
pub struct ConsensusConfig {
    /// Number of nodes to query (odd number, 3 minimum)
    pub consensus_count: usize,
    /// Timeout per query in milliseconds
    pub query_timeout_ms: u64,
    /// Coordination port for leader map queries
    pub coordination_port: u16,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            consensus_count: constants::system::CONSENSUS_COUNT,
            query_timeout_ms: constants::time::QUERY_TIMEOUT_MS,
            coordination_port: constants::network::DEFAULT_COORDINATION_PORT,
        }
    }
}

/// Leader map request/response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderMapRequest {
    pub request_type: String, // "get_leader_map"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderMapResponse {
    pub leader_map: LeaderMap,
    pub node_id: u64,
    pub timestamp: u64,
}

/// LeaderMapClient - Gets leader map via consensus
pub struct LeaderMapClient {
    config: ConsensusConfig,
    /// Known cluster nodes (hostname -> SocketAddr)
    nodes: HashMap<String, SocketAddr>,
}

impl LeaderMapClient {
    /// Create new client
    pub fn new(config: ConsensusConfig, nodes: HashMap<String, SocketAddr>) -> Self {
        Self { config, nodes }
    }

    /// Query a single node for leader map
    async fn query_node(&self, addr: SocketAddr) -> Result<LeaderMapResponse, String> {
        let request = LeaderMapRequest {
            request_type: "get_leader_map".to_string(),
        };

        let data =
            serde_json::to_string(&request).map_err(|e| format!("Serialize error: {}", e))?;

        let timeout_duration = Duration::from_millis(self.config.query_timeout_ms);

        match timeout(timeout_duration, TcpStream::connect(addr)).await {
            Ok(Ok(mut stream)) => {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};

                // Send request
                stream
                    .write_all(data.as_bytes())
                    .await
                    .map_err(|e| format!("Write error: {}", e))?;

                // Read response
                let mut response = Vec::new();
                stream
                    .read_to_end(&mut response)
                    .await
                    .map_err(|e| format!("Read error: {}", e))?;

                let response: LeaderMapResponse = serde_json::from_slice(&response)
                    .map_err(|e| format!("Deserialize error: {}", e))?;

                Ok(response)
            }
            Ok(Err(e)) => Err(format!("Connection error: {}", e)),
            Err(_) => Err("Timeout".to_string()),
        }
    }

    /// Get leader map via consensus
    /// Asks N nodes (config.consensus_count), returns map from majority
    pub async fn get_leader_map(&self) -> Result<LeaderMap, String> {
        if self.nodes.is_empty() {
            return Err("No known nodes".to_string());
        }

        // Collect node addresses
        let addrs: Vec<SocketAddr> = self.nodes.values().cloned().collect();
        let query_count = self.config.consensus_count.min(addrs.len());

        // Query N random nodes
        let mut handles = Vec::new();
        for i in 0..query_count {
            let addr = addrs[i];
            let client = self.clone();
            handles.push(tokio::spawn(async move { client.query_node(addr).await }));
        }

        // Collect responses
        let mut responses: Vec<LeaderMapResponse> = Vec::new();
        for handle in handles {
            if let Ok(Ok(response)) = handle.await {
                responses.push(response);
            }
        }

        if responses.is_empty() {
            return Err("No responses from any node".to_string());
        }

        // Count votes for each leader map version
        let mut version_counts: HashMap<u64, usize> = HashMap::new();
        for response in &responses {
            let version = response.leader_map.version();
            *version_counts.entry(version).or_insert(0) += 1;
        }

        // Find majority version
        let majority_entry = version_counts
            .iter()
            .max_by_key(|item| item.1)
            .ok_or_else(|| "No versions found".to_string())?;
        let majority_version = *majority_entry.0;

        // Find the response with majority version
        let majority_response = responses
            .iter()
            .find(|r| r.leader_map.version() == majority_version)
            .ok_or_else(|| "Majority response not found".to_string())?;

        Ok(majority_response.leader_map.clone())
    }

    /// Get leader ID for a specific metric via consensus
    pub async fn get_leader_for_metric(&self, metric: &str) -> Result<u64, String> {
        let leader_map = self.get_leader_map().await?;
        leader_map
            .get_leader(metric)
            .ok_or_else(|| format!("No leader for metric '{}'", metric))
    }
}

impl Clone for LeaderMapClient {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            nodes: self.nodes.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consensus_config_default() {
        let config = ConsensusConfig::default();
        assert_eq!(config.consensus_count, 3);
        assert_eq!(config.query_timeout_ms, 1000);
        assert_eq!(config.coordination_port, 6968);
    }
}
