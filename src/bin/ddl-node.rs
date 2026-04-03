//! DDL Node Binary - Standalone Raft Cluster Node
//!
//! This binary runs a single DDL node as part of a distributed cluster.
//! It can be used for:
//! - Running benchmark clusters
//! - Integration testing with real network communication
//! - Production deployments
//!
//! # Usage
//!
//! ```bash
//! # Start bootstrap node (first node in cluster)
//! cargo run --bin ddl-node -- --id 1 --port 7000 --bootstrap
//!
//! # Start joining nodes
//! cargo run --bin ddl-node -- --id 2 --port 7001 --peers "1@localhost:7000"
//! cargo run --bin ddl-node -- --id 3 --port 7002 --peers "1@localhost:7000"
//!
//! # With persistent storage
//! cargo run --bin ddl-node -- --id 1 --port 7000 --bootstrap --data-dir ./data/node1
//! ```

use ddl::cluster::RaftClusterNode;
use ddl::network::TcpNetworkConfig;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::signal;

/// Command-line arguments for the DDL node
struct Args {
    /// Node ID (unique identifier in cluster)
    id: u64,
    /// Port for coordination (Raft) traffic
    port: u16,
    /// List of peer addresses (format: "node_id@host:port,node_id@host:port")
    peers: Vec<(u64, String)>,
    /// Is this the bootstrap node?
    bootstrap: bool,
    /// Data directory for persistent storage (optional)
    data_dir: PathBuf,
    /// Host address to bind to
    host: String,
}

fn parse_args() -> Result<Args, String> {
    let args: Vec<String> = std::env::args().collect();
    let mut arg_iter = args.iter().skip(1);

    let mut id: Option<u64> = None;
    let mut port: Option<u16> = None;
    let mut raw_peers = Vec::new();
    let mut bootstrap = false;
    let mut data_dir = None;
    let mut host = "0.0.0.0".to_string();

    while let Some(arg) = arg_iter.next() {
        match arg.as_str() {
            "--id" | "-i" => {
                id = Some(
                    arg_iter
                        .next()
                        .and_then(|s| s.parse::<u64>().ok())
                        .ok_or("--id requires a numeric argument")?,
                );
            }
            "--port" | "-p" => {
                port = Some(
                    arg_iter
                        .next()
                        .and_then(|s| s.parse::<u16>().ok())
                        .ok_or("--port requires a numeric argument")?,
                );
            }
            "--peers" => {
                let peers_str = arg_iter.next().ok_or("--peers requires an argument")?;
                for peer_str in peers_str.split(',') {
                    let peer_str = peer_str.trim();
                    if peer_str.is_empty() {
                        continue;
                    }
                    // Parse format: "node_id@host:port"
                    let parts: Vec<&str> = peer_str.split('@').collect();
                    if parts.len() != 2 {
                        return Err(format!(
                            "Invalid peer format '{}'. Expected: node_id@host:port",
                            peer_str
                        ));
                    }
                    let node_id = parts[0]
                        .parse::<u64>()
                        .map_err(|_| format!("Invalid node_id in peer: {}", parts[0]))?;
                    let addr = parts[1].to_string();
                    raw_peers.push((node_id, addr));
                }
            }
            "--bootstrap" | "-b" => {
                bootstrap = true;
            }
            "--data-dir" | "-d" => {
                if let Some(dir_str) = arg_iter.next() {
                    data_dir = Some(PathBuf::from(dir_str));
                }
            }
            "--host" | "-h" => {
                if let Some(host_str) = arg_iter.next() {
                    host = host_str.to_string();
                }
            }
            "--help" => {
                print_help();
                std::process::exit(0);
            }
            _ => {
                return Err(format!("Unknown argument: {}", arg));
            }
        }
    }

    // Apply defaults
    let id = id.ok_or("--id is required")?;
    let port = port.unwrap_or(6967);
    let data_dir = data_dir.unwrap_or_else(|| {
        let mut dir = std::env::temp_dir();
        dir.push(format!("ddl-node-{}", id));
        dir
    });

    Ok(Args {
        id,
        port,
        peers: raw_peers,
        bootstrap,
        data_dir,
        host,
    })
}

fn print_help() {
    println!("DDL Node - Distributed DDL Cluster Node");
    println!();
    println!("USAGE:");
    println!("  ddl-node [OPTIONS]");
    println!();
    println!("REQUIRED:");
    println!("  --id, -i <NUM>            Node ID (unique cluster identifier)");
    println!();
    println!("OPTIONS:");
    println!("  --port, -p <NUM>         Coordination port (default: 6967)");
    println!("  --peers <ADDRS>          Comma-separated peer addresses (format: node_id@host:port)");
    println!("  --bootstrap, -b          Bootstrap a new cluster (first node only)");
    println!("  --data-dir, -d <PATH>    Data directory for persistence (default: /tmp/ddl-node-<id>)");
    println!("  --host <HOST>            Host address to bind (default: 0.0.0.0)");
    println!("  --help                   Show this help message");
    println!();
    println!("EXAMPLES:");
    println!();
    println!("  # Start bootstrap node (first node in cluster)");
    println!("  ddl-node --id 1 --port 7000 --bootstrap");
    println!();
    println!("  # Start joining node connecting to bootstrap node");
    println!("  ddl-node --id 2 --port 7001 --peers 1@localhost:7000");
    println!();
    println!("  # With persistent storage");
    println!("  ddl-node --id 1 --port 7000 --bootstrap --data-dir ./data/node1");
}

