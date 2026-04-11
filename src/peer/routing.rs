#![allow(clippy::single_line_fn)]
use super::{
    advertisement::AdvertisementManager,
    error::PeerError,
    registry::{PeerInfo, PeerRegistry},
    types::MetricKey,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
pub struct PeerRouter {
    registry: Arc<PeerRegistry>,
    advertisement_mgr: Arc<AdvertisementManager>,
    round_robin_counter: AtomicU64,
}
impl PeerRouter {
    pub fn new(registry: Arc<PeerRegistry>, advertisement_mgr: Arc<AdvertisementManager>) -> Self {
        Self {
            registry,
            advertisement_mgr,
            round_robin_counter: AtomicU64::new(0),
        }
    }
    pub fn route(&self, mk: &MetricKey) -> Result<u64, PeerError> {
        if let Some(ad) = self.advertisement_mgr.find_provider(&mk.0) {
            if self.registry.get(ad.node_id).is_some_and(|p| p.is_alive) {
                return Ok(ad.node_id);
            }
        }
        let alive = self.registry.list_alive();
        if alive.is_empty() {
            return Err(PeerError::RoutingError {
                message: "no alive peers".into(),
            });
        }
        Ok(
            alive[(self.round_robin_counter.fetch_add(1, Ordering::SeqCst) as usize) % alive.len()]
                .node_id,
        )
    }
    pub fn get_peer_info(&self, node_id: u64) -> Option<PeerInfo> {
        self.registry.get(node_id)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer::advertisement::ServiceAdvertisement;
    #[test]
    fn test_route_to_specific_provider() {
        let (reg, ad) = (
            Arc::new(PeerRegistry::new(30)),
            Arc::new(AdvertisementManager::new(60)),
        );
        reg.register(PeerInfo {
            node_id: 1,
            address: "127.0.0.1:8080".into(),
            capabilities: vec![],
            last_seen: std::time::Instant::now(),
            is_alive: true,
        });
        ad.advertise(ServiceAdvertisement {
            node_id: 1,
            metrics_offered: vec!["cpu".into()],
            address: "127.0.0.1:8080".into(),
            timestamp: 1234567890,
        });
        assert_eq!(
            PeerRouter::new(reg, ad)
                .route(&MetricKey("cpu".into()))
                .unwrap(),
            1
        );
    }
    #[test]
    fn test_round_robin_cycles_through_peers() {
        let reg = Arc::new(PeerRegistry::new(30));
        for i in 1..=3 {
            reg.register(PeerInfo {
                node_id: i,
                address: format!("127.0.0.1:808{}", i),
                capabilities: vec![],
                last_seen: std::time::Instant::now(),
                is_alive: true,
            });
        }
        let r = PeerRouter::new(reg, Arc::new(AdvertisementManager::new(60)));
        let [r1, r2, r3, r4] = [(); 4].map(|_| r.route(&MetricKey("x".into())).unwrap());
        assert_ne!(r1, r2);
        assert_ne!(r2, r3);
        assert_ne!(r3, r4);
        assert_eq!(r1, r4);
    }
    #[test]
    fn test_route_no_peers_error() {
        assert!(matches!(
            PeerRouter::new(
                Arc::new(PeerRegistry::new(30)),
                Arc::new(AdvertisementManager::new(60))
            )
            .route(&MetricKey("cpu".into())),
            Err(PeerError::RoutingError { .. })
        ));
    }
}
