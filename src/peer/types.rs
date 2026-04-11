//! Core type definitions for peer-to-peer communication.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

/// Unique request identifier combining node_id and sequence number.
///
/// Format: high 64 bits = node_id, low 64 bits = sequence number
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct RequestId(pub u128);

impl RequestId {
    /// Create a new unique request ID for a node.
    ///
    /// Uses a thread-safe atomic counter to ensure uniqueness.
    pub fn new(node_id: u64) -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let seq = SEQUENCE.fetch_add(1, Ordering::SeqCst);
        let id = ((node_id as u128) << 64) | (seq as u128);
        RequestId(id)
    }
}

impl From<u128> for RequestId {
    fn from(id: u128) -> Self {
        RequestId(id)
    }
}

/// Endpoint identifying a peer node and its topic.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PeerEndpoint {
    pub node_id: u64,
    pub topic: String,
}

/// Key for identifying metrics in the peer system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct MetricKey(pub String);

impl From<String> for MetricKey {
    fn from(s: String) -> Self {
        MetricKey(s)
    }
}

impl From<&str> for MetricKey {
    fn from(s: &str) -> Self {
        MetricKey(s.to_string())
    }
}

/// Returns current Unix timestamp in milliseconds
pub fn now_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before Unix epoch")
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_id_uniqueness() {
        let id1 = RequestId::new(42);
        let id2 = RequestId::new(42);
        let id3 = RequestId::new(42);

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_request_id_format() {
        let id = RequestId::new(42);
        let high_bits = (id.0 >> 64) as u64;
        assert_eq!(high_bits, 42);
    }

    #[test]
    fn test_peer_endpoint() {
        let endpoint = PeerEndpoint {
            node_id: 1,
            topic: "test".to_string(),
        };
        assert_eq!(endpoint.node_id, 1);
        assert_eq!(endpoint.topic, "test");
    }

    #[test]
    fn test_metric_key_from_string() {
        let key: MetricKey = "metric_name".into();
        assert_eq!(key.0, "metric_name");
    }

    #[test]
    fn test_metric_key_from_str() {
        let key = MetricKey::from("test_metric");
        assert_eq!(key.0, "test_metric");
    }
}
