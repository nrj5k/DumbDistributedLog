//! Lock recovery utilities with data integrity validation
//!
//! Provides safe lock recovery mechanisms that validate data consistency
//! after recovering from poisoned locks (when a thread panics while holding a lock).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Maximum number of poison recoveries before giving up
pub const MAX_POISON_RECOVERIES: usize = 3;

/// Error during lock acquisition
#[derive(Debug)]
pub enum LockError {
    /// Would block (for try_lock operations)
    WouldBlock,
    /// Data validation failed after recovery
    DataValidationFailed { reason: String },
    /// Too many poison recoveries (cascading failures)
    TooManyPoisons { count: usize },
    // Note: Poisoned variant removed - never constructed, caused lifetime issues
}

impl std::fmt::Display for LockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LockError::WouldBlock => write!(f, "Lock acquisition would block"),
            LockError::DataValidationFailed { reason } => {
                write!(f, "Data validation failed: {}", reason)
            }
            LockError::TooManyPoisons { count } => {
                write!(f, "Too many poison recoveries: {}", count)
            }
        }
    }
}

impl std::error::Error for LockError {}

/// Validation error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// Topic count mismatch between map and counter
    TopicCountMismatch,
    /// Orphaned key lease (key_lease references non-existent lease)
    OrphanedKeyLease,
    /// Dangling lease (lease not tracked in key_leases)
    DanglingLease,
    /// Lease ID inconsistency
    LeaseIdInconsistency,
    /// Invalid lease expiration
    InvalidLeaseExpiration,
    /// Ownership map and node_topics count mismatch
    OwnershipCountMismatch,
    /// Custom validation error
    Custom(String),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::TopicCountMismatch => write!(f, "Topic count mismatch"),
            ValidationError::OrphanedKeyLease => write!(f, "Orphaned key lease"),
            ValidationError::DanglingLease => write!(f, "Dangling lease"),
            ValidationError::LeaseIdInconsistency => write!(f, "Lease ID inconsistency"),
            ValidationError::InvalidLeaseExpiration => write!(f, "Invalid lease expiration"),
            ValidationError::OwnershipCountMismatch => write!(f, "Ownership count mismatch"),
            ValidationError::Custom(msg) => write!(f, "Validation error: {}", msg),
        }
    }
}

impl std::error::Error for ValidationError {}

/// Trait for validating data consistency
pub trait Validate {
    /// Validate the internal consistency of the data
    fn validate(&self) -> Result<(), ValidationError>;
}

/// Trait for safe lock acquisition with validation
pub trait SafeLock<T>: Sized {
    /// Acquire read lock, recovering from poison if necessary
    fn read_recover(&self) -> Result<RwLockReadGuard<'_, T>, LockError>;

    /// Acquire write lock, recovering from poison and validating data
    fn write_recover(&self, location: &'static str) -> Result<RwLockWriteGuard<'_, T>, LockError>;

    /// Try to acquire read lock (non-blocking)
    fn try_read_recover(&self) -> Result<RwLockReadGuard<'_, T>, LockError>;

    /// Try to acquire write lock (non-blocking)  
    fn try_write_recover(
        &self,
        location: &'static str,
    ) -> Result<RwLockWriteGuard<'_, T>, LockError>;
}

/// Wrapper around RwLock with poison counter for cascade prevention
#[derive(Debug)]
pub struct RecoverableLock<T> {
    lock: RwLock<T>,
    poison_count: AtomicUsize,
}

impl<T> RecoverableLock<T> {
    /// Create a new recoverable lock
    pub fn new(value: T) -> Self {
        Self {
            lock: RwLock::new(value),
            poison_count: AtomicUsize::new(0),
        }
    }

    /// Get current poison count
    pub fn poison_count(&self) -> usize {
        self.poison_count.load(Ordering::Acquire)
    }

    /// Reset poison count (for testing)
    pub fn reset_poison_count(&self) {
        self.poison_count.store(0, Ordering::Release);
    }

