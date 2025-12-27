# Distributed AutoQueues Implementation Plan
## From First Flight to Multi-Node Cluster

---

## Current State (After First Flight)

| Component | Status |
|-----------|--------|
| Single-node demo | Working |
| Leader REP handler | Port 6969 |
| Expression engine | atomic/cluster vars |
| LeaderMapClient | 3-node consensus |
| Tests | 38/38 passing |

---

## Vision: Multi-Node Distributed Cluster

```
Node 1 (server1)          Node 2 (server2)           Node 3 (server3)
     │                         │                          │
     │ PUB:6966               │ PUB:6966                │ PUB:6966
     ▼                         ▼                          ▼
[atomic metrics] ───────► [Leader subs] ─────────────► [Leader subs]
     │                         │                          │
     │ REQ:6969               │                          │
     └──────────────────────► │ REQ:6969                 │
           Query cluster vars │                          │
                             ◄──────────────────────────┘
                                   Consensus
```

---

## Implementation Steps

### Phase 1: Node Configuration (1 day)

**Goal:** Define node list in config, support multi-address binding

```
config.toml:
  [cluster]
  nodes = ["server1:6966", "server2:6966", "server3:6966"]
  query_port = 6969
  
  [derived_metrics.avg_cpu]
  aggregation = "average"
  sources = ["atomic.cpu_percent"]
```

Files to modify:
- `src/config.rs` - Add node list, cluster config
- `src/port_config.rs` - Support multiple bind addresses

### Phase 2: Metric Publishing (1-2 days)

**Goal:** Nodes publish atomic metrics via ZMQ PUB

```rust
// src/metrics/publisher.rs (NEW)
pub struct MetricPublisher {
    socket: zmq::Socket,
    node_id: u64,
}

impl MetricPublisher {
    pub fn new(node_id: u64, peers: &[SocketAddr]) -> Self {
        let ctx = zmq::Context::new();
        let socket = ctx.socket(zmq::PUB).unwrap();
        socket.set_linger(0);
        socket.bind("tcp://0.0.0.0:6966").unwrap();
        
        // Connect to peers for peer-to-peer pub/sub
        for peer in peers {
            socket.connect(&format!("tcp://{}", peer)).unwrap();
        }
        
        Self { socket, node_id }
    }
    
    pub fn publish(&self, metric: &str, value: f64) {
        let msg = format!("{}:{}:{}", metric, self.node_id, value);
        self.socket.send(&msg, 0).unwrap();
    }
}
```

Files to create:
- `src/metrics/mod.rs`
- `src/metrics/publisher.rs`

### Phase 3: Leader Subscription (1-2 days)

**Goal:** Leaders subscribe to atomic topics from all nodes

```rust
// In Leader::new()
let socket = context.socket(zmq::SUB).unwrap();
socket.set_linger(0);

// Subscribe to all atomic.* topics from all nodes
socket.set_subscribe(b"atomic.").unwrap();

// Connect to each node's PUB port
for node in &config.nodes {
    socket.connect(&format!("tcp://{}/pub", node)).unwrap();
}
```

Files to modify:
- `src/cluster/leader.rs` - Add SUB socket, subscription logic

### Phase 4: Node Startup (1 day)

**Goal:** Node connects to cluster, gets leader map

```rust
// Node startup sequence
async fn join_cluster(config: &ClusterConfig) {
    // 1. Connect to known nodes
    let client = LeaderMapClient::new(config.nodes.clone());
    
    // 2. Get leader map via consensus
    let leader_map = client.get_leader_map().await?;
    
    // 3. Start metric publisher
    let publisher = MetricPublisher::new(config.node_id, &config.nodes);
    
    // 4. Start local leader if this node is a leader
    for (metric, leader_id) in leader_map.iter() {
        if *leader_id == config.node_id {
            start_leader_for_metric(metric).await?;
        }
    }
    
    Ok(())
}
```

Files to modify:
- `src/lib.rs` - Add `AutoQueues::initialize()` 
- Create `src/node.rs` - Node startup logic

### Phase 5: QueueManager Integration (1 day)

**Goal:** Expressions use real cluster vars from leaders

```rust
// In QueueManager::update_derived_queues()
async fn update_derived_queues(&self) {
    // Get cluster vars from leaders
    let cluster_vars = self.query_cluster_vars().await;
    
    // Evaluate expressions
    for (_, queue) in &self.queues {
        if let Some(expr) = &queue.expression {
            let result = expr.evaluate(&self.atomic_vars, &cluster_vars);
            // ... publish result
        }
    }
}
```

Files to modify:
- `src/queue_manager.rs` - Add cluster var querying
- `src/cluster/cluster_var_client.rs` - Query multiple leaders

### Phase 6: Integration Test (1 day)

**Goal:** 3-node demo proving distributed operation

```bash
# Terminal 1 (server1)
cargo run --example distributed_demo -- server1

# Terminal 2 (server2)  
cargo run --example distributed_demo -- server2

# Terminal 3 (server3)
cargo run --example distributed_demo -- server3
```

Files to create:
- `examples/distributed_demo.rs`

---

## File Changes Summary

### New Files
```
src/metrics/
  ├── mod.rs
  └── publisher.rs
src/node.rs
examples/distributed_demo.rs
```

### Modified Files
```
src/config.rs          - Add node list, cluster config
src/port_config.rs     - Multi-address support
src/cluster/leader.rs  - Add SUB socket for subscriptions
src/cluster/mod.rs     - Export new modules
src/queue_manager.rs   - Query cluster vars for expressions
src/lib.rs             - Add AutoQueues::initialize()
```

---

## Timeline

| Phase | Days | Deliverable |
|-------|------|-------------|
| 1. Node Config | 1 | Config supports node list |
| 2. Metric PUB | 1-2 | Nodes publish atomic metrics |
| 3. Leader SUB | 1-2 | Leaders subscribe to all nodes |
| 4. Node Startup | 1 | Node joins cluster, gets leader map |
| 5. QueueManager | 1 | Expressions use real cluster vars |
| 6. Integration | 1 | 3-node working demo |
| **Total** | **6-8** | **Distributed cluster** |

---

## Success Criteria

```
□ 3 nodes can form a cluster
□ Each node publishes atomic metrics
□ Leaders aggregate from all nodes
□ Expressions query real cluster values
□ Node failure triggers leader reassignment
□ Leader map consistent across cluster
```

---

## Running the Distributed Demo

```bash
# After all phases complete

# Terminal 1
cargo run --example distributed_demo -- server1

# Terminal 2
cargo run --example distributed_demo -- server2

# Terminal 3
cargo run --example distributed_demo -- server3

# Expected output on each node:
# [node1] publishing: atomic.cpu_percent=45.2
# [node1] cluster avg_cpu: 53.1%
# [node1] health deviation: -14.9% (COLD)
```

---

*Plan generated: December 26, 2025*
*Status: Ready to start Phase 1*
