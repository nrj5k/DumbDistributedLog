//! leaderleader - Meta-leader for cluster coordination
//!
//! Responsibilities:
//! - Maintains leader map (derived metric → node leader)
//! - Monitors derived metric freshness
//! - Detects leader failures via timeout
//! - Reassigns leaders on failure
//! - Propagates leader map to all nodes

use crate::cluster::leader_assignment::{DerivedMetric, LeaderMap};
use crate::constants;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::interval;
use zmq::{Context, Socket, SocketType};

#[derive(Debug, thiserror::Error)]
pub enum LeaderLeaderError {
    #[error("Lock error: {0}")]
    LockError(String),
    #[error("ZMQ error: {0}")]
    ZmqError(#[from] zmq::Error),
}

/// LeaderLeader configuration
#[derive(Debug, Clone)]
pub struct LeaderLeaderConfig {
    /// This node's ID
    pub node_id: u64,
    /// ZMQ coordination port
    pub coordination_port: u16,
    /// ZMQ bind address
    pub bind_addr: String,
    /// Freshness timeout (2 × AIMD_max)
    pub freshness_timeout_ms: u64,
    /// Check interval for monitoring
    pub check_interval_ms: u64,
}

impl Default for LeaderLeaderConfig {
    fn default() -> Self {
        Self {
            node_id: 1,
            coordination_port: constants::network::DEFAULT_COORDINATION_PORT,
            bind_addr: "0.0.0.0".to_string(),
            freshness_timeout_ms: constants::time::FRESHNESS_TIMEOUT_MS, // 2 × 5000
            check_interval_ms: constants::time::LEADER_CHECK_INTERVAL_MS, // Check every second
        }
    }
}

impl LeaderLeaderConfig {
    /// Create config from QueueConfig
    pub fn from_queue_config(
        node_id: u64,
        coordination_port: u16,
        freshness_timeout_ms: u64,
    ) -> Self {
        Self {
            node_id,
            coordination_port,
            bind_addr: "0.0.0.0".to_string(),
            freshness_timeout_ms,
            check_interval_ms: constants::time::LEADER_CHECK_INTERVAL_MS,
        }
    }

    /// Get ZMQ bind address
    pub fn zmq_bind_addr(&self) -> String {
        format!("tcp://{}:{}", self.bind_addr, self.coordination_port)
    }
}

/// Derived metric value with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivedMetricValue {
    pub metric_name: String,
    pub value: f64,
    pub timestamp: u64,
    pub leader_id: u64,
}

/// LeaderLeader state
#[derive(Debug, Clone)]
pub struct LeaderLeaderState {
    /// Current leader map
    pub leader_map: LeaderMap,
    /// Derived metrics configuration
    pub derived_metrics: Vec<DerivedMetric>,
    /// Last seen timestamp for each derived metric (as Instant)
    pub metric_timestamps: HashMap<String, Instant>,
    /// Whether this node is the leaderleader
    pub is_leaderleader: bool,
    /// Current term (for Raft-like election)
    pub current_term: u64,
    /// Voted for in current term
    pub voted_for: Option<u64>,
}

impl Default for LeaderLeaderState {
    fn default() -> Self {
        Self {
            leader_map: LeaderMap::new(),
            derived_metrics: Vec::new(),
            metric_timestamps: HashMap::new(),
            is_leaderleader: false,
            current_term: 0,
            voted_for: None,
        }
    }
}

/// leaderleader - Meta-leader for cluster coordination
pub struct LeaderLeader {
    /// Configuration
    config: LeaderLeaderConfig,
    /// Shared state (protected by RwLock)
    state: Arc<RwLock<LeaderLeaderState>>,
    /// ZMQ context
    _context: Context,
    /// ZMQ PUB socket for broadcasting leader map
    broadcast_socket: Socket,
    /// Channel for receiving metric updates
    _metric_rx: mpsc::Receiver<DerivedMetricValue>,
    /// Shutdown signal
    shutdown_rx: mpsc::Receiver<()>,
}

impl LeaderLeader {
    /// Create new leaderleader
    pub fn new(
        config: LeaderLeaderConfig,
        derived_metrics: Vec<DerivedMetric>,
    ) -> Result<Self, zmq::Error> {
        let context = Context::new();

        // Create PUB socket for broadcasting leader map
        let broadcast_socket = context.socket(SocketType::PUB)?;
        let _ = broadcast_socket.set_linger(0);
        broadcast_socket.bind(&config.zmq_bind_addr())?;

        // Create channels
        let (_metric_tx, metric_rx) = mpsc::channel(constants::memory::METRICS_CHANNEL_BUFFER_SIZE);
        let (shutdown_tx, shutdown_rx) =
            mpsc::channel(constants::memory::SMALL_CHANNEL_BUFFER_SIZE);

        let state = Arc::new(RwLock::new(LeaderLeaderState {
            leader_map: LeaderMap::new(),
            derived_metrics: derived_metrics.clone(),
            metric_timestamps: HashMap::new(),
            is_leaderleader: false,
            current_term: 0,
            voted_for: None,
        }));

        // Note: _metric_tx and shutdown_tx are dropped (used for creation only)
        let _ = shutdown_tx;

        Ok(Self {
            config,
            state,
            _context: context,
            broadcast_socket,
            _metric_rx: metric_rx,
            shutdown_rx,
        })
    }

