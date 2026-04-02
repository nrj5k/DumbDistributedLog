//! Distributed Test Utilities
//!
//! Provides helpers for specifying and setting up distributed test clusters
//! with flexible node specification formats.

use ddl::cluster::raft_cluster::RaftClusterNode;
use ddl::cluster::types::NodeConfig;
use std::collections::{BTreeSet, HashMap};
use std::env;

/// Default port for node communication (same as constants.rs)
pub const DEFAULT_COMMUNICATION_PORT: u16 = 6967;
/// Default port for coordination (Raft)
#[allow(dead_code)]
pub const DEFAULT_COORDINATION_PORT: u16 = 6970;
/// Default port offset between communication and coordination
pub const DEFAULT_PORT_OFFSET: u16 = 3;

/// Parse node specifications like:
/// - "ares-comp-[13-16]" → ares-comp-13, ares-comp-14, ares-comp-15, ares-comp-16
/// - "node[1-5]" → node1, node2, node3, node4, node5
/// - "localhost:8080,localhost:8081" → explicit list with ports
/// - "10.0.0.[1-3]:9090" → IP ranges with ports
/// - "node[1,3-5,7]" → mixed ranges and single values
/// - "192.168.1.[10-20]:8080" → complex IP ranges
pub fn parse_node_spec(spec: &str) -> Result<Vec<String>, String> {
    let spec = spec.trim();
    
    if spec.is_empty() {
        return Err("Empty node specification".to_string());
    }
    
    // Check for empty brackets before parsing
    if spec.contains("[]") {
        return Err("Empty brackets [] are not valid in node specification".to_string());
    }
    
    // Split by comma for multiple specs, but respect brackets
    let parts = split_respecting_brackets(spec);
    let mut result = Vec::new();
    
    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        
        // Check if it has a port specification
        let (host_part, port) = extract_port(part)?;
        
        // Parse the host part (may contain ranges)
        let hosts = parse_host_with_ranges(&host_part)?;
        
        // Apply port to all hosts if specified
        if let Some(p) = port {
            for host in hosts {
                result.push(format!("{}:{}", host, p));
            }
        } else {
            result.extend(hosts);
        }
    }
    
    if result.is_empty() {
        return Err(format!("No valid hosts found in specification: {}", spec));
    }
    
    Ok(result)
}

/// Split on commas, but only those outside of brackets
fn split_respecting_brackets(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut bracket_depth: i32 = 0;
    
    for c in s.chars() {
        match c {
            '[' => {
                bracket_depth += 1;
                current.push(c);
            }
            ']' => {
                bracket_depth = bracket_depth.saturating_sub(1);
                current.push(c);
            }
            ',' if bracket_depth == 0 => {
                if !current.trim().is_empty() {
                    parts.push(current.trim().to_string());
                }
                current.clear();
            }
            _ => {
                current.push(c);
            }
        }
    }
    
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    
    parts
}

/// Extract port from specification like "host:8080"
fn extract_port(spec: &str) -> Result<(String, Option<u16>), String> {
    // Check for IPv6 addresses first (they contain colons)
    if spec.starts_with('[') {
        // IPv6 format: [2001:db8::1]:8080 or [2001:db8::1]
        if let Some(bracket_end) = spec.find(']') {
            let host_part = &spec[..=bracket_end];
            let rest = &spec[bracket_end + 1..];
            
            if rest.starts_with(':') {
                let port_str = &rest[1..];
                let port: u16 = port_str.parse()
                    .map_err(|_| format!("Invalid port: {}", port_str))?;
                Ok((host_part.to_string(), Some(port)))
            } else if rest.is_empty() {
                Ok((host_part.to_string(), None))
            } else {
                Err(format!("Invalid IPv6 specification: {}", spec))
            }
        } else {
            Err(format!("Invalid IPv6 specification: {}", spec))
        }
    } else {
        // Regular host:port or just host
        let parts: Vec<&str> = spec.rsplitn(2, ':').collect();
        
        if parts.len() == 2 {
            // Check if the second part is actually a port number
            if let Ok(port) = parts[0].parse::<u16>() {
                Ok((parts[1].to_string(), Some(port)))
            } else {
                // Not a valid port, treat entire spec as host
                Ok((spec.to_string(), None))
            }
        } else {
            Ok((spec.to_string(), None))
        }
    }
}

