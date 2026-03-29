//! TCP-based Raft network implementation for production multi-node deployment.
//!
//! Provides reliable RPC communication between Raft nodes using length-prefixed
//! binary protocol with bincode serialization.
//!
//! # Architecture
//!
//! - **TcpNetworkFactory**: Factory for creating TCP network clients for specific peers
//! - **TcpNetwork**: Client for sending RPCs to a specific peer
//! - **TcpRaftServer**: Server for receiving incoming RPCs from peers
//! - **Message Framing**: Length-prefixed binary messages

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info};

use openraft::network::{RaftNetwork, RaftNetworkFactory, RPCOption};
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse,
    InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
};
use openraft::error::{InstallSnapshotError, RaftError, RPCError, NetworkError};
use openraft::impls::BasicNode;

use crate::cluster::types::TypeConfig;
use crate::cluster::raft_router::RaftMessageRouter;

/// Maximum allowed message size (16MB)
const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

/// Default connection timeout
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Network error for TCP operations
#[derive(Debug, Clone)]
struct TcpNetworkError(String);

impl std::fmt::Display for TcpNetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for TcpNetworkError {}

/// RPC message type codes
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RpcType {
    AppendEntries = 1,
    Vote = 2,
    InstallSnapshot = 3,
}

impl TryFrom<u8> for RpcType {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(RpcType::AppendEntries),
            2 => Ok(RpcType::Vote),
            3 => Ok(RpcType::InstallSnapshot),
            _ => Err(format!("Unknown RPC type: {}", value)),
        }
    }
}

/// TCP network configuration for production deployments
#[derive(Debug, Clone)]
pub struct TcpNetworkConfig {
    /// Address to bind for incoming connections (e.g., "0.0.0.0:9090")
    pub bind_addr: String,
    /// Peer addresses by node ID (node_id -> "host:port")
    pub peers: HashMap<u64, String>,
    /// Connection and operation timeout
    pub timeout: Duration,
    /// Maximum connections per peer
    pub max_connections_per_peer: usize,
}

impl Default for TcpNetworkConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:9090".to_string(),
            peers: HashMap::new(),
            timeout: DEFAULT_TIMEOUT,
            max_connections_per_peer: 3,
        }
    }
}

impl TcpNetworkConfig {
    /// Create a new configuration with the given port
    pub fn new(_node_id: u64, port: u16) -> Self {
        Self {
            bind_addr: format!("0.0.0.0:{}", port),
            peers: HashMap::new(),
            timeout: DEFAULT_TIMEOUT,
            max_connections_per_peer: 3,
        }
    }

    /// Add a peer to the configuration
    pub fn add_peer(&mut self, node_id: u64, addr: String) {
        self.peers.insert(node_id, addr);
    }
}

/// TCP network factory for creating client connections to Raft peers
#[derive(Clone)]
pub struct TcpNetworkFactory {
    /// Network configuration
    config: TcpNetworkConfig,
}

impl TcpNetworkFactory {
    /// Create a new TCP network factory
    pub fn new(config: TcpNetworkConfig) -> Self {
        Self { config }
    }
}

impl RaftNetworkFactory<TypeConfig> for TcpNetworkFactory {
    type Network = TcpNetwork;

    async fn new_client(&mut self, target: u64, _node: &BasicNode) -> Self::Network {
        TcpNetwork::new(target, self.config.clone())
    }
}

/// TCP-based Raft network client for sending RPCs to a specific peer
pub struct TcpNetwork {
    /// Target node ID
    target: u64,
    /// Network configuration
    config: TcpNetworkConfig,
}

impl TcpNetwork {
    /// Create a new TCP network instance for a specific target
    pub fn new(target: u64, config: TcpNetworkConfig) -> Self {
        Self { target, config }
    }

