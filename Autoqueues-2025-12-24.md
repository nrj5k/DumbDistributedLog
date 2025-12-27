# AutoQueues Cluster Architecture Plan
## Generated: December 24, 2025
## Last Updated: December 25, 2025 (Phase 4 Checkpoint Commit d501dbc)

---

**Commit:** `d501dbc` - Phase 4 complete, atomic/cluster naming, leaderleader, Leader, LeaderMapClient

---

# Executive Summary

AutoQueues is a distributed queue system with expression-based metric aggregation. This document defines the cluster coordination layer using OpenRaft for consensus, with ZMQ for data transport on separate ports.

**Key Design Decisions:**
- **Ports:** 6966-6969 (configurable, all derived from base)
- **Naming:** `atomic` for local, `cluster` for aggregated metrics
- **leaderleader** pattern for leader map management
- **Weighted bag** assignment for leader selection
- **Parallel queries** for all atomic/cluster accesses in expressions
- **Consensus:** Ask 3 nodes, majority for leader map
- **Expression Engine:** evalexpr (migrated from abandoned `eval` crate)
- **AIMD intervals** for adaptive metric push frequency
- **Freshness timeout:** 2 × AIMD_max for failure detection

---

# Phase Status

## ✅ Phase 1: Foundation (Complete)

| Component | Status | Details |
|-----------|--------|---------|
| Config structures | ✅ Complete | `derived_metrics`, `node_scores`, `node_order`, `aimd_*_interval_ms` |
| hostname → node_id mapping | ✅ Complete | Deterministic mapping via `node_order` config |
| WeightedBag | ✅ Complete | Weighted random selection, round-robin assignment |
| LeaderMap | ✅ Complete | Metric → Leader assignments with version tracking |
| Tests | ✅ Complete | 34/34 lib tests passing |

## ✅ Phase 2: leaderleader (Complete)

| Component | Status | Details |
|-----------|--------|---------|
| leaderleader | ✅ Complete | Meta-leader for leader map management |
| Leader map replication | ✅ Complete | ZMQ broadcast on changes |
| Failure detection | ✅ Complete | Freshness timeout (2 × AIMD_max) |
| Leader reassignment | ✅ Complete | Simple round-robin to next node |
| Tests | ✅ Complete | 3/3 tests passing |

## ✅ Phase 3: Leader (Complete)

| Component | Status | Details |
|-----------|--------|---------|
| Leader struct | ✅ Complete | Subscription, aggregation, query service |
| Aggregation types | ✅ Complete | Sum, Average, Max, Min |
| Query service | ✅ Complete | REQ/REP handler ready |
| Tests | ✅ Complete | 8/8 tests passing |

## ✅ Phase 4: Integration (Complete, Commit d501dbc)

| Component | Status | Details |
|-----------|--------|---------|
| Variable renaming | ✅ Complete | `local` → `atomic`, `global` → `cluster` |
| Leader.get_aggregated_value() | ✅ Complete | Simple getter for cached values |
| LeaderMapClient | ✅ Complete | 3-node consensus for leader map |
| Expression engine | ✅ Complete | atomic/cluster syntax support |
| Ports config | ✅ Complete | ClusterPorts (6966-6969) |
| Tests | ✅ Complete | 34/34 tests passing |

## ⏳ Phase 5: First Flight (Next)

| Component | Status | Description |
|-----------|--------|-------------|
| Leader REQ/REP handler | Next | Network handler for leader queries |
| Expression → Leader integration | Next | Expression calls leaders for cluster.* values |
| QueueManager → cluster_vars | Next | Populate cluster_vars from leaders |
| Demo script | Next | End-to-end proof it works |

## Phase 6: Post-Beta Work (Future)

| Component | Status | Description |
|-----------|--------|-------------|
| Parallel cluster reads | Future | tokio::join_all for expression evaluation |
| Node health scoring | Future | Replace static scores with live metrics |
| Leader failure recovery | Future | Trigger election when leader down |
| Cache fallback | Future | Use cached value if leader query fails |
| End-to-end integration | Future | Full cluster integration tests |
| Snapshotting | Future | Raft log compaction |
| Dynamic metric addition | Future | Add metrics without restart |
| ZMQ Optimization | Future | Eliminate locks, use ZMQ thread-safe sockets, async receive |

### Config Structures Added

