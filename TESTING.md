# Comprehensive Testing Suite

This document describes the comprehensive testing suite for AutoQueues critical fixes.

## Test Categories

### 1. Chaos Testing (`tests/chaos_tests.rs`)

Tests for random failures, lock poison cascades, and concurrent recovery scenarios using thread panics and chaos engineering.

**Key Tests:**
- `test_random_panic_during_operation` - Simulates panic during critical section
- `test_lock_poison_cascade` - Tests panic recovery chain (A panics → B recovers → B panics → C recovers)
- `test_concurrent_poison_scenarios` - Multiple threads panicking simultaneously
- `test_ownership_state_poison_recovery` - Tests actual OwnershipState poison recovery
- `test_data_integrity_after_repeated_recovery` - Verifies data integrity after multiple panics

**Run:**
```bash
cargo test --test chaos_tests -- --include-ignored
```

**CI/CD:** Run weekly and on push to main/develop branches.

### 2. Stress Testing (`tests/stress_tests.rs`)

Tests for high-concurrency workloads, sustained operations, and resource exhaustion.

**Key Tests:**
- `test_high_concurrent_topic_creation` - 100 threads creating topics concurrently
- `test_sustained_operations_60_seconds` - 60-second runtime test with metrics
- `test_topic_limit_under_stress` - Verifies limits under high concurrency
- `test_production_load_10k_connections` - 10k connections with 10 ops each
- `test_rapid_lock_recovery` - Rapid lock acquisition/recovery under stress
- `test_memory_stress_under_load` - Memory usage under sustained load
- `test_lock_contention_under_stress` - Behavior under high lock contention

**Run:**
```bash
cargo test --test stress_tests -- --ignored
```

**CI/CD:** Run in stress-testing job with 45-minute timeout.

### 3. Fuzz Testing (`tests/fuzz_tests.rs`)

Tests for malformed input handling, boundary conditions, and property-based testing.

**Key Tests:**
- `test_fuzz_rpc_types` - All RPC types 0-255
- `test_fuzz_message_sizes` - Boundary message sizes (0, 1, MAX-1, MAX, MAX+1)
- `test_fuzz_concurrent_operations` - QuickCheck property-based testing
- `test_fuzz_byte_arrays` - Various byte vector sizes
- `test_fuzz_boundaries` - Edge case boundaries
- `test_fuzz_lease_operations` - Lease operations with arbitrary parameters
- `test_fuzz_lease_expiration` - Various expiration scenarios

**Run:**
```bash
cargo test --test fuzz_tests
```

**CI/CD:** Run in fuzz-testing job with 15-minute timeout.

### 4. Integration Tests (`tests/integration_tests.rs`)

End-to-end tests for cluster scenarios, network partitions, and interoperability.

**Key Tests:**
- `test_cluster_partition_recovery` - 5-node cluster with partition and merge
- `test_snapshot_transfer_size_limit` - Oversized snapshot graceful error handling
- `test_message_size_during_rolling_upgrade` - Node A 16MB, Node B 10MB interoperability
- `test_raft_consensus_with_majority` - 5-node cluster consistency
- `test_snapshot_transfer_graceful_error` - Error handling for oversized transfer
- `test_multiple_partitions_convergence` - After multiple partition events
- `test_snapshot_recovery_from_backup` - Backup and recovery scenario
- `test_cluster_reintegration` - Split-brain reintegration

**Run:**
```bash
cargo test --test integration_tests
```

**CI/CD:** Run in integration-tests job with 60-minute timeout.

### 5. Lock Recovery Tests (`tests/lock_recovery_tests.rs`)

Tests for poison handling, data integrity verification, and logging.

**Key Tests:**
- `test_data_integrity_after_panic` - Panic during critical section recovery
- `test_poison_count_prevents_cascade` - MAX_POISON_RECOVERIES limit verification
- `test_lock_recovery_logging` - Context logged during recovery
- `test_multiple_recovery_attempts` - System handles multiple recovery attempts
- `test_data_validation_after_recovery` - Validation catches inconsistencies
- `test_recovery_with_actual_state_changes` - State changes persist after recovery
- `test_concurrent_recovery_thread_safety` - Thread-safe recovery
- `test_recovery_state_consistency` - Consistency through multiple recoveries
- `test_ownership_state_recovery` - Actual OwnershipState with panic recovery
- `test_lock_recovery_without_panic` - Normal operation verification
- `test_backtrace_captured_during_poison` - Backtrace is available
- `test_large_state_recovery` - Recovery with large ownership state
- `test_lease_recovery` - Lease state recovery after panic

**Run:**
```bash
cargo test --test lock_recovery_tests
```

**CI/CD:** Run in lock-recovery-tests job with 30-minute timeout.

### 6. Performance Benchmarks (`benches/critical_fixes_bench.rs`)

Benchmarks for performance regression detection.