#[tokio::main]
async fn main() -> Result<(), String> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let args = parse_args()?;

    println!("Starting DDL Node {}", args.id);
    println!("  Host: {}", args.host);
    println!("  Port: {}", args.port);
    println!("  Data directory: {}", args.data_dir.display());
    println!("  Bootstrap: {}", args.bootstrap);
    println!("  Peers: {:?}", args.peers);

    // Create data directory if needed
    std::fs::create_dir_all(&args.data_dir)
        .map_err(|e| format!("Failed to create data directory: {}", e))?;

    // Build TCP network config
    let bind_addr = format!("{}:{}", args.host, args.port);
    let mut network_config = TcpNetworkConfig::new(args.id, args.port).with_bind_addr(&bind_addr);

    // Add peers
    for (peer_id, peer_addr) in &args.peers {
        network_config.add_peer(*peer_id, peer_addr.clone());
        println!("  Added peer {} at {}", peer_id, peer_addr);
    }

    // Create nodes configuration for initialization
    // We need at least our own node config for initialize() to work
    let mut nodes_config = HashMap::new();
    nodes_config.insert(args.id, ddl::cluster::types::NodeConfig {
        node_id: args.id,
        host: args.host.clone(),
        communication_port: args.port.saturating_sub(3),  // Default offset
        coordination_port: args.port,
    });
    
    // Add peer configs if available
    for (peer_id, peer_addr) in &args.peers {
        // Parse peer address format: "host:port"
        let parts: Vec<&str> = peer_addr.split(':').collect();
        if parts.len() == 2 {
            if let Ok(port) = parts[1].parse::<u16>() {
                nodes_config.insert(*peer_id, ddl::cluster::types::NodeConfig {
                    node_id: *peer_id,
                    host: parts[0].to_string(),
                    communication_port: port.saturating_sub(3),
                    coordination_port: port,
                });
            }
        }
    }

    // Create node with REAL TCP networking
    println!("Creating TCP node...");
    let (node, _network_factory) =
        RaftClusterNode::new_tcp(args.id, nodes_config, network_config, &args.data_dir)
            .await
            .map_err(|e| format!("Failed to create TCP node: {}", e))?;

    // Verify TCP server is listening
    println!("Verifying TCP server is listening...");
    let bind_addr = format!("{}:{}", args.host, args.port);
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Check if port is actually bound
    match tokio::net::TcpStream::connect(&bind_addr).await {
        Ok(_) => {
            println!("✓ TCP server confirmed listening on {}", bind_addr);
        }
        Err(e) => {
            eprintln!("✗ CRITICAL: TCP server NOT listening on {}", bind_addr);
            eprintln!("✗ Error: {}", e);
            eprintln!("\nPossible causes:");
            eprintln!("  1. Port {} is already in use by another process", args.port);
            eprintln!("  2. Address {} is not available on this machine", args.host);
            eprintln!("  3. Permission denied (try ports > 1024 or run as root)");
            eprintln!("\nTo diagnose:");
            eprintln!("  ss -tlnp | grep {}", args.port);
            eprintln!("  netstat -tlnp | grep {}", args.port);
            return Err(format!("TCP server failed to bind: {}", e));
        }
    }

    // Initialize if bootstrap
    if args.bootstrap {
        println!("Initializing bootstrap node...");
        node.initialize().await?;
        println!("Bootstrap node initialized and ready");
    } else {
        // For joining nodes, we need to wait for them to be added
        // Currently there's no auto-join mechanism
        println!("Note: Joining nodes require manual cluster membership");
        println!("Use admin API to add this node to the cluster");

        if args.peers.is_empty() {
            return Err("Non-bootstrap nodes require --peers argument".to_string());
        }

        // Wait for leadership or error
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }

    // Wait for leader election
    println!("Waiting for leader election...");
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Check leadership
    let is_leader = node.is_leader().await;
    if is_leader {
        println!("This node is the LEADER");
    } else {
        let leader = node.get_leader().await;
        println!("Leader is node: {:?}", leader);
    }

    // For bootstrap nodes, start lease expiration task
    let _lease_handle = if args.bootstrap {
        Some(node.start_lease_expiration(5))
    } else {
        None
    };

    // Wait for shutdown signal
    println!("Node is running. Press Ctrl+C to shutdown.");
    let _ = signal::ctrl_c().await;
    println!("Shutting down gracefully...");

    // Shutdown node
    node.shutdown().await?;

    println!("Node shutdown complete");
    Ok(())
}