```toml
[derived_metrics.nvme_drive_capacity]
aggregation = "sum"
sources = ["atomic.nvme_size"]

[node_scores]
server1 = 10.0
server2 = 5.0
server3 = 1.0

[node_order]
order = ["server1", "server2", "server3", "server4"]

[bootstrap]
data_port = 6966          # Base port for data plane
coordination_port = 6968  # For leaderleader broadcasts
query_port = 6969         # For leader queries (REQ/REP)
aimd_min_interval_ms = 100
aimd_max_interval_ms = 5000
```

### Files Created/Modified

| File | Change |
|------|--------|
| `src/config.rs` | Added derived_metrics, node_scores, node_order, AIMD intervals, helper methods |
| `src/cluster/mod.rs` | Added leader, leader_assignment, leaderleader modules |
| `src/cluster/config.rs` | ClusterConfig with coordination_port |
| `src/cluster/leader_assignment.rs` | WeightedBag, LeaderMap, leader assignment |
| `src/cluster/leader.rs` | Leader for derived metric aggregation |
| `src/cluster/leaderleader.rs` | leaderleader meta-leader |
| `src/cluster/node.rs` | RaftNode wrapper (minimal) |
| `src/cluster/state_machine.rs` | Cluster state, commands |

---

# Problem Statement

## The Gap

The current system can:
- Collect local metrics per node
- Evaluate expressions
- Pass queue data via ZMQ

But it cannot:
- Aggregate global metrics across the cluster
- Ensure consistent leader assignment
- Handle node failures gracefully
- Maintain cluster-wide state

## The Solution

A cluster coordination layer that:
- Elects leaders for global metrics via OpenRaft
- Aggregates local metrics into global values
- Reassigns leaders on failure
- Maintains leader map consistency via Raft log

---

# Architecture Overview

## Two ZMQ Channels (Configurable, Default: 6966-6969)

| Port | Purpose | Data Type | Consumers |
|------|---------|-----------|-----------|
| 6966 | Data Plane | Queue messages, pub/sub topics | All nodes |
| 6967 | Reserved | Future use | - |
| 6968 | Coordination Plane | Raft RPC, leader map broadcasts | Cluster nodes only |
| 6969 | Leader Queries | REQ/REP for metric queries | Any node |

**Note:** All ports derived from base (6966), all configurable in config.

```
┌──────────────────────────────────────────────────────────────┐
│                    Data Plane (Port 6967)                     │
│   Queue messages, local metrics, pub/sub topics              │
│   All nodes subscribe/publish freely                          │
└──────────────────────────────────────────────────────────────┘
                              ▲
                              │ Commands from leader
                              ▼
┌──────────────────────────────────────────────────────────────┐
│                  Coordination Plane (Port 6968)               │
│   Raft consensus, leader map, metric queries (REQ/REP)       │
└──────────────────────────────────────────────────────────────┘
```

---

# Metric Types

## 1. Atomic Metrics

**Definition:** Raw metrics from a single node (atomic = indivisible, single source of truth).

**Examples:**
- `atomic.cpu_percent` - CPU usage on this node
- `atomic.nvme_size` - NVMe capacity on this node
- `atomic.memory_usage` - Memory usage on this node

**Collection:** Each node collects its own atomic metrics via SystemMetrics.

## 2. Cluster Metrics

**Definition:** Aggregations of atomic metrics across ALL cluster nodes, computed by elected leaders.

**Rationale:** "Cluster" clearly indicates "across all nodes" - unambiguous.

**Examples:**
- `cluster.avg_cpu` - Average of all atomic.cpu_percent
- `cluster.nvme_drive_capacity` - Sum of all atomic.nvme_size
- `cluster.ram_capacity` - Sum of all atomic.ram_size

**Configuration:**
```toml
[derived_metrics.avg_cpu]
aggregation = "average"
sources = ["atomic.cpu_percent"]

[derived_metrics.nvme_drive_capacity]
aggregation = "sum"
sources = ["atomic.nvme_size"]
```
sources = ["atomic.cpu_percent"]
```

**Aggregation Types:**
- `sum` - Add all source values
- `average` - Arithmetic mean
- `max` - Maximum value
- `min` - Minimum value

**Flow for Derived Metrics:**

```
Leader (e.g., server1) for derived.nvme_drive_capacity:
1. Reads leader map → "I am leader for derived.nvme_drive_capacity"
2. Reads sources → ["atomic.nvme_size"]
3. Subscribes to "atomic.nvme_size" on ALL nodes via ZMQ
4. Receives values with timestamps from all nodes
5. Aggregates per AIMD interval (bounded by max_interval)
6. Stores derived value with timestamp
7. Serves on REQ/REP query