    /// Get connection to the target peer
    async fn get_connection(&self) -> Result<TcpStream, TcpNetworkError> {
        // Get peer address
        let addr = self
            .config
            .peers
            .get(&self.target)
            .ok_or_else(|| TcpNetworkError(format!("Unknown peer {}", self.target)))?
            .clone();

        // Connect with timeout
        let stream = tokio::time::timeout(self.config.timeout, TcpStream::connect(&addr))
            .await
            .map_err(|_| TcpNetworkError(format!("Connection timeout to {}", addr)))?
            .map_err(|e| TcpNetworkError(format!("Connection failed: {}", e)))?;

        info!(target = self.target, %addr, "Connected to peer");

        Ok(stream)
    }

    /// Send RPC and receive response using simple length-prefixed protocol
    async fn send_rpc<Req: serde::Serialize + Send, Resp: serde::de::DeserializeOwned + Send>(
        &self,
        rpc_type: RpcType,
        request: &Req,
    ) -> Result<Resp, TcpNetworkError> {
        let mut stream = self.get_connection().await?;

        // Serialize request with type prefix
        let mut buf = Vec::new();
        buf.push(rpc_type as u8);
        buf.extend_from_slice(
            &bincode::serialize(request).map_err(|e| TcpNetworkError(format!("Serialize error: {}", e)))?,
        );

        // Send length + data with timeout
        let len = buf.len() as u32;
        let mut len_bytes = [0u8; 4];
        len_bytes.copy_from_slice(&len.to_be_bytes());

        tokio::time::timeout(self.config.timeout, async {
            stream.write_all(&len_bytes).await.map_err(|e| TcpNetworkError(format!("Write length error: {}", e)))?;
            stream.write_all(&buf).await.map_err(|e| TcpNetworkError(format!("Write data error: {}", e)))?;
            stream.flush().await.map_err(|e| TcpNetworkError(format!("Flush error: {}", e)))
        })
        .await
        .map_err(|_| TcpNetworkError(format!("Send timeout to peer {}", self.target)))??;

        // Receive response length
        let mut response_len_bytes = [0u8; 4];
        tokio::time::timeout(self.config.timeout, stream.read_exact(&mut response_len_bytes))
            .await
            .map_err(|_| TcpNetworkError(format!("Receive timeout from peer {}", self.target)))?
            .map_err(|e| TcpNetworkError(format!("Read length error: {}", e)))?;

        let response_len = u32::from_be_bytes(response_len_bytes) as usize;
        if response_len > MAX_MESSAGE_SIZE {
            return Err(TcpNetworkError(format!("Response too large: {} bytes", response_len)));
        }

        // Receive response data
        let mut response_buf = vec![0u8; response_len];
        tokio::time::timeout(self.config.timeout, stream.read_exact(&mut response_buf))
            .await
            .map_err(|_| TcpNetworkError(format!("Receive data timeout from peer {}", self.target)))?
            .map_err(|e| TcpNetworkError(format!("Read data error: {}", e)))?;

        // Deserialize response
        bincode::deserialize(&response_buf).map_err(|e| TcpNetworkError(format!("Deserialize error: {}", e)))
    }
}

