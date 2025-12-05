//! Simple TCP transport implementation for AutoQueues
//!
//! Provides basic TCP networking following KISS principle.
//! Minimal implementation for distributed queue functionality.

use crate::traits::transport::{Transport, TransportError};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Simple TCP transport implementation
pub struct TcpTransport {
    stream: Option<TcpStream>,
    listener: Option<TcpListener>,
    timeout: Duration,
}

impl Clone for TcpTransport {
    fn clone(&self) -> Self {
        Self {
            stream: None, // Cannot clone TcpStream
            listener: None, // Cannot clone TcpListener
            timeout: self.timeout,
        }
    }
}

impl TcpTransport {
    /// Create new TCP transport
    pub fn new() -> Self {
        Self {
            stream: None,
            listener: None,
            timeout: Duration::from_secs(30),
        }
    }

    /// Create TCP transport with custom timeout
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            stream: None,
            listener: None,
            timeout,
        }
    }

    /// Create TCP transport as server
    pub fn bind(addr: &str) -> Result<Self, TransportError> {
        let listener = TcpListener::bind(addr).map_err(|_| TransportError::NetworkUnreachable)?;

        Ok(Self {
            stream: None,
            listener: Some(listener),
            timeout: Duration::from_secs(30),
        })
    }

    /// Create TCP transport as server with custom timeout
    pub fn bind_with_timeout(addr: &str, timeout: Duration) -> Result<Self, TransportError> {
        let listener = TcpListener::bind(addr).map_err(|_| TransportError::NetworkUnreachable)?;

        Ok(Self {
            stream: None,
            listener: Some(listener),
            timeout,
        })
    }

    /// Accept incoming connection (server mode)
    pub fn accept(&mut self) -> Result<(), TransportError> {
        if let Some(listener) = &self.listener {
            listener
                .set_nonblocking(true)
                .map_err(|_| TransportError::NetworkUnreachable)?;

            let start_time = std::time::Instant::now();
            
            while start_time.elapsed() < self.timeout {
                match listener.accept() {
                    Ok((stream, addr)) => {
                        println!("Connected to: {}", addr);
                        self.stream = Some(stream);
                        return Ok(());
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                    Err(_) => {
                        return Err(TransportError::ConnectionLost {
                            remote_addr: "0.0.0.0:0".parse().unwrap(),
                        });
                    }
                }
            }
            
            Err(TransportError::Timeout { duration: self.timeout })
        } else {
            Err(TransportError::NetworkUnreachable)
        }
    }

    /// Check if transport is connected
    pub fn is_connected(&self) -> bool {
        self.stream.is_some()
    }

    /// Get remote address if connected
    pub fn remote_addr(&self) -> Option<String> {
        self.stream.as_ref().and_then(|s| s.peer_addr().ok()).map(|a| a.to_string())
    }
}

impl Transport for TcpTransport {
    fn send(&mut self, data: &[u8]) -> Result<(), TransportError> {
        if let Some(stream) = &mut self.stream {
            stream
                .write_all(data)
                .map_err(|_| TransportError::ConnectionLost {
                    remote_addr: stream.peer_addr().unwrap_or_else(|_| "0.0.0.0:0".parse().unwrap()),
                })?;
            stream.flush().map_err(|_| TransportError::ConnectionLost {
                remote_addr: stream.peer_addr().unwrap_or_else(|_| "0.0.0.0:0".parse().unwrap()),
            })?;
            Ok(())
        } else {
            Err(TransportError::NetworkUnreachable)
        }
    }

    fn receive(&mut self) -> Result<Vec<u8>, TransportError> {
        if let Some(stream) = &mut self.stream {
            let mut buffer = vec![0u8; 4096];
            
            stream
                .set_read_timeout(Some(self.timeout))
                .map_err(|_| TransportError::NetworkUnreachable)?;

            let bytes_read =
                stream
                    .read(&mut buffer)
                    .map_err(|_| TransportError::ConnectionLost {
                        remote_addr: stream.peer_addr().unwrap_or_else(|_| "0.0.0.0:0".parse().unwrap()),
                    })?;

            if bytes_read == 0 {
                return Err(TransportError::ConnectionLost {
                    remote_addr: stream.peer_addr().unwrap_or_else(|_| "0.0.0.0:0".parse().unwrap()),
                });
            }

            buffer.truncate(bytes_read);
            Ok(buffer)
        } else {
            Err(TransportError::NetworkUnreachable)
        }
    }

    fn connect(&mut self, addr: &str) -> Result<(), TransportError> {
        let stream = TcpStream::connect(addr).map_err(|_| TransportError::NetworkUnreachable)?;
        
        stream
            .set_read_timeout(Some(self.timeout))
            .map_err(|_| TransportError::NetworkUnreachable)?;
            
        stream
            .set_write_timeout(Some(self.timeout))
            .map_err(|_| TransportError::NetworkUnreachable)?;
            
        println!("Connected to: {}", addr);
        self.stream = Some(stream);
        Ok(())
    }
}

