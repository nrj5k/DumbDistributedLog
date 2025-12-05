//! Stratified Networking - Core Transport Traits
//!
//! Defines clean interfaces for control and data planes sharing
//! Quinn foundation with different optimization strategies.

use std::error::Error;
use std::fmt;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

/// Protocol configuration for transport behavior
#[derive(Debug, Clone)]
pub struct ProtocolConfig {
    pub support_multi_message: bool,
    pub max_messages_per_packet: usize,
}

impl Default for ProtocolConfig {
    fn default() -> Self {
        Self {
            support_multi_message: false,
            max_messages_per_packet: 1,
        }
    }
}

/// Core transport errors shared by both planes
#[derive(Debug, Clone)]
pub enum TransportError {
    ConnectionLost { remote_addr: SocketAddr },
    NetworkUnreachable,
    TransportShutdown,
    SerializationError { source: String },
    Timeout { duration: Duration },
    BufferOverflow { size: usize },
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportError::ConnectionLost { remote_addr } => {
                write!(f, "Connection lost to {}", remote_addr)
            }
            TransportError::NetworkUnreachable => write!(f, "Network unreachable"),
            TransportError::TransportShutdown => write!(f, "Transport shutdown"),
            TransportError::SerializationError { source } => {
                write!(f, "Serialization error: {}", source)
            }
            TransportError::Timeout { duration } => write!(f, "Timeout after {:?}", duration),
            TransportError::BufferOverflow { size } => write!(f, "Buffer overflow: size {}", size),
        }
    }
}

impl Error for TransportError {}

/// Base trait for all message types
pub trait Message: Send + Sync {
    /// Serialize message to bytes for transport with protocol configuration
    fn serialize(&self, config: &ProtocolConfig) -> Result<Vec<u8>, TransportError>;

    /// Deserialize message from bytes with protocol configuration
    fn deserialize(data: &[u8], config: &ProtocolConfig) -> Result<Self, TransportError>
    where
        Self: Sized;

    /// Message timestamp for ordering and debugging
    fn timestamp(&self) -> Instant;

    /// Origin node identifier for routing
    fn node_id(&self) -> NodeId;

    /// Unique message identifier for tracking
    fn message_id(&self) -> u64;
}

/// Protocol message serialization support
pub trait ProtocolMessage: Message {
    /// Protocol constants
    fn protocol_constants() -> impl std::any::Any;

    /// Simple byte tags for message type identification
    fn get_tag(&self) -> u8;

    /// Extract payload data for serialization
    fn get_payload(&self) -> Vec<u8>;

    /// Validate message integrity
    fn validate(&self) -> Result<(), TransportError>;

    /// Serialize with protocol versioning and config support
    fn serialize_with_protocol(&self, config: &ProtocolConfig) -> Result<Vec<u8>, TransportError> {
        if config.support_multi_message {
            self.serialize_multi_message(config)
        } else {
            self.serialize_single_message()
        }
    }

    /// Single message serialization: [Version][Tag][Length][Payload]
    fn serialize_single_message(&self) -> Result<Vec<u8>, TransportError>;

    /// Multi-message serialization (future extension point)
    fn serialize_multi_message(&self, config: &ProtocolConfig) -> Result<Vec<u8>, TransportError> {
        let _ = config;
        unimplemented!("Multi-message serialization not implemented yet")
    }

    /// Deserialize with configurable protocol support
    fn deserialize_with_protocol(
        data: &[u8],
        config: &ProtocolConfig,
    ) -> Result<Self, TransportError>
    where
        Self: Sized,
    {
        if config.support_multi_message {
            Self::deserialize_multi_message(data, config)
        } else {
            Self::deserialize_single_message(data)
        }
    }

    /// Parse single message format
    fn deserialize_single_message(data: &[u8]) -> Result<Self, TransportError>
    where
        Self: Sized;

