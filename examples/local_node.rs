//! AutoQueues Node Example - Run a distributed node
//!
//! This example demonstrates how to run an AutoQueues node from a config file.
//!
//! Usage:
//!   NODE_ID=1 cargo run --example local_node -- --config config-local.toml
//!
//! The node will:
//! 1. Load configuration from the TOML file
//! 2. Detect its node ID from NODE_ID environment variable
//! 3. Collect local metrics based on [local] config
//! 4. Publish values to other nodes via ZMQ
//! 5. Compute global aggregations

use autoqueues::config::Config;
use autoqueues::node::AutoQueuesNode;
use std::path::PathBuf;
use structopt::StructOpt;

/// Command line arguments
#[derive(StructOpt, Debug)]
struct Args {
    /// Path to config file
    #[structopt(short, long, parse(from_os_str))]
    config: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Parse command line arguments
    let args = Args::from_args();

    // Determine config path
    let config_path = args.config.unwrap_or_else(|| {
        PathBuf::from("config-local.toml")
    });

    println!("AutoQueues Node");
    println!("==============");
    println!("Config file: {:?}", config_path);

    // Load configuration
    let config = Config::load(&config_path)?;
    println!("Configuration loaded successfully");

    // Detect node ID
    let node_id = config.detect_node_id()?;
    println!("Node ID: {}", node_id);

    // Get my node configuration
    let my_node = config.my_node(node_id)?;
    println!(
        "My address: {}:{} (pubsub: {})",
        my_node.host, my_node.communication_port, my_node.pubsub_port
    );

    // Show other nodes
    let other_nodes = config.other_nodes(node_id);
    println!("Other nodes: {}", other_nodes.len());

    // Show local metrics
    println!("Local metrics:");
    for (name, source) in &config.local {
        println!("  {} = {}", name, source);
    }

    // Show global aggregations
    println!("Global aggregations:");
    for (name, config) in &config.global {
        let expr_str = config.expression.as_deref().unwrap_or("");
        println!(
            "  {}: {} {} (interval: {}ms) {}",
            name,
            config.aggregation,
            config.sources.join(", "),
            config.interval_ms,
            expr_str
        );
    }

    // Create and run the node
    let node = AutoQueuesNode::new(&config).await?;
    node.run(&config).await?;

    Ok(())
}
