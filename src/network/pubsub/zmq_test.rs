//! Test for ZMQ implementation to verify the fix

use crate::network::pubsub::zmq::{ZmqPubSubBroker, ZmqPubSubClient};
use serde::Serialize;
use tokio;

#[derive(Serialize)]
struct TestMessage {
    content: String,
    value: i32,
}

#[tokio::test]
async fn test_zmq_broker_creation() {
    let broker = ZmqPubSubBroker::new();
    assert!(broker.is_ok());
}

#[tokio::test]
async fn test_zmq_client_creation() {
    let client = ZmqPubSubClient::new();
    assert!(client.is_ok());
}

#[tokio::test]
async fn test_zmq_broker_publish() {
    let broker = ZmqPubSubBroker::new().expect("Failed to create broker");
    
    let message = TestMessage {
        content: "test".to_string(),
        value: 42,
    };
    
    let result = broker.publish("test.topic", &message);
    // We can't verify delivery without a subscriber, but we can check it doesn't error
    assert!(result.is_ok());
}