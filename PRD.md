# AutoQueues Product Requirements Document

**Version:** 0.1.0
**Status:** In Development
**Last Updated:** 2025-01-21

---

## Executive Summary

AutoQueues is a **high-performance distributed queue system** designed for low-latency live telemetry collection and feedback systems. It provides two APIs:

1. **Server Mode** - Redis-like server for push/subscribe operations
2. **Programmatic Mode** - Library API for building custom queue topologies

**Primary Use Case:** Live telemetry collection for real-time feedback loops in HPC/ML systems.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           AutoQueues Architecture                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   ┌─────────────────────┐       ┌─────────────────────────────────────────┐ │
│   │   atomic.* queues   │       │         cluster.* queues               │ │
│   │   (SPMC per-metric) │       │     (Leader-published aggregations)    │ │
│   │                     │       │                                         │ │
│   │  Producer:          │       │  Leader:                                │ │
│   │    Node collector   │       │    - Subscribes to atomic.*            │ │
│   │                     │       │    - Computes aggregations             │ │
│   │  Consumers:         │       │    - Publishes to cluster.* via ZMQ    │ │
│   │    - Leader         │       │                                         │ │
│   │    - Subscribers    │       │  Subscribers:                           │ │
│   │    - Monitors       │       │    - Dashboards                         │ │
│   └──────────┬──────────┘       │    - Alerting systems                   │ │
│              │                  │    - Reporting                          │ │
│              ▼                  └─────────────────┬───────────────────────┘ │
│   ┌─────────────────────┐                            │                     │
│   │  openraft Leader    │◄───────────────────────────┘                     │
│   │  Election           │                                                  │
│   └─────────────────────┘                                                  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Core Concepts

### 1. atomic.* Queues (Single Producer, Multiple Consumer)

| Property | Value |
|----------|-------|
| Pattern | SPMC (Single Producer, Multiple Consumers) |
| Scope | Local to node |
| Overflow | Drop oldest (circular buffer behavior) |
| Implementation | Lock-free (atomic indices), no mutex contention |
| Latency Target | < 10μs per operation |

#### Invariants

- **Exactly one producer** per queue - the node's metric collector
- **Multiple consumers** can read without coordination
- Each consumer tracks its own read position
- Pre-allocated circular buffer (no heap allocation, no reallocation)

#### Pattern

```
atomic.cpu → CPU usage from this node (local-only)
atomic.memory → Memory usage from this node
atomic.temperature → Temperature sensor reading
atomic.vibration → Vibration sensor reading
```

### 2. cluster.* Queues (Leader-Published Aggregations)

| Property | Value |
|----------|-------|
| Pattern | Leader-computed, ZMQ pub/sub published |
| Scope | Cluster-wide (all nodes) |
| Leader | Elected via openraft |
| Subscribers | Many concurrent (dashboards, alerts, autoscaling) |
| Latency Target | < 1ms for aggregation computation |

#### Pattern

```
cluster.cpu_avg → Average CPU across all nodes
cluster.temperature_max → Maximum temperature across cluster
cluster.memory_p95 → 95th percentile memory usage
```

### 3. Leader Election (openraft)

| Property | Value |
|----------|-------|
| Scope | One leader per cluster |
| Responsibility | Manage all cluster.* aggregations |
| Protocol | Raft consensus |
| Storage | Durable state for aggregation configuration |

---

## Queue Implementation Options

### Option A: SPMC Lock-Free Queue (Recommended)

