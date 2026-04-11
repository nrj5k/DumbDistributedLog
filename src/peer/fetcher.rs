//! PeerFetcher orchestrator - high-level API for peer metric fetching.
//!
//! Orchestrates: cache → circuit breaker → route → send → receive → cache store

use std::sync::Arc;
use tokio::time::timeout;

use super::{
    cache::PeerCache,
    circuit_breaker::CircuitBreaker,
    codec::PeerCodec,
    config::PeerConfig,
    error::PeerError,
    handler::PeerRequestHandler,
    health::HealthMonitor,
    pending::PendingRequestTracker,
    request::PeerRequest,
    response::PeerResponse,
    routing::PeerRouter,
    types::{MetricKey, RequestId},
};
use crate::gossip::GossipCoordinator;

/// High-level orchestrator for peer metric fetching.
pub struct PeerFetcher {
    node_id: u64,
    config: PeerConfig,
    router: Arc<PeerRouter>,
    pending: Arc<PendingRequestTracker>,
    #[allow(dead_code)]
    health: Arc<HealthMonitor>,
    circuit_breaker: Arc<CircuitBreaker>,
    cache: Arc<PeerCache<Vec<u8>>>,
    gossip: Option<Arc<GossipCoordinator>>,
    handler: Option<Arc<dyn PeerRequestHandler>>,
}

impl PeerFetcher {
    /// Creates a new PeerFetcher with all dependencies injected.
    pub fn new(
        node_id: u64,
        config: PeerConfig,
        router: Arc<PeerRouter>,
        pending: Arc<PendingRequestTracker>,
        health: Arc<HealthMonitor>,
        circuit_breaker: Arc<CircuitBreaker>,
        cache: Arc<PeerCache<Vec<u8>>>,
        gossip: Option<Arc<GossipCoordinator>>,
    ) -> Self {
        Self {
            node_id,
            config,
            router,
            pending,
            health,
            circuit_breaker,
            cache,
            gossip,
            handler: None,
        }
    }

    /// Sets the request handler for processing incoming peer requests.
    pub fn set_handler(&mut self, handler: Arc<dyn PeerRequestHandler>) {
        self.handler = Some(handler);
    }

    /// Returns a reference to the request handler if one is set.
    pub fn handler(&self) -> Option<&Arc<dyn PeerRequestHandler>> {
        self.handler.as_ref()
    }

    /// Fetches a metric from a peer with full orchestration.
    pub async fn fetch_metric(
        &self,
        metric_key: MetricKey,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, PeerError> {
        // 1. Check cache first
        let cache_key = metric_key.0.clone();
        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(cached);
        }

        // 2. Route to target peer
        let target_peer = self.router.route(&metric_key)?;

        // 3. Check circuit breaker
        if !self.circuit_breaker.allow_request() {
            return Err(PeerError::CircuitOpen {
                peer_id: target_peer,
            });
        }

        // 4. Create request with new ID
        let request_id = RequestId::new(self.node_id);
        let request = PeerRequest::new(self.node_id, target_peer, metric_key.clone(), payload);

        // 5. Register with pending tracker and get receiver
        let receiver = self.pending.register(request_id);

        // 6. Encode request
        let encoded = PeerCodec::encode_request(&request)?;

        // 7. Send via gossip
        if let Some(ref gossip) = self.gossip {
            let topic = format!(
                "peer.request.{}.{}.{}",
                self.node_id, target_peer, metric_key.0
            );
            gossip
                .publish(&topic, encoded)
                .await
                .map_err(|e| PeerError::RoutingError {
                    message: format!("Gossip publish failed: {}", e),
                })?;
        } else {
            return Err(PeerError::RoutingError {
                message: "No gossip coordinator available".to_string(),
            });
        }

        // 8. Wait for response with timeout
        let timeout_secs = self.config.request_timeout_secs;
        let result = timeout(std::time::Duration::from_secs(timeout_secs), receiver).await;

        match result {
            Ok(Ok(response)) => {
                // 9. Success: cache result, record success, return payload
                self.cache.set(cache_key, response.payload.clone(), None);
                self.circuit_breaker.record_success();
                Ok(response.payload)
            }
            Ok(Err(_)) => {
                self.circuit_breaker.record_failure();
                Err(PeerError::RequestCancelled {
                    request_id: format!("{:?}", request_id.0),
                })
            }
            Err(_) => {
                self.circuit_breaker.record_failure();
                Err(PeerError::Timeout {
                    duration_ms: timeout_secs * 1000,
                })
            }
        }
    }

    /// Handles an incoming response by completing the pending request.
    pub fn handle_response(&self, response: PeerResponse) -> bool {
        self.pending.complete(response.request_id, response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer::{advertisement::AdvertisementManager, registry::PeerRegistry};

    fn create_test_fetcher() -> (PeerFetcher, Arc<PeerCache<Vec<u8>>>) {
        let config = PeerConfig::default();
        let registry = Arc::new(PeerRegistry::new(30));
        let advertisement = Arc::new(AdvertisementManager::new(60));
        let router = Arc::new(PeerRouter::new(registry, advertisement));
        let pending = Arc::new(PendingRequestTracker::new(&config));
        let health = Arc::new(HealthMonitor::new(
            Arc::new(PeerRegistry::new(30)),
            config.clone(),
        ));
        let circuit_breaker = Arc::new(CircuitBreaker::new(&config));
        let cache = Arc::new(PeerCache::new(60, 100));

        let fetcher = PeerFetcher::new(
            1,
            config,
            router,
            pending,
            health,
            circuit_breaker,
            cache.clone(),
            None, // No gossip in tests
        );
        (fetcher, cache)
    }

    #[tokio::test]
    async fn test_fetch_metric_cache_hit() {
        let (fetcher, cache) = create_test_fetcher();
        let metric_key = MetricKey("test.metric".to_string());
        let cached_value = b"cached_data".to_vec();
        cache.set("test.metric".to_string(), cached_value.clone(), None);

        let result = fetcher.fetch_metric(metric_key, vec![]).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), cached_value);
    }

    #[tokio::test]
    async fn test_handle_response_completes_pending() {
        let (fetcher, _cache) = create_test_fetcher();
        let request_id = RequestId::new(1);
        let receiver = fetcher.pending.register(request_id);
        let response = PeerResponse::success(request_id, 2, b"response_data".to_vec());

        let completed = fetcher.handle_response(response);
        assert!(completed);
        assert!(receiver.await.is_ok());
    }

    #[tokio::test]
    async fn test_handle_response_unknown_request() {
        let (fetcher, _cache) = create_test_fetcher();
        let unknown_id = RequestId::new(1);
        let response = PeerResponse::success(unknown_id, 2, b"data".to_vec());
        assert!(!fetcher.handle_response(response));
    }
}