Local nodes (server2, server3):
1. Push "atomic.nvme_size" at AIMD-adjusted intervals
2. No knowledge of who the leader is (just publish to topic)
```
```
1. Leader subscribes to source topics on ALL nodes
2. Local nodes push values with timestamps
3. Leader aggregates per configured interval
4. Leader stores global value
5. Querying nodes REQ/REP to leader for value
```

## 3. Hybrid Metrics (Computed Locally)

**Definition:** Expressions combining local and global metrics.

**Examples:**
- `atomic.nvme_size / derived.nvme_drive_capacity` - Ratio of local to total
- `atomic.cpu_percent / derived.avg_cpu` - Deviation from cluster average

**Why No Leader Required:**
- Each node computes its own result
- Local value comes from SystemMetrics
- Global value queried from leader via REQ/REP
- Expression evaluated locally

**Flow:**
```
Node evaluates: atomic.nvme_size / derived.nvme_drive_capacity
              = local_value / query_leader()
```

---

# Leader Architecture

## Why Leaders?

Global metrics need a single source of truth. Without leaders:
- Multiple nodes aggregate the same metric → inconsistent values
- No ownership → race conditions
- No accountability → can't detect failures

## What Leaders Do

For each assigned global metric:
1. Read leader map → "I am leader for derived.X"
2. Subscribe to source topics on ALL nodes
3. Aggregate incoming local values
4. Store global value
5. Serve on REQ/REP query

## Why Not All Nodes Aggregate?

- Unnecessary computation (N nodes computing same value)
- Inconsistent results without coordination
- Wasted network bandwidth

---

# leaderleader (Meta-Leader)

## Purpose

A single Raft-elected leader that manages the leader map.

## Responsibilities

| Responsibility | Description |
|----------------|-------------|
| Leader Map Maintenance | Store "Metric → Leader" assignments |
| Weighted Assignment | Assign leaders using node scores |
| Failure Detection | Timeout-based heartbeat detection |
| Reassignment | Reassign metrics when leaders fail |
| Map Propagation | Replicate leader map via Raft log |

## Why leaderleader?

1. **Performance** - Single point of assignment, not N² negotiations
2. **Consistency** - Raft ensures leader map is consistent across cluster
3. **Simplicity** - Clear separation of concerns
4. **Failure Recovery** - Centralized failure detection

---

# Node Identification

## Hostname → node_id Mapping

**Purpose:** Deterministic, string-free internal representation.

**Config:**
```toml
[node_order]
order = ["server1", "server2", "server3", "server4"]
# server1 → node_id = 1
# server2 → node_id = 2
# server3 → node_id = 3
# server4 → node_id = 4
```

**Why This Matters:**
- No string operations in hot paths
- Deterministic (same config = same mapping)
- Each node can compute any other node's ID locally

## Node Scores

**Purpose:** Weighted leader assignment based on node stability.

**Config:**
```toml
[node_scores]
server1 = 10.0   # Stable, high-priority node
server2 = 5.0    # Medium priority
server3 = 1.0    # New/unstable node
```

**Why Scores Matter:**
- Stable nodes handle more metrics
- Prioritize known-good hardware
- Prevent flaky nodes from becoming leaders

---

# Leader Assignment Algorithm

## Weighted Bag Selection

**Algorithm:**
1. Build bag of (node_id, score) pairs
2. For each metric, select node with weighted probability
3. Higher score = higher probability of selection

**Example:**
```
Nodes: [(1, 10.0), (2, 5.0), (3, 1.0)]
Total weight: 16.0

Metric 1 (nvme_drive_capacity):
  Random = 0.7 → falls in server1's weight range → server1

Metric 2 (ram_capacity):
  Random = 0.2 → falls in server2's weight range → server2

Metric 3 (avg_cpu):
  Random = 0.4 → falls in server1's weight range → server1