```
┌─────────────────────────────────────────────────────────────────┐
│                    SPMC Lock-Free Queue                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   Producer:                    Consumers:                       │
│   ┌─────────┐                  ┌─────────┐ ┌─────────┐         │
│   │ write   │────────────────▶│ read ptr│ │ read ptr│ ...      │
│   │ to tail │                  │ (per    │ │ (per    │         │
│   └─────────┘                  │ consumer│ │ consumer│ )        │
│                                └─────────┘ └─────────┘         │
│                                                                 │
│   Buffer Layout:                                               │
│   ┌───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐      │
│   │ 0 │ 1 │ 2 │ 3 │ 4 │ 5 │ 6 │ 7 │ 8 │ 9 │10 │11 │...│      │
│   └───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘      │
│                      │                                     │
│                  tail (atomic)                          head │
│                                                                 │
│   Producer atomic increment tail                              │
│   Consumer atomic read tail, track own head                   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Advantages:**
- Truly lock-free for producer (single atomic fetch_add)
- Each consumer has independent position (no coordination)
- Pre-allocated, cache-friendly
- Sub-5μs achievable

**Disadvantages:**
- More complex implementation
- Need to handle slow consumers (don't catch up indefinitely)

### Option B: SPSC with Cloning

**Single Producer, Single Consumer** where consumer clones data for multiple subscribers.

**Advantages:**
- Simplest implementation
- Guaranteed ordering

**Disadvantages:**
- Must copy on every read
- Latency increases with subscriber count

---

## Expression Engine

Per design decision: **Use evalexpr** instead of custom parser.

```rust
// Expression examples using evalexpr
let expr = evalexpr::build_expression("local.cpu_percent > 80");
let context = hashmap!{
    "local.cpu_percent" => Value::Float(85.0),
};
let result = expr.eval_with_context(&context)?;
```

**Supported Operations:**
- Arithmetic: `+`, `-`, `*`, `/`, `^`
- Comparisons: `>`, `>=`, `<`, `<=`, `==`, `!=`
- Logical: `&&`, `||`, `!`
- Math functions: `abs`, `sqrt`, `pow`, `round`, `floor`, `ceil`
- Trigonometric: `sin`, `cos`, `tan`

---

## Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Queue push (atomic.*) | < 10μs | SPMC, lock-free |
| Queue pop (atomic.*) | < 5μs | Per consumer |
| Aggregation compute | < 1ms | Leader computes cluster.* |
| Pub/sub publish | < 500μs | ZMQ publish |
| Leader election | < 100ms | openraft fail-over |
| Memory per queue | ~8KB | 1024 capacity, 8-byte values |

---

## CURRENT PROJECT STATUS (Updated: 2025-01-21)

### ✅ FIXED - Expression Module Compilation (2025-01-21)

**Status**: Compilation fixed, all tests passing!

**Changes Made**:
1. Added `DefaultNumericTypes` import from evalexpr
2. Specified explicit generic type for `HashMapContext<DefaultNumericTypes>`
3. Fixed `compile_expression()` to handle undefined variables gracefully
4. Updated tests to use syntactically valid expressions

**Current Status**:
```bash
cargo check --lib  # ✅ Compiles (19 warnings only)
cargo test --lib   # ✅ 27 tests passing
cargo build --lib  # ✅ Builds successfully
```

**Warnings Remain** (non-blocking):
- 5 unused imports
- 4 unused variables  
- 3 dead code warnings
- 1 unnecessary mut

These can be cleaned up in Priority 3 of the TODO list.

### ✅ Fully Implemented Modules (8)

| Module | Lines | Status | Notes |
|--------|-------|--------|-------|
| `src/lib.rs` | 25 | Ready | Module exports, clean |
| `src/config.rs` | 791 | Ready | TOML parsing, config validation |
| `src/types.rs` | 131 | Ready | QueueData, Timestamp, AtomicQueueStats |
| `src/constants.rs` | 302 | Ready | Tuning constants, memory/queue limits |
| `src/network/pubsub/zmq/` | ~500 | Ready | Broker, client, transport |
| `src/dist/` | ~400 | Ready | Aggregator, node client, discovery |
| `src/node.rs` | 446 | Ready | Node collection/subscription |
| `src/queue/spmc_lockfree_queue.rs` | 547 | **Ready** | SPMC lock-free queue, well-implemented |

### ✅ SPMC Lock-Free Queue - Working Implementation

| Property | Value |
|----------|-------|
| **File** | `src/queue/spmc_lockfree_queue.rs` (547 lines) |
| **Pattern** | SPMC (Single Producer, Multiple Consumers) |
| **Features** | Atomic tail, per-consumer head, drop-oldest |
| **Exports** | `SPMCConsumer<T, N>`, `SPMCLockFreeQueue<T, N>` |
| **Status** | Compiles and runs correctly |
| **Tests** | 7 unit tests passing |

**Module Exports (`src/queue/mod.rs`):**
```rust
pub mod spmc_lockfree_queue;
pub use spmc_lockfree_queue::{SPMCConsumer, SPMCLockFreeQueue};
```

### ⚠️ Partial/Stub Implementation (4)

| Module | Issue | Action |
|--------|-------|--------|
| `src/queue/simple_queue.rs` | Uses TokioMutex | Keep for testing ONLY |
| `src/queue/lockfree.rs` | Uses RwLock per slot, NOT lock-free | **DO NOT MODIFY** (per user request) |
| `src/queue/source.rs` | All `start()` methods are stubs | TODO: Implement with evalexpr |
| `src/expression/` | **COMPILATION ERROR** | Add ContextWithMutableVariables trait |

### ❌ Not Implemented (2)

| Module | Status | Notes |
|--------|--------|-------|
| `src/server.rs` | Not implemented | Redis-like API, low priority |
| openraft leader election | Dependency present but unwired | Next major task |

### 📊 Warnings Summary (14 total)

| Category | Count | Action |
|----------|-------|--------|
| Unused imports | 5 | Clean up when time permits |
| Unused variables | 4 | Prefix with `_` or use |
| Unused mut | 1 | Remove `mut` keyword |

### 📋 Compilation Fixes Needed

**File**: `src/expression/mod.rs`

| Line | Issue | Fix |
|------|-------|-----|
| 6 | Missing trait import | Add `ContextWithMutableVariables` to use statement |
| 33 | Type annotation needed | Already resolved by fixing line 6 |

---

## SPMCLockFreeQueue API

### Queue Operations (Producer)

```rust
let queue = SPMCLockFreeQueue::<f64, 1024>::new();