    /// Parse multi-message format (future extension point)
    fn deserialize_multi_message(
        data: &[u8],
        config: &ProtocolConfig,
    ) -> Result<Self, TransportError>
    where
        Self: Sized,
    {
        let _ = config;
        unimplemented!("Multi-message deserialization not implemented yet")
    }

    /// Common serialization helpers
    fn serialize_header(tag: u8) -> Vec<u8> {
        vec![protocol::VERSION, tag]
    }

    fn serialize_length(payload_len: usize) -> Vec<u8> {
        (payload_len as u32).to_be_bytes().to_vec()
    }

    fn parse_header(payload: &[u8]) -> Result<u8, TransportError> {
        let version = payload
            .get(0)
            .ok_or_else(|| TransportError::SerializationError {
                source: "No version header".to_string(),
            })?;

        if *version != protocol::VERSION {
            return Err(TransportError::SerializationError {
                source: format!(
                    "Protocol version mismatch: expected {}, got {}",
                    protocol::VERSION,
                    version
                ),
            });
        }

        payload
            .get(1)
            .copied()
            .ok_or_else(|| TransportError::SerializationError {
                source: "No message tag".to_string(),
            })
    }

    fn parse_length(payload: &[u8]) -> Result<usize, TransportError> {
        if payload.len() < 6 {
            return Err(TransportError::SerializationError {
                source: "Insufficient header size".to_string(),
            });
        }

        let bytes: [u8; 4] =
            payload[2..6]
                .try_into()
                .map_err(|_| TransportError::SerializationError {
                    source: "Invalid length format".to_string(),
                })?;
        Ok(u32::from_be_bytes(bytes) as usize)
    }
}

/// Protocol constants and standards
mod protocol {
    pub const VERSION: u8 = 0x01;

    // Simple small tag numbers for 3 message types
    pub const TAG_ACK: u8 = 0x01; // Success response
    pub const TAG_NAK: u8 = 0x02; // Error response  
    pub const TAG_DATA: u8 = 0x03; // Data payload

    // Future multi-message indicator
    pub const MULTI_MESSAGE_FLAG: u8 = 0xFF;

    // Protocol limits
    pub const MAX_PACKET_SIZE: usize = 64 * 1024; // 64KB
    pub const MAX_PAYLOAD_SIZE: usize = MAX_PACKET_SIZE - 6; // Header space
}

/// Control message types
#[derive(Debug, Clone)]
pub enum ControlMessageType {
    Vote { term: u64, candidate_id: NodeId },
    Heartbeat { term: u64, leader_id: NodeId },
    VariableUpdate { name: String, value: f64 },
    QueueStatusRequest { queue_name: String },
    HealthCheck { node_id: NodeId },
}

/// Control plane messages
#[derive(Debug, Clone)]
pub struct ControlMessage {
    pub message_type: ControlMessageType,
    pub timestamp: Instant,
    pub node_id: NodeId,
    pub message_id: u64,
}

impl Message for ControlMessage {
    fn serialize(&self, config: &ProtocolConfig) -> Result<Vec<u8>, TransportError> {
        // Use default config for now - will be used by protocols
        self.serialize_with_protocol(config)
    }

    fn deserialize(data: &[u8], config: &ProtocolConfig) -> Result<Self, TransportError>
    where
        Self: Sized,
    {
        Self::deserialize_with_protocol(data, config)
    }

    fn timestamp(&self) -> Instant {
        self.timestamp
    }

    fn node_id(&self) -> NodeId {
        self.node_id.clone()
    }

    fn message_id(&self) -> u64 {
        self.message_id
    }
}

impl ProtocolMessage for ControlMessage {
    fn protocol_constants() -> impl std::any::Any {
        protocol::VERSION
    }

    fn get_tag(&self) -> u8 {
        // Control messages could use different tags based on message type in future
        protocol::TAG_DATA // For now treat as data payload
    }

    fn get_payload(&self) -> Vec<u8> {
        // Simple serialization of message type for now
        unimplemented!("Payload serialization not implemented yet")
    }

