# AutoQueues - Fire-and-Forget Distributed Queue System

## Status: Phase 6 Complete

### The API

```rust
use autoqueues::AutoQueuesNode;

// Simple fire-and-forget
AutoQueuesNode::from_config("config.toml")?.run().await;
```

### Config-Driven

```toml
# config.toml
[node]
hostname = "server1"

[cluster]
nodes = [
    "server1:6966",
    "server2:6966",
    "server3:6966"
]

[derived_metrics.avg_cpu]
aggregation = "average"
sources = ["atomic.cpu_percent"]
```

### Usage

```bash
# Terminal 1
cargo run --example distributed_node --config node1.toml

# Terminal 2  
cargo run --example distributed_node --config node2.toml

# Terminal 3
cargo run --example distributed_node --config node3.toml
```

### Query Cluster

```bash
echo "get_metric:avg_cpu" | nc 127.0.0.1 6969
# Returns: OK:53.3
```

### What Works

- [x] Config-driven node startup
- [x] Auto-publishing of local metrics
- [x] Leader aggregation
- [x] REQ/REP query interface
- [x] Fire-and-forget API

### Files Changed

| File | Change |
|------|--------|
| `src/node.rs` | Fire-and-forget AutoQueuesNode |
| `examples/distributed_node.rs` | Config-driven demo |
| `node1.toml`, `node2.toml`, `node3.toml` | Node configs |

### Tests

```
cargo test --lib  # 37 tests pass
```

### What Still Needed

- Leader assignment (currently all nodes are leaders)
- Real metric collection (currently simulated)
- Graceful shutdown
- Error recovery