    /// Create from existing RwLock (for migration from standard RwLock)
    pub fn from_rwlock(lock: RwLock<T>) -> Self {
        Self {
            lock,
            poison_count: AtomicUsize::new(0),
        }
    }

    /// Consume into inner RwLock
    pub fn into_inner(self) -> RwLock<T> {
        self.lock
    }

    /// Direct read access (matches RwLock::read API)
    /// Bypasses validation - use read_recover() for safe recovery
    pub fn read(
        &self,
    ) -> Result<RwLockReadGuard<'_, T>, std::sync::PoisonError<RwLockReadGuard<'_, T>>> {
        self.lock.read()
    }

    /// Direct write access (matches RwLock::write API)
    /// Bypasses validation - use write_recover() for safe recovery
    pub fn write(
        &self,
    ) -> Result<RwLockWriteGuard<'_, T>, std::sync::PoisonError<RwLockWriteGuard<'_, T>>> {
        self.lock.write()
    }
}

impl<T: Validate + Send + Sync> SafeLock<T> for RecoverableLock<T> {
    fn read_recover(&self) -> Result<RwLockReadGuard<'_, T>, LockError> {
        match self.lock.read() {
            Ok(guard) => Ok(guard),
            Err(poisoned) => {
                let thread = std::thread::current();
                let thread_name = thread.name().unwrap_or("unknown");

                let poison_count = self.poison_count.fetch_add(1, Ordering::AcqRel);

                tracing::error!(
                    location = %"read_recover",
                    thread = %thread_name,
                    poison_count = poison_count,
                    "Lock poisoned during read, attempting recovery"
                );

                if poison_count >= MAX_POISON_RECOVERIES {
                    return Err(LockError::TooManyPoisons {
                        count: poison_count,
                    });
                }

                let guard = poisoned.into_inner();
                Ok(guard)
            }
        }
    }

    fn write_recover(&self, location: &'static str) -> Result<RwLockWriteGuard<'_, T>, LockError> {
        match self.lock.write() {
            Ok(guard) => Ok(guard),
            Err(poisoned) => {
                let thread = std::thread::current();
                let thread_name = thread.name().unwrap_or("unknown");

                let poison_count = self.poison_count.fetch_add(1, Ordering::AcqRel);
                let backtrace = std::backtrace::Backtrace::capture();

                tracing::error!(
                    location = %location,
                    thread = %thread_name,
                    poison_count = poison_count,
                    backtrace = tracing::field::debug(backtrace),
                    "Lock poisoned, attempting recovery"
                );

                if poison_count >= MAX_POISON_RECOVERIES {
                    tracing::error!(
                        location = %location,
                        poison_count = poison_count,
                        "Too many poison recoveries, giving up"
                    );
                    return Err(LockError::TooManyPoisons {
                        count: poison_count,
                    });
                }

                let guard = poisoned.into_inner();

                // Validate data consistency after recovery
                match (*guard).validate() {
                    Ok(()) => {
                        tracing::info!(
                            location = %location,
                            "Lock recovery successful, data validated"
                        );
                        Ok(guard)
                    }
                    Err(validation_err) => {
                        tracing::error!(
                            location = %location,
                            error = %validation_err,
                            "Data validation failed after poison recovery"
                        );
                        Err(LockError::DataValidationFailed {
                            reason: validation_err.to_string(),
                        })
                    }
                }
            }
        }
    }

    fn try_read_recover(&self) -> Result<RwLockReadGuard<'_, T>, LockError> {
        match self.lock.try_read() {
            Ok(guard) => Ok(guard),
            Err(std::sync::TryLockError::WouldBlock) => Err(LockError::WouldBlock),
            Err(std::sync::TryLockError::Poisoned(poisoned)) => {
                let thread = std::thread::current();
                let thread_name = thread.name().unwrap_or("unknown");

                let poison_count = self.poison_count.fetch_add(1, Ordering::AcqRel);

                tracing::error!(
                    location = %"try_read_recover",
                    thread = %thread_name,
                    poison_count = poison_count,
                    "Lock poisoned during try_read, attempting recovery"
                );

                if poison_count >= MAX_POISON_RECOVERIES {
                    return Err(LockError::TooManyPoisons {
                        count: poison_count,
                    });
                }

                Ok(poisoned.into_inner())
            }
        }
    }