Result: { nvme: 1, ram: 2, avg_cpu: 1 }
```

## Round-Robin Override (Initial Assignment)

**Purpose:** Avoid Raft election cost on initial startup.

**Logic:**
- If no leader map exists: round-robin assignment
- If leader map exists: weighted bag assignment
- This is a one-time optimization, not persistent

---

# Data Flow

## Global Metric Aggregation Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                    Step 1: Leader Assignment                    │
└─────────────────────────────────────────────────────────────────┘
leaderleader reads config:
- global_metrics → [nvme_drive_capacity, ram_capacity, avg_cpu]
- node_scores → {server1: 10.0, server2: 5.0, server3: 1.0}
- node_order → {server1: 1, server2: 2, server3: 3}

Weighted bag assignment:
- nvme_drive_capacity → server1
- ram_capacity → server2
- avg_cpu → server1

Leader map stored in Raft, replicated to all nodes.

┌─────────────────────────────────────────────────────────────────┐
│                    Step 2: Metric Subscription                  │
└─────────────────────────────────────────────────────────────────┘
Server1 (leader for derived.nvme_drive_capacity):
1. Reads leader map → "I am leader for derived.nvme_drive_capacity"
2. Reads sources → ["atomic.nvme_size"]
3. Subscribes to "atomic.nvme_size" on ALL nodes via ZMQ

Server2, Server3:
1. Push "atomic.nvme_size" to topic periodically (AIMD interval)

┌─────────────────────────────────────────────────────────────────┐
│                    Step 3: Aggregation                          │
└─────────────────────────────────────────────────────────────────┘
Server1 receives:
- {server1: 512, server2: 1024, server3: 256}

Aggregates:
- sum: 512 + 1024 + 256 = 1792

Stores: derived.nvme_drive_capacity = 1792 (with timestamp)

┌─────────────────────────────────────────────────────────────────┐
│                    Step 4: Query                                │
└─────────────────────────────────────────────────────────────────┘
Server3 evaluating: atomic.nvme_size / derived.nvme_drive_capacity

1. atomic.nvme_size = 256 (from SystemMetrics)
2. derived.nvme_drive_capacity = ?
   - REQ to leader (server1)
   - REP: 1792
3. Expression: 256 / 1792 = 0.1428...

Result: 0.1428...
```

## Leader Failure Flow

### Failure Detection: Freshness Timeout

**Key Insight:** leaderleader monitors derived metric freshness using AIMD upper bound.

```
AIMD Interval Range: [min_interval, max_interval]

Example:
- min_interval = 100ms (high activity)
- max_interval = 5000ms (low activity)

Failure Detection Timeout: 2 × max_interval = 10000ms

Why This Works:
- Leader pushes at most every 5 seconds
- If no update in 10 seconds → leader is down
- Provides buffer for normal variance
```

### Complete Failure Flow

```
Before Failure:
Leader map: { derived.nvme_drive_capacity → server1 }
leaderleader monitors:
- derived.nvme_drive_capacity.last_update = t=5000
- timeout = 2 × AIMD_max = 10000ms

Failure Event:
- server1 (leader) goes offline
- No new updates to derived.nvme_drive_capacity

Detection:
- At t=15000, leaderleader checks
- t=15000 - t=5000 = 10000ms elapsed
- Elapsed ≥ timeout → leader is marked as down

Reassignment:
- leaderleader reassigns derived.nvme_drive_capacity → server2
- Raft log replicates new leader map
- server2 reads leader map → "I am leader"
- server2 subscribes to "atomic.nvme_size" on all nodes
- Aggregation continues

After Failure:
Leader map: { derived.nvme_drive_capacity → server2 }
```

**Note:** The failure is detected by derived metric staleness, not by monitoring the leader directly. If derived.nvme_drive_capacity stops updating, something is wrong with the leader.

---

# Configuration Reference

## Full Config Example

```toml
[derived_metrics]

[derived_metrics.nvme_drive_capacity]
aggregation = "sum"
sources = ["atomic.nvme_size"]

[derived_metrics.ram_capacity]
aggregation = "sum"
sources = ["atomic.ram_size"]

[derived_metrics.avg_cpu]
aggregation = "average"
sources = ["atomic.cpu_percent"]

[node_scores]
server1 = 10.0   # Stable, high-priority node
server2 = 5.0    # Medium priority
server3 = 1.0    # New/unstable node

[node_order]
order = ["server1", "server2", "server3", "server4"]

[bootstrap]
coordination_port = 6968
data_port = 6967
```

---

# Why This Design?

## OpenRaft Over Custom Implementation

| Reason | Explanation |
|--------|-------------|
| Proven correctness | Raft algorithm is mathematically proven |
| Battle-tested | Used in production systems (etcd, TiKV) |
| Feature-rich | Log replication, snapshots, membership changes |
| Maintained | Active development community |

