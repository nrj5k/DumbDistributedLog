# Distributed Benchmark Guide

This guide explains how to run distributed benchmarks for DDL (Dumb Distributed Log).

## Overview

DDL supports two benchmark modes:

1. **In-Memory (Local Tests)** - Fast unit tests with no network overhead
2. **Integration (Real Network)** - Full distributed tests with actual network communication

## Quick Start

### 1. Build the Node Binary

```bash
cargo build --release --bin ddl-node
```

### 2. Start a Cluster (3 nodes)

```bash
# Start 3-node cluster on ports 7000-7002
./scripts/start-cluster.sh 3

# Or custom base port
./scripts/start-cluster.sh 3 8000
```

### 3. Run Benchmarks

```bash
# Run all benchmarks against the cluster
./scripts/run-integration-tests.sh

# Or run specific tests
cargo test --test distributed_benchmark --features integration-tests

# Or use the benchmark binary
RUST_TEST_NODES="localhost:[7000-7002]" cargo run --release --bin ddl-benchmark
```

### 4. Stop the Cluster

```bash
./scripts/stop-cluster.sh
```

## Benchmark Categories

### A. Cluster Setup

Measures cluster initialization and leader election performance.

- **Node Creation Time**: Time to create Raft nodes
- **Cluster Initialization**: Time to bootstrap cluster
- **Leader Election**: Time to elect a leader
- **Total Setup Time**: End-to-end cluster ready time

```bash
cargo test --test distributed_benchmark --features integration-tests -- benchmark_cluster_setup
```

### B. Topic Operations

Measures topic ownership operations throughput and latency.

- **Topic Claims**: Operations per second for claiming topics
- **Topic Releases**: Operations per second for releasing topics
- **Ownership Queries**: Latency for querying topic ownership (p50, p95, p99)

```bash
cargo test --test distributed_benchmark --features integration-tests -- benchmark_topic_operations
```

### C. Consensus Operations

Measures Raft consensus performance.

- **Log Append Latency**: Time to append entries to Raft log
- **Leader Failover**: Time to elect new leader after failure (multi-node)
- **Partition Recovery**: Time to heal network partitions (multi-node)

```bash
cargo test --test distributed_benchmark --features integration-tests -- benchmark_consensus_operations
```

### D. Concurrent Load

Measures performance under concurrent operations.

- **Throughput**: Operations per second under load
- **Memory Usage**: Peak and average memory usage
- **Lock Contention**: Percentage of time spent waiting for locks

```bash
cargo test --test distributed_benchmark --features integration-tests -- benchmark_concurrent_load
```

## Running Benchmarks

### Local Tests (Default)

In-memory tests with no network overhead. These are fast and suitable for CI/CD.

```bash
# Run all local tests
cargo test --test distributed_benchmark

# Run with custom configuration
RUST_TEST_NODES="localhost:[8080-8082]" cargo test --test distributed_benchmark
```

### Integration Tests (Real Network)

Tests against actual running nodes. Requires cluster to be started first.

```bash
# Start cluster
./scripts/start-cluster.sh 3

# Run integration tests
cargo test --test distributed_benchmark --features integration-tests

# Cleanup
./scripts/stop-cluster.sh
```

### Custom Configuration

Use environment variables to customize benchmarks:

```bash
# Node specification
export RUST_TEST_NODES="ares-comp-[13-16]"

# Duration (seconds)
export RUST_TEST_DURATION="120"

# Number of topics
export RUST_TEST_TOPICS="1000"

# Concurrent operations
export RUST_TEST_THREADS="200"

# Run benchmark
cargo test --test distributed_benchmark --features integration-tests -- full_benchmark_suite
```

### Stress Tests

Long-running tests for stability:

```bash
# Run stress test (ignored by default)
cargo test --test distributed_benchmark --features integration-tests -- --ignored high_load_stress_test

# Run sustained load test
cargo test --test distributed_benchmark --features integration-tests -- --ignored sustained_load_test
```

## Node Specification Format

The benchmark supports flexible node specifications:

### Examples

| Specification | Result |
|--------------|--------|
| `node-[01-33]` | node-01, node-02, ..., node-33 |
| `ares-comp-[13-16]` | ares-comp-13, ares-comp-14, ares-comp-15, ares-comp-16 |
| `localhost:[8080-8082]` | localhost:8080, localhost:8081, localhost:8082 |
| `10.0.0.[1-10]:9090` | 10.0.0.1:9090, ..., 10.0.0.10:9090 |
| `node[1-5]` | node1, node2, node3, node4, node5 |
| `192.168.1.[10-12]:8080` | 192.168.1.10:8080, 192.168.1.11:8080, 192.168.1.12:8080 |
| `node[1,3-5,7]` | node1, node3, node4, node5, node7 |

### Advanced Patterns

- **Leading zeros**: `node-[01-05]` → node-01, node-02, ..., node-05
- **Mixed ranges**: `cluster-[1-2]-node[0-1]` → cluster-1-node0, cluster-1-node1, ..., cluster-2-node1
- **IP ranges**: `10.0.[0-1].[1-3]:9090` → All combinations

## Binary: ddl-node

Standalone DDL node for manual cluster deployment.

### Options