    fn validate(&self) -> Result<(), TransportError> {
        // Validate message integrity
        if self.message_id == 0 {
            return Err(TransportError::SerializationError {
                source: "Invalid message ID: cannot be zero".to_string(),
            });
        }
        Ok(())
    }

    fn serialize_single_message(&self) -> Result<Vec<u8>, TransportError> {
        self.validate()?;

        let payload = self.get_payload();
        let mut packet = Self::serialize_header(self.get_tag());
        packet.extend_from_slice(&Self::serialize_length(payload.len()));
        packet.extend_from_slice(&payload);
        Ok(packet)
    }

    fn deserialize_single_message(data: &[u8]) -> Result<Self, TransportError>
    where
        Self: Sized,
    {
        let _tag = Self::parse_header(data)?;
        let payload_len = Self::parse_length(data)?;

        if data.len() < 6 + payload_len {
            return Err(TransportError::SerializationError {
                source: format!(
                    "Insufficient data: expected {}, got {}",
                    data.len(),
                    6 + payload_len
                ),
            });
        }

        let payload = &data[6..6 + payload_len];

        // TODO: Implement proper deserialization from payload
        unimplemented!("Message deserialization not implemented yet")
    }
}

/// Control plane response types
#[derive(Debug, Clone)]
pub enum ControlResponse {
    Ack,
    Nak { reason: String },
    Data { payload: Vec<u8> },
}

impl Message for ControlResponse {
    fn serialize(&self, config: &ProtocolConfig) -> Result<Vec<u8>, TransportError> {
        self.serialize_with_protocol(config)
    }

    fn deserialize(data: &[u8], config: &ProtocolConfig) -> Result<Self, TransportError>
    where
        Self: Sized,
    {
        Self::deserialize_with_protocol(data, config)
    }

    fn timestamp(&self) -> Instant {
        std::time::Instant::now()
    }

    fn node_id(&self) -> NodeId {
        NodeId("unknown".to_string())
    }

    fn message_id(&self) -> u64 {
        0 // Responses don't need message IDs
    }
}

impl ProtocolMessage for ControlResponse {
    fn protocol_constants() -> impl std::any::Any {
        protocol::VERSION
    }

    fn get_tag(&self) -> u8 {
        match self {
            ControlResponse::Ack => protocol::TAG_ACK,
            ControlResponse::Nak { .. } => protocol::TAG_NAK,
            ControlResponse::Data { .. } => protocol::TAG_DATA,
        }
    }

    fn get_payload(&self) -> Vec<u8> {
        match self {
            ControlResponse::Ack => Vec::new(),
            ControlResponse::Nak { reason } => {
                let mut payload = Vec::new();
                payload.extend_from_slice(&reason.len().to_be_bytes());
                payload.extend_from_slice(reason.as_bytes());
                payload
            }
            ControlResponse::Data { payload } => payload.clone(),
        }
    }

    fn validate(&self) -> Result<(), TransportError> {
        // Validate response integrity
        if let ControlResponse::Nak { reason } = self {
            if reason.is_empty() {
                return Err(TransportError::SerializationError {
                    source: "Nak response cannot have empty reason".to_string(),
                });
            }
        }
        Ok(())
    }

    fn serialize_single_message(&self) -> Result<Vec<u8>, TransportError> {
        self.validate()?;

        let tag = self.get_tag();
        let payload = self.get_payload();

        let mut packet = Self::serialize_header(tag);
        packet.extend_from_slice(&Self::serialize_length(payload.len()));
        packet.extend_from_slice(&payload);
        Ok(packet)
    }