    /// Run the leaderleader
    pub async fn run(&mut self) {
        let mut check_interval = interval(Duration::from_millis(self.config.check_interval_ms));

        loop {
            tokio::select! {
                _ = check_interval.tick() => {
                    self.check_freshness().await;
                }
                _ = self.shutdown_rx.recv() => {
                    println!("LeaderLeader shutting down");
                    break;
                }
            }
        }
    }

    /// Check freshness of derived metrics
    async fn check_freshness(&mut self) {
        let now = Instant::now();
        let timeout = Duration::from_millis(self.config.freshness_timeout_ms);

        // Clone needed data to avoid borrow issues
        let derived_metrics: Vec<DerivedMetric> = match self.state.read() {
            Ok(state) => state.derived_metrics.clone(),
            Err(e) => {
                eprintln!("Lock error reading derived metrics: {}", e);
                return;
            }
        };

        let stale_metrics: Vec<String> = match self.state.read() {
            Ok(state) => {
                let mut stale = Vec::new();
                for metric in &derived_metrics {
                    if let Some(last_instant) = state.metric_timestamps.get(&metric.name) {
                        let elapsed = now.duration_since(*last_instant);
                        if elapsed > timeout {
                            stale.push(metric.name.clone());
                        }
                    } else {
                        stale.push(metric.name.clone());
                    }
                }
                stale
            }
            Err(e) => {
                eprintln!("Lock error reading metric timestamps: {}", e);
                Vec::new()
            }
        };

        // Handle stale metrics
        for metric_name in stale_metrics {
            let has_leader = match self.state.read() {
                Ok(state) => state.leader_map.get_leader(&metric_name).is_some(),
                Err(e) => {
                    eprintln!("Lock error checking leader: {}", e);
                    false
                }
            };

            if has_leader {
                self.handle_stale_metric(&metric_name);
            } else {
                self.assign_initial_leader(&metric_name);
            }
        }
    }

    /// Handle a stale derived metric
    fn handle_stale_metric(&mut self, metric_name: &str) {
        let current_leader = match self.state.read() {
            Ok(state) => state.leader_map.get_leader(metric_name),
            Err(e) => {
                eprintln!("Lock error reading current leader: {}", e);
                None
            }
        };

        if let Some(leader_id) = current_leader {
            println!(
                "Leader {} for '{}' may have failed, reassigning...",
                leader_id, metric_name
            );

            // Reassign to a different node
            let new_leader = self.select_new_leader(metric_name, leader_id);

            if let Some(new_id) = new_leader {
                match self.state.write() {
                    Ok(mut state) => {
                        state.leader_map.set_leader(metric_name.to_string(), new_id);
                    }
                    Err(e) => {
                        eprintln!("Lock error writing leader assignment: {}", e);
                    }
                }
                println!(
                    "Reassigned '{}' from leader {} to leader {}",
                    metric_name, leader_id, new_id
                );

                // Broadcast the updated leader map
                self.broadcast_leader_map();
            }
        }
    }

    /// Assign initial leader for a metric
    fn assign_initial_leader(&mut self, metric_name: &str) {
        let metric_index = match self.state.read() {
            Ok(state) => state
                .derived_metrics
                .iter()
                .position(|m| &m.name == metric_name)
                .unwrap_or(0),
            Err(e) => {
                eprintln!("Lock error reading derived metrics for index: {}", e);
                0
            }
        };

        let node_count = match self.state.read() {
            Ok(state) => state.leader_map.assignments().len().max(1),
            Err(e) => {
                eprintln!("Lock error reading node count: {}", e);
                1
            }
        };

        let node_id = ((metric_index % node_count) + 1) as u64;

        {
            match self.state.write() {
                Ok(mut state) => {
                    state
                        .leader_map
                        .set_leader(metric_name.to_string(), node_id);
                }
                Err(e) => {
                    eprintln!("Lock error writing initial leader assignment: {}", e);
                }
            }
        }
        println!("Assigned initial leader {} for '{}'", node_id, metric_name);

        // Broadcast the updated leader map
        self.broadcast_leader_map();
    }