```
ddl-node [OPTIONS]

REQUIRED:
  --id, -i <NUM>            Node ID (unique cluster identifier)

OPTIONS:
  --port, -p <NUM>         Coordination port (default: 6967)
  --peers <ADDRS>          Comma-separated peer addresses (host:port)
  --bootstrap, -b          Bootstrap a new cluster (first node only)
  --data-dir, -d <PATH>    Data directory for persistence (default: /tmp/ddl-node-<id>)
  --host <HOST>            Host address to bind (default: 0.0.0.0)
  --communication-port, -c <NUM>  Communication port (default: coordination-port - 3)
  --help                   Show help message
```

### Examples

```bash
# Single-node bootstrap
ddl-node --id 1 --port 7000 --bootstrap

# Multi-node cluster
# Node 1 (bootstrap)
ddl-node --id 1 --port 7000 --bootstrap --data-dir ./data/node1

# Node 2
ddl-node --id 2 --port 7001 --peers localhost:7000 --data-dir ./data/node2

# Node 3
ddl-node --id 3 --port 7002 --peers localhost:7000 --data-dir ./data/node3
```

## Binary: ddl-benchmark

Benchmark driver for automated testing.

### Options

```
ddl-benchmark [OPTIONS]

OPTIONS:
  --nodes, -n <SPEC>      Node specification (e.g., "node-[01-33]")
  --duration, -d <SEC>    Benchmark duration in seconds (default: 60)
  --topics, -t <COUNT>    Number of topics (default: 100)
  --threads, -T <COUNT>   Concurrent threads (default: 100)
  --help, -h              Show help message
```

### Examples

```bash
# Default benchmark
cargo run --bin ddl-benchmark

# Custom nodes and duration
cargo run --bin ddl-benchmark -- --nodes "ares-comp-[13-16]" --duration 120

# High-load test
cargo run --bin ddl-benchmark -- --nodes "localhost:[8080-8082]" --threads 200 --topics 1000
```

## Performance Targets

For production-ready DDL clusters, we target:

| Metric | Target |
|---------|--------|
| Setup Time | < 10 seconds |
| Leader Election | < 5 seconds |
| Claim Throughput | > 100,000 ops/sec |
| Query Latency (p99) | < 100ms |
| Concurrent Operations | > 10,000 ops/sec |

## Benchmark Output Format

The benchmark outputs structured results:

```
================================================================================
DISTRIBUTED DDL BENCHMARK
================================================================================

Node Specification: localhost:[7000-7002] (3 nodes)
Started: 2026-04-01 12:00:00 UTC

--------------------------------------------------------------------------------
CLUSTER SETUP
--------------------------------------------------------------------------------
Node Creation:           3 nodes    50ms    16.67ms/node
Cluster Initialization:   1 cluster   200ms    200.00ms/cluster
Leader Election:         1 election  1500ms    1500.00ms/election
Total Setup Time:           1750ms

--------------------------------------------------------------------------------
TOPIC OPERATIONS
--------------------------------------------------------------------------------
Topic Claim (1 node):      100 ops    15ms    6666.67 ops/sec
Topic Claim (all nodes):   100 ops    20ms    5000.00 ops/sec
Topic Release:             50 ops     8ms    6250.00 ops/sec
Ownership Query:         10000 ops   120ms    83333.33 ops/sec
    p50 latency:  0.005ms
    p95 latency:  0.015ms
    p99 latency:  0.050ms

--------------------------------------------------------------------------------
PERFORMANCE SUMMARY
--------------------------------------------------------------------------------
✓ Setup Time:            1.8s (target: <10s)
✓ Leader Election:         1500ms (target: <5000ms)
✓ Claim Throughput:        6667 ops/sec (target: 100000+)
✓ Query Latency p99:      0.05ms (target: <100ms)
================================================================================
```

## Troubleshooting

### Cluster won't start

```bash
# Check logs
tail -f /tmp/ddl-node-*.log

# Verify ports are free
netstat -tulpn | grep 7000

# Clear old data
rm -rf /tmp/ddl-data-*
```

### Connection refused errors

```bash
# Ensure cluster is running
ps aux | grep ddl-node

# Restart cluster
./scripts/stop-cluster.sh
./scripts/start-cluster.sh 3
```

### Low performance

```bash
# Increase benchmark parameters
RUST_TEST_THREADS=200 RUST_TEST_TOPICS=1000 cargo test --test distributed_benchmark

# Check system resources
top -p $(pgrep ddl-node)
```

### Port conflicts

```bash
# Use different base port
./scripts/start-cluster.sh 3 9000
RUST_TEST_NODES="localhost:[9000-9002]" cargo test --test distributed_benchmark
```

## Architecture

For architectural details, see:
- [ARCHITECTURE.md](./ARCHITECTURE.md) - System architecture
- [API.md](./API.md) - API documentation
- [LIMITATIONS.md](./LIMITATIONS.md) - Current limitations

## Contributing

When contributing benchmarks:

1. **Add new tests to `tests/distributed_benchmark.rs`**
2. **Follow existing patterns** for test naming and structure
3. **Document configuration** in this guide
4. **Add meaningful assertions** - avoid tests that always pass
5. **Test locally and integration modes**

## Future Improvements

Planned benchmark enhancements:

- [ ] Network partition simulation
- [ ] Leader failure and recovery tests
- [ ] Snapshot creation/transfer benchmarks
- [ ] Multi-datacenter latency tests
- [ ] Long-running stability tests
- [ ] Memory leak detection
- [ ] GC pause impact measurement