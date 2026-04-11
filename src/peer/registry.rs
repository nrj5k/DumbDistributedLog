//! Peer registry for tracking peer information in the DDL network.

use dashmap::DashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub node_id: u64,
    pub address: String,
    pub capabilities: Vec<String>,
    pub last_seen: Instant,
    pub is_alive: bool,
}

pub struct PeerRegistry {
    peers: DashMap<u64, PeerInfo>,
    default_ttl: Duration,
}

impl PeerRegistry {
    pub fn new(default_ttl_secs: u64) -> Self {
        Self {
            peers: DashMap::new(),
            default_ttl: Duration::from_secs(default_ttl_secs),
        }
    }

    pub fn register(&self, info: PeerInfo) -> Option<PeerInfo> {
        self.peers.insert(info.node_id, info)
    }

    pub fn unregister(&self, node_id: u64) -> Option<PeerInfo> {
        self.peers.remove(&node_id).map(|(_, v)| v)
    }

    pub fn get(&self, node_id: u64) -> Option<PeerInfo> {
        self.peers.get(&node_id).map(|v| v.clone())
    }

    pub fn list_alive(&self) -> Vec<PeerInfo> {
        self.peers
            .iter()
            .filter(|e| e.value().is_alive)
            .map(|e| e.value().clone())
            .collect()
    }

    pub fn update_heartbeat(&self, node_id: u64) -> bool {
        if let Some(mut entry) = self.peers.get_mut(&node_id) {
            entry.last_seen = Instant::now();
            entry.is_alive = true;
            true
        } else {
            false
        }
    }

    pub fn mark_dead(&self, node_id: u64) -> bool {
        if let Some(mut entry) = self.peers.get_mut(&node_id) {
            entry.is_alive = false;
            true
        } else {
            false
        }
    }

    #[allow(dead_code)]
    pub fn ttl(&self) -> Duration {
        self.default_ttl
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_get_lifecycle() {
        let registry = PeerRegistry::new(30);
        let info = PeerInfo {
            node_id: 1,
            address: "127.0.0.1:8080".to_string(),
            capabilities: vec!["queue".to_string()],
            last_seen: Instant::now(),
            is_alive: true,
        };
        assert!(registry.register(info.clone()).is_none());
        let retrieved = registry.get(1).expect("peer should exist");
        assert_eq!(retrieved.node_id, 1);
        assert_eq!(retrieved.address, "127.0.0.1:8080");
        let alive = registry.list_alive();
        assert_eq!(alive.len(), 1);
        assert!(registry.mark_dead(1));
        assert!(registry.list_alive().is_empty());
        let removed = registry.unregister(1).expect("peer should be removed");
        assert_eq!(removed.node_id, 1);
        assert!(registry.get(1).is_none());
    }

    #[test]
    fn test_update_heartbeat() {
        let registry = PeerRegistry::new(30);
        let info = PeerInfo {
            node_id: 2,
            address: "127.0.0.1:8081".to_string(),
            capabilities: vec![],
            last_seen: Instant::now() - Duration::from_secs(10),
            is_alive: false,
        };
        registry.register(info);
        let old_last_seen = registry.get(2).unwrap().last_seen;
        std::thread::sleep(Duration::from_millis(10));
        assert!(registry.update_heartbeat(2));
        let updated = registry.get(2).unwrap();
        assert!(updated.is_alive);
        assert!(updated.last_seen > old_last_seen);
    }
}