// Push data (single producer only!)
queue.push(value).unwrap();  // Returns Result<bool, QueueError>

// Check queue state
queue.is_empty();
queue.is_full();
queue.len();
queue.dropped_count();  // Track overflow drops
```

### Consumer Operations

```rust
// Create consumer (can create multiple per queue)
let consumer = queue.consumer();
let consumer2 = queue.consumer();

// Pop data (each consumer has independent head position)
while let Some((timestamp, value)) = consumer.pop() {
    println!("{}: {}", timestamp, value);
}

// Batch operations
let batch = consumer.pop_batch(10);  // Get up to 10 items
let all = consumer.drain();           // Get all available

// Peek without consuming
let peek = consumer.peek();  // Next item without advancing

// Get nth item from unread
let fifth = consumer.get_n(5);
```

### QueueTrait Compatibility

```rust
// Implements QueueTrait for integration with existing API
let mut queue: SPMCLockFreeQueue<f64, 1024> = SPMCLockFreeQueue::new();
queue.publish(data)?;    // Uses push internally
let latest = queue.get_latest();
let recent = queue.get_latest_n(5);
```

---

## TODO LIST FOR NEXT DEVELOPER

### Priority 1: ✅ FIXED - Expression Module Compilation

**Completed**: 2025-01-21

Changes:
- Fixed generic type parameters in `src/expression/mod.rs`
- Added `DefaultNumericTypes` import
- Fixed `compile_expression()` to handle undefined variables

Current status:
```bash
cargo check --lib  # ✅ Compiles (warnings only)
cargo test --lib   # ✅ 27 tests passing
```

### Priority 2: Complete ExpressionSource Implementation (1-2 hours)

In `src/queue/source.rs`:
1. Implement `ExpressionSource::start()` using evalexpr
   - Create tokio interval or subscribe to source topics
   - Evaluate expression with context
   - Push results to derived queue
2. Implement `FunctionSource::start()` with tokio interval
   - Periodically call the function
   - Push results to queue

Example implementation pattern:
```rust
impl QueueSource<f64> for ExpressionSource {
    fn start(&self, queue: Arc<RwLock<SimpleQueue<f64>>>, _config: &Config) {
        let expression = self.expression.clone();
        let source_queue = self.source_queue.clone();
        // TODO: Spawn tokio task that:
        // 1. Evaluates expression on interval OR trigger_on_push
        // 2. Uses crate::expression::evaluate_expression()
        // 3. Pushes result to queue
    }
}
```

### Priority 3: Clean Up Warnings (30 minutes)

| File | Warning | Fix |
|------|---------|-----|
| `src/network/pubsub/zmq/transport.rs:5` | Unused `crate::constants` | Remove import |
| `src/network/pubsub/zmq.rs:10` | Unused transport traits | Remove unused imports |
| `src/node.rs:37` | Unused `crate::*` | Remove import |
| `src/queue/registry.rs:6` | Unused expression imports | Remove or use |
| `src/queue/registry.rs:12` | Unused `HashMap` | Remove import |
| `src/config.rs:646,742` | Unused `name` | Use `_name` prefix |
| `src/dist/node_client.rs:222` | Unused `data` | Use `_data` prefix |
| `src/dist/aggregator.rs:246` | Unused `var` | Use `_var` prefix |
| `src/network/pubsub/zmq/transport.rs:87` | Unused `mut` | Remove `mut` |

### Priority 4: openraft Integration (1-2 days)

#### Phase 1: Framework Foundation ✅ COMPLETED (2025-01-23)

**Status**: Complete - Basic openraft infrastructure in place

**Files Created**:
- `src/cluster/types.rs` - TypeConfig using openraft macros
- `src/cluster/raft_node.rs` - RaftClusterNode stub structure

**Files Modified**:
- `src/cluster/mod.rs` - Updated exports
- `src/network/transport_traits.rs` - Added RaftNetworkConnection trait

**TypeConfig Implementation**:
```rust
openraft::declare_raft_types!(
    pub TypeConfig:
        D            = EntryData,
        R            = EntryData,
        NodeId       = u64,
        Node         = BasicNode,
        Entry        = Entry<TypeConfig>,
        SnapshotData = std::io::Cursor<Vec<u8>>,
        AsyncRuntime = TokioRuntime,
);
```

#### Phase 2: RaftStorage Implementation ✅ COMPLETED (2025-01-23)

**Status**: Complete - Full RaftStorage implementation

**Files Created**:
- `src/cluster/storage.rs` (~240 lines)

**Features**:
- In-memory storage using BTreeMap
- Thread-safe access with RwLock
- Full RaftStorage trait implementation
- Vote persistence
- Log state management
- State machine application
- Snapshot handling

**Key Structures**:
```rust
pub struct AutoqueuesRaftStorage {
    log: RwLock<BTreeMap<u64, Entry<TypeConfig>>>,
    vote: RwLock<Option<Vote<u64>>>,
    last_applied: RwLock<Option<LogId<u64>>>,
    committed: RwLock<Option<LogId<u64>>>,
    snapshot_meta: RwLock<Option<SnapshotMeta<u64, BasicNode>>>,
    snapshot: RwLock<Option<Snapshot<TypeConfig>>>,
}
```

#### Phase 3: Cluster Coordination ✅ COMPLETED (2025-01-26)

**Status**: Complete - Full cluster coordination with leader election

**Files Created**:
- `src/cluster/raft_cluster.rs` (~110 lines)
- `src/network/raft_transport.rs` (~100 lines)

**Files Modified**:
- `src/cluster/mod.rs` - Added raft_cluster exports
- `src/network/mod.rs` - Added raft_transport exports
- `src/dist/aggregator.rs` - Added leader check

**Key Structures**:
```rust
pub struct RaftClusterNode {
    pub node_id: u64,
    pub storage: Arc<AutoqueuesRaftStorage>,
    pub network: Arc<ZmqRaftNetwork>,
    pub peers: HashMap<u64, NodeConfig>,
    pub is_leader: Arc<RwLock<bool>>,
}

