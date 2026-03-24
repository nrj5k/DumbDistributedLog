# DDL (Dumb Distributed Log)

**DDL is a distributed append-only log. That's it.**

No expressions. No metrics. No aggregation. No health monitoring. Just a reliable, high-performance distributed log system designed for large-scale HPC deployments where Redis Streams falls apart.

[![Tests](https://img.shields.io/badge/tests-passing-green)](#)
[![License](https://img.shields.io/badge/license-MIT-blue)](#)
[![Rust](https://img.shields.io/badge/rust-1.70+-orange)](#)

## Table of Contents

- [What DDL Is](#what-ddl-is)
- [Quick Start](#quick-start)
- [Core Operations](#core-operations)
- [Documentation](#documentation)
- [Why Redis Streams Falls Apart](#why-redis-streams-falls-apart)
- [Installation](#installation)
- [What DDL Is NOT](#what-ddl-is-not)
- [Performance](#performance)
- [Limitations](#limitations)
- [Contributing](#contributing)

---

## What DDL Is

A distributed append-only log system where:

- **Producers** push data to topics
- **Consumers** subscribe to topics and receive data
- **System** guarantees delivery and ordering within each topic
- **Cluster** uses Raft for shard assignment only (which node owns which topic)

### Data Model

Each entry is a simple tuple:

```rust
pub struct Entry {
    pub id: u64,        // Monotonically increasing ID within a topic
    pub timestamp: u64, // Nanoseconds since epoch
    pub topic: String,  // Topic name
    pub payload: Vec<u8>, // The actual data
}
```

Simple. Predictable. Efficient.

---

## Quick Start

### Installation

```toml
[dependencies]
ddl = "0.2"
```

### Single Node (In-Memory)

```rust
use ddl::{DDL, DdlInMemory};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create DDL instance
    let ddl: Arc<dyn DDL> = Arc::new(DdlInMemory::new());
    
    // Create a topic
    ddl.create_topic("metrics.cpu", 10000).await?;
    
    // Push data
    let entry_id = ddl.push("metrics.cpu", vec![1, 2, 3, 4]).await?;
    println!("Pushed entry: {}", entry_id);
    
    // Subscribe and receive
    let mut stream = ddl.subscribe("metrics.cpu").await?;
    while let Some(entry) = stream.next().await {
        println!("Received: {:?}", entry.payload);
        stream.ack(entry.id).await?;
    }
    
    Ok(())
}
```

### Cluster (Distributed with WAL)

```rust
use ddl::{DDL, DdlDistributed, DdlConfig};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = DdlConfig {
        node_id: 1,
        peers: vec!["node1:6969", "node2:6969", "node3:6969"],
        owned_topics: vec!["metrics.cpu".to_string()],
        buffer_size: 1024 * 1024,
        gossip_enabled: true,
        gossip_bind_addr: "0.0.0.0:7001".to_string(),
        gossip_bootstrap: vec!["node1:7001".to_string()],
        data_dir: std::path::PathBuf::from("/data/ddl"),
        wal_enabled: true, // Enable durability
    };
    
    let ddl: Arc<dyn DDL> = Arc::new(DdlDistributed::new(config).await?);
    ddl.listen("0.0.0.0:6969").await?;
    
    Ok(())
}
```

---

## Core Operations

Everything you need:

```rust
// 1. Push data to a topic
let entry_id = ddl.push("metrics.cpu", data).await?;

// 2. Subscribe to receive data (pattern matching supported)
let mut stream = ddl.subscribe("metrics.*").await?;
while let Some(entry) = stream.next().await {
    println!("Topic: {}, Payload: {:?}", entry.topic, entry.payload);
}

// 3. Acknowledge processing (for at-least-once delivery)
stream.ack(entry.id).await?;
```

Three operations. Push, subscribe, ack. That's the entire API.

---

## Documentation

### Getting Started

- **[Quick Start](docs/USAGE.md#quick-start)**: Get up and running in 5 minutes
- **[Configuration Guide](docs/USAGE.md#configuration-examples)**: Configure for your deployment
- **[Common Patterns](docs/USAGE.md#common-patterns)**: Examples for typical use cases

### Reference

- **[API Reference](docs/API.md)**: Complete API documentation
- **[Configuration Options](docs/API.md#configuration)**: All configuration parameters
- **[Error Types](docs/API.md#error-types)**: Error handling guide

### Deep Dive

- **[Architecture](docs/ARCHITECTURE.md)**: Technical architecture and design decisions
- **[Transport Layer](docs/ARCHITECTURE.md#transport-layer)**: TCP vs ZMQ vs Hybrid
- **[Storage Layer](docs/ARCHITECTURE.md#storage-layer)**: In-memory vs WAL
- **[Discovery Layer](docs/ARCHITECTURE.md#discovery-layer)**: Gossip and Raft

### Limitations

- **[Honest Assessment](docs/LIMITATIONS.md)**: What DDL does and doesn't do
- **[Trade-offs](docs/LIMITATIONS.md#trade-offs-made)**: Intentional design decisions
- **[Alternatives](docs/LIMITATIONS.md#comparison-with-alternatives)**: When to use something else

---

## Why Redis Streams Falls Apart

Redis Streams work great until they don't:

| Aspect | Redis Streams | DDL |
|--------|---------------|-----|
| 100K topics | Memory pressure, slow replica sync | Sharded, linear scaling |
| Cross-region | Complex GSL, eventual consistency | Raft, strong consistency |
| Topic ownership | Unclear, clients guess | Raft decides definitively |
| Performance at scale | Degrades after 10K topics | Linear scaling with cluster |
| Protocol | Redis RESP | TCP or ZMQ |

### ORNL Frontier Use Case

- 10,000+ compute nodes
- Each node produces metrics to its own topic
- Central analysis subscribes to all topics
- No single point of failure
- Strong ordering guarantees per topic

DDL handles this. Redis Streams struggle.

---

## Installation

### From Crates.io

```toml
[dependencies]
ddl = "0.2"
```

### From Source

```bash
git clone https://github.com/your-org/ddl.git
cd ddl
cargo build --release
```

### Running Examples

```bash
# Basic single-node example
cargo run --example basic_usage

# Cluster example
cargo run --example cluster_example

# WAL durability example
cargo run --example wal_example
```

---

## What DDL Is NOT

This is important. DDL is intentionally limited:

- **NOT an expression engine** - No math, no filters, no aggregations
- **NOT a metrics system** - No CPU monitoring, no health scores, no dashboards
- **NOT an aggregation system** - No combining data from multiple topics
- **NOT a complex monitoring platform** - No alerts, no alerts

DDL moves bytes reliably. That's its entire job.

### When to Use Something Else

| Need | Use Instead |
|------|-------------|
| Expression evaluation | `evalexpr` or `fasteval` crate |
| Metrics collection | Prometheus + Grafana |
| Stream processing | Apache Flink |
| Complex queuing | RabbitMQ or NATS |
| Querying data | Elasticsearch |
| Managed service | AWS Kinesis or Google Pub/Sub |

---

## Performance

### Latency Targets

| Operation | In-Memory | WAL-Backed |
|-----------|-----------|------------|
| Push | <10μs | ~100μs |
| Subscribe receive | <5μs | <5μs |
| Shard migration | ~50ms | ~50ms |

### Throughput Targets

| Configuration | Messages/Second |
|---------------|-----------------|
| Single node (in-memory) | 100,000+ |
| Single node (WAL) | 10,000+ |
| Cluster (3 nodes) | 300,000+ |
| Cluster (10 nodes) | 1,000,000+ |

Performance is a byproduct of simplicity, not a feature.

---

## Limitations

DDL is honest about what it provides:

### Guarantees

- **At-least-once delivery** (with acknowledgments)
- **Per-topic ordering** (entries delivered in order)
- **Topic ownership** (each topic owned by exactly one node)

### What DDL Does NOT Provide

- **No automatic failover** - Topics owned by failed nodes become unavailable
- **No message replay** - Once acknowledged, messages are gone
- **No consumer groups** - No shared offsets or message distribution
- **No exactly-once delivery** - Duplicates possible
- **No expression evaluation** - Build separately if needed
- **No built-in monitoring** - Use external tools

If you need stronger guarantees, build them on top of DDL's simple foundation.

### See Also

- [Limitations Document](docs/LIMITATIONS.md) for complete assessment
- [Trade-offs Made](docs/LIMITATIONS.md#trade-offs-made)
- [Comparison with Alternatives](docs/LIMITATIONS.md#comparison-with-alternatives)

---

## Design Philosophy

DDL follows the **KISS (Keep It Simple, Stupid)** principle:

### Core Principles

1. **Simple Interfaces, Powerful Backends**
   - Clean trait-based APIs
   - Proven libraries doing heavy lifting (Raft, TCP, tokio)
   - User sees simple interface, library handles complexity

2. **Use Existing Tools, Don't Reinvent**
   - Dependencies are leverage, not complexity
   - Raft, TCP, tokio - proven libraries
   - Your code stays minimal by leveraging existing solutions

3. **Comprehensive Validation**
   - Catch issues early
   - Better thorough validation than subtle runtime bugs
   - Use existing validation libraries

4. **Right Tool for the Right Job**
   - Use the best available tool for each specific use case
   - If TCP is better than QUIC, use TCP
   - Don't create artificial distinctions

5. **Honest, Straightforward Communication**
   - No inflated claims
   - No marketing speak
   - Clear, direct documentation

### Architecture Pattern

```rust
// KISS architecture - simple interfaces, powerful backends
#[async_trait]
pub trait DDL {
    async fn push(&self, topic: &str, payload: &[u8]) -> Result<u64, DdlError>;
    async fn subscribe(&self, pattern: &str) -> Result<Box<dyn SubscriptionStream>, DdlError>;
    async fn ack(&self, topic: &str, entry_id: u64) -> Result<(), DdlError>;
}
```

---

## Transport Options

Choose the right transport for your deployment:

### TCP (Default)

- Reliable, connection-oriented
- Works well across regions
- Standard networking tools work with it
- **Best for**: Production, cross-region, reliability-critical

### ZMQ

- High-performance pub/sub
- Lower latency than TCP
- Best for single-datacenter
- **Best for**: Maximum throughput, single location

### Hybrid

- TCP for control plane (reliability)
- ZMQ for data plane (performance)
- **Best of both worlds**

See [Transport Layer](docs/ARCHITECTURE.md#transport-layer) for details.

---

## Storage Options

Choose the right durability for your data:

### In-Memory

- Fastest possible performance
- Data lost on crash
- No disk I/O
- **Best for**: Non-critical data, maximum latency

### WAL-Backed (Write-Ahead Log)

- Data survives crashes
- Slightly slower (disk sync)
- Replay capability
- **Best for**: Critical data, durability required

See [Storage Layer](docs/ARCHITECTURE.md#storage-layer) for details.

---

## Contributing

Contributions are welcome! Please read our contributing guidelines before submitting PRs.

### Development Setup

```bash
# Clone the repository
git clone https://github.com/your-org/ddl.git

# Install dependencies
cargo build --all-features

# Run tests
cargo test --lib

# Run benchmarks
cargo bench --lib
```

### Code Style

- Follow Rust idioms and best practices
- Use `cargo fmt` for formatting
- Use `cargo clippy` for linting
- Write tests for new features

---

## License

MIT License - see LICENSE file for details.

---

## Contact

- GitHub Issues for bugs and feature requests
- Discussions for questions and community support
- Contributing guidelines for PR submissions

---

**DDL: Simple distributed logging. Nothing more, nothing less.**