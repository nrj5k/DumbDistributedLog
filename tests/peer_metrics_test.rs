//! Integration tests for the peer metrics system.
//!
//! Tests cover: standalone error handling, peer registry lifecycle,
//! service advertisement, circuit breaker states, cache TTL, and routing.

use ddl::peer::advertisement::AdvertisementManager;
use ddl::peer::registry::PeerInfo;
use ddl::{
    CircuitBreaker, MetricKey, PeerCache, PeerConfig, PeerRegistry, PeerRouter,
    ServiceAdvertisement,
};
use std::sync::Arc;
use std::time::Duration;

/// Test 1: Standalone mode returns error on fetch_metric
#[tokio::test]
async fn test_standalone_returns_error() {
    use ddl::DdlConfig;
    use ddl::DdlDistributed;

    let ddl = DdlDistributed::new_standalone(DdlConfig::default());
    let result = ddl.fetch_metric("cpu", vec![]).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{}", err).contains("Peer fetching not available"));
}

/// Test 2: Peer registry lifecycle - register, get, mark dead, unregister
#[test]
fn test_peer_registry_lifecycle() {
    let registry = PeerRegistry::new(30);

    let info = PeerInfo {
        node_id: 1,
        address: "127.0.0.1:8080".to_string(),
        capabilities: vec!["cpu".to_string()],
        last_seen: std::time::Instant::now(),
        is_alive: true,
    };

    registry.register(info.clone());
    assert!(registry.get(1).is_some());
    assert_eq!(registry.list_alive().len(), 1);

    registry.mark_dead(1);
    assert!(registry.list_alive().is_empty());

    registry.unregister(1);
    assert!(registry.get(1).is_none());
}

/// Test 3: Advertisement and find provider
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

    let found_cpu = manager.find_provider("cpu");
    assert!(found_cpu.is_some());
    assert_eq!(found_cpu.unwrap().node_id, 1);

    let found_disk = manager.find_provider("disk");
    assert!(found_disk.is_none());
}

/// Test 4: Circuit breaker state transitions
#[test]
fn test_circuit_breaker_state_transitions() {
    let mut config = PeerConfig::default();
    config.circuit_breaker_threshold = 5;
    config.circuit_breaker_reset_secs = 1;

    let cb = CircuitBreaker::new(&config);
    assert_eq!(cb.state(), "closed");

    for _ in 0..5 {
        cb.record_failure();
    }
    assert_eq!(cb.state(), "open");
    assert!(!cb.allow_request());

    std::thread::sleep(Duration::from_secs(2));
    assert!(cb.allow_request());
    assert_eq!(cb.state(), "half_open");

    cb.record_success();
    assert_eq!(cb.state(), "closed");
}

/// Test 5: Cache TTL expiration
#[test]
fn test_cache_ttl_expiration() {
    let cache: PeerCache<String> = PeerCache::new(1, 100);

    cache.set(
        "key1".to_string(),
        "value1".to_string(),
        Some(Duration::from_secs(1)),
    );
    assert_eq!(cache.get("key1"), Some("value1".to_string()));

    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(cache.get("key1"), None);
}

/// Test 6: Round-robin routing through peers
#[test]
fn test_round_robin_routing() {
    let registry = Arc::new(PeerRegistry::new(30));
    let advertisement = Arc::new(AdvertisementManager::new(60));

    for i in 1..=3 {
        registry.register(PeerInfo {
            node_id: i,
            address: format!("127.0.0.1:808{}", i),
            capabilities: vec![],
            last_seen: std::time::Instant::now(),
            is_alive: true,
        });
    }

    let router = PeerRouter::new(registry, advertisement);

    let results: Vec<u64> = (0..6)
        .map(|_| router.route(&MetricKey("x".into())).unwrap())
        .collect();

    assert_eq!(results[0], results[3]);
    assert_eq!(results[1], results[4]);
    assert_eq!(results[2], results[5]);
    assert_ne!(results[0], results[1]);
    assert_ne!(results[1], results[2]);
}
