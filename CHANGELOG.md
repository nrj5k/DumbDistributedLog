# Changelog

All notable changes to this project will be documented in this file.

## [1.1.0] - 2025-03-30

### Added
- **Membership API on DdlDistributed** - Three new methods:
  - `subscribe_membership() -> Option<broadcast::Receiver<MembershipEvent>>`
  - `membership() -> Option<MembershipView>`
  - `metrics() -> Option<DdlMetrics>`
- **Root-level exports for SCORE integration**:
  - `MembershipEvent`
  - `MembershipEventType`
  - `MembershipView`
  - `DdlMetrics`
  - `NodeInfo`
  - `RaftClusterNode`

### Fixed
- Fixed 6 failing Raft storage tests (snapshot metadata, persistence, purge tracking)
- Fixed unused `local_node_id` field in FailureDetector

### Documentation
- Added SCORE integration guide to README
- Enhanced doc comments for membership types
- Documented Option vs Result design decision

## [1.0.0] - 2024-01-XX

### Added
- Raft-based topic ownership with strong consistency
- TCP networking for multi-node communication
- Membership events for node lifecycle tracking
- Lease-based topic ownership with TTL
- Failure detection and automatic failover
- Gossip-based node discovery
- File persistence with atomic writes
- Snapshot serialization for log compaction

### Architecture
- Consolidated DDL implementations into single `DdlDistributed`
- Standalone and distributed modes
- Topic queue with lock-free SPMC design
- Arc<RwLock<>> for thread-safe ownership state

### Performance
- Push latency: < 10µs (in-memory)
- Subscribe latency: < 5µs
- Topic operations: O(1) with DashMap
- Atomic topic counting to avoid DashMap::len() O(n)

### Breaking Changes
- `InMemoryDdl` removed - use `DdlDistributed::new_standalone()`
- `DdlConfig` structure changed significantly
- `DDL` trait methods are now async