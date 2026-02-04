//! Node client for distributed AutoQueues
//!
//! Handles communication between cluster nodes using ZMQ transport.

use crate::network::pubsub::zmq::transport::ZmqTransport;
use crate::traits::transport::Transport;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;

/// Error types for node client operations
#[derive(Debug, thiserror::Error)]
pub enum NodeClientError {
    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Timeout error")]
    Timeout,

    #[error("Invalid node address: {0}")]
    InvalidAddress(String),
}

/// Message types for node-to-node communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeMessage {
    /// Request a value from a specific queue
    RequestValue { queue_name: String },

    /// Response with a value from a queue
    ValueResponse {
        queue_name: String,
        value: Option<f64>,
    },

    /// Request an expression evaluation
    RequestExpression { sources: Vec<String> },

    /// Response with expression evaluation result
    ExpressionResponse {
        sources: Vec<String>,
        score: Option<f64>,
    },

    /// Request an aggregation operation
    RequestAggregation {
        queue_name: String,
        operation: String,
        args: Vec<String>,
    },

    /// Response with aggregation result
    AggregationResponse { result: f64 },

    /// Broadcast a value to all nodes
    BroadcastValue { queue_name: String, value: f64 },
}

/// Node client for communicating with other nodes in the cluster
#[derive(Clone)]
pub struct NodeClient {
    _node_id: String,
    nodes: Vec<String>,
    timeout: Duration,
}

impl NodeClient {
    /// Create a new node client
    pub fn new(node_id: String, nodes: Vec<SocketAddr>) -> Self {
        let node_addrs: Vec<String> = nodes.iter().map(|addr| format!("tcp://{}", addr)).collect();

        Self {
            _node_id: node_id,
            nodes: node_addrs,
            timeout: Duration::from_secs(5),
        }
    }

    /// Set the timeout duration
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Request a value from a specific node and queue
    pub async fn request_value(
        &self,
        node: &SocketAddr,
        queue_name: &str,
    ) -> Result<Option<f64>, NodeClientError> {
        let addr = format!("tcp://{}", node);
        let message = NodeMessage::RequestValue {
            queue_name: queue_name.to_string(),
        };

        let response = self.send_request(&addr, message).await?;

        match response {
            NodeMessage::ValueResponse { value, .. } => Ok(value),
            _ => Err(NodeClientError::Serialization(
                "Unexpected response type".to_string(),
            )),
        }
    }

    /// Request an aggregation operation from a specific node
    pub async fn request_aggregation(
        &self,
        node: &SocketAddr,
        queue_name: &str,
        operation: &str,
        args: Vec<String>,
    ) -> Result<f64, NodeClientError> {
        let addr = format!("tcp://{}", node);
        let message = NodeMessage::RequestAggregation {
            queue_name: queue_name.to_string(),
            operation: operation.to_string(),
            args,
        };

        let response = self.send_request(&addr, message).await?;

        match response {
            NodeMessage::AggregationResponse { result } => Ok(result),
            _ => Err(NodeClientError::Serialization(
                "Unexpected response type".to_string(),
            )),
        }
    }

    /// Broadcast a value to all nodes
    pub async fn broadcast_value(
        &self,
        queue_name: &str,
        value: f64,
    ) -> Result<(), NodeClientError> {
        let message = NodeMessage::BroadcastValue {
            queue_name: queue_name.to_string(),
            value,
        };

        // Send to all nodes concurrently
        let mut handles = Vec::new();
        for addr in &self.nodes {
            let msg = message.clone();
            let addr_clone = addr.clone();
            handles.push(tokio::spawn(async move {
                send_message(&addr_clone, &msg).await
            }));
        }

        for handle in handles {
            handle
                .await
                .map_err(|_| NodeClientError::Transport("Task cancelled".to_string()))??;
        }

        Ok(())
    }

    /// Collect values from all nodes for a specific queue
    pub async fn collect_all_values(&self, queue_name: &str) -> Result<Vec<f64>, NodeClientError> {
        let mut values = Vec::new();

        // Collect from all nodes concurrently
        let mut handles = Vec::new();
        for addr in &self.nodes {
            let queue_name = queue_name.to_string();
            let addr_clone = addr.clone();
            handles.push(tokio::spawn(async move {
                request_value(&addr_clone, &queue_name).await
            }));
        }

        for handle in handles {
            if let Ok(Ok(Some(value))) = handle.await {
                values.push(value);
            }
        }

        Ok(values)
    }