    fn deserialize_single_message(data: &[u8]) -> Result<Self, TransportError>
    where
        Self: Sized,
    {
        let tag = Self::parse_header(data)?;
        let payload_len = Self::parse_length(data)?;

        if data.len() < 6 + payload_len {
            return Err(TransportError::SerializationError {
                source: "Insufficient data for response".to_string(),
            });
        }

        let payload = &data[6..6 + payload_len];

        match tag {
            protocol::TAG_ACK => Ok(ControlResponse::Ack),
            protocol::TAG_NAK => {
                if payload_len < 4 {
                    return Err(TransportError::SerializationError {
                        source: "Nak payload too short for length".to_string(),
                    });
                }

                let reason_len = u32::from_be_bytes(payload[0..4].try_into().unwrap()) as usize;
                if payload_len < 4 + reason_len {
                    return Err(TransportError::SerializationError {
                        source: "Nak payload too short for reason".to_string(),
                    });
                }

                let reason =
                    String::from_utf8(payload[4..4 + reason_len].to_vec()).map_err(|e| {
                        TransportError::SerializationError {
                            source: format!("Invalid UTF-8 in Nak reason: {}", e).to_string(),
                        }
                    })?;

                Ok(ControlResponse::Nak { reason })
            }
            protocol::TAG_DATA => Ok(ControlResponse::Data {
                payload: payload.to_vec(),
            }),
            _ => Err(TransportError::SerializationError {
                source: format!("Unknown response tag: {}", tag).to_string(),
            }),
        }
    }
}

/// Data plane messages
#[derive(Debug, Clone)]
pub struct DataMessage {
    pub stream_id: StreamId,
    pub data: Vec<u8>,
    pub timestamp: Instant,
    pub node_id: NodeId,
    pub message_id: u64,
}

impl Message for DataMessage {
    fn serialize(&self, config: &ProtocolConfig) -> Result<Vec<u8>, TransportError> {
        self.serialize_with_protocol(config)
    }

    fn deserialize(data: &[u8], config: &ProtocolConfig) -> Result<Self, TransportError>
    where
        Self: Sized,
    {
        Self::deserialize_with_protocol(data, config)
    }

    fn timestamp(&self) -> Instant {
        self.timestamp
    }

    fn node_id(&self) -> NodeId {
        self.node_id.clone()
    }

    fn message_id(&self) -> u64 {
        self.message_id
    }
}

impl ProtocolMessage for DataMessage {
    fn protocol_constants() -> impl std::any::Any {
        protocol::VERSION
    }

    fn get_tag(&self) -> u8 {
        protocol::TAG_DATA // All data messages use data tag
    }

    fn get_payload(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&self.stream_id.0.to_be_bytes());
        payload.extend_from_slice(&self.data.len().to_be_bytes());
        payload.extend_from_slice(&self.data);
        payload
    }

    fn validate(&self) -> Result<(), TransportError> {
        if self.data.is_empty() {
            return Err(TransportError::SerializationError {
                source: "Data message cannot have empty payload".to_string(),
            });
        }

        if self.data.len() > protocol::MAX_PAYLOAD_SIZE {
            return Err(TransportError::SerializationError {
                source: format!(
                    "Data exceeds maximum payload size: {} > {}",
                    self.data.len(),
                    protocol::MAX_PAYLOAD_SIZE
                )
                .to_string(),
            });
        }

        Ok(())
    }

    fn serialize_single_message(&self) -> Result<Vec<u8>, TransportError> {
        self.validate()?;

        let tag = self.get_tag();
        let payload = self.get_payload();

        let mut packet = Self::serialize_header(tag);
        packet.extend_from_slice(&Self::serialize_length(payload.len()));
        packet.extend_from_slice(&payload);
        Ok(packet)
    }

    fn deserialize_single_message(data: &[u8]) -> Result<Self, TransportError>
    where
        Self: Sized,
    {
        let _tag = Self::parse_header(data)?;
        let payload_len = Self::parse_length(data)?;

        if data.len() < 6 + payload_len {
            return Err(TransportError::SerializationError {
                source: "Insufficient data for data message".to_string(),
            });
        }

        let payload = &data[6..6 + payload_len];

        if payload_len < 12 {
            return Err(TransportError::SerializationError {
                source: "Data payload too short for headers".to_string(),
            });
        }

        // Parse stream_id (8 bytes) + data_len (4 bytes)
        let stream_id_bytes: [u8; 8] = payload[0..8].try_into().unwrap();
        let stream_id = u64::from_be_bytes(stream_id_bytes);

        let data_len_bytes: [u8; 4] = payload[8..12].try_into().unwrap();
        let data_len = u32::from_be_bytes(data_len_bytes) as usize;

        if payload_len < 12 + data_len {
            return Err(TransportError::SerializationError {
                source: "Data payload inconsistent with declared length".to_string(),
            });
        }

        let actual_data = payload[12..12 + data_len].to_vec();

        Ok(DataMessage {
            stream_id: StreamId(stream_id),
            data: actual_data,
            timestamp: std::time::Instant::now(),
            node_id: NodeId("unknown".to_string()),
            message_id: 0, // Will be set by caller
        })
    }
}

