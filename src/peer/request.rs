//! Peer request definitions for distributed DDL communication.

use super::types::{now_millis, MetricKey, RequestId};
use serde::{Deserialize, Serialize};

/// A peer-to-peer request for metric data exchange.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerRequest {
    pub request_id: RequestId,
    pub source_node: u64,
    pub target_node: u64,
    pub metric_key: MetricKey,
    pub timestamp: u64,
    pub payload: Vec<u8>,
}

impl PeerRequest {
    /// Create a new peer request with auto-generated request_id and timestamp.
    pub fn new(
        source_node: u64,
        target_node: u64,
        metric_key: MetricKey,
        payload: Vec<u8>,
    ) -> Self {
        let request_id = RequestId::new(source_node);

        Self {
            request_id,
            source_node,
            target_node,
            metric_key,
            timestamp: now_millis(),
            payload,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_request_creation() {
        let source = 42;
        let target = 99;
        let metric_key = MetricKey::from("cpu_usage");
        let payload = vec![1, 2, 3, 4];

        let before = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let request = PeerRequest::new(source, target, metric_key.clone(), payload.clone());

        let after = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        assert_eq!(request.source_node, source);
        assert_eq!(request.target_node, target);
        assert_eq!(request.metric_key.0, metric_key.0);
        assert_eq!(request.payload, payload);
        assert!(request.timestamp >= before);
        assert!(request.timestamp <= after);
    }
}
