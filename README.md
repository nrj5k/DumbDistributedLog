# DDL - Dumb Distributed Log

A minimal, high-performance distributed append-only log for HPC clusters.

## Features

- **Dumb by Design**: No clever features, just reliable log storage
- **High Performance**: Lock-free SPMC queues, O(1) operations
- **Strong Consistency**: Raft-based consensus for topic ownership
- **Fault Tolerant**: Automatic failover, leader election
- **Flexible**: Standalone or distributed modes

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      DdlDistributed                             │
│  ┌─────────────────┐  ┌───────────────────────────────────────┐│
│  │   TopicQueue    │  │         RaftClusterNode              ││
│  │   SPMC ring      │  │  ┌────────────────────────────────────┐│
│  │   lock-free      │  │  │      openraft::Raft               │││
│  │                 │  │  │  ┌────────────────────────────────┐ │ ││
│  │                 │  │  │  │   AutoqueuesRaftStorage        │ │││
│  │                 │  │  │  │   ┌──────────────────────────┐ │ │││
│  │                 │  │  │  │   │Arc<RwLock<OwnershipState>│ │ │││
│  └─────────────────┘  │  │  │   └──────────────────────────┘ │ │││
│                       │  │  └────────────────────────────────────┘││
│  ┌─────────────────┐  │                                      ││
│  │ GossipCoordinator│  │  ┌──────────────────────────────────┐ │ │
│  │ (node discovery) │  │  │        TcpNetwork               │ │ │
│  │ (NOT ownership)   │  │  │  ┌──────────────────────────────┐ │ │ │
│  └─────────────────┘  │  │  │  TcpRaftServer              │ │ │ │
│                       │  │  └──────────────────────────────┘ │ │ │
│                       │  └──────────────────────────────────┘ │ │
└─────────────────────────────────────────────────────────────────┘
```

## Quick Start

```rust
use ddl::{DdlDistributed, DDL, DdlConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Standalone mode (single-node, testing)
    let config = DdlConfig::default();
    let ddl = DdlDistributed::new_standalone(config);
    
    // Subscribe to a topic
    let mut stream = ddl.subscribe("metrics.cpu").await?;
    
    // Push metrics
    ddl.push("metrics.cpu", vec![42]).await?;
    
    // Read from stream
    while let Some(entry) = stream.next().await {
        println!("Got metric: {:?}", entry);
        ddl.ack("metrics.cpu", entry.id).await?;
    }
    
    Ok(())
}
```

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
ddl = { git = "https://github.com/nrj5k/DumbDistributedLog", branch = "main" }
```

## Modes of Operation

### Standalone Mode

For single-node deployments or testing:

```rust
let ddl = DdlDistributed::new_standalone(DdlConfig::default());
```

### Distributed Mode with Raft

For multi-node clusters with strong consistency:

```rust
let config = DdlConfig {
    raft_enabled: true,
    is_bootstrap: true,  // Only first node
    owned_topics: vec!["metrics.cpu".to_string()],
    ..Default::default()
};

let ddl = DdlDistributed::new_distributed(config).await?;
```

## Configuration

```rust
let config = DdlConfig {
    // Node Configuration
    node_id: 1,
    
    // Buffer sizes
    buffer_size: 1_000_000,        // Ring buffer size
    subscription_buffer_size: 1000,   // Subscription queue size
    
    // Topic limits
    max_topics: 10_000,             // Maximum number of topics
    
    // Ownership (distributed mode)
    owned_topics: vec![],           // Topics this node owns
    raft_enabled: false,            // Enable Raft consensus
    is_bootstrap: false,            // Bootstrap node flag
    
    // Network (distributed mode)
    peers: HashMap::new(),          // Peer addresses
    gossip_bind_addr: "0.0.0.0:9090".to_string(),
    
    // Heartbeat/Timeout
    heartbeat_interval_secs: 5,
    owner_timeout_secs: 30,
};
```

## API Reference

### Push

