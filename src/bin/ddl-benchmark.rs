//! DDL Benchmark Binary
//!
//! Run distributed DDL benchmarks from the command line.
//!
//! # Usage
//!
//! ```bash
//! # Run with default settings
//! cargo run --bin ddl-benchmark
//!
//! # Custom node specification
//! cargo run --bin ddl-benchmark -- --nodes "node-[01-33]"
//!
//! # Custom duration and topics
//! cargo run --bin ddl-benchmark -- --duration 120 --topics 1000
//! ```

// Re-export the test module's main functionality
// This is a thin wrapper around the distributed_benchmark test

fn main() {
    println!("DDL Benchmark Tool");
    println!();
    println!("Usage:");
    println!("  cargo run --bin ddl-benchmark -- [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --nodes, -n <SPEC>      Node specification (e.g., \"node-[01-33]\")");
    println!("  --duration, -d <SEC>    Benchmark duration in seconds (default: 60)");
    println!("  --topics, -t <COUNT>    Number of topics (default: 100)");
    println!("  --threads, -T <COUNT>   Concurrent threads (default: 100)");
    println!("  --help, -h              Show this help message");
    println!();
    println!("Note: For full benchmark functionality, run as a test:");
    println!("  cargo test --test distributed_benchmark");
    println!();
    println!("Or to run specific benchmarks:");
    println!("  cargo test --test distributed_benchmark benchmark_cluster_setup");
    println!("  cargo test --test distributed_benchmark benchmark_topic_operations");
    println!("  cargo test --test distributed_benchmark benchmark_consensus_operations");
    println!("  cargo test --test distributed_benchmark benchmark_concurrent_load");
    println!("  cargo test --test distributed_benchmark full_benchmark_suite");
}
