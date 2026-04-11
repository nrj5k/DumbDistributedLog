use super::config::PeerConfig;
/// Circuit breaker for peer request resilience.
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
const CLOSED: u8 = 0;
const OPEN: u8 = 1;
const HALF_OPEN: u8 = 2;
pub struct CircuitBreaker {
    state: AtomicU8,
    failure_count: AtomicU32,
    last_failure: AtomicU64,
    threshold: u32,
    reset_timeout: Duration,
}
impl CircuitBreaker {
    pub fn new(config: &PeerConfig) -> Self {
        Self {
            state: AtomicU8::new(CLOSED),
            failure_count: AtomicU32::new(0),
            last_failure: AtomicU64::new(0),
            threshold: config.circuit_breaker_threshold,
            reset_timeout: Duration::from_secs(config.circuit_breaker_reset_secs),
        }
    }
    pub fn allow_request(&self) -> bool {
        match self.state.load(Ordering::SeqCst) {
            CLOSED => true,
            OPEN => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                if now >= self.last_failure.load(Ordering::SeqCst) + self.reset_timeout.as_secs() {
                    self.state.store(HALF_OPEN, Ordering::SeqCst);
                    true
                } else {
                    false
                }
            }
            HALF_OPEN => true,
            _ => unreachable!(),
        }
    }
    pub fn record_success(&self) {
        if self.state.load(Ordering::SeqCst) != OPEN {
            self.state.store(CLOSED, Ordering::SeqCst);
            self.failure_count.store(0, Ordering::SeqCst);
        }
    }
    pub fn record_failure(&self) {
        let count = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
        if count >= self.threshold {
            self.state.store(OPEN, Ordering::SeqCst);
            self.last_failure.store(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                Ordering::SeqCst,
            );
        }
    }
    pub fn state(&self) -> &'static str {
        match self.state.load(Ordering::SeqCst) {
            CLOSED => "closed",
            OPEN => "open",
            HALF_OPEN => "half_open",
            _ => unreachable!(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_success_stays_closed() {
        let cb = CircuitBreaker::new(&PeerConfig::default());
        assert_eq!(cb.state(), "closed");
        cb.record_success();
        assert_eq!(cb.state(), "closed");
        assert!(cb.allow_request());
    }
    #[test]
    fn test_failures_to_open_after_threshold() {
        let mut config = PeerConfig::default();
        config.circuit_breaker_threshold = 3;
        let cb = CircuitBreaker::new(&config);
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), "closed");
        cb.record_failure();
        assert_eq!(cb.state(), "open");
        assert!(!cb.allow_request());
    }
    #[test]
    fn test_open_to_half_open_after_timeout() {
        let mut config = PeerConfig::default();
        config.circuit_breaker_threshold = 1;
        config.circuit_breaker_reset_secs = 1;
        let cb = CircuitBreaker::new(&config);
        cb.record_failure();
        std::thread::sleep(Duration::from_secs(2));
        assert!(cb.allow_request());
        assert_eq!(cb.state(), "half_open");
    }
    #[test]
    fn test_half_open_success_to_closed() {
        let mut config = PeerConfig::default();
        config.circuit_breaker_threshold = 1;
        config.circuit_breaker_reset_secs = 1;
        let cb = CircuitBreaker::new(&config);
        cb.record_failure();
        std::thread::sleep(Duration::from_secs(2));
        cb.allow_request();
        cb.record_success();
        assert_eq!(cb.state(), "closed");
    }
}