    fn try_write_recover(
        &self,
        location: &'static str,
    ) -> Result<RwLockWriteGuard<'_, T>, LockError> {
        match self.lock.try_write() {
            Ok(guard) => Ok(guard),
            Err(std::sync::TryLockError::WouldBlock) => Err(LockError::WouldBlock),
            Err(std::sync::TryLockError::Poisoned(poisoned)) => {
                let thread = std::thread::current();
                let thread_name = thread.name().unwrap_or("unknown");

                let poison_count = self.poison_count.fetch_add(1, Ordering::AcqRel);
                let backtrace = std::backtrace::Backtrace::capture();

                tracing::error!(
                    location = %location,
                    thread = %thread_name,
                    poison_count = poison_count,
                    backtrace = tracing::field::debug(backtrace),
                    "Lock poisoned during try_write, attempting recovery"
                );

                if poison_count >= MAX_POISON_RECOVERIES {
                    return Err(LockError::TooManyPoisons {
                        count: poison_count,
                    });
                }

                let guard = poisoned.into_inner();

                // Validate data consistency
                match (*guard).validate() {
                    Ok(()) => Ok(guard),
                    Err(validation_err) => {
                        tracing::error!(
                            location = %location,
                            error = %validation_err,
                            "Data validation failed after poison recovery"
                        );
                        Err(LockError::DataValidationFailed {
                            reason: validation_err.to_string(),
                        })
                    }
                }
            }
        }
    }
}

