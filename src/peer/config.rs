/// Configuration for peer request/response system in the DDL distributed system.
#[derive(Debug, Clone, PartialEq)]
pub struct PeerConfig {
    /// Timeout in seconds for peer requests before considering them failed.
    pub request_timeout_secs: u64,
    /// Maximum number of retry attempts for failed peer requests.
    pub max_retries: u32,
    /// Interval in seconds between heartbeat messages to detect peer availability.
    pub heartbeat_interval_secs: u64,
    /// Number of consecutive failures before circuit breaker opens.
    pub circuit_breaker_threshold: u32,
    /// Time in seconds before an open circuit breaker resets to closed.
    pub circuit_breaker_reset_secs: u64,
    /// Time-to-live in seconds for cached peer responses.
    pub cache_ttl_secs: u64,
    /// Maximum number of entries to store in the peer response cache.
    pub cache_max_entries: usize,
}

impl Default for PeerConfig {
    fn default() -> Self {
        Self {
            request_timeout_secs: 5,
            max_retries: 3,
            heartbeat_interval_secs: 10,
            circuit_breaker_threshold: 5,
            circuit_breaker_reset_secs: 30,
            cache_ttl_secs: 60,
            cache_max_entries: 1000,
        }
    }
}

impl PeerConfig {
    /// Creates a new PeerConfig with default values.
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_request_timeout() {
        assert_eq!(PeerConfig::default().request_timeout_secs, 5);
    }

    #[test]
    fn test_default_max_retries() {
        assert_eq!(PeerConfig::default().max_retries, 3);
    }

    #[test]
    fn test_default_heartbeat_interval() {
        assert_eq!(PeerConfig::default().heartbeat_interval_secs, 10);
    }

    #[test]
    fn test_default_circuit_breaker_threshold() {
        assert_eq!(PeerConfig::default().circuit_breaker_threshold, 5);
    }

    #[test]
    fn test_default_circuit_breaker_reset() {
        assert_eq!(PeerConfig::default().circuit_breaker_reset_secs, 30);
    }

    #[test]
    fn test_default_cache_ttl() {
        assert_eq!(PeerConfig::default().cache_ttl_secs, 60);
    }

    #[test]
    fn test_default_cache_max_entries() {
        assert_eq!(PeerConfig::default().cache_max_entries, 1000);
    }

    #[test]
    fn test_new_equals_default() {
        assert_eq!(PeerConfig::new(), PeerConfig::default());
    }
}
