//! Pending request tracker for managing in-flight peer requests.

use super::{config::PeerConfig, response::PeerResponse, types::RequestId};
use dashmap::DashMap;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

/// Entry for a pending request with its response channel and expiry.
pub struct PendingEntry {
    pub request_id: RequestId,
    pub sender: oneshot::Sender<PeerResponse>,
    pub expires_at: Instant,
}

/// Thread-safe tracker for in-flight requests with timeout management.
pub struct PendingRequestTracker {
    pending: DashMap<RequestId, PendingEntry>,
    default_timeout: Duration,
}

impl PendingRequestTracker {
    /// Creates a new tracker with timeout from config.
    pub fn new(config: &PeerConfig) -> Self {
        Self {
            pending: DashMap::new(),
            default_timeout: Duration::from_secs(config.request_timeout_secs),
        }
    }

    /// Registers a request and returns the receiver for the response.
    pub fn register(&self, request_id: RequestId) -> oneshot::Receiver<PeerResponse> {
        let (sender, receiver) = oneshot::channel();
        let expires_at = Instant::now() + self.default_timeout;
        let entry = PendingEntry {
            request_id,
            sender,
            expires_at,
        };
        self.pending.insert(request_id, entry);
        receiver
    }

    /// Completes a request by sending the response and removing the entry.
    /// Returns true if the request was found and completed.
    pub fn complete(&self, request_id: RequestId, response: PeerResponse) -> bool {
        if let Some((_, entry)) = self.pending.remove(&request_id) {
            entry.sender.send(response).ok();
            true
        } else {
            false
        }
    }

    /// Cancels all expired entries and returns the count removed.
    pub fn cancel_expired(&self) -> usize {
        let now = Instant::now();
        let expired: Vec<_> = self
            .pending
            .iter()
            .filter(|e| e.value().expires_at <= now)
            .map(|e| *e.key())
            .collect();

        let count = expired.len();
        for request_id in expired {
            self.pending.remove(&request_id);
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_complete() {
        let config = PeerConfig::default();
        let tracker = PendingRequestTracker::new(&config);
        let request_id = RequestId::new(1);

        let receiver = tracker.register(request_id);
        let response = PeerResponse::success(request_id, 42, vec![1, 2, 3]);

        let completed = tracker.complete(request_id, response.clone());
        assert!(completed);

        let received = receiver.blocking_recv().unwrap();
        assert_eq!(received.request_id, request_id);
        assert_eq!(received.source_node, 42);
        assert!(received.success);
    }

    #[test]
    fn test_complete_unknown_request() {
        let config = PeerConfig::default();
        let tracker = PendingRequestTracker::new(&config);
        let request_id = RequestId::new(1);

        let response = PeerResponse::success(request_id, 42, vec![]);
        let completed = tracker.complete(request_id, response);
        assert!(!completed);
    }

    #[test]
    fn test_cancel_expired() {
        let mut config = PeerConfig::default();
        config.request_timeout_secs = 0;
        let tracker = PendingRequestTracker::new(&config);

        let id1 = RequestId::new(1);
        let id2 = RequestId::new(2);
        tracker.register(id1);
        tracker.register(id2);

        std::thread::sleep(std::time::Duration::from_millis(10));

        let count = tracker.cancel_expired();
        assert!(count >= 2);
    }
}