/// Health status for Raft heartbeat
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub node_id: NodeId,
    pub is_healthy: bool,
    pub timestamp: Instant,
}

/// Foundation trait shared by both control and data planes
pub trait Transport: Send + Sync {
    /// Local socket address
    fn local_addr(&self) -> SocketAddr;

    /// Remote socket address
    fn remote_addr(&self) -> SocketAddr;

    /// Connection health status
    fn is_connected(&self) -> bool;

    /// Graceful shutdown
    fn close(&mut self) -> Result<(), TransportError>;

    /// Connection health check
    fn health_check(&self) -> HealthStatus;
}

/// Control plane - reliable coordination, small messages
pub trait ControlPlaneTransport: Transport {
    /// Send control message (fire and forget)
    fn send_control(&mut self, message: ControlMessage) -> Result<(), TransportError>;

    /// Receive control message
    fn recv_control(&mut self) -> Result<ControlMessage, TransportError>;

    /// Send and wait for response (request/response pattern)
    fn send_control_and_wait(
        &mut self,
        message: ControlMessage,
    ) -> Result<ControlResponse, TransportError>;

    /// Get current retry configuration
    fn max_retries(&self) -> u32;

    /// Get timeout configuration
    fn timeout_duration(&self) -> Duration;
}

/// Data plane - high-throughput streaming, large payloads
pub trait DataPlaneTransport: Transport {
    /// Send data message
    fn send_data(&mut self, message: DataMessage) -> Result<(), TransportError>;

    /// Receive data message
    fn recv_data(&mut self) -> Result<DataMessage, TransportError>;
}

/// Factory for creating plane-specific transports
pub trait TransportFactory: Send + Sync {
    type ControlTransport: ControlPlaneTransport;
    type DataTransport: DataPlaneTransport;

    /// Create control transport from shared connection
    fn create_control_transport(&self) -> Result<Self::ControlTransport, FactoryError>;

    /// Create data transport from shared connection
    fn create_data_transport(&self) -> Result<Self::DataTransport, FactoryError>;
}

// Supporting types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StreamId(pub u64);

#[derive(Debug, Clone)]
pub struct NodeId(pub String);

impl From<&str> for NodeId {
    fn from(s: &str) -> Self {
        NodeId(s.to_string())
    }
}

/// Factory errors
#[derive(Debug, Clone)]
pub enum FactoryError {
    ConnectionFailed { reason: String },
    ConfigurationError { issue: String },
    ResourceUnavailable { resource: String },
}

impl fmt::Display for FactoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FactoryError::ConnectionFailed { reason } => {
                write!(f, "Connection establishment failed: {}", reason)
            }
            FactoryError::ConfigurationError { issue } => {
                write!(f, "Configuration error: {}", issue)
            }
            FactoryError::ResourceUnavailable { resource } => {
                write!(f, "Resource unavailable: {}", resource)
            }
        }
    }
}

impl Error for FactoryError {}
