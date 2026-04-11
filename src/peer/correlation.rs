//! Correlation table for tracking pending request/response pairs.
#![allow(clippy::single_line_fn)]
use super::response::PeerResponse;
use super::types::RequestId;
use dashmap::DashMap;
use tokio::sync::oneshot;
/// Thread-safe correlation table for matching requests with responses.
#[rustfmt::skip]
pub struct CorrelationTable { pending: DashMap<RequestId, oneshot::Sender<PeerResponse>> }
#[rustfmt::skip]
impl CorrelationTable {
    pub fn new() -> Self { CorrelationTable { pending: DashMap::new() } }
    /// Insert a pending request. Returns Err with existing sender if duplicate.
    pub fn insert(&self, request_id: RequestId, sender: oneshot::Sender<PeerResponse>) -> Result<(), PeerResponse> {
        match self.pending.insert(request_id, sender) {
            Some(_) => Err(PeerResponse::failure(request_id, 0, format!("duplicate: {}", request_id.0))),
            None => Ok(()),
        }
    }
    /// Complete a pending request by sending the response. Returns true if found.
    pub fn complete(&self, request_id: RequestId, response: PeerResponse) -> bool {
        self.pending.remove(&request_id).map_or(false, |(_, s)| s.send(response).is_ok())
    }
    /// Remove a pending request without sending. Returns sender if found.
    pub fn remove(&self, request_id: RequestId) -> Option<oneshot::Sender<PeerResponse>> {
        self.pending.remove(&request_id).map(|(_, s)| s)
    }
}
impl Default for CorrelationTable {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_insert_and_complete() {
        let table = CorrelationTable::new();
        let (tx, rx) = oneshot::channel();
        let rid = RequestId::new(1);
        assert!(table.insert(rid, tx).is_ok());
        assert!(table.complete(rid, PeerResponse::success(rid, 100, vec![1, 2, 3])));
        let r = rx.await.unwrap();
        assert!(r.success);
        assert_eq!(r.payload, vec![1, 2, 3]);
    }
    #[test]
    fn test_complete_unknown_request() {
        assert!(!CorrelationTable::new().complete(
            RequestId::new(2),
            PeerResponse::failure(RequestId::new(2), 100, "timeout")
        ));
    }
    #[tokio::test]
    async fn test_remove() {
        let table = CorrelationTable::new();
        let (tx, rx) = oneshot::channel();
        let rid = RequestId::new(3);
        assert!(table.insert(rid, tx).is_ok());
        let s = table.remove(rid);
        assert!(s.is_some());
        s.unwrap()
            .send(PeerResponse::success(rid, 100, vec![4, 5, 6]))
            .unwrap();
        assert!(rx.await.is_ok());
    }
    #[test]
    fn test_duplicate_insert() {
        let (tx1, _) = oneshot::channel();
        let (tx2, _) = oneshot::channel();
        let rid = RequestId::new(4);
        let table = CorrelationTable::new();
        assert!(table.insert(rid, tx1).is_ok());
        assert!(table.insert(rid, tx2).is_err());
    }
}
