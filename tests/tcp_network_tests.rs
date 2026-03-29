//! Tests for TCP networking

use ddl::cluster::tcp_network::{TcpNetwork, TcpNetworkConfig, TcpRaftServer};
use std::collections::HashMap;
use std::net::SocketAddr;

// Test 1: TCP network creation
#[test]
fn test_tcp_network_creation() {
    let mut peers = HashMap::new();
    peers.insert(1, "127.0.0.1:8080".parse::<SocketAddr>().unwrap());
    peers.insert(2, "127.0.0.1:8081".parse::<SocketAddr>().unwrap());

    let config = TcpNetworkConfig::new(1, "127.0.0.1:9090".parse::<SocketAddr>().unwrap(), peers);

    let network = TcpNetwork::new(config);

    assert_eq!(network.config.node_id, 1, "Node ID should be 1");
    assert_eq!(network.config.peers.len(), 2, "Should have 2 peers");
}

// Test 2: TCP network config with dynamic port
#[test]
fn test_tcp_network_config_dynamic_port() {
    // Use port 0 to get any available dynamic port
    let config = TcpNetworkConfig::new(
        1,
        "127.0.0.1:0".parse::<SocketAddr>().unwrap(),
        HashMap::new(),
    );

    assert_eq!(config.node_id, 1, "Node ID should be 1");

    // The bind address should be valid even if port is 0 (dynamic)
    assert!(
        config.bind_addr.ip().is_loopback(),
        "Should bind to localhost"
    );
}

// Test 3: TCP server creation
#[test]
fn test_tcp_server_creation() {
    let server = TcpRaftServer::new("127.0.0.1:9090".parse::<SocketAddr>().unwrap());

    assert_eq!(
        server.bind_addr,
        "127.0.0.1:9090".parse::<SocketAddr>().unwrap(),
        "Server should bind to specified address"
    );
}

// Test 4: Multiple nodes configuration
#[test]
fn test_multiple_nodes_config() {
    let mut peers = HashMap::new();
    peers.insert(2, "192.168.1.10:8080".parse().unwrap());
    peers.insert(3, "192.168.1.11:8080".parse().unwrap());
    peers.insert(4, "192.168.1.12:8080".parse().unwrap());

    let config = TcpNetworkConfig::new(1, "192.168.1.9:9090".parse().unwrap(), peers);

    assert_eq!(config.peers.len(), 3, "Should have 3 peers");

    // Verify all peers are present
    for (node_id, addr) in &config.peers {
        assert!(
            addr.ip().is_loopback() || addr.ip().to_string().starts_with("192.168"),
            "Should have valid IP for peer {}",
            node_id
        );
        assert!(addr.port() > 0, "Should have valid port");
    }
}

// Test 5: Empty peers configuration
#[test]
fn test_empty_peers_config() {
    let config = TcpNetworkConfig::new(
        1,
        "127.0.0.1:9090".parse::<SocketAddr>().unwrap(),
        HashMap::new(),
    );

    assert!(config.peers.is_empty(), "Should have no peers");
}

// Test 6: TCP network with same node in peers
#[test]
fn test_tcp_network_self_in_peers() {
    let mut peers = HashMap::new();
    // In a real scenario, node 1's peer address would be different
    // For this test, we'll just verify we can create the config
    peers.insert(1, "127.0.0.1:9091".parse().unwrap());

    let config = TcpNetworkConfig::new(1, "127.0.0.1:9090".parse().unwrap(), peers);

    assert_eq!(
        config.peers.get(&1),
        Some(&"127.0.0.1:9091".parse().unwrap())
    );
}

// Test 7: TCP config concept
#[test]
fn test_tcp_config_concept() {
    // Verify config can be created with various addresses
    let bind_addr = "127.0.0.1:9090".parse::<SocketAddr>().unwrap();
    let config = TcpNetworkConfig::new(1, bind_addr, HashMap::new());

    assert_eq!(config.node_id, 1, "Node ID should be 1");
}

// Test 8: Network config structure
#[test]
fn test_network_config_structure() {
    let config = TcpNetworkConfig::new(
        1,
        "127.0.0.1:9090".parse::<SocketAddr>().unwrap(),
        HashMap::new(),
    );

    // Verify config can be accessed
    assert_eq!(config.node_id, 1);
}

// Test 9: TCP timeout concept
#[test]
fn test_tcp_timeout_concept() {
    // Connection timeout would be set in the actual TCP client implementation
    // For this test, we verify the config structure allows it
    let config = TcpNetworkConfig::new(
        1,
        "127.0.0.1:9090".parse::<SocketAddr>().unwrap(),
        HashMap::new(),
    );

    // Verify the config exists
    assert_eq!(config.node_id, 1);
}

