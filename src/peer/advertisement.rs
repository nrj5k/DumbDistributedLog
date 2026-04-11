//! Service advertisement for peer capability discovery.

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Service advertisement for peer capability discovery.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceAdvertisement {
    pub node_id: u64,
    pub metrics_offered: Vec<String>,
    pub address: String,
    pub timestamp: u64,
}

/// Manages service advertisements with TTL-based expiration.
pub struct AdvertisementManager {
    advertisements: DashMap<u64, ServiceAdvertisement>,
    inserted_at: DashMap<u64, Instant>,
    ttl: Duration,
}

impl AdvertisementManager {
    /// Creates a new advertisement manager with the specified TTL.
    pub fn new(ttl_secs: u64) -> Self {
        Self {
            advertisements: DashMap::new(),
            inserted_at: DashMap::new(),
            ttl: Duration::from_secs(ttl_secs),
        }
    }

    /// Stores a service advertisement.
    pub fn advertise(&self, ad: ServiceAdvertisement) {
        let node_id = ad.node_id;
        self.advertisements.insert(node_id, ad);
        self.inserted_at.insert(node_id, Instant::now());
    }

    /// Removes an advertisement by node ID.
    pub fn remove(&self, node_id: u64) -> Option<ServiceAdvertisement> {
        self.inserted_at.remove(&node_id);
        self.advertisements.remove(&node_id).map(|(_, v)| v)
    }

    /// Finds a provider offering the specified metric.
    pub fn find_provider(&self, metric_key: &str) -> Option<ServiceAdvertisement> {
        for entry in self.advertisements.iter() {
            let (node_id, ad) = entry.pair();
            if ad.metrics_offered.iter().any(|m| m == metric_key) && !self.is_expired(*node_id) {
                return Some(ad.clone());
            }
        }
        None
    }

    /// Gets an advertisement by node ID.
    pub fn get(&self, node_id: u64) -> Option<ServiceAdvertisement> {
        if self.is_expired(node_id) {
            self.remove(node_id);
            None
        } else {
            self.advertisements.get(&node_id).map(|v| v.clone())
        }
    }

    /// Lists all non-expired advertisements.
    pub fn list_providers(&self) -> Vec<ServiceAdvertisement> {
        self.advertisements
            .iter()
            .filter(|entry| !self.is_expired(*entry.key()))
            .map(|entry| entry.value().clone())
            .collect()
    }

    fn is_expired(&self, node_id: u64) -> bool {
        self.inserted_at
            .get(&node_id)
            .map(|instant| instant.elapsed() > self.ttl)
            .unwrap_or(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_advertise_and_find_provider() {
        let manager = AdvertisementManager::new(60);
        let ad = ServiceAdvertisement {
            node_id: 1,
            metrics_offered: vec!["cpu".to_string(), "memory".to_string()],
            address: "192.168.1.1:8080".to_string(),
            timestamp: 1234567890,
        };
        manager.advertise(ad.clone());
        let found = manager.find_provider("cpu");
        assert!(found.is_some());
        assert_eq!(found.unwrap().node_id, 1);
    }

    #[test]
    fn test_find_provider_unknown_metric() {
        let manager = AdvertisementManager::new(60);
        let ad = ServiceAdvertisement {
            node_id: 1,
            metrics_offered: vec!["cpu".to_string()],
            address: "192.168.1.1:8080".to_string(),
            timestamp: 1234567890,
        };
        manager.advertise(ad);
        let found = manager.find_provider("disk");
        assert!(found.is_none());
    }
}