pub struct ZmqRaftNetwork {
    pub context: Arc<Context>,
    pub node_id: u64,
    pub peers: HashMap<u64, (String, u16)>,
    pub request_socket: Socket,
}
```

**Features**:
- Full RaftClusterNode implementation
- ZMQ-based Raft network transport
- Leader election checking
- Cluster initialization
- Node management (add/remove)
- Integration with distributed aggregator

**Leader-Only Publishing**:
```rust
if self.raft_node.is_leader().await {
    let entry = EntryData { ... };
    self.raft_node.propose_aggregation(entry).await?;
}
```

**Verification**:
```bash
cargo check --lib  # ✅ Compiles
cargo test --lib   # ✅ 27 tests passing
```

---

## 🚀 OPENRAFT INTEGRATION COMPLETE!

All 3 phases of openraft implementation are now complete:

#### Phase 3: Cluster Coordination (Later)

- RaftClusterNode full implementation
- Leader election logic
- Network transport using ZMQ

```bash
# Dependencies already present in Cargo.toml:
openraft = { version = "0.9.21", features = ["serde", "serde_json"] }
```

### Priority 5: Benchmark and Optimize (1 day)

After core functionality:
1. Run queue benchmarks:
   ```bash
   cargo bench --lib
   ```
2. Profile multi-consumer scenarios
3. Tune constants in `src/constants.rs`

### Dependency Already Added (evalexpr 13.1)

The evalexpr dependency is already in `Cargo.toml`:
```toml
evalexpr = { version = "13.1", features = ["regex", "serde"] }
```

No need to add it again. The expression module needs minor fix only.

---

## File Structure (Current)

```
autoqueues/
├── Cargo.toml                # Dependencies (evalexpr 13.1 present)
├── config.toml               # Example configuration
├── README.md
├── AGENTS.md                 # Agent instructions
├── PRD.md                    ← Updated 2025-01-21
├── src/
│   ├── lib.rs                # Module exports (25 lines)
│   ├── autoqueues.rs         # Main API
│   ├── config.rs             # TOML parsing (791 lines)
│   ├── constants.rs          # Tuning constants (302 lines)
│   ├── types.rs              # Core types (131 lines)
│   ├── node.rs               # Node management (446 lines)
│   ├── cluster.rs            # Cluster coordination
│   ├── metrics.rs            # System metrics collection
│   ├── traits/
│   │   ├── mod.rs
│   │   ├── queue.rs          # QueueTrait definition
│   │   └── transport.rs      # Transport traits
│   ├── queue/
│   │   ├── mod.rs            # Module exports (19 lines) ✅
│   │   ├── spmc_lockfree_queue.rs  # SPMC queue (547 lines) ✅ DONE
│   │   ├── simple_queue.rs   # Keep for testing
│   │   ├── lockfree.rs       # DO NOT MODIFY
│   │   ├── source.rs         # QueueSource trait (162 lines) ⚠️ STUBS
│   │   ├── registry.rs       # Queue registry
│   │   └── queue_server.rs   # Queue server handle
│   ├── expression/           # Expression module (165 lines) ⚠️ FIX NEEDED
│   │   └── mod.rs            # evalexpr wrapper
│   ├── network/
│   │   ├── mod.rs
│   │   ├── transport_traits.rs
│   │   └── pubsub/
│   │       ├── mod.rs
│   │       └── zmq/
│   │           ├── broker.rs
│   │           ├── client.rs
│   │           ├── transport.rs
│   │           └── mod.rs
│   └── dist/
│       ├── mod.rs
│       ├── aggregator.rs     # Distributed aggregator
│       ├── node_client.rs    # Node client
│       ├── sync.rs           # Sync logic
│       └── discovery.rs      # Node discovery
└── tests/
    └── ...                   # Integration tests