    /// Collect expression scores from all nodes
    ///
    /// Each node computes the expression with its local values
    /// and returns the computed score.
    pub async fn collect_all_expression(
        &self,
        _expression: &str,
        sources: Vec<String>,
    ) -> Result<Vec<f64>, NodeClientError> {
        let mut scores = Vec::new();

        // Collect from all nodes concurrently
        let mut handles = Vec::new();
        for addr in &self.nodes {
            let addr_clone = addr.clone();
            let sources_clone = sources.clone();
            handles.push(tokio::spawn(async move {
                request_expression_score(&addr_clone, &sources_clone).await
            }));
        }

        for handle in handles {
            if let Ok(Ok(Some(score))) = handle.await {
                scores.push(score);
            }
        }

        Ok(scores)
    }

    /// Send a message and get response
    async fn send_request(
        &self,
        addr: &str,
        message: NodeMessage,
    ) -> Result<NodeMessage, NodeClientError> {
        let _data = serde_json::to_vec(&message)
            .map_err(|e| NodeClientError::Serialization(e.to_string()))?;

        send_message(addr, &message).await?;
        receive_message(addr).await
    }
}

/// Create a transport and connect to address
async fn create_transport(addr: &str) -> Result<ZmqTransport, NodeClientError> {
    let addr_without_prefix = addr.trim_start_matches("tcp://");
    let transport = ZmqTransport::new().map_err(|e| NodeClientError::Transport(e.to_string()))?;

    transport
        .connect(addr_without_prefix)
        .await
        .map_err(|e| NodeClientError::Transport(e.to_string()))?;

    Ok(transport)
}

/// Send a message to an address
async fn send_message(addr: &str, message: &NodeMessage) -> Result<(), NodeClientError> {
    let transport = create_transport(addr).await?;
    let data =
        serde_json::to_vec(message).map_err(|e| NodeClientError::Serialization(e.to_string()))?;

    Transport::send(&transport, &data)
        .await
        .map_err(|e| NodeClientError::Transport(e.to_string()))?;

    Ok(())
}

/// Request a value from an address
async fn request_value(addr: &str, queue_name: &str) -> Result<Option<f64>, NodeClientError> {
    let message = NodeMessage::RequestValue {
        queue_name: queue_name.to_string(),
    };

    let transport = create_transport(addr).await?;
    let _data =
        serde_json::to_vec(&message).map_err(|e| NodeClientError::Serialization(e.to_string()))?;

    Transport::send(&transport, &_data)
        .await
        .map_err(|e| NodeClientError::Transport(e.to_string()))?;

    let response = transport
        .receive()
        .await
        .map_err(|e| NodeClientError::Transport(e.to_string()))?;

    let response_msg: NodeMessage = serde_json::from_slice(&response)
        .map_err(|e| NodeClientError::Serialization(e.to_string()))?;

    match response_msg {
        NodeMessage::ValueResponse { value, .. } => Ok(value),
        _ => Err(NodeClientError::Serialization(
            "Unexpected response".to_string(),
        )),
    }
}

/// Request an expression score from an address
async fn request_expression_score(
    addr: &str,
    sources: &[String],
) -> Result<Option<f64>, NodeClientError> {
    // Create a request for expression evaluation
    let message = NodeMessage::RequestExpression {
        sources: sources.to_vec(),
    };

    let transport = create_transport(addr).await?;
    let data =
        serde_json::to_vec(&message).map_err(|e| NodeClientError::Serialization(e.to_string()))?;

    Transport::send(&transport, &data)
        .await
        .map_err(|e| NodeClientError::Transport(e.to_string()))?;

    let response = transport
        .receive()
        .await
        .map_err(|e| NodeClientError::Transport(e.to_string()))?;

    let response_msg: NodeMessage = serde_json::from_slice(&response)
        .map_err(|e| NodeClientError::Serialization(e.to_string()))?;

    match response_msg {
        NodeMessage::ExpressionResponse { score, .. } => Ok(score),
        _ => Err(NodeClientError::Serialization(
            "Unexpected response".to_string(),
        )),
    }
}

/// Receive a message from an address
async fn receive_message(_addr: &str) -> Result<NodeMessage, NodeClientError> {
    // In a real implementation, this would receive from a specific transport
    // For now, return an error since we don't persist transports
    Err(NodeClientError::Transport(
        "Receive not yet implemented - need persistent transport".to_string(),
    ))
}