/// Parse host part with range expansions
fn parse_host_with_ranges(host: &str) -> Result<Vec<String>, String> {
    let mut result = vec![host.to_string()];
    
    // Keep expanding ranges until no more changes
    loop {
        let mut new_result = Vec::new();
        let mut changed = false;
        
        for current in result {
            if let Some((prefix, range, suffix)) = find_range(&current) {
                let expanded = expand_range(range)?;
                for value in expanded {
                    new_result.push(format!("{}{}{}", prefix, value, suffix));
                }
                changed = true;
            } else {
                new_result.push(current);
            }
        }
        
        result = new_result;
        
        if !changed {
            break;
        }
    }
    
    Ok(result)
}

/// Find a range pattern [N-M] or [N,M-P,Q] in the string
fn find_range(s: &str) -> Option<(&str, &str, &str)> {
    let open_pos = s.find('[')?;
    let close_pos = s.find(']')?;
    
    if open_pos >= close_pos {
        return None;
    }
    
    let prefix = &s[..open_pos];
    let range = &s[open_pos + 1..close_pos];
    let suffix = &s[close_pos + 1..];
    
    // Empty brackets are invalid
    if range.trim().is_empty() {
        return None;
    }
    
    Some((prefix, range, suffix))
}

/// Expand a range specification like "1-5" or "1,3-5,7"
fn expand_range(range: &str) -> Result<Vec<String>, String> {
    let mut result = Vec::new();
    
    // Split by comma for multiple parts
    for part in range.split(',') {
        let part = part.trim();
        
        if part.contains('-') {
            // Range like "1-5"
            let dash_parts: Vec<&str> = part.split('-').collect();
            if dash_parts.len() != 2 {
                return Err(format!("Invalid range in specification: {}", part));
            }
            
            let start_str = dash_parts[0].trim();
            let end_str = dash_parts[1].trim();
            
            // Determine if numeric or alphanumeric
            if let Ok(start) = start_str.parse::<i64>() {
                let end = end_str.parse::<i64>()
                    .map_err(|_| format!("Invalid range end: {}", end_str))?;
                
                // Detect if we should preserve leading zeros
                let width = if start_str.starts_with('0') || start < 0 {
                    Some(start_str.len())
                } else {
                    None
                };
                
                if start > end {
                    return Err(format!("Invalid range: {} > {}", start, end));
                }
                
                for i in start..=end {
                    if let Some(w) = width {
                        result.push(format!("{:0width$}", i, width = w));
                    } else {
                        result.push(i.to_string());
                    }
                }
            } else {
                // Alphanumeric range (like A-Z)
                let start_chars: Vec<char> = start_str.chars().collect();
                let end_chars: Vec<char> = end_str.chars().collect();
                
                if start_chars.len() != 1 || end_chars.len() != 1 {
                    return Err(format!("Unsupported alphanumeric range: {}", part));
                }
                
                let start_char = start_chars[0];
                let end_char = end_chars[0];
                
                if start_char > end_char {
                    return Err(format!("Invalid range: {} > {}", start_char, end_char));
                }
                
                for c in start_char..=end_char {
                    result.push(c.to_string());
                }
            }
        } else {
            // Single value
            result.push(part.to_string());
        }
    }
    
    Ok(result)
}

/// Builder for creating NodeConfig from node specifications
pub struct NodeSpecBuilder {
    spec: String,
    base_port: u16,
    comm_port_offset: u16,
    coord_port_offset: u16,
    starting_id: u64,
}

impl NodeSpecBuilder {
    /// Create a new builder with the node specification
    pub fn new(spec: &str) -> Self {
        Self {
            spec: spec.to_string(),
            base_port: DEFAULT_COMMUNICATION_PORT,
            comm_port_offset: 0,
            coord_port_offset: DEFAULT_PORT_OFFSET,
            starting_id: 1,
        }
    }
    
    /// Set the base communication port (default: 6967)
    pub fn base_port(mut self, port: u16) -> Self {
        self.base_port = port;
        self
    }
    
