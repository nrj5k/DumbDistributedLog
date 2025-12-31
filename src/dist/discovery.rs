//! Cluster discovery for AutoQueues
//!
//! Provides discovery mechanisms for finding cluster nodes.

use std::net::SocketAddr;

/// Discovery method for cluster nodes
#[derive(Debug, Clone)]
pub enum DiscoveryMethod {
    /// Static list of nodes
    Static(Vec<SocketAddr>),
    
    /// Multicast DNS discovery
    Mdns,
}

/// Cluster discovery service
pub struct ClusterDiscovery {
    method: DiscoveryMethod,
    nodes: Vec<SocketAddr>,
}

impl ClusterDiscovery {
    /// Create a new cluster discovery instance
    pub fn new(method: DiscoveryMethod) -> Self {
        let nodes = match &method {
            DiscoveryMethod::Static(nodes) => nodes.clone(),
            DiscoveryMethod::Mdns => Vec::new(),
        };
        
        Self { method, nodes }
    }
    
    /// Discover nodes in the cluster
    pub async fn discover(&mut self) -> Result<Vec<SocketAddr>, Box<dyn std::error::Error>> {
        match &self.method {
            DiscoveryMethod::Static(nodes) => {
                self.nodes = nodes.clone();
                Ok(nodes.clone())
            }
            DiscoveryMethod::Mdns => {
                // In a real implementation, this would use mDNS to discover nodes
                // For now, we'll just return the current nodes
                Ok(self.nodes.clone())
            }
        }
    }
    
    /// Add a node to the discovery service
    pub fn add_node(&mut self, addr: SocketAddr) {
        if !self.nodes.contains(&addr) {
            self.nodes.push(addr);
        }
    }
    
    /// Remove a node from the discovery service
    pub fn remove_node(&mut self, addr: &SocketAddr) {
        self.nodes.retain(|node| node != addr);
    }
    
    /// Get the current list of nodes
    pub fn nodes(&self) -> &[SocketAddr] {
        &self.nodes
    }
}