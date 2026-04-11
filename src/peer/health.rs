//! Peer health monitoring for the DDL distributed system.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::interval;

use super::{config::PeerConfig, registry::PeerRegistry};

/// Health status of a peer node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Suspect,
    Dead,
}

/// Monitors peer health by checking last_seen timestamps.
pub struct HealthMonitor {
    registry: Arc<PeerRegistry>,
    config: PeerConfig,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl HealthMonitor {
    /// Creates a new HealthMonitor.
    pub fn new(registry: Arc<PeerRegistry>, config: PeerConfig) -> Self {
        Self {
            registry,
            config,
            handle: None,
        }
    }

    /// Starts the background monitoring task.
    pub fn start_monitoring(&mut self) {
        let registry = Arc::clone(&self.registry);
        let interval_secs = self.config.heartbeat_interval_secs;
        let failure_threshold = self.config.circuit_breaker_threshold as u64;
        let timeout = Duration::from_secs(interval_secs * failure_threshold);

        self.handle = Some(tokio::spawn(async move {
            let mut int = interval(Duration::from_secs(interval_secs));
            loop {
                int.tick().await;
                let now = Instant::now();
                for peer in registry.list_alive() {
                    if now.duration_since(peer.last_seen) > timeout {
                        registry.mark_dead(peer.node_id);
                    }
                }
            }
        }));
    }

    /// Stops the monitoring task.
    pub fn stop_monitoring(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }

    /// Checks the health status of a specific peer.
    pub fn check_health(&self, node_id: u64) -> HealthStatus {
        let Some(peer) = self.registry.get(node_id) else {
            return HealthStatus::Dead;
        };

        let elapsed = Instant::now().duration_since(peer.last_seen);
        let timeout = Duration::from_secs(
            self.config.heartbeat_interval_secs * self.config.circuit_breaker_threshold as u64,
        );

        if elapsed > timeout {
            HealthStatus::Dead
        } else if elapsed > timeout / 2 {
            HealthStatus::Suspect
        } else {
            HealthStatus::Healthy
        }
    }
}

impl Drop for HealthMonitor {
    fn drop(&mut self) {
        self.stop_monitoring();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer::registry::PeerInfo;

    #[tokio::test]
    async fn test_peer_health_lifecycle() {
        let registry = Arc::new(PeerRegistry::new(30));
        let mut config = PeerConfig::default();
        config.heartbeat_interval_secs = 1;
        config.circuit_breaker_threshold = 2;

        let info = PeerInfo {
            node_id: 1,
            address: "127.0.0.1:8080".to_string(),
            capabilities: vec![],
            last_seen: Instant::now(),
            is_alive: true,
        };
        registry.register(info);

        let mut monitor = HealthMonitor::new(Arc::clone(&registry), config);
        assert_eq!(monitor.check_health(1), HealthStatus::Healthy);

        monitor.start_monitoring();
        tokio::time::sleep(Duration::from_secs(3)).await;

        assert_eq!(monitor.check_health(1), HealthStatus::Dead);
        let peer = registry.get(1).unwrap();
        assert!(!peer.is_alive);

        monitor.stop_monitoring();
    }
}