```rust
// Push a message to a topic
let id = ddl.push("metrics.cpu", payload).await?;
```

### Subscribe

```rust
// Subscribe to a topic (returns async stream)
let mut stream = ddl.subscribe("metrics.cpu").await?;

while let Some(entry) = stream.next().await {
    // Process entry
    println!("Entry {}: {:?}", entry.id, entry.payload);
    
    // Acknowledge processing
    ddl.ack("metrics.cpu", entry.id).await?;
}
```

### Position

```rust
// Get current position (highest acknowledged entry)
let pos = ddl.position("metrics.cpu").await?;
```

### Ack

```rust
// Acknowledge processing of an entry
ddl.ack("metrics.cpu", entry_id).await?;
```

### Topic Ownership

```rust
// Check if this node owns a topic
if ddl.owns_topic("metrics.cpu").await {
    // This node owns the topic
}

// Claim topic ownership (distributed mode with Raft)
ddl.claim_topic("metrics.cpu").await?;

// Release topic ownership
ddl.release_topic("metrics.cpu").await?;
```

## Membership Events

Subscribe to cluster membership changes:

```rust
let mut events = ddl.membership().subscribe().await?;

while let Ok(event) = events.recv().await {
    match event.event_type {
        MembershipEventType::NodeFailed { node_id } => {
            // Handle node failure - initiate failover
            handle_failover(node_id).await;
        }
        MembershipEventType::NodeJoined { node_id, addr } => {
            println!("Node {} joined at {}", node_id, addr);
        }
        MembershipEventType::NodeRecovered { node_id } => {
            println!("Node {} recovered", node_id);
        }
        MembershipEventType::NodeLeft { node_id } => {
            println!("Node {} left gracefully", node_id);
        }
    }
}
```

## Architecture Decisions

### Why Dumb?

DDL is intentionally minimal:

1. **No clever routing** - Push to topic, subscribe to topic
2. **No complex APIs** - Push, Subscribe, Ack, Position
3. **No retries** - If push fails, caller retries
4. **No ordering guarantees** - Per-topic ordering only

### Topic Ownership via Raft

Topic ownership is managed by Raft consensus:

- **Strong consistency** - Only one node can own a topic at a time
- **Automatic failover** - If owner crashes, another node claims it
- **Lease-based** - Ownership has TTL, auto-renewed by owner

### Lock-Free Design

TopicQueue uses lock-free SPMC queues:

- **Single producer, multiple consumers**
- **Atomic positions** - No locks on read/write
- **Cache-friendly** - Ring buffer layout

## Performance

Benchmarks on AMD EPYC 7742:

| Operation | Latency | Throughput |
|-----------|---------|------------|
| Push (in-mem) | < 10µs | 100K+ ops/s |
| Subscribe receive | < 5µs | 200K+ ops/s |
| Push (WAL) | ~100µs | 10K+ ops/s |
| Topic claim (Raft) | ~5ms | 200 claims/s |

## HPC Integration

DDL is designed for HPC workloads:

- **Wildcards**: Subscribe to `metrics.*/` for hierarchical aggregation
- **Batching**: Push multiple entries efficiently
- **Backpressure**: Drop oldest when subscriber falls behind
- **Metrics**: Export metrics for monitoring

## Comparison

| Feature | DDL | Kafka | Redis Streams | NATS JetStream |
|----------|-----|-------|----------------|-----------------|
| Ordering | Per-topic | Per-partition | Per-stream | Per-subject |
| Persistence | Optional | Yes | Optional | Optional |
| Distributed | Raft | Raft/ZK | AOF/RDB | Raft |
| Complexity | Low | High | Medium | Medium |
| Performance | Excellent | Good | Good | Good |
| HPC Native | Yes | No | No | No |

## License

GNU General Public License v3.0 or later. See [LICENSE](LICENSE) file.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Credits

Developed for HPC metric aggregation systems.

Special thanks to the openraft and iroh-gossip projects.