    /// Set the coordination port offset from base port (default: 3)
    pub fn coord_offset(mut self, offset: u16) -> Self {
        self.coord_port_offset = offset;
        self
    }
    
    /// Set the starting node ID (default: 1)
    pub fn starting_id(mut self, id: u64) -> Self {
        self.starting_id = id;
        self
    }
    
    /// Build HashMap of node_id -> NodeConfig from specification
    pub fn build(self) -> Result<HashMap<u64, NodeConfig>, String> {
        let hosts = parse_node_spec(&self.spec)?;
        let mut configs = HashMap::new();
        
        for (idx, host_spec) in hosts.iter().enumerate() {
            let node_id = self.starting_id + idx as u64;
            
            // Parse host and optional port from the host_spec
            let (host, comm_port, coord_port) = parse_host_and_ports(
                host_spec,
                self.base_port,
                self.comm_port_offset,
                self.coord_port_offset,
            )?;
            
            // Each node gets a unique port by incrementing based on index
            // This allows multiple nodes on the same machine (localhost)
            let port_increment = idx as u16;
            
            let config = NodeConfig {
                node_id,
                host,
                communication_port: comm_port + port_increment,
                coordination_port: coord_port + port_increment,
            };
            
            configs.insert(node_id, config);
        }
        
        Ok(configs)
    }
}

/// Parse host specification and extract ports
/// Returns (host, communication_port, coordination_port)
fn parse_host_and_ports(
    host_spec: &str,
    base_port: u16,
    _comm_offset: u16,
    coord_offset: u16,
) -> Result<(String, u16, u16), String> {
    // Check if specification includes a port
    let parts: Vec<&str> = host_spec.rsplitn(2, ':').collect();
    
    if parts.len() == 2 {
        // Has a port
        let port: u16 = parts[0].parse()
            .map_err(|_| format!("Invalid port in specification: {}", parts[0]))?;
        let host = parts[1].to_string();
        
        // For custom port, assume it's the coordination port
        // In a real system, you might want separate port specifications
        Ok((host, port.saturating_sub(coord_offset), port))
    } else {
        // No port specified, use defaults
        Ok((host_spec.to_string(), base_port, base_port + coord_offset))
    }
}

/// Test cluster helper for distributed tests
pub struct TestCluster {
    /// Created Raft nodes (keyed by node_id)
    pub nodes: Vec<RaftClusterNode>,
    /// Node configurations (keyed by node_id)
    pub configs: HashMap<u64, NodeConfig>,
    /// Bootstrap node ID (first node)
    pub bootstrap_id: u64,
    /// Temporary directories for persistence (TCP mode only)
    temp_dirs: Vec<tempfile::TempDir>,
}

impl TestCluster {
    /// Create and initialize a test cluster from node specification
    /// 
    /// # Example
    /// ```ignore
    /// let cluster = TestCluster::setup("node[1-3]").await?;
    /// 
    /// // All nodes are created and ready to use
    /// assert_eq!(cluster.nodes.len(), 3);
    /// ```
    pub async fn setup(spec: &str) -> Result<Self, String> {
        Self::setup_with_options(spec, DEFAULT_COMMUNICATION_PORT, 1).await
    }
    
    /// Create a test cluster with custom options
    pub async fn setup_with_options(
        spec: &str,
        base_port: u16,
        starting_id: u64,
    ) -> Result<Self, String> {
        // Parse node specifications
        let configs = NodeSpecBuilder::new(spec)
            .base_port(base_port)
            .starting_id(starting_id)
            .build()?;
        
        // Create Raft nodes
        let mut nodes = Vec::new();
        let bootstrap_id = starting_id;
        
        for (node_id, config) in &configs {
            let mut node_configs = HashMap::new();
            node_configs.insert(*node_id, config.clone());
            
            let node = RaftClusterNode::new(*node_id, configs.clone())
                .await
                .map_err(|e| format!("Failed to create node {}: {}", node_id, e))?;
            
            nodes.push(node);
        }
        
        Ok(Self {
            nodes,
            configs,
            bootstrap_id,
            temp_dirs: Vec::new(),  // No temp dirs for in-memory
        })
    }
    