## ZMQ for Data Plane

| Reason | Explanation |
|--------|-------------|
| Mature | Battle-tested over decades |
| Simple | PUB/SUB, REQ/REP patterns |
| Performant | Low overhead, high throughput |
| Separable | Different port from coordination |

## leaderleader Pattern

| Reason | Explanation |
|--------|-------------|
| Performance | Single point of assignment |
| Consistency | Raft ensures leader map consistency |
| Simplicity | Clear separation of concerns |
| Failure recovery | Centralized detection and reassignment |

## Explicit Derived Metrics

| Reason | Explanation |
|--------|-------------|
| No surprises | User defines what derived metrics exist |
| Resource control | No unnecessary computation |
| Clear intent | Configuration documents intent |
| Debugging | Easy to trace metric flow |

---

# Test Status

```
$ cargo test --lib
test result: ok. 34 passed; 0 failed
```

### Test Coverage

| Module | Tests | Status |
|--------|-------|--------|
| `cluster::leader` | 8 | ✅ |
| `cluster::leader_assignment` | 3 | ✅ |
| `cluster::leaderleader` | 3 | ✅ |
| `port_config` | 6 | ✅ |
| `queue::implementation` | 3 | ✅ |
| `networking::zmq_pubsub` | 3 | ✅ |
| `expression` | 8 | ✅ |

---

# Open Questions (Deferred)

1. **AIMD Interval Adjustment**
   - Future enhancement: variable push intervals based on TCP-style AIMD
   - Currently: fixed interval from config

2. **Snapshotting**
   - Raft log compaction for long-running clusters
   - Future consideration

3. **Dynamic Metric Addition**
   - Add global metrics without restart
   - Requires Raft membership change

4. **Leader Transfer**
   - Voluntary leader handoff
   - For maintenance scenarios

---

# Glossary

| Term | Definition |
|------|------------|
| Data Plane | ZMQ port 6967 - queue messages, pub/sub |
| Coordination Plane | ZMQ port 6968 - Raft, leader map, queries |
| Local Metric | Per-node metric (atomic.cpu_percent) |
| Derived Metric | Aggregated metric (derived.avg_cpu) - leader-computed |
| Hybrid Metric | Local + derived expression (local/derived) |
| Leader | Node responsible for a derived metric |
| leaderleader | Meta-leader managing leader map |
| Weighted Bag | Probabilistic selection by node score |
| Raft Log | Replicated log for leader map consistency |
| Freshness Timeout | 2 × AIMD_max_interval for failure detection |
| AIMD | Additive Increase Multiplicative Decrease - interval adjustment |

---

# References

- OpenRaft Documentation: https://docs.rs/openraft/
- ZMQ Guide: http://zguide.zeromq.org/
- Raft Paper: https://raft.github.io/raft.pdf

---

# ZMQ Optimization Plan (Post-Beta)

## Current Issues

| Issue | Current | Impact |
|-------|---------|--------|
| Excessive locks | `Mutex<HashMap<>>` everywhere | Contention |
| Sync receive | `fn receive()` not async | Inconsistent API |
| No socket options | Default ZMQ settings | Suboptimal throughput |
| JSON serialization | serde_json | Slower than binary |

## ZMQ Thread Safety Facts

- **ZMQ sockets are thread-safe** for sending from multiple threads
- **One socket = One task** eliminates broker-level locking
- `Arc<Socket>` is safe for concurrent sends

## Optimized Architecture

```rust
pub struct ZmqPubSubBroker {
    context: Context,
    publisher: Arc<Socket>,  // ZMQ handles concurrent sends
    subscribers: Arc<RwLock<HashMap<String, Subscriber>>>,  // One lock for map
}

pub struct Subscriber {
    socket: Arc<Socket>,  // Owned by one task - no lock needed
    topic: String,
    recv_task: JoinHandle<()>,  // Owns receive loop
}

// Client - no locks at all
pub struct ZmqPubSubClient {
    publisher: Arc<Socket>,  // ZMQ send-safe
    // No HashMap - subscribe returns channel
}
```

## Benefits

1. **No broker-level locks** - ZMQ handles concurrent sends
2. **Per-subscriber ownership** - Each socket owned by one task
3. **Async receive** - Consistent async API
4. **Channel-based** - Clean message passing

---

*Document generated for AutoQueues cluster coordination implementation*
*Architecture designed by: AutoQueues Team*
*ZMQ optimization added: December 25, 2025*