```

---

## Implementation Roadmap (Updated: 2025-01-21)

### Phase 0: Critical Fixes (Day 0)

**Status**: SPMC Queue is DONE. Need to fix expression module.

1. **Fix Expression Module Compilation** (5 minutes)
   - Add missing trait import in `src/expression/mod.rs:6`
   - Run `cargo check --lib` to verify

2. **Clean Minor Warnings** (30 minutes)
   - Remove unused imports
   - Prefix unused variables with `_`
   - Remove unnecessary `mut`

### Phase 1: Core Queue System (Days 1-3) ✅ PARTIALLY DONE

1. **SPMCQueue Implementation** ✅ DONE
   - Single producer, multiple consumers ✅
   - Atomic tail index, per-consumer head indices ✅
   - Pre-allocated circular buffer ✅
   - Drop-oldest on overflow ✅

2. **Expression Module Fix** ⚠️ IN PROGRESS
   - Missing trait import ⚠️ FIX NEEDED
   - evalexpr already integrated

### Phase 2: Source Traits Implementation (Days 3-5)

3. **Implement ExpressionSource::start()**
   - Expression evaluation loop
   - Subscribe to source atomic.* topics
   - Push results to derived queue

4. **Complete FunctionSource**
   - Periodic function invocation with tokio interval
   - Push results to queue

5. **Clean Up SimpleQueue**
   - Keep for testing only
   - Document as not for production

### Phase 3: Distributed Coordination (Days 5-8)

6. **Wire openraft**
   - Create RaftStorage for cluster.* aggregation state
   - Implement leader election logic
   - Handle leader changes

7. **Implement Global Aggregation**
   - Leader subscribes to all relevant atomic.* topics
   - Computes aggregations (avg, max, min, p95, etc.)
   - Publishes cluster.* via ZMQ

### Phase 4: Testing & Optimization (Days 8-14)

8. **Benchmark Suite**
   - Queue latency benchmarks
   - Multi-consumer scenarios
   - Cluster aggregation throughput

9. **Performance Tuning**
   - Constant tuning in constants.rs
   - Batch pub/sub operations
   - Minimize allocations

### Phase 3: Distributed Coordination (Days 7-10)

6. **Wire openraft**
   - Create RaftStorage
   - Implement leader election
   - Handle leader changes

7. **Implement Global Aggregation**
   - Leader subscribes to all relevant atomic.* topics
   - Computes aggregations (avg, max, min, p95, etc.)
   - Publishes cluster.* via ZMQ

8. **Connect Node Aggregation**
   - Wire aggregate_global() to leader
   - Handle node joins/leaves

### Phase 4: Testing & Optimization (Days 11-14)

9. **Benchmark Suite**
   - Queue latency benchmarks
   - Multi-consumer scenarios
   - Cluster aggregation throughput

10. **Performance Tuning**
    - Constant tuning in constants.rs
    - Batch pub/sub operations
    - Minimize allocations

---

## Configuration Format

```toml
# config.toml