/// Simple distributed queue manager
pub struct DistributedQueueManager<T> {
    transport: Box<dyn Transport>,
    local_queue: Arc<Mutex<Vec<T>>>,
    connected: bool,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> DistributedQueueManager<T>
where
    T: serde::Serialize + for<'de> serde::Deserialize<'de> + Send + Sync + 'static,
{
    /// Create new distributed queue manager
    pub fn new(transport: Box<dyn Transport>) -> Self {
        Self {
            transport,
            local_queue: Arc::new(Mutex::new(Vec::new())),
            connected: false,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Send item to remote queue
    pub fn send_item(&mut self, item: &T) -> Result<(), TransportError> {
        if !self.connected {
            return Err(TransportError::NetworkUnreachable);
        }

        let serialized =
            serde_json::to_vec(item).map_err(|e| TransportError::SerializationError {
                source: e.to_string(),
            })?;

        self.transport.send(&serialized)
    }

    /// Receive item from remote queue
    pub fn receive_item(&mut self) -> Result<T, TransportError> {
        if !self.connected {
            return Err(TransportError::NetworkUnreachable);
        }

        let data = self.transport.receive()?;
        let item: T =
            serde_json::from_slice(&data).map_err(|e| TransportError::SerializationError {
                source: e.to_string(),
            })?;
        Ok(item)
    }

    /// Add item to local queue
    pub fn add_local(&self, item: T) {
        let mut queue = self.local_queue.lock().unwrap();
        queue.push(item);
    }

    /// Get local queue items
    pub fn get_local(&self) -> Vec<T>
    where
        T: Clone,
    {
        let queue = self.local_queue.lock().unwrap();
        queue.clone()
    }

    /// Check if connected to remote
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Set connection status
    pub fn set_connected(&mut self, connected: bool) {
        self.connected = connected;
    }

    /// Get local queue size
    pub fn local_size(&self) -> usize {
        self.local_queue.lock().unwrap().len()
    }

    /// Clear local queue
    pub fn clear_local(&self) {
        self.local_queue.lock().unwrap().clear();
    }
}

/// Network node for distributed queue system
pub struct NetworkNode<T> {
    pub manager: DistributedQueueManager<T>,
    transport: TcpTransport,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> NetworkNode<T>
where
    T: serde::Serialize + for<'de> serde::Deserialize<'de> + Send + Sync + 'static,
{
    /// Create new network node
    pub fn new() -> Self {
        let transport = TcpTransport::new();
        let manager = DistributedQueueManager::new(Box::new(transport.clone()));
        
        Self {
            manager,
            transport,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Start as server
    pub fn start_server(&mut self, addr: &str) -> Result<(), TransportError> {
        self.transport = TcpTransport::bind(addr)?;
        self.manager.set_connected(true);
        println!("Server listening on: {}", addr);
        Ok(())
    }

    /// Connect to remote server
    pub fn connect(&mut self, addr: &str) -> Result<(), TransportError> {
        self.transport.connect(addr)?;
        self.manager.set_connected(true);
        Ok(())
    }

    /// Accept incoming connection
    pub fn accept(&mut self) -> Result<(), TransportError> {
        self.transport.accept()?;
        self.manager.set_connected(true);
        Ok(())
    }

    /// Send data to remote
    pub fn send(&mut self, item: &T) -> Result<(), TransportError> {
        self.manager.send_item(item)
    }

    /// Receive data from remote
    pub fn receive(&mut self) -> Result<T, TransportError> {
        self.manager.receive_item()
    }

    /// Add to local queue
    pub fn add_local(&self, item: T) {
        self.manager.add_local(item);
    }

    /// Get local items
    pub fn get_local(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.manager.get_local()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_tcp_transport_creation() {
        let transport = TcpTransport::new();
        assert!(!transport.is_connected());
        assert!(transport.remote_addr().is_none());
    }

    #[test]
    fn test_tcp_transport_with_timeout() {
        let timeout = Duration::from_secs(60);
        let transport = TcpTransport::with_timeout(timeout);
        assert!(!transport.is_connected());
    }

    #[test]
    fn test_distributed_queue_creation() {
        let transport = TcpTransport::new();
        let manager = DistributedQueueManager::<String>::new(Box::new(transport));
        assert!(!manager.is_connected());
        assert_eq!(manager.local_size(), 0);
    }

    #[test]
    fn test_network_node_creation() {
        let node = NetworkNode::<String>::new();
        assert!(!node.manager.is_connected());
        assert_eq!(node.manager.local_size(), 0);
    }

    #[test]
    fn test_local_queue_operations() {
        let node = NetworkNode::<String>::new();
        
        // Add items to local queue
        node.add_local("item1".to_string());
        node.add_local("item2".to_string());
        
        // Check local queue
        let items = node.get_local();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0], "item1");
        assert_eq!(items[1], "item2");
        
        // Clear local queue
        node.manager.clear_local();
        assert_eq!(node.manager.local_size(), 0);
    }
}