impl RaftNetwork<TypeConfig> for TcpNetwork {
    async fn append_entries(
        &mut self,
        rpc: AppendEntriesRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<AppendEntriesResponse<u64>, RPCError<u64, openraft::impls::BasicNode, RaftError<u64>>> {
        self.send_rpc(RpcType::AppendEntries, &rpc)
            .await
            .map_err(|e| RPCError::Network(NetworkError::new(&e)))
    }

    async fn install_snapshot(
        &mut self,
        rpc: InstallSnapshotRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<InstallSnapshotResponse<u64>, RPCError<u64, openraft::impls::BasicNode, RaftError<u64, InstallSnapshotError>>> {
        self.send_rpc(RpcType::InstallSnapshot, &rpc)
            .await
            .map_err(|e| RPCError::Network(NetworkError::new(&e)))
    }

    async fn vote(
        &mut self,
        rpc: VoteRequest<u64>,
        _option: RPCOption,
    ) -> Result<VoteResponse<u64>, RPCError<u64, openraft::impls::BasicNode, RaftError<u64>>> {
        self.send_rpc(RpcType::Vote, &rpc)
            .await
            .map_err(|e| RPCError::Network(NetworkError::new(&e)))
    }
}

/// TCP server for receiving Raft RPCs from peers
pub struct TcpRaftServer {
    listener: TcpListener,
    node_id: u64,
}

impl TcpRaftServer {
    /// Bind to the specified address
    pub async fn bind(addr: &str, node_id: u64) -> Result<Self, String> {
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| format!("Failed to bind TCP server: {}", e))?;

        info!(%addr, node_id, "TCP Raft server listening");

        Ok(Self { listener, node_id })
    }

    /// Get the local address the server is bound to
    pub fn local_addr(&self) -> Result<String, String> {
        self.listener
            .local_addr()
            .map(|a| a.to_string())
            .map_err(|e| format!("Failed to get local address: {}", e))
    }

    /// Run the server, handling incoming connections
    pub async fn run<F>(self, handler: F)
    where
        F: Fn(u8, &[u8]) -> Result<Vec<u8>, String> + Send + Sync + 'static,
    {
        let handler = Arc::new(handler);

        loop {
            match self.listener.accept().await {
                Ok((stream, addr)) => {
                    debug!(%addr, node_id = self.node_id, "Accepting connection");
                    let handler = Arc::clone(&handler);

                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(stream, handler).await {
                            debug!(%addr, error = %e, "Connection error");
                        }
                    });
                }
                Err(e) => {
                    error!(error = %e, "Accept error");
                }
            }
        }
    }

    /// Handle a single connection
    async fn handle_connection<F>(mut stream: TcpStream, handler: Arc<F>) -> Result<(), String>
    where
        F: Fn(u8, &[u8]) -> Result<Vec<u8>, String> + Send + Sync,
    {
        loop {
            // Read message length (long timeout for idle connections)
            let mut len_bytes = [0u8; 4];
            tokio::time::timeout(Duration::from_secs(30), stream.read_exact(&mut len_bytes))
                .await
                .map_err(|_| "Read timeout".to_string())?
                .map_err(|e| format!("Read error: {}", e))?;
            let len = u32::from_be_bytes(len_bytes) as usize;

            // Check size limit
            if len > MAX_MESSAGE_SIZE {
                return Err(format!("Message too large: {} bytes", len));
            }

            // Read message data
            let mut buf = vec![0u8; len];
            stream.read_exact(&mut buf).await
                .map_err(|e| format!("Read data error: {}", e))?;

            // Extract RPC type
            if buf.is_empty() {
                return Err("Empty message".to_string());
            }

            let rpc_type = buf[0];
            let request_data = &buf[1..];

            // Handle RPC
            let response = handler(rpc_type, request_data)?;

            // Send response length + data
            let response_len = response.len() as u32;
            let mut response_len_bytes = [0u8; 4];
            response_len_bytes.copy_from_slice(&response_len.to_be_bytes());

            stream.write_all(&response_len_bytes).await
                .map_err(|e| format!("Write length error: {}", e))?;
            stream.write_all(&response).await
                .map_err(|e| format!("Write data error: {}", e))?;
            stream.flush().await
                .map_err(|e| format!("Flush error: {}", e))?;
        }
    }
}

/// Create a Raft message handler for the TCP server
pub fn create_raft_handler(
    router: Arc<RaftMessageRouter>,
) -> impl Fn(u8, &[u8]) -> Result<Vec<u8>, String> + Send + Sync + 'static {
    move |rpc_type: u8, data: &[u8]| {
        match rpc_type {
            1 => handle_append_entries(&router, data),
            2 => handle_vote(&router, data),
            3 => handle_install_snapshot(&router, data),
            _ => Err(format!("Unknown RPC type: {}", rpc_type)),
        }
    }
}