# Cluster nodes
[node1]
host = "127.0.0.1"
communication_port = 7067
coordination_port = 7070  # Raft port

[node2]
host = "127.0.0.1"
communication_port = 7167
coordination_port = 7170

# Local metric sources (atomic.*)
// Producer: Node's collector
// Queue: SPMC, drop-oldest
[local]
cpu = "function:cpu"
memory = "function:memory"
temperature = "function:sensor_temp"

# Global aggregations (cluster.*)
// Computed by leader, published to all
[global]
[global.cpu_avg]
aggregation = "avg"
sources = ["cpu"]  // Subscribe to atomic.cpu from all nodes
interval_ms = 1000

[global.temperature_max]
aggregation = "max"
sources = ["temperature"]
interval_ms = 500
```

---

## API Reference

### Programmatic Mode

```rust
// Create AutoQueues with configuration
let autoqueues = AutoQueues::new(config);

// Add function-based queue (atomic.cpu)
autoqueues.add_queue_fn("cpu", || get_cpu_percent())?;

// Add expression-based derived queue
autoqueues.add_queue_expr(
    "high_cpu_warning",
    "atomic.cpu > 80",  // Using evalexpr
    "atomic.cpu",
    true,   // trigger_on_push
    Some(1000)  // evaluate interval
)?;

// Add distributed aggregation (cluster.*)
autoqueues.add_distributed_queue(
    "cluster_cpu_avg",
    "avg",
    vec!["cpu".to_string()],
)?;

// Start all queues
autoqueues.start();

// Pop from queue
let value: Option<f64> = autoqueues.pop("atomic.cpu")?;

