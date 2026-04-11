use thiserror::Error;

/// Error types for peer request/response operations.
#[derive(Debug, Error)]
pub enum PeerError {
    #[error("request timed out after {duration_ms}ms")]
    Timeout { duration_ms: u64 },
    #[error("peer {peer_id} is unavailable")]
    PeerUnavailable { peer_id: u64 },
    #[error("request {request_id} was cancelled")]
    RequestCancelled { request_id: String },
    #[error("circuit breaker is open for peer {peer_id}")]
    CircuitOpen { peer_id: u64 },
    #[error("cache miss for key {key}")]
    CacheMiss { key: String },
    #[error("codec error: {message}")]
    CodecError { message: String },
    #[error("routing error: {message}")]
    RoutingError { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_all_variants() {
        assert_eq!(
            format!("{}", PeerError::Timeout { duration_ms: 5000 }),
            "request timed out after 5000ms"
        );
        assert_eq!(
            format!("{}", PeerError::PeerUnavailable { peer_id: 42 }),
            "peer 42 is unavailable"
        );
        assert_eq!(
            format!(
                "{}",
                PeerError::RequestCancelled {
                    request_id: "req-123".into()
                }
            ),
            "request req-123 was cancelled"
        );
        assert_eq!(
            format!("{}", PeerError::CircuitOpen { peer_id: 7 }),
            "circuit breaker is open for peer 7"
        );
        assert_eq!(
            format!(
                "{}",
                PeerError::CacheMiss {
                    key: "user:123".into()
                }
            ),
            "cache miss for key user:123"
        );
        assert_eq!(
            format!(
                "{}",
                PeerError::CodecError {
                    message: "invalid".into()
                }
            ),
            "codec error: invalid"
        );
        assert_eq!(
            format!(
                "{}",
                PeerError::RoutingError {
                    message: "no route".into()
                }
            ),
            "routing error: no route"
        );
    }
}