fn handle_append_entries(router: &Arc<RaftMessageRouter>, data: &[u8]) -> Result<Vec<u8>, String> {
    let request: AppendEntriesRequest<TypeConfig> = bincode::deserialize(data)
        .map_err(|e| format!("Deserialize error: {}", e))?;

    // Use tokio runtime to run async
    let rt = tokio::runtime::Handle::try_current()
        .map_err(|_| "No tokio runtime".to_string())?;

    let response = rt.block_on(async {
        router.handle_append_entries(request).await
    }).map_err(|e| format!("Raft error: {:?}", e))?;

    bincode::serialize(&response).map_err(|e| format!("Serialize error: {}", e))
}

fn handle_vote(router: &Arc<RaftMessageRouter>, data: &[u8]) -> Result<Vec<u8>, String> {
    let request: VoteRequest<u64> = bincode::deserialize(data)
        .map_err(|e| format!("Deserialize error: {}", e))?;

    let rt = tokio::runtime::Handle::try_current()
        .map_err(|_| "No tokio runtime".to_string())?;

    let response = rt.block_on(async {
        router.handle_vote(request).await
    }).map_err(|e| format!("Raft error: {:?}", e))?;

    bincode::serialize(&response).map_err(|e| format!("Serialize error: {}", e))
}

fn handle_install_snapshot(router: &Arc<RaftMessageRouter>, data: &[u8]) -> Result<Vec<u8>, String> {
    let request: InstallSnapshotRequest<TypeConfig> = bincode::deserialize(data)
        .map_err(|e| format!("Deserialize error: {}", e))?;

    let rt = tokio::runtime::Handle::try_current()
        .map_err(|_| "No tokio runtime".to_string())?;

    let response = rt.block_on(async {
        router.handle_install_snapshot(request).await
    }).map_err(|e| format!("Raft error: {:?}", e))?;

    bincode::serialize(&response).map_err(|e| format!("Serialize error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_type_conversion() {
        assert_eq!(RpcType::try_from(1).unwrap(), RpcType::AppendEntries);
        assert_eq!(RpcType::try_from(2).unwrap(), RpcType::Vote);
        assert_eq!(RpcType::try_from(3).unwrap(), RpcType::InstallSnapshot);
        assert!(RpcType::try_from(99).is_err());
    }

    #[test]
    fn test_config_default() {
        let config = TcpNetworkConfig::default();
        assert_eq!(config.bind_addr, "0.0.0.0:9090");
        assert!(config.peers.is_empty());
        assert_eq!(config.timeout, DEFAULT_TIMEOUT);
    }

    #[test]
    fn test_config_add_peer() {
        let mut config = TcpNetworkConfig::default();
        config.add_peer(2, "127.0.0.1:9091".to_string());
        config.add_peer(3, "127.0.0.1:9092".to_string());

        assert_eq!(config.peers.len(), 2);
        assert_eq!(config.peers.get(&2), Some(&"127.0.0.1:9091".to_string()));
    }

    #[test]
    fn test_config_new() {
        let config = TcpNetworkConfig::new(1, 8888);
        assert_eq!(config.bind_addr, "0.0.0.0:8888");
    }

    #[test]
    fn test_tcp_network_creation() {
        let config = TcpNetworkConfig::default();
        let network = TcpNetwork::new(1, config);
        assert_eq!(network.target, 1);
    }

    #[tokio::test]
    async fn test_tcp_server_bind() {
        let result = TcpRaftServer::bind("127.0.0.1:0", 1).await;
        assert!(result.is_ok());

        let server = result.unwrap();
        let addr = server.local_addr().unwrap();
        assert!(addr.contains("127.0.0.1"));
    }

    // Note: Integration tests for RPC handlers with actual Raft are in the cluster module tests.
    // The create_raft_handler function requires a live Raft instance, which is tested there.
}