    /// Select a new leader for a metric (excluding current leader)
    fn select_new_leader(&self, _metric_name: &str, exclude_node: u64) -> Option<u64> {
        let assignments: Vec<(String, u64)> = match self.state.read() {
            Ok(state) => state
                .leader_map
                .assignments()
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect(),
            Err(e) => {
                eprintln!("Lock error reading assignments: {}", e);
                Vec::new()
            }
        };

        // Collect all unique node IDs from assignments
        let mut nodes: Vec<u64> = assignments.iter().map(|(_, v)| *v).collect();
        nodes.sort();
        nodes.dedup();

        // Remove the excluded node
        nodes.retain(|&n| n != exclude_node);

        if nodes.is_empty() {
            // No other nodes, assign to next available
            Some((exclude_node % 4) + 1) // Simple fallback
        } else {
            // Return first available node
            Some(nodes[0])
        }
    }

    /// Broadcast leader map to all nodes
    fn broadcast_leader_map(&self) {
        let leader_map = match self.state.read() {
            Ok(state) => state.leader_map.clone(),
            Err(e) => {
                eprintln!("Lock error reading leader map: {}", e);
                return;
            }
        };

        let leader_map_json = match serde_json::to_string(&leader_map) {
            Ok(json) => json,
            Err(e) => {
                eprintln!("Failed to serialize leader map: {}", e);
                return;
            }
        };

        let topic = "leaderleader.leader_map";
        let mut message = Vec::new();
        message.extend_from_slice(topic.as_bytes());
        message.push(0); // Separator
        message.extend_from_slice(leader_map_json.as_bytes());

        // Send broadcast (fire and forget, errors are logged)
        if let Err(e) = self.broadcast_socket.send(&message, zmq::DONTWAIT) {
            println!("Failed to broadcast leader map: {}", e);
        }
    }

    /// Update metric timestamp (called when receiving metric update)
    pub fn update_metric_timestamp(&mut self, metric_name: String) {
        match self.state.write() {
            Ok(mut state) => {
                state.metric_timestamps.insert(metric_name, Instant::now());
            }
            Err(e) => {
                eprintln!("Lock error updating metric timestamp: {}", e);
            }
        }
    }

    /// Get current leader map
    pub fn leader_map(&self) -> LeaderMap {
        match self.state.read() {
            Ok(state) => state.leader_map.clone(),
            Err(e) => {
                eprintln!("Lock error reading leader map: {}", e);
                LeaderMap::new()
            }
        }
    }

    /// Check if this node is leaderleader
    pub fn is_leaderleader(&self) -> bool {
        match self.state.read() {
            Ok(state) => state.is_leaderleader,
            Err(e) => {
                eprintln!("Lock error checking leaderleader status: {}", e);
                false
            }
        }
    }

    /// Become leaderleader (simple election for now)
    pub fn become_leaderleader(&mut self) {
        match self.state.write() {
            Ok(mut state) => {
                state.is_leaderleader = true;
                state.current_term += 1;
                state.voted_for = Some(self.config.node_id);
                println!(
                    "Node {} became leaderleader (term {})",
                    self.config.node_id, state.current_term
                );
            }
            Err(e) => {
                eprintln!("Lock error becoming leaderleader: {}", e);
            }
        }
    }

    /// Step down as leaderleader
    pub fn step_down(&mut self) {
        match self.state.write() {
            Ok(mut state) => {
                state.is_leaderleader = false;
                println!("Node {} stepped down as leaderleader", self.config.node_id);
            }
            Err(e) => {
                eprintln!("Lock error stepping down: {}", e);
            }
        }
    }
}

/// Start leaderleader with config derived from QueueConfig
pub async fn start_leaderleader(
    node_id: u64,
    derived_metrics: Vec<DerivedMetric>,
    coordination_port: u16,
    freshness_timeout_ms: u64,
) -> Result<LeaderLeader, zmq::Error> {
    let config =
        LeaderLeaderConfig::from_queue_config(node_id, coordination_port, freshness_timeout_ms);

    let mut leaderleader = LeaderLeader::new(config, derived_metrics)?;
    leaderleader.become_leaderleader();

    Ok(leaderleader)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leaderleader_config() {
        let config = LeaderLeaderConfig::default();
        assert_eq!(config.node_id, 1);
        assert_eq!(config.coordination_port, 6968);
        assert_eq!(config.freshness_timeout_ms, 10000);
    }

    #[test]
    fn test_leaderleader_state_default() {
        let state = LeaderLeaderState::default();
        assert!(!state.is_leaderleader);
        assert_eq!(state.current_term, 0);
        assert!(state.voted_for.is_none());
        assert!(state.leader_map.assignments().is_empty());
    }

    #[test]
    fn test_derived_metric_value() {
        let value = DerivedMetricValue {
            metric_name: "nvme_drive_capacity".to_string(),
            value: 1792.0,
            timestamp: 1234567890,
            leader_id: 1,
        };

        assert_eq!(value.metric_name, "nvme_drive_capacity");
        assert_eq!(value.value, 1792.0);
        assert_eq!(value.leader_id, 1);
    }
}