// Try pop (non-blocking)
let value: Option<f64> = autoqueues.try_pop("atomic.cpu")?;
```

### Server Mode (TODO - Not Yet Implemented)

```rust
// Redis-like API over ZMQ
let server = AutoQueuesServer::new(config)?;
server.bind("tcp://*:6969").await?;
server.run().await;
```

Commands:
- `PUSH queue_name value` → Add to atomic.*
- `POP queue_name` → Remove from queue
- `SUBSCRIBE topic_pattern` → Subscribe to cluster.* or atomic.*
- `PUBLISH topic value` → Publish raw value

---

## File Structure (Reference)

**See "File Structure (Current)" section earlier in this document for the complete, up-to-date file structure.** The earlier section was updated on 2025-01-21.

**Quick Reference:**
- ✅ `src/queue/spmc_lockfree_queue.rs` (547 lines) - DONE
- ⚠️ `src/queue/source.rs` (162 lines) - Stubs need implementation
- ⚠️ `src/expression/mod.rs` (165 lines) - Minor fix needed
- ❌ `src/server.rs` - Not yet implemented
- ✅ `src/dist/` (~400 lines) - Working
- ✅ `src/network/pubsub/zmq/` (~500 lines) - Working

---

## Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| `tokio` | 1 (full) | Async runtime |
| `zmq` | 0.10.0 | Pub/sub messaging |
| `dashmap` | 6.1 | Concurrent registry |
| `evalexpr` | 13.1 | Expression evaluation |
| `openraft` | 0.9.21 | Leader election |
| `circular-buffer` | 1.2 | Ring buffer (simple queue) |
| `sysinfo` | 0.37 | System metrics |
| `serde` + `toml` | 1.0 / 0.9 | Config serialization |

---

## Known Issues

1. **LockFreeQueue not truly lock-free**
   - Uses `RwLock` per buffer slot
   - Fix: Use atomic CAS only, or deprecate

2. **ExpressionSource::start() is stub**
   - No actual evaluation loop
   - Fix: Implement using evalexpr

3. **Global aggregation incomplete**
   - Leader election needed first
   - Wire through openraft

4. **Server mode not implemented**
   - `src/server.rs` missing
   - Lower priority (per requirements)

---

## Testing Strategy

### Unit Tests

- SPMC queue: Single producer, multiple consumers
- Expression evaluation: evalexpr integration
- Configuration parsing
- Aggregation functions (avg, max, min, p95)

### Integration Tests

- ZMQ pub/sub between two nodes
- Leader election (3-node cluster)
- Global aggregation computation
- Fail-over handling

### Benchmarks

- Queue push/pop latency (atomic.*)
- Aggregation compute time
- Pub/sub throughput
- Leader election time

---

## Open Questions

| # | Question | Status |
|---|----------|--------|
| 1 | SPMC consumer position tracking? | Need decision |
| 2 | Topic naming convention? | `metric:cpu` vs `atomic.cpu`? |
| 3 | Maximum subscribers per topic? | Limit or unlimited? |
| 4 | Aggregation windowing? | Time-based or count-based? |

---

## Appendix: Code Review Summary (Updated: 2025-01-21)

### ✅ RESOLVED: `src/queue/spmc_lockfree_queue.rs`

The SPMC lock-free queue implementation is complete and working:
- Atomic tail for single producer ✅
- Per-consumer head positions ✅
- Pre-allocated circular buffer ✅
- Drop-oldest semantics ✅
- QueueTrait implementation ✅

**Tests**: 7 unit tests passing.

### ✅ RESOLVED: `src/expression/mod.rs` Compilation

**Fixed**: 2025-01-21

Changes:
- Added `DefaultNumericTypes` import
- Specified explicit generic type for HashMapContext
- Fixed `compile_expression()` error handling

**Status**: ✅ All 27 tests passing

### ⚠️ Current Issue: `src/queue/source.rs` Stubs

Lines 49-67, 94-110, 142-161: All `start()` methods are TODO stubs.

**Next Task**: Implement background tasks that:
1. **ExpressionSource**: Evaluate expression on interval or trigger_on_push
2. **FunctionSource**: Call function periodically with tokio interval
3. **DistributedAggregationSource**: Wire to aggregator

### ⚠️ Current Issue: `src/queue/lockfree.rs` Naming

Line 23: `buffer: Vec<std::sync::RwLock<Option<QueueData<T>>>>`

**Status**: Per user request - DO NOT MODIFY. The name "LockFreeQueue" is misleading since it uses RwLock, but the implementation is kept as-is for compatibility.

### ✅ RESOLVED: Expression Module Decision

**Status**: Keep the custom expression module (`src/expression/`) as a thin wrapper around evalexpr.

**Reason**: Provides custom error types (`ExpressionError`) and helper functions that simplify usage for the rest of the codebase.

---

## Appendix: Known Warnings (14 Total)

| File | Line | Warning | Priority |
|------|------|---------|----------|
| `src/network/pubsub/zmq/transport.rs` | 5 | Unused `crate::constants` | Low |
| `src/network/pubsub/zmq.rs` | 10 | Unused transport traits | Low |
| `src/node.rs` | 37 | Unused `crate::*` import | Low |
| `src/queue/registry.rs` | 6 | Unused expression imports | Low |
| `src/queue/registry.rs` | 12 | Unused `HashMap` import | Low |
| `src/config.rs` | 646, 742 | Unused `name` variable | Low |
| `src/dist/node_client.rs` | 222 | Unused `data` variable | Low |
| `src/dist/aggregator.rs` | 246 | Unused `var` variable | Low |
| `src/network/pubsub/zmq/transport.rs` | 65, 88, 99 | Unused `e` variable | Low |
| `src/network/pubsub/zmq/transport.rs` | 87 | Unnecessary `mut` | Low |

**Action**: Clean up when time permits - does not block development.

---

## References

- [evalexpr crate](https://crates.io/crates/evalexpr)
- [openraft crate](https://crates.io/crates/openraft)
- [ZMQ Guide](http://zguide.zeromq.org/)
- [Lock-Free Queue Patterns](https://www.1024cores.net/home/lock-free-algorithms)