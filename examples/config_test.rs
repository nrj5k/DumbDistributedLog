use autoqueues::config::{Config, ConfigGenerator};

fn main() {
    // Test the new flexible NodeTable
    let config = ConfigGenerator::local_test(5, 7000);
    println!("Created config with {} nodes", config.all_nodes().len());

    // Test accessing nodes
    match config.my_node(1) {
        Ok(node) => println!("Node 1: {}:{}", node.host, node.communication_port),
        Err(e) => println!("Error: {}", e),
    }

    match config.my_node(5) {
        Ok(node) => println!("Node 5: {}:{}", node.host, node.communication_port),
        Err(e) => println!("Error: {}", e),
    }

    // Test the builder pattern
    let builder_config = Config::builder()
        .add_node("node1", "127.0.0.1", 7001, 7002, 7003, 7004)
        .add_node("node2", "127.0.0.1", 7101, 7102, 7103, 7104)
        .add_node("node10", "127.0.0.1", 8001, 8002, 8003, 8004) // This would have failed before
        .build();

    println!(
        "Builder config has {} nodes",
        builder_config.all_nodes().len()
    );

    match builder_config.my_node(10) {
        Ok(node) => println!("Node 10: {}:{}", node.host, node.communication_port),
        Err(e) => println!("Error: {}", e),
    }
}
