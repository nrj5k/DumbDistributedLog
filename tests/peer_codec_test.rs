//! Comprehensive tests for peer codec serialization.

use ddl::gossip_protocol::GossipMessage;
use ddl::peer::{codec::PeerCodec, request::PeerRequest, response::PeerResponse, types::RequestId};

#[test]
fn test_encode_decode_request_roundtrip() {
    let request = PeerRequest::new(
        1,
        2,
        ddl::peer::MetricKey("cpu.usage".to_string()),
        vec![1, 2, 3, 4],
    );
    let encoded = PeerCodec::encode_request(&request).expect("encode failed");
    let decoded = PeerCodec::decode_request(&encoded).expect("decode failed");
    assert_eq!(request.source_node, decoded.source_node);
    assert_eq!(request.target_node, decoded.target_node);
    assert_eq!(request.metric_key, decoded.metric_key);
    assert_eq!(request.payload, decoded.payload);
}

#[test]
fn test_encode_decode_response_roundtrip() {
    let request_id = RequestId::new(2);
    let response = PeerResponse::success(request_id, 2, vec![5, 6, 7, 8]);
    let encoded = PeerCodec::encode_response(&response).expect("encode failed");
    let decoded = PeerCodec::decode_response(&encoded).expect("decode failed");
    assert_eq!(response.request_id, decoded.request_id);
    assert_eq!(response.source_node, decoded.source_node);
    assert_eq!(response.success, decoded.success);
    assert_eq!(response.payload, decoded.payload);
}

#[test]
fn test_encode_decode_response_failure() {
    let request_id = RequestId::new(2);
    let response = PeerResponse::failure(request_id, 2, "Metric not found");
    let encoded = PeerCodec::encode_response(&response).expect("encode failed");
    let decoded = PeerCodec::decode_response(&encoded).expect("decode failed");
    assert!(!decoded.success);
    assert_eq!(decoded.error_message, Some("Metric not found".to_string()));
}

#[test]
fn test_invalid_version_byte() {
    let request = PeerRequest::new(1, 2, ddl::peer::MetricKey("test".to_string()), vec![]);
    let mut encoded = PeerCodec::encode_request(&request).expect("encode failed");
    encoded[0] = 0x02;
    assert!(PeerCodec::decode_request(&encoded).is_err());
}

#[test]
fn test_empty_payload_roundtrip() {
    let request = PeerRequest::new(1, 2, ddl::peer::MetricKey("test".to_string()), vec![]);
    let encoded = PeerCodec::encode_request(&request).expect("encode failed");
    let decoded = PeerCodec::decode_request(&encoded).expect("decode failed");
    assert!(decoded.payload.is_empty());
}

#[test]
fn test_large_payload_roundtrip() {
    let large_payload = vec![0u8; 1000];
    let request = PeerRequest::new(
        1,
        2,
        ddl::peer::MetricKey("test".to_string()),
        large_payload.clone(),
    );
    let encoded = PeerCodec::encode_request(&request).expect("encode failed");
    let decoded = PeerCodec::decode_request(&encoded).expect("decode failed");
    assert_eq!(decoded.payload.len(), 1000);
    assert_eq!(decoded.payload, large_payload);
}

#[test]
fn test_gossip_message_peer_request_roundtrip() {
    let request_id = RequestId::new(1);
    let msg = GossipMessage::PeerRequest {
        request_id: request_id.0,
        source_node: 1,
        target_node: 2,
        metric_key: "cpu.usage".to_string(),
        payload: vec![1, 2, 3, 4],
        timestamp: 1234567890,
    };
    let serialized = serde_json::to_string(&msg).expect("serialize failed");
    let deserialized: GossipMessage =
        serde_json::from_str(&serialized).expect("deserialize failed");
    match deserialized {
        GossipMessage::PeerRequest {
            request_id: rid,
            source_node: src,
            target_node: tgt,
            metric_key: key,
            payload: pay,
            ..
        } => {
            assert_eq!(rid, request_id.0);
            assert_eq!(src, 1);
            assert_eq!(tgt, 2);
            assert_eq!(key, "cpu.usage");
            assert_eq!(pay, vec![1, 2, 3, 4]);
        }
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_gossip_message_peer_response_roundtrip() {
    let request_id = RequestId::new(2);
    let msg = GossipMessage::PeerResponse {
        request_id: request_id.0,
        source_node: 2,
        success: true,
        payload: vec![5, 6, 7, 8],
        timestamp: 1234567890,
        error_message: None,
    };
    let serialized = serde_json::to_string(&msg).expect("serialize failed");
    let deserialized: GossipMessage =
        serde_json::from_str(&serialized).expect("deserialize failed");
    match deserialized {
        GossipMessage::PeerResponse {
            request_id: rid,
            source_node: src,
            success,
            payload: pay,
            ..
        } => {
            assert_eq!(rid, request_id.0);
            assert_eq!(src, 2);
            assert!(success);
            assert_eq!(pay, vec![5, 6, 7, 8]);
        }
        _ => panic!("Wrong message type"),
    }
}