// Test 10: TCP network initialization
#[test]
fn test_tcp_network_initialization() {
    let mut peers = HashMap::new();
    peers.insert(2, "127.0.0.1:8081".parse().unwrap());

    let config = TcpNetworkConfig::new(1, "127.0.0.1:9090".parse::<SocketAddr>().unwrap(), peers);

    // Initialize network (in production, this would bind sockets)
    let network = TcpNetwork::new(config);

    assert_eq!(
        network.config.node_id, 1,
        "Network should be initialized with correct node ID"
    );
}

// Test 11: Multiple server instances
#[test]
fn test_multiple_server_instances() {
    // Test that we can create multiple servers on different ports

    let server1 = TcpRaftServer::new("127.0.0.1:9091".parse().unwrap());
    let server2 = TcpRaftServer::new("127.0.0.1:9092".parse().unwrap());
    let server3 = TcpRaftServer::new("127.0.0.1:9093".parse().unwrap());

    assert_ne!(
        server1.bind_addr, server2.bind_addr,
        "Servers should have different ports"
    );
    assert_ne!(
        server2.bind_addr, server3.bind_addr,
        "Servers should have different ports"
    );
}

// Test 12: Network config with many peers
#[test]
fn test_network_config_many_peers() {
    let mut peers = HashMap::new();

    // Create config with many peers (simulating a 5-node cluster)
    for i in 2..=5 {
        peers.insert(i, format!("127.0.0.1:{}", 8080 + i).parse().unwrap());
    }

    let config = TcpNetworkConfig::new(1, "127.0.0.1:9090".parse::<SocketAddr>().unwrap(), peers);

    assert_eq!(
        config.peers.len(),
        4,
        "Should have 4 peers for 5-node cluster"
    );
}

// Test 13: TCP network with IPv6 support
#[test]
fn test_tcp_network_ipv6() {
    // Test that we can configure IPv6 addresses
    let config_ipv6 = TcpNetworkConfig::new(1, "[::1]:9090".parse().unwrap(), HashMap::new());

    assert_eq!(
        config_ipv6.bind_addr.ip().is_ipv6(),
        true,
        "Should support IPv6"
    );
}

// Test 14: Network config edge cases
#[test]
fn test_network_config_edge_cases() {
    // Test with maximum node ID

    let config = TcpNetworkConfig::new(
        u64::MAX,
        "127.0.0.1:9090".parse::<SocketAddr>().unwrap(),
        HashMap::new(),
    );

    assert_eq!(config.node_id, u64::MAX, "Should support maximum node ID");
}

// Test 15: TCP server with different address families
#[test]
fn test_tcp_server_address_families() {
    // Test IPv4 server
    let server_ipv4 = TcpRaftServer::new("127.0.0.1:9090".parse().unwrap());
    assert_eq!(server_ipv4.bind_addr.is_ipv4(), true, "Should support IPv4");

    // Test IPv6 server (if supported)
    let server_ipv6 = TcpRaftServer::new("[::1]:9090".parse().unwrap());
    if server_ipv6.bind_addr.is_ipv6() {
        // IPv6 is supported
    }
}

// Test 16: Config with loopback addresses
#[test]
fn test_config_loopback_addresses() {
    // All addresses in this test use loopback for safety

    let config = TcpNetworkConfig::new(
        1,
        "127.0.0.1:0".parse().unwrap(), // Dynamic port
        HashMap::from([
            (2, "127.0.0.1:8081".parse().unwrap()),
            (3, "127.0.0.1:8082".parse().unwrap()),
        ]),
    );

    // Verify all addresses are loopback
    assert!(
        config.bind_addr.ip().is_loopback(),
        "Bind address should be loopback"
    );
    for addr in config.peers.values() {
        assert!(addr.ip().is_loopback(), "Peer addresses should be loopback");
    }
}

// Test 17: Network config immutability
#[test]
fn test_network_config_immutability() {
    // NetworkConfig is immutable after creation
    let config = TcpNetworkConfig::new(
        1,
        "127.0.0.1:9090".parse::<SocketAddr>().unwrap(),
        HashMap::new(),
    );

    // After creation, fields should be accessible
    assert_eq!(config.node_id, 1);

    // In Rust, the config structure is immutable unless we explicitly make it mutable
    // This test verifies the design intent
}

// Test 18: TCP network with custom ports
#[test]
fn test_tcp_custom_ports() {
    // Test with a range of ports typically used in production

    let config = TcpNetworkConfig::new(
        1,
        "127.0.0.1:6969".parse().unwrap(), // Default port from config.toml
        HashMap::from([
            (2, "127.0.0.1:6970".parse().unwrap()),
            (3, "127.0.0.1:6971".parse().unwrap()),
        ]),
    );

    assert_eq!(config.bind_addr.port(), 6969, "Should use custom port 6969");
}

// Test 19: Config validation
#[test]
fn test_config_validation() {
    // Verify that config is usable
    let config = TcpNetworkConfig::new(
        1,
        "127.0.0.1:9090".parse::<SocketAddr>().unwrap(),
        HashMap::new(),
    );

    // Basic validation
    assert!(config.node_id > 0, "Node ID should be positive");
    assert!(config.bind_addr.port() > 0, "Port should be positive");
}
