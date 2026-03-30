# Changelog

All notable changes to this project will be documented in this file.

## [1.2.0] - 2025-03-30

### Added - Lease API (Phase 2 for SCORE Integration)

#### New Types
- `LeaseEntry` - Full lease entry stored in state machine
- `LeaseInfo` - Public lease information (key, owner, acquired_at, expires_at, ttl_secs)
- `LeaseError` - Error enum for lease operations (LeaseHeld, LeaseExpired, LeaseNotFound)

#### New Commands
- `OwnershipCommand::AcquireLease` - Acquire a lease with TTL
- `OwnershipCommand::RenewLease` - Renew an existing lease
- `OwnershipCommand::ReleaseLease` - Release a lease gracefully
- `OwnershipCommand::ExpireLeases` - Batch expire stale leases

#### New Methods on DdlDistributed
- `acquire_lease(key, ttl)` - Acquire a lease with TTL (async)
- `renew_lease(lease_id)` - Renew an existing lease (async)
- `release_lease(key)` - Release a lease gracefully (async)
- `get_lease_owner(key)` - Get current lease owner (async, linearizable)
- `list_leases(owner)` - List all leases owned by a node (async)

#### New Methods on RaftClusterNode
- `acquire_lease(key, ttl_secs)` - Acquire lease via Raft
- `renew_lease(lease_id)` - Renew lease via Raft
- `release_lease(key)` - Release lease via Raft
- `get_lease_owner(key)` - Query lease owner
- `list_leases(owner)` - List leases for node
- `expire_leases()` - Batch expiration
- `start_lease_expiration(interval_secs)` - Background task

#### New Error Variants on DdlError
- `LeaseHeld { key, owner }` - Key already leased by another node
- `LeaseExpired(lease_id)` - Lease has expired
- `LeaseNotFound(lease_id)` - Lease ID not found

#### Key Behaviors
- One lease per key (exclusive ownership)
- Automatic expiration via background task
- Raft consensus guarantees exactly one lease succeeds for concurrent claims
- Linearizable reads for `get_lease_owner()`
- `acquire_lease()` on already-held key by same owner renews the lease

### Tests
- 18 new tests for lease operations (total: 288 tests)

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