    /// Create REAL distributed cluster with TCP networking
    /// 
    /// This spawns actual TCP servers on unique ports for each node,
    /// creating a true multi-node cluster (not in-memory simulation).
    /// 
    /// # Example
    /// ```ignore
    /// let cluster = TestCluster::setup_tcp("node[1-3]").await?;
    /// // All nodes are connected via TCP on real network ports
    /// ```
    pub async fn setup_tcp(spec: &str) -> Result<Self, String> {
        Self::setup_tcp_with_options(spec, DEFAULT_COMMUNICATION_PORT + 10000, 1).await
    }
    
    /// Create TCP cluster with custom options
    pub async fn setup_tcp_with_options(
        spec: &str,
        base_port: u16,
        starting_id: u64,
    ) -> Result<Self, String> {
        eprintln!("setup_tcp_with_options: Parsing node spec '{}'", spec);
        
        // Parse node specifications
        let base_configs = NodeSpecBuilder::new(spec)
            .base_port(base_port)
            .starting_id(starting_id)
            .build()?;
        
        eprintln!("setup_tcp: Parsed {} nodes", base_configs.len());
        
        // Use high ports for TCP coordination (avoid conflicts with default ports)
        // If base_port is 6967, TCP coordination uses 16967
        let tcp_coord_port_offset = 10000u16;
        
        // Adjust configs to reflect actual TCP ports and replace hostnames with localhost
        // For TCP testing, all nodes run on the same machine
        let mut configs = base_configs.clone();
        for (_, config) in configs.iter_mut() {
            config.host = "127.0.0.1".to_string();
            config.coordination_port += tcp_coord_port_offset;
        }
        
        let mut nodes = Vec::new();
        let bootstrap_id = starting_id;
        
        eprintln!("setup_tcp: Creating temp directories");
        
        // Create temporary directories for persistence
        let temp_dirs: Vec<tempfile::TempDir> = (0..configs.len())
            .map(|_| tempfile::TempDir::new().unwrap())
            .collect();
        
        eprintln!("setup_tcp: Creating {} nodes", configs.len());
        
        // Create all nodes with TCP networking
        for (idx, (node_id, config)) in configs.iter().enumerate() {
            eprintln!("setup_tcp: Creating node {} at {}:{}", node_id, config.host, config.coordination_port);
            
            let mut network_config = ddl::network::TcpNetworkConfig::new(*node_id, config.coordination_port);
            
            // Add all other peers
            for (peer_id, peer_config) in &configs {
                if peer_id != node_id {
                    network_config.add_peer(*peer_id, format!("{}:{}", peer_config.host, peer_config.coordination_port));
                }
            }
            
            let (node, _network_factory) = ddl::cluster::RaftClusterNode::new_tcp(
                *node_id,
                configs.clone(),
                network_config,
                temp_dirs[idx].path()
            ).await.map_err(|e| format!("Failed to create node {}: {:?}", node_id, e))?;
            
            eprintln!("setup_tcp: Node {} created successfully", node_id);
            nodes.push(node);
        }
        
        // Wait for TCP servers to start
        eprintln!("setup_tcp: Waiting for TCP servers to start");
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        // Initialize bootstrap node
        eprintln!("setup_tcp: Finding bootstrap node");
        let bootstrap = nodes.iter()
            .find(|n| n.node_id == bootstrap_id)
            .ok_or_else(|| "Bootstrap node not found")?;
        
        eprintln!("setup_tcp: Initializing bootstrap node {}", bootstrap_id);
        bootstrap.initialize().await
            .map_err(|e| format!("Failed to initialize bootstrap: {:?}", e))?;
        
        eprintln!("setup_tcp: Waiting for leader election");
        // Wait for leader election
        let is_leader = tokio::time::timeout(
            tokio::time::Duration::from_secs(10),
            async {
                loop {
                    if bootstrap.is_leader().await {
                        eprintln!("setup_tcp: Bootstrap is leader!");
                        return true;
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }
        ).await.map_err(|_| "Leader election timeout")?;
        
        if !is_leader {
            return Err("Bootstrap failed to become leader".to_string());
        }
        
        eprintln!("setup_tcp: Adding learner nodes");
        // Add other nodes as learners and promote to voters
        for (node_id, _config) in &configs {
            if *node_id != bootstrap_id {
                let addr = format!("{}:{}", 
                    configs.get(node_id).unwrap().host,
                    configs.get(node_id).unwrap().coordination_port
                );
                eprintln!("setup_tcp: Adding learner {} at {}", node_id, addr);
                bootstrap.add_learner(*node_id, addr).await
                    .map_err(|e| format!("Failed to add learner {}: {:?}", node_id, e))?;
            }
        }
        
        eprintln!("setup_tcp: Changing membership to include all nodes");
        // Promote all to voters
        let members: BTreeSet<u64> = configs.keys().cloned().collect();
        bootstrap.change_membership(members, true).await
            .map_err(|e| format!("Failed to change membership: {:?}", e))?;
        
        eprintln!("setup_tcp: Waiting for membership propagation");
        // Wait for membership change to propagate
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        
        eprintln!("setup_tcp: Verifying cluster");
        // Verify cluster
        for node in &nodes {
            let leader = node.current_leader().await;
            if leader.is_none() {
                return Err(format!("Node {} has no leader", node.node_id));
            }
            eprintln!("setup_tcp: Node {} sees leader {:?}", node.node_id, leader);
        }
        
        eprintln!("setup_tcp: Cluster setup complete");
        Ok(Self {
            nodes,
            configs,
            bootstrap_id,
            temp_dirs,  // Keep temp dirs alive for persistence
        })
    }
    
    /// Initialize the cluster (bootstraps the first node)
    pub async fn initialize(&self) -> Result<(), String> {
        // Find and initialize bootstrap node
        let bootstrap_node = self.nodes.iter()
            .find(|n| n.node_id == self.bootstrap_id)
            .ok_or_else(|| format!("Bootstrap node {} not found", self.bootstrap_id))?;
        
        bootstrap_node.initialize().await
            .map_err(|e| format!("Failed to initialize bootstrap node: {}", e))?;
        
        // Wait for leader election
        tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
        
        Ok(())
    }
    
    /// Shutdown all nodes in the cluster
    pub async fn shutdown(&self) -> Result<(), String> {
        for node in &self.nodes {
            node.shutdown().await
                .map_err(|e| format!("Failed to shutdown node {}: {}", node.node_id, e))?;
        }
        Ok(())
    }
    
    /// Get a node by ID
    pub fn get_node(&self, node_id: u64) -> Option<&RaftClusterNode> {
        self.nodes.iter().find(|n| n.node_id == node_id)
    }
    
    /// Get all node IDs
    pub fn node_ids(&self) -> Vec<u64> {
        self.nodes.iter().map(|n| n.node_id).collect()
    }
    
    /// Get the number of nodes
    pub fn size(&self) -> usize {
        self.nodes.len()
    }
}

/// Get node specification from environment variable
/// 
/// Usage:
/// ```bash
/// RUST_TEST_NODES="ares-comp-[13-16]" cargo test --test distributed
/// ```
/// 
/// Or:
/// ```bash
/// export RUST_TEST_NODES="node[1-5]"
/// cargo test
/// ```
pub fn get_node_spec_from_env() -> Option<String> {
    env::var("RUST_TEST_NODES").ok()
}

/// Get node specification or use a default
/// 
/// # Arguments
/// * `default_spec` - Default specification if env var not set
/// 
/// # Example
/// ```ignore
/// // Use RUST_TEST_NODES or fall back to localhost:8080,localhost:8081
/// let spec = get_node_spec_or_default("localhost:8080,localhost:8081");
/// ```
pub fn get_node_spec_or_default(default_spec: &str) -> String {
    get_node_spec_from_env().unwrap_or_else(|| default_spec.to_string())
}

/// Validate a node specification without creating nodes
/// 
/// Useful for checking configuration before setup
pub fn validate_node_spec(spec: &str) -> Result<Vec<(String, Option<u16>)>, String> {
    let nodes = parse_node_spec(spec)?;
    
    nodes.iter().map(|node_spec| {
        let parts: Vec<&str> = node_spec.rsplitn(2, ':').collect();
        if parts.len() == 2 {
            if let Ok(port) = parts[0].parse::<u16>() {
                Ok((parts[1].to_string(), Some(port)))
            } else {
                Ok((node_spec.clone(), None))
            }
        } else {
            Ok((node_spec.clone(), None))
        }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_simple_range() {
        let result = parse_node_spec("node[1-3]").unwrap();
        assert_eq!(result, vec!["node1", "node2", "node3"]);
    }
    
    #[test]
    fn test_parse_named_range() {
        let result = parse_node_spec("ares-comp-[13-16]").unwrap();
        assert_eq!(result, vec!["ares-comp-13", "ares-comp-14", "ares-comp-15", "ares-comp-16"]);
    }
    
    #[test]
    fn test_parse_explicit_list() {
        let result = parse_node_spec("host1,host2,host3").unwrap();
        assert_eq!(result, vec!["host1", "host2", "host3"]);
    }
    
    #[test]
    fn test_parse_with_ports() {
        let result = parse_node_spec("localhost:8080,localhost:8081").unwrap();
        assert_eq!(result, vec!["localhost:8080", "localhost:8081"]);
    }
    
    #[test]
    fn test_parse_ip_range() {
        let result = parse_node_spec("10.0.0.[1-3]:9090").unwrap();
        assert_eq!(result, vec!["10.0.0.1:9090", "10.0.0.2:9090", "10.0.0.3:9090"]);
    }
    
    #[test]
    fn test_parse_mixed_range() {
        let result = parse_node_spec("node[1,3-5,7]").unwrap();
        assert_eq!(result, vec!["node1", "node3", "node4", "node5", "node7"]);
    }
    
    #[test]
    fn test_parse_leading_zeros() {
        let result = parse_node_spec("node[01-03]").unwrap();
        assert_eq!(result, vec!["node01", "node02", "node03"]);
    }
    
    #[test]
    fn test_parse_single_value() {
        let result = parse_node_spec("node[5]").unwrap();
        assert_eq!(result, vec!["node5"]);
    }
    
    #[test]
    fn test_parse_empty_brackets() {
        let result = parse_node_spec("node[]");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_parse_invalid_range() {
        let result = parse_node_spec("node[5-2]");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_node_spec_builder() {
        let configs = NodeSpecBuilder::new("node[1-3]")
            .base_port(8080)
            .coord_offset(10)
            .starting_id(10)
            .build()
            .unwrap();
        
        assert_eq!(configs.len(), 3);
        assert_eq!(configs.get(&10).unwrap().host, "node1");
        assert_eq!(configs.get(&10).unwrap().communication_port, 8080);
        assert_eq!(configs.get(&10).unwrap().coordination_port, 8090);
        
        assert_eq!(configs.get(&12).unwrap().host, "node3");
    }
    
    #[test]
    fn test_node_spec_builder_with_ports() {
        let configs = NodeSpecBuilder::new("host1:9090,host2:9091")
            .base_port(8080)
            .coord_offset(3)
            .build()
            .unwrap();
        
        assert_eq!(configs.len(), 2);
        assert_eq!(configs.get(&1).unwrap().host, "host1");
        // Port specified, coordination uses that port
        assert_eq!(configs.get(&1).unwrap().coordination_port, 9090);
        assert_eq!(configs.get(&2).unwrap().coordination_port, 9091);
    }
    
    #[test]
    fn test_validate_node_spec() {
        let validated = validate_node_spec("node[1-3]").unwrap();
        assert_eq!(validated.len(), 3);
        
        let validated = validate_node_spec("host1:8080,host2").unwrap();
        assert_eq!(validated.len(), 2);
        assert_eq!(validated[0], ("host1".to_string(), Some(8080)));
        assert_eq!(validated[1], ("host2".to_string(), None));
    }
    
    #[test]
    fn test_composed_ranges() {
        // Multiple range expansions in one specification
        let result = parse_node_spec("node[1-2]-cluster[0-1]").unwrap();
        assert_eq!(result.len(), 4);
        assert!(result.contains(&"node1-cluster0".to_string()));
        assert!(result.contains(&"node2-cluster1".to_string()));
    }
    
    #[test]
    fn test_ip_ranges_with_ports() {
        let result = parse_node_spec("192.168.1.[10-12]:8080").unwrap();
        assert_eq!(result, vec![
            "192.168.1.10:8080",
            "192.168.1.11:8080",
            "192.168.1.12:8080"
        ]);
    }
}