**Key Benchmarks:**
- `bench_lock_recovery_happy_path` - Overhead when lock NOT poisoned
- `bench_lock_recovery_poisoned` - Recovery when lock IS poisoned
- `bench_lock_read_vs_write` - Read vs write operation comparison
- `bench_ownership_state_operation` - OwnershipState command processing
- `bench_lock_recovery_overhead` - Recovery overhead measurements
- `bench_memory_ordering_comparison` - Ordering::Relaxed vs Ordering::Acquire
- `bench_poison_validation_overhead` - Validation overhead after recovery
- `bench_concurrent_lock_operations` - Concurrent operations stress
- `bench_state_serialization_overhead` - Serialization time measurement
- `bench_ownership_query_operations` - Query performance metrics

**Run:**
```bash
cargo bench --bench critical_fixes_bench
```

**CI/CD:** Run in performance-benchmarks job with nightly toolchain.

## CI/CD Configuration

### GitHub Actions Workflow (`.github/workflows/chaos-tests.yml`)

**Triggers:**
- Push to main/develop branches
- Pull requests to main/develop
- Weekly schedule (Sunday at midnight)
- Manual workflow_dispatch

**Jobs:**
1. `chaos-testing` - Runs chaos tests (30 min timeout)
2. `stress-testing` - Runs stress tests (45 min timeout)
3. `lock-recovery-tests` - Lock recovery verification (15 min timeout)
4. `fuzz-tests` - Property-based testing (15 min timeout)
5. `integration-tests` - End-to-end scenarios (30 min timeout)
6. `performance-benchmarks` - Performance measurements (20 min timeout)
7. `nightly-chaos-schedule` - Weekly comprehensive chaos tests (60 min timeout)

**Environments:**
- **Chaos Testing:** Single Ubuntu runner, RUST_BACKTRACE=1, RUST_LOG=debug
- **Stress Testing:** 8 threads, 45-minute timeout for sustained tests
- **Lock Recovery:** Standard Rust stable toolchain
- **Fuzz Testing:** Uses quickcheck property-based testing
- **Integration:** 5-node cluster simulation, network partition tests
- **Benchmarks:** Nightly toolchain with cargo-bench-nightly component

## Running Tests

### Quick Tests (Fast Feedback)
```bash
cargo test --test chaos_tests --test lock_recovery_tests --test fuzz_tests -- --test-threads=4
```

### Full Test Suite
```bash
cargo test --test chaos_tests --test stress_tests --test lock_recovery_tests --test fuzz_tests --test integration_tests
```

### With Coverage
```bash
cargo test --test chaos_tests --test lock_recovery_tests -- --include-ignored
cargo test --test stress_tests -- --ignored
```

### Performance Benchmarks
```bash
cargo bench --bench critical_fixes_bench
```

## Test Philosophy

### What Makes These Tests Effective:

1. **Chaos Testing:** Proves system survives unexpected failures (poisoned locks, panics)
2. **Stress Testing:** Validates performance under production-like load
3. **Fuzz Testing:** Finds edge cases and input validation bugs
4. **Integration Tests:** Verifies end-to-end behavior and cluster coordination
5. **Lock Recovery Tests:** Ensures poison recovery preserves data integrity
6. **Benchmarks:** Detects performance regressions before they reach production

### Test Quality Checklist:
- [ ] Tests verify actual behavior, not implementation details
- [ ] Tests would fail if the fix is broken
- [ ] Tests are independent (can run in any order)
- [ ] Tests clean up resources
- [ ] Assertions are specific (not just `is_ok()` or `is_err()`)
- [ ] Error cases tested with specific error types
- [ ] Edge cases and boundaries covered
- [ ] Tests serve as living documentation

## Troubleshooting

### Common Issues:

1. **Test Panics**: Tests intentionally panic → Look for panic messages in output
2. **Timeouts**: Adjust `timeout-minutes` in CI config or use `--test-threads=N`
3. **Ignored Tests**: Run with `-- --ignored` flag
4. **Flaky Tests**: Increase timing margins or use `thread::sleep` before panics

### Debugging:

```bash
# Enable backtrace
RUST_BACKTRACE=1 cargo test --test chaos_tests

# Enable verbose logging
RUST_LOG=debug cargo test --test lock_recovery_tests

# Run specific test
cargo test --test chaos_tests test_lock_poison_cascade -- --nocapture
```

## File Locations

```
autoqueues/
  tests/
    chaos_tests.rs              - Chaos & panic scenarios
    stress_tests.rs             - Production load tests
    fuzz_tests.rs               - Malformed input + QuickCheck
    integration_tests.rs        - Cluster & network scenarios
    lock_recovery_tests.rs      - Poison recovery & validation
  benches/
    critical_fixes_bench.rs     - Performance benchmarks
  .github/workflows/
    chaos-tests.yml             - CI/CD configuration
```

## Maintenance Notes

### Updating Test Files:
1. Add new test functions with descriptive names
2. Include relevant `#[test]` attributes
3. Use `#[ignore]` for tests requiring manual execution
4. Add CI/CD job for new test types if needed

### Adding New Test Categories:
1. Create test file in `tests/`
2. Add to Cargo.toml dev-dependencies if needed
3. Create CI/CD job in `.github/workflows/chaos-tests.yml`
4. Add documentation to this file

### CI/CD Maintenance:
- Monitor test duration and adjust timeouts
- Watch for flaky tests (add `#[ignore]` if needed)
- Update dependencies in Cargo.toml
- Keep benchmark baselines updated
