# DDL Limitations and Trade-offs

**Honest assessment of what DDL provides, what it doesn't, and when NOT to use it.**

## Table of Contents

1. [What DDL Is](#what-ddl-is)
2. [What DDL Is NOT](#what-ddl-is-not)
3. [Trade-offs Made](#trade-offs-made)
4. [When NOT to Use DDL](#when-not-to-use-ddl)
5. [Comparison with Alternatives](#comparison-with-alternatives)
6. [Known Limitations](#known-limitations)
7. [Roadmap Considerations](#roadmap-considerations)

---

## What DDL Is

DDL is a **distributed append-only log** with a clear, focused purpose:

- **Yes**: Reliable data movement between producers and consumers
- **Yes**: Topic-based organization with pattern subscriptions
- **Yes**: At-least-once delivery semantics (with acknowledgments)
- **Yes**: Raft-based shard assignment for cluster coordination
- **Yes**: Multiple transport options (TCP, ZMQ, Hybrid)
- **Yes**: Multiple storage options (in-memory, WAL)
- **Yes**: Simple, clean trait-based API

DDL excels at moving bytes reliably from producers to consumers in a distributed system.

---

## What DDL Is NOT

These are features DDL deliberately does NOT provide:

### 1. NOT an Expression Engine

DDL does not evaluate expressions, perform calculations, or filter data.

```rust
// DDL: No
ddl.push("metrics", compute_expression("cpu * 100 / max_cpu"))

// You must: Pre-compute before pushing
let value = cpu_value / max_value * 100.0;
ddl.push("metrics", value.to_le_bytes());
```

**What to use instead:**
- `evalexpr` crate for expression evaluation
- `fasteval` crate for fast expression evaluation
- Build expression evaluation as a separate service

### 2. NOT a Metrics System

DDL does not collect, aggregate, or visualize metrics.

```rust
// DDL: No
// - No CPU monitoring
// - No health scores
// - No dashboards
// - No alerts
```

**What to use instead:**
- Prometheus for metrics collection and alerting
- Grafana for visualization
- Datadog for cloud monitoring

### 3. NOT an Aggregation System

DDL does not combine or aggregate data from multiple topics.

```rust
// DDL: No
// - No sum/avg/count across topics
// - No time-window aggregations
// - No stream processing
```

**What to use instead:**
- Apache Flink for stream processing
- Apache Kafka Streams for aggregation
- Rust `rxr` crate for reactive extensions

### 4. NOT a General-Purpose Message Queue

DDL is designed specifically for log-style workloads, not general messaging.

```rust
// DDL is NOT designed for:
// - Request/reply patterns (use with care)
// - Priority queues
// - Message scheduling
// - Dead letter queues
// - Message transformation
```

**What to use instead:**
- RabbitMQ for complex messaging patterns
- Apache Pulsar for feature-rich queuing
- Redis Streams for simpler queuing needs

### 5. NOT a Database

DDL does not provide query capabilities, indexing, or random access.

```rust
// DDL: No
// - No WHERE clauses
// - No JOIN operations
// - No indexing
// - No SQL-like queries
```

**What to use instead:**
- PostgreSQL for relational data
- MongoDB for document data
- Elasticsearch for search

### 6. NOT a Transaction System

DDL does not support multi-topic atomic operations.

```rust
// DDL: No
// - No distributed transactions
// - No two-phase commit
// - No saga support
```

**What to use instead:**
- Distributed databases with transaction support
- Event sourcing with eventual consistency
- Saga pattern implementation

---

## Trade-offs Made

DDL made intentional trade-offs for simplicity and performance:

### Trade-off 1: Simple API, Limited Features

**Decision**: Minimal trait-based API vs. feature-rich framework

**Trade-off:**
- **Pro**: Easy to understand, use, and extend
- **Con**: Must build features on top or use other tools

**Rationale**: Complexity breeds bugs. Simple systems are more reliable.

### Trade-off 2: Topic-Owner Model vs. Proxied Access

**Decision**: Direct producer-to-owner connections vs. proxy/middleware

**Trade-off:**
- **Pro**: Lower latency, no single point of bottleneck
- **Con**: Clients must know topic ownership (handled via gossip)

**Rationale**: Performance is critical for HPC workloads. Gossip provides discovery without introducing latency.

### Trade-off 3: Raft for Shard Assignment Only

**Decision**: Raft consensus only for ownership, not data

**Trade-off:**
- **Pro**: Consensus overhead only during ownership changes
- **Con**: Cannot use Raft for strongly consistent data transfer

**Rationale**: Consensus is expensive. Data transfer doesn't need it if ownership is correct.

### Trade-off 4: Optional Durability

**Decision**: In-memory as default, WAL as optional

**Trade-off:**
- **Pro**: Maximum performance for non-critical data
- **Con**: Data can be lost on crash (if WAL disabled)

**Rationale**: Users should choose reliability vs. speed. No hidden costs.

### Trade-off 5: At-Least-Once Only

**Decision**: No exactly-once delivery guarantee

**Trade-off:**
- **Pro**: Simpler implementation, higher performance
- **Con**: Consumers must handle duplicates

**Rationale**: Exactly-once is extremely expensive. Most use cases can handle duplicates.

### Trade-off 6: No Built-in Compression

**Decision**: Raw bytes only, no built-in compression

**Trade-off:**
- **Pro**: Zero overhead for already-compressed data
- **Con**: Higher network/storage for text data

**Rationale**: Let users decide. Compress before push if needed.

---

## When NOT to Use DDL

### 1. When You Need Complex Querying

**Don't use DDL if**:
- You need to query data by attributes
- You need SQL-like filtering
- You need joins between data sources

**Use instead**: Elasticsearch, PostgreSQL, MongoDB

### 2. When You Need Strong Ordering Across Topics

**Don't use DDL if**:
- You need global ordering of events across all topics
- You need transactional consistency across topics

**Use instead**: Kafka (with careful partition design), distributed databases

### 3. When You Need Exactly-Once Semantics

**Don't use DDL if**:
- Duplicate processing is unacceptable
- Financial transactions require exactly-once

**Use instead**:Kafka with transactions, specialized event sourcing systems

### 4. When You Need Automatic Failover

**Don't use DDL if**:
- Topic availability during node failure is critical
- You cannot tolerate any unavailability

**Use instead**: Managed services with automatic failover (Cloud Pub/Sub, Azure Event Hubs)

### 5. When You Need Built-in Monitoring

**Don't use DDL if**:
- You need built-in metrics, dashboards, alerts
- You want zero-config observability

**Use instead**: Managed logging/monitoring services, Prometheus+Grafana

### 6. When Your Team Lacks Rust Expertise

**Don't use DDL if**:
- Your team is not familiar with Rust
- You need easy debugging in other languages
- Quick onboarding is critical

**Use instead**: Java (Kafka), Go (NATS), Python (Celery)

### 7. When You Need Schema Registry

**Don't use DDL if**:
- You need schema evolution and validation
- Data contracts are critical

**Use instead**:Confluent Schema Registry, Apache Avro

### 8. When Deployment Complexity Is a Concern

**Don't use DDL if**:
- You need a managed service
- Operational overhead is unacceptable
- You need multi-cloud support

**Use instead**: AWS Kinesis, Google Pub/Sub, Azure Event Hubs

---

## Comparison with Alternatives

### vs. Redis Streams

| Aspect | Redis Streams | DDL |
|--------|---------------|-----|
| Scalability | Degrades after 10K topics | Linear with cluster size |
| Topic ownership | Unclear, clients guess | Raft decides definitively |
| Cross-region | Complex GSL, eventual consistency | Raft, strong consistency |
| Latency | ~1ms | <100μs |
| Durability | AOF/RDB (async by default) | WAL (sync or async) |
| Cluster mode | Redis Cluster (hash slots) | Custom Raft-based |
| Client libraries | Many (all languages) | Rust (primarily) |

**When Redis Streams is better:**
- Team already uses Redis
- Simpler operational requirements
- Multi-language ecosystem

**When DDL is better:**
- 10K+ topics (ORNL Frontier use case)
- Strong consistency requirements
- Maximum performance needed
- Rust-based infrastructure

### vs. Apache Kafka

| Aspect | Kafka | DDL |
|--------|-------|-----|
| Architecture | Log-based, partition-based | Topic-owner, direct connections |
| Ordering | Per-partition global | Per-topic only |
| Scalability | Horizontal (partitions) | Horizontal (topics) |
| Protocol | Binary over TCP | TCP or ZMQ |
| Management | ZooKeeper/Kraft, complex | Simple, no external dependencies |
| Learning curve | High (concepts, ops) | Low (simple API) |
| Ecosystem | Large (connect, streams) | Minimal (focus on log) |

**When Kafka is better:**
- Need complex stream processing (Kafka Streams)
- Large ecosystem integration required
- Managed service available
- Team already knows Kafka

**When DDL is better:**
- Simpler operational requirements
- Direct producer-consumer without broker
- Lower resource footprint
- Custom deployment requirements

### vs. NATS

| Aspect | NATS | DDL |
|--------|------|-----|
| Messaging pattern | Pub/sub, request/reply, queue | Pub/sub only |
| Message durability | JetStream (optional) | WAL (optional) |
| Ordering | No guarantees | Per-topic |
| Protocol | TEXT-based | Binary |
| Scalability | Very high | High |
| Language support | Many | Rust (primarily) |

**When NATS is better:**
- Need request/reply patterns
- Multi-language environment
- Text protocol preferred
- Lightweight deployment

**When DDL is better:**
- Need message durability with ordering
- Need Raft-based ownership
- Rust ecosystem
- Higher throughput requirements

### vs. Apache Pulsar

| Aspect | Pulsar | DDL |
|--------|--------|-----|
| Architecture | Broker + BookKeeper | Direct owner connections |
| Durability | BookKeeper (mandatory) | WAL (optional) |
| Subscription types | Exclusive, shared, failover | Simple subscription |
| Multi-tenancy | Native | Not built-in |
| Protocol | Binary (ARQ, WebSocket) | TCP or ZMQ |

**When Pulsar is better:**
- Need multi-tenancy
- Need built-in geo-replication
- Need complex subscription types
- Managed service available

**When DDL is better:**
- Simpler architecture needed
- No BookKeeper overhead
- Direct connections for performance
- Custom deployment

---

## Known Limitations

### Current Limitations (v0.2)

1. **No Automatic Topic Creation**
   - Must create topic before pushing (no auto-create)
   - Workaround: Create topics during initialization

2. **No Consumer Groups**
   - No shared offsets across consumers
   - Each consumer gets all messages
   - Workaround: Build on DDL trait for consumer groups

3. **No Message Replay**
   - Once acknowledged, cannot replay
   - Cannot seek to previous positions
   - Workaround: Keep unacknowledged until ready

4. **No Compaction**
   - Log grows indefinitely
   - No built-in cleanup of old data
   - Workaround: Implement in application layer

5. **Single Acknowledgment**
   - No acknowledgment hierarchies
   - Simple ack per entry
   - Workaround: Batch acknowledgments

6. **Limited Monitoring**
   - No built-in metrics endpoint
   - No health checks
   - Workaround: Implement via callbacks

7. **No Schema Support**
   - No built-in schema registry
   - No schema validation
   - Workaround: Use external registry, validate before push

### Future Limitations (Intentional Non-Goals)

These will likely NEVER be implemented:

1. **No Built-in Expression Engine**
   - Keep DDL simple, build expressions separately
   - Use `evalexpr` or `fasteval` if needed

2. **No Built-in Aggregation**
   - Keep DDL simple, build aggregations separately
   - Use Flink, Kafka Streams, or Rust streams

3. **No Built-in Monitoring**
   - Keep DDL simple, use external tools
   - Use Prometheus, Grafana, Datadog

4. **No Built-in alerting**
   - Keep DDL simple, use external tools
   - Use Prometheus Alertmanager

---

## Roadmap Considerations

### Potential Future Additions

These are POSSIBLE additions, not commitments:

1. **Consumer Groups** (may be implemented)
   - Shared offsets across consumers
   - Message distribution across group
   - Workaround: Build on DDL trait

2. **Compaction** (may be implemented)
   - Automatic cleanup of old entries
   - Key-based compaction
   - Workaround: Implement in application

3. **Monitoring Integration** (may be implemented)
   - Prometheus metrics endpoint
   - Health check endpoints
   - Workaround: Use callbacks

### Never Going to Happen

1. Expression evaluation built-in
2. Complex stream processing built-in
3. Built-in dashboard/UI
4. Built-in alerting
5. Full SQL-like query language
6. Multi-master replication
7. Built-in schema registry

These violate the KISS principle and belong in separate, specialized tools.

---

## Summary

DDL is intentionally simple with clear boundaries:

**Use DDL when**:
- You need reliable distributed logging
- You have 10K+ topics (ORNL Frontier scale)
- You want maximum performance with minimal complexity
- You're in the Rust ecosystem
- You can build additional features on the DDL trait

**Don't use DDL when**:
- You need complex querying (use Elasticsearch)
- You need exactly-once semantics (use Kafka transactions)
- You need built-in monitoring (use Prometheus+Grafana)
- Your team lacks Rust expertise (use Kafka or NATS)
- You need managed service (use cloud offerings)

**Honest assessment**:
- DDL trades features for simplicity and performance
- DDL requires building some features on top
- DDL is not a silver bullet
- DDL is a solid foundation for specific use cases

Choose wisely based on your actual requirements, not theoretical capabilities.