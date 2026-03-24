# DDL Usage Guide

**Practical usage guide for the Dumb Distributed Log system.**

## Table of Contents

1. [Quick Start](#quick-start)
2. [Choosing Transport](#choosing-transport)
3. [Choosing Durability](#choosing-durability)
4. [Configuration Examples](#configuration-examples)
5. [Common Patterns](#common-patterns)
6. [Performance Tuning](#performance-tuning)
7. [Deployment Considerations](#deployment-considerations)

---

## Quick Start

### Installation

Add DDL to your `Cargo.toml`:

```toml
[dependencies]
ddl = "0.2"
```

### Basic Operations

DDL provides three operations: push, subscribe, and ack.

```rust
use ddl::{DDL, DdlInMemory};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create DDL instance (in-memory, single node)
    let ddl: Arc<dyn DDL> = Arc::new(DdlInMemory::new());
    
    // Create a topic with 10,000 entry buffer
    ddl.create_topic("metrics.cpu", 10000).await?;
    
    // Push data to the topic
    let entry_id = ddl.push("metrics.cpu", vec![1, 2, 3, 4]).await?;
    println!("Pushed entry with ID: {}", entry_id);
    
    // Subscribe to receive data
    let mut stream = ddl.subscribe("metrics.*").await?;
    
    while let Some(entry) = stream.next().await {
        println!("Topic: {}, ID: {}, Payload: {:?}",
                 entry.topic, entry.id, entry.payload);
        
        // Acknowledge processing (enables at-least-once delivery)
        stream.ack(entry.id).await?;
    }
    
    Ok(())
}
```

### Running the Example

```bash
cargo run --example basic_usage
```

---

## Choosing Transport

### Transport Options

DDL supports three transport types. Choose based on your deployment requirements.

**TCP Transport** (Recommended for most cases):
- Reliable, connection-oriented messaging
- Works well across regions
- Standard networking tools work with it
- Default choice for production

**ZMQ Transport** (High throughput):
- Optimized for pub/sub patterns
- Lower latency than TCP
- Best for single-datacenter deployments
- Requires ZMQ expertise

**Hybrid Transport** (Mixed workloads):
- TCP for control plane (reliability)
- ZMQ for data plane (performance)
- Best of both worlds

### TCP Transport Example

```rust
use ddl::transport::TcpTransport;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let transport = TcpTransport::new("0.0.0.0:6969").await?;
    
    // Configure for production
    let config = TcpConfig {
        bind_addr: "0.0.0.0:6969".parse()?,
        max_connections: 10000,
        connection_timeout: Duration::from_secs(30),
        keepalive_interval: Duration::from_secs(60),
    };
    
    Ok(())
}
```

### ZMQ Transport Example

```rust
use ddl::transport::ZmqTransport;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let transport = ZmqTransport::new("tcp://*:6969")?;
    
    // Configure for high throughput
    let config = ZmqConfig {
        context: zmq::Context::new(),
        publish_endpoint: "tcp://*:6969".to_string(),
        subscribe_endpoint: "tcp://*:6970".to_string(),
        batch_size: 100,
        flush_interval: Duration::from_millis(10),
    };
    
    Ok(())
}
```

### When to Use Each Transport

| Scenario | Recommended Transport | Reason |
|----------|----------------------|--------|
| Cross-region deployment | TCP | Reliability across networks |
| Single datacenter, max throughput | ZMQ | Lower latency |
| Mixed workloads | Hybrid | Best of both |
| Standard production | TCP | Simplicity and reliability |
| Complex messaging patterns | ZMQ | Built-in patterns |
| Firewall traversal | TCP | Standard protocols |

---

## Choosing Durability

### Storage Options

DDL provides two storage implementations. Choose based on your data requirements.

**In-Memory Storage**:
- Fastest possible performance
- Data lost on crash
- No disk I/O overhead
- Suitable for non-critical or recomputable data

**WAL-Backed Storage** (Write-Ahead Log):
- Data survives crashes
- Slightly slower (disk sync)
- Replay capability available
- Suitable for critical data

### In-Memory Example

```rust
use ddl::{DDL, DdlInMemory};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // In-memory - fastest, no persistence
    let ddl: Arc<dyn DDL> = Arc::new(DdlInMemory::new());
    
    // All operations are in-memory
    ddl.create_topic("metrics.cpu", 10000).await?;
    
    Ok(())
}
```

### WAL-Backed Example

```rust
use ddl::{DDL, DdlWal};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // WAL-backed - durable, survives crashes
    let ddl: Arc<dyn DDL> = Arc::new(DdlWal::new("/data/ddl").await?);
    
    // Create a topic (WAL created automatically)
    ddl.create_topic("metrics.cpu", 10000).await?;
    
    // Push - writes to WAL before returning
    let entry_id = ddl.push("metrics.cpu", data).await?;
    
    // After crash, data is recovered from WAL
    Ok(())
}
```

### Durability Trade-offs

| Aspect | In-Memory | WAL-Backed |
|--------|-----------|------------|
| Push latency | <10μs | ~100μs |
| Crash survival | No | Yes |
| Replay capability | No | Yes |
| Disk usage | None | O(entries) |
| Memory usage | Higher | Lower |

### When to Use Each Storage

**Use In-Memory when**:
- Data is non-critical or recomputable
- Maximum latency is required
- Crash recovery is handled externally
- Memory is plentiful

**Use WAL when**:
- Data must survive crashes
- Replay capability is needed
- Moderate latency is acceptable
- Storage space is available

---

## Configuration Examples

### Single Node Configuration

```rust
use ddl::{DdlConfig, DdlInMemory};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Simple single-node configuration
    let config = DdlConfig {
        node_id: 1,
        peers: vec![],
        owned_topics: vec!["metrics.cpu".to_string(), "metrics.memory".to_string()],
        buffer_size: 1024 * 1024, // 1MB per topic
        gossip_enabled: false,
        gossip_bind_addr: "0.0.0.0:0".to_string(),
        gossip_bootstrap: vec![],
        data_dir: std::path::PathBuf::from("/tmp/ddl"),
        wal_enabled: false, // In-memory only
    };
    
    let ddl: Arc<dyn DDL> = Arc::new(DdlInMemory::with_config(config));
    
    Ok(())
}
```

### Cluster Configuration

```rust
use ddl::{DdlConfig, DdlDistributed};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Three-node cluster configuration
    let config = DdlConfig {
        node_id: 1,
        peers: vec![
            "node1:6969".to_string(),
            "node2:6969".to_string(),
            "node3:6969".to_string(),
        ],
        owned_topics: vec!["metrics.cpu".to_string()],
        buffer_size: 1024 * 1024,
        gossip_enabled: true,
        gossip_bind_addr: "0.0.0.0:7001".to_string(),
        gossip_bootstrap: vec!["node1:7001".to_string()],
        data_dir: std::path::PathBuf::from("/data/ddl"),
        wal_enabled: true, // Enable durability
    };
    
    let ddl: Arc<dyn DDL> = Arc::new(DdlDistributed::new(config).await?);
    
    // Start listening for connections
    ddl.listen("0.0.0.0:6969").await?;
    
    Ok(())
}
```

### High-Performance Configuration

```rust
use ddl::{DdlConfig, DdlInMemory};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Optimized for maximum throughput
    let config = DdlConfig {
        node_id: 1,
        peers: vec![],
        owned_topics: vec!["metrics.*".to_string()], // Catch-all pattern
        buffer_size: 10 * 1024 * 1024, // 10MB per topic
        gossip_enabled: false,
        gossip_bind_addr: "0.0.0.0:0".to_string(),
        gossip_bootstrap: vec![],
        data_dir: std::path::PathBuf::from("/tmp/ddl"),
        wal_enabled: false, // No durability for speed
    };
    
    let ddl: Arc<dyn DDL> = Arc::new(DdlInMemory::with_config(config));
    
    Ok(())
}
```

### Production Configuration

```rust
use ddl::{DdlConfig, DdlDistributed};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Production-ready configuration
    let config = DdlConfig {
        node_id: 1,
        peers: vec![
            "node1:6969".to_string(),
            "node2:6969".to_string(),
            "node3:6969".to_string(),
            "node4:6969".to_string(),
            "node5:6969".to_string(),
        ],
        owned_topics: vec![
            "metrics.cpu".to_string(),
            "metrics.memory".to_string(),
            "logs.application".to_string(),
        ],
        buffer_size: 5 * 1024 * 1024, // 5MB per topic
        gossip_enabled: true,
        gossip_bind_addr: "0.0.0.0:7001".to_string(),
        gossip_bootstrap: vec![
            "node1:7001".to_string(),
            "node2:7001".to_string(),
        ],
        data_dir: std::path::PathBuf::from("/data/ddl"),
        wal_enabled: true, // Enable durability
    };
    
    let ddl: Arc<dyn DDL> = Arc::new(DdlDistributed::new(config).await?);
    
    // Graceful shutdown handling
    tokio::signal::ctrl_c().await;
    
    Ok(())
}
```

---

## Common Patterns

### Pattern 1: Metrics Collection

Collect metrics from multiple sources and aggregate.

```rust
use ddl::{DDL, DdlInMemory};
use std::sync::Arc;

async fn metrics_collection() -> Result<(), Box<dyn std::error::Error>> {
    let ddl: Arc<dyn DDL> = Arc::new(DdlInMemory::new());
    
    // Create topics for different metric types
    ddl.create_topic("metrics.cpu", 10000).await?;
    ddl.create_topic("metrics.memory", 10000).await?;
    ddl.create_topic("metrics.disk", 10000).await?;
    
    // Producers push metrics
    let cpu_data = collect_cpu_metrics();
    ddl.push("metrics.cpu", cpu_data).await?;
    
    let memory_data = collect_memory_metrics();
    ddl.push("metrics.memory", memory_data).await?;
    
    // Consumer subscribes to all metrics
    let mut stream = ddl.subscribe("metrics.*").await?;
    
    while let Some(entry) = stream.next().await {
        let metric_type = extract_metric_type(&entry.topic);
        process_metric(metric_type, &entry.payload);
        stream.ack(entry.id).await?;
    }
    
    Ok(())
}
```

### Pattern 2: Event Logging

Log events with durable storage.

```rust
use ddl::{DDL, DdlWal};
use std::sync::Arc;

async fn event_logging() -> Result<(), Box<dyn std::error::Error>> {
    // Use WAL for durability
    let ddl: Arc<dyn DDL> = Arc::new(DdlWal::new("/data/events").await?);
    
    ddl.create_topic("events.application", 100000).await?;
    ddl.create_topic("events.audit", 100000).await?;
    
    // Log application events
    let event = serde_json::to_vec(&serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "level": "INFO",
        "message": "User logged in",
        "user_id": 12345,
    }))?;
    
    ddl.push("events.application", event).await?;
    
    // Log audit events
    let audit_event = serde_json::to_vec(&serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "action": "LOGIN",
        "user_id": 12345,
        "ip_address": "192.168.1.1",
    }))?;
    
    ddl.push("events.audit", audit_event).await?;
    
    Ok(())
}
```

### Pattern 3: Request/Response with DDL

Use DDL for async request/response patterns.

```rust
use ddl::{DDL, DdlInMemory};
use std::sync::Arc;
use std::time::Duration;

async fn request_response() -> Result<(), Box<dyn std::error::Error>> {
    let ddl: Arc<dyn DDL> = Arc::new(DdlInMemory::new());
    
    ddl.create_topic("requests", 10000).await?;
    ddl.create_topic("responses", 10000).await?;
    
    // Request producer
    let request_id = 42;
    let request = serde_json::to_vec(&serde_json::json!({
        "request_id": request_id,
        "command": "process_data",
        "data": "...",
    }))?;
    
    ddl.push("requests", request).await?;
    
    // Response consumer (separate task)
    let mut stream = ddl.subscribe("responses").await?;
    
    tokio::select! {
        entry = stream.next() => {
            if let Some(response) = entry {
                let parsed: serde_json::Value = serde_json::from_slice(&response.payload)?;
                if parsed["request_id"] == request_id {
                    println!("Got response: {:?}", parsed);
                }
            }
        }
        _ = tokio::time::sleep(Duration::from_secs(5)) => {
            println!("Request timeout");
        }
    }
    
    Ok(())
}
```

### Pattern 4: Topic Fan-Out

Single producer, multiple consumers on different topics.

```rust
use ddl::{DDL, DdlInMemory};
use std::sync::Arc;

async fn topic_fan_out() -> Result<(), Box<dyn std::error::Error>> {
    let ddl: Arc<dyn DDL> = Arc::new(DdlInMemory::new());
    
    // Multiple consumers subscribe to different topics
    let cpu_stream = ddl.subscribe("metrics.cpu").await?;
    let memory_stream = ddl.subscribe("metrics.memory").await?;
    let disk_stream = ddl.subscribe("metrics.disk").await?;
    
    // Producer pushes to all metric topics
    ddl.push("metrics.cpu", collect_cpu()).await?;
    ddl.push("metrics.memory", collect_memory()).await?;
    ddl.push("metrics.disk", collect_disk()).await?;
    
    // Each consumer receives from their topic independently
    Ok(())
}
```

---

## Performance Tuning

### Buffer Size Tuning

Increase buffer size to handle burst traffic:

```rust
let config = DdlConfig {
    buffer_size: 10 * 1024 * 1024, // 10MB per topic
    ..Default::default()
};
```

### Batch Operations

Batch multiple entries to reduce overhead:

```rust
async fn batch_push(ddl: &Arc<dyn DDL>, topic: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut batch = Vec::new();
    
    for i in 0..1000 {
        let data = format!("entry_{}", i).into_bytes();
        batch.push(ddl.push(topic, data));
    }
    
    // Wait for all pushes to complete
    let results = futures::future::join_all(batch).await;
    
    for result in results {
        println!("Pushed: {:?}", result);
    }
    
    Ok(())
}
```

### Connection Pooling

Reuse connections for better performance:

```rust
use std::sync::Arc;

struct ConnectionPool {
    ddl: Arc<dyn DDL>,
    // ... pool management
}

impl ConnectionPool {
    async fn get_connection(&self) -> Connection {
        // Return cached connection or create new one
    }
}
```

### Subscription Optimization

Use specific patterns instead of catch-all:

```rust
// Good: Specific pattern
let stream = ddl.subscribe("metrics.cpu").await?;

// Avoid if possible: Catch-all pattern (may match more topics)
let stream = ddl.subscribe("metrics.*").await?;
```

### Network Optimization

For TCP transport, tune socket options:

```rust
use std::net::SocketAddr;

let config = TcpConfig {
    bind_addr: "0.0.0.0:6969".parse()?,
    // Increase buffer sizes
    send_buffer_size: 1024 * 1024,
    recv_buffer_size: 1024 * 1024,
    // Enable TCP_NODELAY for lower latency
    nodelay: true,
    // Keep-alive for long-lived connections
    keepalive: Some(Duration::from_secs(60)),
};
```

---

## Deployment Considerations

### Single Node Deployment

Best for development, testing, or single-machine workloads.

**Pros:**
- Simple setup
- Lowest latency
- No network overhead

**Cons:**
- Single point of failure
- Limited scalability

**Configuration:**
```rust
let config = DdlConfig {
    node_id: 1,
    gossip_enabled: false,
    wal_enabled: false, // or true for durability
    ..Default::default()
};
```

### Cluster Deployment

Best for production workloads requiring scalability and reliability.

**Pros:**
- Horizontal scalability
- Fault tolerance
- Higher total throughput

**Cons:**
- More complex setup
- Network latency between nodes
- Raft overhead for coordination

**Configuration:**
```rust
let config = DdlConfig {
    node_id: 1,
    peers: vec![
        "node1:6969".to_string(),
        "node2:6969".to_string(),
        "node3:6969".to_string(),
    ],
    gossip_enabled: true,
    gossip_bind_addr: "0.0.0.0:7001".to_string(),
    gossip_bootstrap: vec!["node1:7001".to_string()],
    wal_enabled: true,
    ..Default::default()
};
```

### Monitoring Considerations

Monitor these metrics in production:

```rust
// Entry rate
let position = ddl.position("metrics.cpu").await?;

// Topic ownership
if ddl.owns_topic("metrics.cpu") {
    // This node is the owner
}

// Buffer utilization (implementation-specific)
// Monitor queue depth to detect backpressure
```

### Resource Requirements

**Minimum:**
- 1 CPU core
- 256MB RAM
- 1GB disk (if WAL enabled)

**Recommended:**
- 4+ CPU cores
- 2GB+ RAM
- SSD storage (if WAL enabled)

**For Large Deployments (10,000+ topics):**
- 8+ CPU cores
- 8GB+ RAM
- NVMe storage
- 10GbE network

### Shutdown and Recovery

Graceful shutdown:

```rust
use tokio::signal;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ddl = DdlDistributed::new(config).await?;
    
    // Start server
    ddl.listen("0.0.0.0:6969").await?;
    
    // Wait for shutdown signal
    signal::ctrl_c().await?;
    
    // Graceful shutdown
    ddl.shutdown().await?;
    
    Ok(())
}
```

Crash recovery (with WAL):

```rust
// On startup, DDL automatically recovers from WAL
let ddl = DdlWal::new("/data/ddl").await?;

// After recovery, all unacknowledged entries are available
let mut stream = ddl.subscribe("metrics.*").await?;
```

---

## Summary

This guide covered:

1. **Quick Start**: Basic installation and operations
2. **Transport Selection**: TCP vs ZMQ vs Hybrid
3. **Durability Selection**: In-memory vs WAL
4. **Configuration Examples**: Single node, cluster, production
5. **Common Patterns**: Metrics, events, request/response, fan-out
6. **Performance Tuning**: Buffer sizes, batching, subscriptions
7. **Deployment**: Single node vs cluster, monitoring, resources

For more details, see:
- [API Reference](API.md) for complete method documentation
- [Architecture](ARCHITECTURE.md) for technical details
- [Limitations](LIMITATIONS.md) for honest assessment of trade-offs