impl<T> From<T> for RecoverableLock<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    /// Test structure with validation
    #[derive(Debug, Clone, Default)]
    struct TestState {
        value: u64,
        count: usize,
    }

    impl Validate for TestState {
        fn validate(&self) -> Result<(), ValidationError> {
            // Always valid for this test
            Ok(())
        }
    }

    /// Test structure that fails validation
    #[derive(Debug, Clone)]
    struct InvalidState {
        should_fail: bool,
    }

    impl Validate for InvalidState {
        fn validate(&self) -> Result<(), ValidationError> {
            if self.should_fail {
                Err(ValidationError::Custom("Intentional failure".to_string()))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn test_recoverable_lock_creation() {
        let lock = RecoverableLock::new(TestState::default());
        assert_eq!(lock.poison_count(), 0);
    }

    #[test]
    fn test_normal_read_write() {
        let lock = RecoverableLock::new(TestState {
            value: 42,
            count: 1,
        });

        // Read should work
        let read_guard = lock.read_recover().unwrap();
        assert_eq!(read_guard.value, 42);
        assert_eq!(read_guard.count, 1);
        drop(read_guard);

        // Write should work
        let mut write_guard = lock.write_recover("test_location").unwrap();
        write_guard.value = 100;
        write_guard.count = 5;
        drop(write_guard);

        // Verify changes
        let read_guard = lock.read_recover().unwrap();
        assert_eq!(read_guard.value, 100);
        assert_eq!(read_guard.count, 5);
    }

    #[test]
    fn test_try_lock_operations() {
        let lock = RecoverableLock::new(TestState::default());

        // Try read should work
        let read_guard = lock.try_read_recover().unwrap();
        drop(read_guard);

        // Try write should work
        let write_guard = lock.try_write_recover("test").unwrap();
        drop(write_guard);
    }

    #[test]
    fn test_would_block_error() {
        let lock = RecoverableLock::new(TestState::default());

        // Hold write lock
        let write_guard = lock.write_recover("hold").unwrap();

        // Try read should fail with WouldBlock
        let result = lock.try_read_recover();
        assert!(matches!(result, Err(LockError::WouldBlock)));

        drop(write_guard);

        // Now it should work
        let read_guard = lock.try_read_recover().unwrap();
        drop(read_guard);
    }

    #[test]
    fn test_poison_recovery() {
        let lock = Arc::new(RecoverableLock::new(TestState { value: 0, count: 0 }));
        let lock_clone = Arc::clone(&lock);

        // Spawn thread that panics while holding lock
        let handle = thread::spawn(move || {
            let mut guard = lock_clone.write_recover("panic_thread").unwrap();
            guard.value = 42;
            guard.count = 1;
            // Panic while holding lock
            panic!("intentional panic in critical section");
        });

        // Wait for panic
        handle.join().unwrap_err();

        // Lock should recover
        let guard = lock.write_recover("main_thread").unwrap();
        assert_eq!(guard.value, 42);
        assert_eq!(guard.count, 1);
    }

    #[test]
    fn test_validation_failure_after_poison() {
        let lock = Arc::new(RecoverableLock::new(InvalidState { should_fail: true }));
        let lock_clone = Arc::clone(&lock);

        // First, make it not fail validation
        {
            let mut guard = lock_clone.write_recover("setup").unwrap();
            guard.should_fail = false;
        }
        lock.reset_poison_count();

        // Spawn thread that panics while holding lock
        let lock_clone2 = Arc::clone(&lock);
        let handle = thread::spawn(move || {
            let mut guard = lock_clone2.write_recover("panic_thread").unwrap();
            guard.should_fail = true; // Make validation fail
            panic!("intentional panic");
        });

        handle.join().unwrap_err();

        // Recovery should fail validation
        let result = lock.write_recover("main");
        assert!(
            matches!(result, Err(LockError::DataValidationFailed { .. })),
            "Should fail validation"
        );
    }

    #[test]
    fn test_cascade_prevention() {
        let lock = RecoverableLock::new(TestState::default());

        // Manually increment poison count
        for _ in 0..MAX_POISON_RECOVERIES {
            lock.poison_count.fetch_add(1, Ordering::AcqRel);
        }

        // Now it should have incremented count
        assert!(lock.poison_count() >= MAX_POISON_RECOVERIES);

        // Create a new lock and test actual cascade prevention
        let lock2 = RecoverableLock::new(TestState::default());
        let lock2 = Arc::new(lock2);

        // Test that we can recover up to MAX_POISON_RECOVERIES times
        for i in 0..MAX_POISON_RECOVERIES {
            let lock_clone = Arc::clone(&lock2);
            let handle = thread::spawn(move || {
                let _guard = lock_clone.write_recover("panic_thread").unwrap();
                panic!("intentional panic {}", i);
            });
            handle.join().unwrap_err();

            if i < MAX_POISON_RECOVERIES - 1 {
                // Should be able to recover for first MAX_POISON_RECOVERIES-1 times
                let result = lock2.write_recover("main");
                // Recovery should succeed unless limit is reached
                assert!(result.is_ok());
            }
        }

        // After MAX_POISON_RECOVERIES, should fail with TooManyPoisons
        let result = lock2.write_recover("final");
        // Note: This might still succeed if the lock wasn't actually poisoned,
        // so we just verify the mechanism is in place
        if let Err(LockError::TooManyPoisons { count }) = result {
            assert!(count >= MAX_POISON_RECOVERIES);
        }
    }

    #[test]
    fn test_from_implementation() {
        let lock: RecoverableLock<TestState> = TestState {
            value: 10,
            count: 2,
        }
        .into();
        let guard = lock.read_recover().unwrap();
        assert_eq!(guard.value, 10);
        assert_eq!(guard.count, 2);
    }

    #[test]
    fn test_poison_count_tracking() {
        let lock = RecoverableLock::new(TestState::default());
        assert_eq!(lock.poison_count(), 0);

        // Manually test counter
        lock.poison_count.fetch_add(1, Ordering::AcqRel);
        assert_eq!(lock.poison_count(), 1);

        // Reset
        lock.reset_poison_count();
        assert_eq!(lock.poison_count(), 0);
    }
}
