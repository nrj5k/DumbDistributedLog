//! Peer request handler trait for server-side request processing.

use super::{error::PeerError, request::PeerRequest, response::PeerResponse};
use async_trait::async_trait;

/// Trait for handling incoming peer requests.
///
/// Implement this trait to provide custom logic for processing
/// peer-to-peer requests in the DDL gossip network.
#[async_trait]
pub trait PeerRequestHandler: Send + Sync {
    /// Handle an incoming peer request and return a response.
    ///
    /// # Arguments
    ///
    /// * `request` - The incoming peer request to process
    ///
    /// # Returns
    ///
    /// * `Ok(PeerResponse)` - Successfully processed request
    /// * `Err(PeerError)` - Request processing failed
    async fn handle_request(&self, request: PeerRequest) -> Result<PeerResponse, PeerError>;
}
