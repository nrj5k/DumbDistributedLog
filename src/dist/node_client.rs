//! Node client for distributed AutoQueues
//!
//! Handles communication between cluster nodes for distributed aggregations.

use crate::networking::{TransportError, TransportType};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;

/// Error types for node client operations
#[derive(Debug, thiserror::Error)]
pub enum NodeClientError {
    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),
    
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
    ValueResponse { queue_name: String, value: Option<f64> },
    
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
pub struct NodeClient {
    _node_id: String,
    nodes: Vec<SocketAddr>,
    transport_type: TransportType,
    timeout_duration: Duration,
}

impl NodeClient {
    /// Create a new node client
    pub fn new(node_id: String, nodes: Vec<SocketAddr>) -> Self {
        Self {
            _node_id: node_id,
            nodes,
            transport_type: TransportType::Tcp, // Default to TCP for compatibility
            timeout_duration: Duration::from_secs(5),
        }
    }
    
    /// Set the transport type
    pub fn with_transport_type(mut self, transport_type: TransportType) -> Self {
        self.transport_type = transport_type;
        self
    }
    
    /// Set the timeout duration
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout_duration = timeout;
        self
    }
    
    /// Request a value from a specific node and queue
    pub async fn request_value(
        &self,
        node: &SocketAddr,
        queue_name: &str,
    ) -> Result<Option<f64>, NodeClientError> {
        let message = NodeMessage::RequestValue {
            queue_name: queue_name.to_string(),
        };
        
        let response = self.send_request(node, message).await?;
        
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
        let message = NodeMessage::RequestAggregation {
            queue_name: queue_name.to_string(),
            operation: operation.to_string(),
            args,
        };
        
        let response = self.send_request(node, message).await?;
        
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
        let mut tasks = Vec::new();
        for node in &self.nodes {
            let message_clone = message.clone();
            let node_clone = *node;
            let task = tokio::spawn(async move {
                // In a real implementation, we would send the message to each node
                // For now, we'll just simulate successful sends
                println!("Broadcasting to node: {} - {:?}", node_clone, message_clone);
                Ok::<(), NodeClientError>(())
            });
            tasks.push(task);
        }
        
        // Wait for all broadcasts to complete
        for task in tasks {
            task.await.map_err(|_| NodeClientError::Transport(
                TransportError::NetworkUnreachable
            ))??;
        }
        
        Ok(())
    }
    
    /// Collect values from all nodes for a specific queue
    pub async fn collect_all_values(
        &self,
        queue_name: &str,
    ) -> Result<Vec<f64>, NodeClientError> {
        let mut values = Vec::new();
        
        // Collect from all nodes concurrently
        let mut tasks = Vec::new();
        for node in &self.nodes {
            let node_clone = *node;
            let queue_name_clone = queue_name.to_string();
            let task = tokio::spawn(async move {
                // In a real implementation, we would actually make the request
                // For now, we'll simulate some values
                println!("Collecting from node: {} for queue: {}", node_clone, queue_name_clone);
                // Simulated value for demonstration
                Ok::<Option<f64>, NodeClientError>(Some(80.0 + (node_clone.port() as f64 % 20.0)))
            });
            tasks.push(task);
        }
        
        // Wait for all responses and collect values
        for task in tasks {
            if let Ok(Ok(Some(value))) = task.await {
                values.push(value);
            }
        }
        
        Ok(values)
    }
    
    /// Send a request to a node and wait for response
    async fn send_request(
        &self,
        _node: &SocketAddr,
        message: NodeMessage,
    ) -> Result<NodeMessage, NodeClientError> {
        // In a real implementation, we would:
        // 1. Establish a connection to the node
        // 2. Serialize and send the message
        // 3. Wait for and deserialize the response
        // 4. Handle timeouts and errors
        
        // For demonstration, we'll return a simulated response
        match message {
            NodeMessage::RequestValue { queue_name } => {
                // Simulate a response with a value
                Ok(NodeMessage::ValueResponse {
                    queue_name,
                    value: Some(85.5),
                })
            }
            NodeMessage::RequestAggregation { operation, .. } => {
                // Simulate an aggregation response
                let result = match operation.as_str() {
                    "avg" => 82.5,
                    "max" => 95.0,
                    "min" => 70.0,
                    "sum" => 330.0,
                    _ => 0.0,
                };
                Ok(NodeMessage::AggregationResponse { result })
            }
            _ => Ok(NodeMessage::ValueResponse {
                queue_name: "default".to_string(),
                value: Some(0.0),
            }),
        }
    }
}