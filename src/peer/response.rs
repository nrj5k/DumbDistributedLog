//! Peer response types for DDL peer-to-peer communication.
use super::types::{now_millis, RequestId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerResponse {
    pub request_id: RequestId,
    pub source_node: u64,
    pub success: bool,
    pub payload: Vec<u8>,
    pub timestamp: u64,
    pub error_message: Option<String>,
}
impl PeerResponse {
    pub fn success(request_id: RequestId, source_node: u64, payload: Vec<u8>) -> Self {
        Self {
            request_id,
            source_node,
            success: true,
            payload,
            timestamp: now_millis(),
            error_message: None,
        }
    }
    pub fn failure(request_id: RequestId, source_node: u64, error: impl Into<String>) -> Self {
        Self {
            request_id,
            source_node,
            success: false,
            payload: Vec::new(),
            timestamp: now_millis(),
            error_message: Some(error.into()),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_success_response() {
        let request_id = RequestId::new(42);
        let response = PeerResponse::success(request_id, 100, vec![1, 2, 3]);
        assert_eq!(response.request_id, request_id);
        assert_eq!(response.source_node, 100);
        assert!(response.success);
        assert_eq!(response.payload, vec![1, 2, 3]);
        assert!(response.error_message.is_none());
        assert!(response.timestamp > 0);
    }
    #[test]
    fn test_failure_response() {
        let request_id = RequestId::new(42);
        let response = PeerResponse::failure(request_id, 100, "connection timeout");
        assert_eq!(response.request_id, request_id);
        assert_eq!(response.source_node, 100);
        assert!(!response.success);
        assert!(response.payload.is_empty());
        assert_eq!(
            response.error_message,
            Some("connection timeout".to_string())
        );
        assert!(response.timestamp > 0);
    }
}
