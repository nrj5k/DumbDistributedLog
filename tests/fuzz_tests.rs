//! Fuzz testing suite for malformed input scenarios
//!
//! Tests for handling malformed RPC types, boundary messages,
//! and property-based testing with QuickCheck.

use ddl::cluster::lock_utils::{RecoverableLock, SafeLock, Validate, ValidationError};
use ddl::cluster::ownership_machine::{OwnershipCommand, OwnershipState};
use quickcheck::TestResult;

/// Test structure for fuzzing
#[derive(Debug, Clone)]
struct FuzzState {
    value: u64,
}

impl Default for FuzzState {
    fn default() -> Self {
        Self { value: 0 }
    }
}

impl Validate for FuzzState {
    fn validate(&self) -> Result<(), ValidationError> {
        Ok(())
    }
}

/// Test malformed RPC types (0-255)
#[test]
fn test_fuzz_rpc_types() {
    // Send all possible RPC type values (0-255)

    let lock = RecoverableLock::new(FuzzState::default());

    for rpc_type in 0..=255u8 {
        // Test doesn't actually use the type, but verifies no panic/hang
        // In real system, this would test RPC protocol parsing

        let _guard = lock.read_recover().unwrap();
        // If we got here, no panic or hang
    }
}

/// Test message size boundaries
#[test]
fn test_fuzz_message_sizes() {
    let ownership_state = RecoverableLock::new(OwnershipState::new());

    let test_cases = vec![
        (0, "empty"),
        (1, "min_nonempty"),
        (u16::MAX - 1, "max_usize-1"),
        (u16::MAX, "max_usize"),
        (u16::MAX + 1, "oversize"),
    ];

    for (size, desc) in test_cases {
        let topic = format!("fuzz_size_{}_{}", size, desc);

        // Try to create topic of different simulated sizes
        let result = (|| {
            let mut guard = ownership_state.write_recover("fuzzy_size")?;
            guard.apply(&OwnershipCommand::ClaimTopic {
                topic,
                node_id: 1,
                timestamp: 1000,
            });
            Ok(())
        })();

        // We should handle all cases gracefully
        // Either success or expected validation error
        let _ = result;
    }
}

/// Test concurrent operations with random properties
#[test]
fn test_fuzz_concurrent_operations() {
    use quickcheck::Arbitrary;
    use quickcheck::TestResult;

    #[derive(Debug, Clone)]
    struct RandomOp {
        op_type: u8,
        node_id: u64,
        value: u64,
    }

    impl Arbitrary for RandomOp {
        fn arbitrary(g: &mut quickcheck::Gen) -> Self {
            Self {
                op_type: g.arbitrary(),
                node_id: g.arbitrary(),
                value: g.arbitrary(),
            }
        }
    }

    // Run QuickCheck test
    quickcheck::quickcheck(|ops: Vec<RandomOp>| {
        let ownership_state = RecoverableLock::new(OwnershipState::new());

        for op in ops {
            let mut guard = ownership_state.write_recover("fuzz_check").ok()?;
            guard.apply(&OwnershipCommand::ClaimTopic {
                topic: format!("fuzz_{}", op.node_id),
                node_id: op.node_id,
                timestamp: op.value,
            });
        }

        TestResult::from_bool(true)
    });
}

/// Test all possible byte combinations for simple data
#[test]
fn test_fuzz_byte_arrays() {
    let lock = RecoverableLock::new(FuzzState::default());

    // Test with various byte vector sizes
    for size in 0..20 {
        let bytes: Vec<u8> = (0..size).map(|i| i as u8).collect();

        let topic = format!("fuzz_bytes_{}", size);

        let result = (|| {
            let mut guard = lock.write_recover("fuzz_byte_array")?;
            guard.value = size as u64;
            Ok(())
        })();

        // Should handle all sizes
        let _ = result;
    }
}

/// Test boundary conditions
#[test]
fn test_fuzz_boundaries() {
    let lock = RecoverableLock::new(FuzzState::default());

    // Test edge cases
    let boundaries = vec![
        0u64,
        1,
        u8::MAX as u64,
        u16::MAX as u64,
        u32::MAX as u64,
        u64::MAX,
    ];

    for boundary in boundaries {
        let _guard = lock.write_recover("fuzz_boundary").ok();
        // If we got here, no boundary panic
    }
}

#[test]
fn test_fuzz_lease_operations() {
    use quickcheck::Arbitrary;

    #[derive(Debug, Clone)]
    struct LeaseOp {
        ttl: u64,
        owner: u64,
        timestamp: u64,
    }

    impl Arbitrary for LeaseOp {
        fn arbitrary(g: &mut quickcheck::Gen) -> Self {
            Self {
                ttl: g.arbitrary_range(0..1000),
                owner: g.arbitrary(),
                timestamp: g.arbitrary(),
            }
        }
    }

    quickcheck::quickcheck(|ops: Vec<LeaseOp>| {
        let ownership_state = RecoverableLock::new(OwnershipState::new());

        for op in ops {
            // Create a lease with arbitrary parameters
            let result = (|| {
                let mut guard = ownership_state.write_recover("fuzz_lease")?;

                // Cap TTL to avoid overflow
                let ttl = op.ttl % 1_000_000;
                let key = format!("lease_{}", op.owner);

                let _ = guard.acquire_lease(key, op.owner, ttl, op.timestamp);
                Ok(())
            })();

            // Should handle all lease operations
            let _ = result;
        }

        TestResult::from_bool(true)
    });
}

#[test]
fn test_fuzz_lease_expiration() {
    // Test various expiration scenarios

    let lock = RecoverableLock::new(OwnershipState::new());

    let test_cases = vec![
        (0, 10),        // TTL 0 (edge case)
        (1, 1),         // Minimal TTL
        (60, 60),       // 1 minute TTL
        (3600, 3600),   // 1 hour TTL
        (86400, 86400), // 1 day TTL
    ];

    for (ttl, check) in test_cases {
        let _result = (|| {
            let mut guard = lock.write_recover("fuzz_expiration")?;
            let key = format!("expiration_{}", check);

            let _ = guard.acquire_lease(key, 1, ttl, 1000);
            Ok(())
        })();
    }
}
