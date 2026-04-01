//! Gossip-based node discovery for DDL using iroh-gossip
//!
//! Implements the real gossip protocol for distributed topic ownership
//! coordination using epidemic broadcast.

use bytes::Bytes;
use iroh::{protocol::Router, Endpoint, endpoint::presets::N0};
use iroh_gossip::{net::Gossip, TopicId, api::{GossipSender, Event}};
use n0_future::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{broadcast, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::gossip_protocol::{GossipMessage, NodeId, TopicOwnership};
use crate::cluster::membership::{MembershipEvent, NodeInfo};

/// Default heartbeat interval in seconds
const DEFAULT_HEARTBEAT_INTERVAL_SECS: u64 = 5;

/// Default owner timeout in seconds (owners not heartbeating are considered dead)
const DEFAULT_OWNER_TIMEOUT_SECS: u64 = 30;

/// Gossip errors
#[derive(Debug, thiserror::Error)]
pub enum GossipError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Gossip error: {0}")]
    Gossip(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Topic not found: {0}")]
    TopicNotFound(String),

    #[error("Not subscribed to topic: {0}")]
    NotSubscribed(String),

    #[error("Channel error: {0}")]
    Channel(String),

    #[error("Already shutdown")]
    AlreadyShutdown,
}

impl From<serde_json::Error> for GossipError {
    fn from(e: serde_json::Error) -> Self {
        GossipError::Serialization(e.to_string())
    }
}

/// Derive iroh-gossip TopicId from topic name using BLAKE3
pub fn derive_iroh_topic_id(topic: &str) -> TopicId {
    TopicId::from(blake3::hash(topic.as_bytes()))
}

/// Convert iroh TopicId to bytes for serialization
pub fn topic_id_to_bytes(topic_id: &TopicId) -> [u8; 32] {
    *topic_id.as_bytes()
}

/// Convert bytes to iroh TopicId
pub fn bytes_to_topic_id(bytes: [u8; 32]) -> TopicId {
    TopicId::from_bytes(bytes)
}

/// The main gossip coordinator using iroh-gossip
pub struct GossipCoordinator {
    /// Our node ID (derived from iroh node ID)
    node_id: NodeId,
    /// iroh endpoint for P2P communication
    endpoint: Endpoint,
    /// iroh router for protocol handling
    #[allow(dead_code)]
    router: Router,
    /// Gossip protocol instance
    gossip: Arc<Gossip>,
    /// Active topic subscriptions (topic_id -> sender)
    topic_senders: Arc<RwLock<HashMap<TopicId, Arc<tokio::sync::Mutex<GossipSender>>>>>,
    /// Local topic ownership cache (topic_name -> ownership info)
    topic_owners: Arc<RwLock<HashMap<String, TopicOwnership>>>,
    /// Topics we own
    owned_topics: Arc<RwLock<Vec<String>>>,
    /// Shutdown signal
    shutdown_tx: broadcast::Sender<()>,
    /// Background task handles
    task_handles: Arc<RwLock<Vec<JoinHandle<()>>>>,
    /// Heartbeat interval handle
    heartbeat_handle: Arc<RwLock<Option<JoinHandle<()>>>>,
    /// Heartbeat interval in seconds
    heartbeat_interval_secs: u64,
    /// Owner timeout in seconds
    owner_timeout_secs: u64,
    /// Membership events broadcaster (shared with RaftClusterNode)
    membership_tx: Option<broadcast::Sender<MembershipEvent>>,
    /// Known peers with last-seen timestamps
    peers: Arc<RwLock<HashMap<u64, InternalPeerInfo>>>,
}

/// Internal peer information for tracking
#[derive(Debug, Clone)]
struct InternalPeerInfo {
    addr: String,
    last_seen: SystemTime,
}

impl GossipCoordinator {
    /// Create a new gossip coordinator
    ///
    /// This initializes the iroh endpoint and spawns the gossip protocol.
    /// The coordinator will start listening for incoming connections and
    /// can join topics to participate in the gossip network.
    ///
    /// # Arguments
    /// * `node_id` - Unique identifier for this node
    /// * `_bind_addr` - Address to bind to (unused, iroh manages binding)
    /// * `_bootstrap_peers` - List of peer addresses (currently unused)
    ///
    /// # Returns
    /// A new GossipCoordinator ready to join topics and participate in gossip.
    pub async fn new(
        node_id: NodeId,
        _bind_addr: &str,
        _bootstrap_peers: Vec<String>,
    ) -> Result<Self, GossipError> {
        // Create iroh endpoint with N0 preset (default discovery)
        let endpoint = Endpoint::builder(N0)
            .bind()
            .await
            .map_err(|e| GossipError::Gossip(format!("Failed to bind endpoint: {}", e)))?;

        // Get our iroh node ID
        let iroh_node_id = endpoint.id();
        info!(
            node_id = %node_id,
            iroh_node_id = %iroh_node_id.fmt_short(),
            "Initialized iroh endpoint"
        );

        // Spawn the gossip protocol
        let gossip = Gossip::builder().spawn(endpoint.clone());

        // Setup router
        let router = Router::builder(endpoint.clone())
            .accept(iroh_gossip::ALPN, gossip.clone())
            .spawn();

        let (shutdown_tx, _) = broadcast::channel(1);

        let coordinator = Self {
            node_id,
            endpoint,
            router,
            gossip: Arc::new(gossip),
            topic_senders: Arc::new(RwLock::new(HashMap::new())),
            topic_owners: Arc::new(RwLock::new(HashMap::new())),
            owned_topics: Arc::new(RwLock::new(Vec::new())),
            shutdown_tx,
            task_handles: Arc::new(RwLock::new(Vec::new())),
            heartbeat_handle: Arc::new(RwLock::new(None)),
            heartbeat_interval_secs: DEFAULT_HEARTBEAT_INTERVAL_SECS,
            owner_timeout_secs: DEFAULT_OWNER_TIMEOUT_SECS,
            membership_tx: None,
            peers: Arc::new(RwLock::new(HashMap::new())),
        };

        // Start background cleanup tasks
        coordinator.start_task_cleanup().await;
        coordinator.start_cache_cleanup().await;
        coordinator.start_sender_cleanup().await;

        Ok(coordinator)
    }

    /// Get the iroh endpoint's node ID (for bootstrap purposes)
    pub fn node_addr(&self) -> Result<String, GossipError> {
        Ok(self.endpoint.id().to_string())
    }

    /// Join the gossip network for a topic
    ///
    /// This subscribes to a topic and starts receiving messages for it.
    /// It also allows broadcasting messages to other nodes subscribed to the same topic.
    pub async fn join_topic(&self, topic: &str) -> Result<(), GossipError> {
        // P0 Fix: Validate topic name (defense-in-depth)
        crate::traits::ddl::validate_topic(topic)
            .map_err(|e| GossipError::Parse(format!("Invalid topic name: {}", e)))?;
        
        let topic_id = derive_iroh_topic_id(topic);

        info!(
            node_id = self.node_id,
            topic = %topic,
            topic_id = ?topic_id.as_bytes()[..5],
            "Joining topic"
        );

        // Check if already subscribed
        {
            let senders = self.topic_senders.read().await;
            if senders.contains_key(&topic_id) {
                debug!(topic = %topic, "Already subscribed to topic");
                return Ok(());
            }
        }

        // Subscribe to the topic
        let topic_subscription = self
            .gossip
            .subscribe(topic_id, vec![])
            .await
            .map_err(|e| GossipError::Gossip(format!("Failed to subscribe: {}", e)))?;

        // Split into sender and receiver
        let (sender, mut receiver) = topic_subscription.split();

        // Store sender for broadcasting
        {
            let mut senders = self.topic_senders.write().await;
            senders.insert(topic_id, Arc::new(tokio::sync::Mutex::new(sender)));
        }

        // Spawn task to handle incoming messages for this topic
        let topic_owners = self.topic_owners.clone();
        let owned_topics = self.owned_topics.clone();
        let node_id = self.node_id;
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        let topic_name = topic.to_string();

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        debug!(topic = %topic_name, "Message handler shutting down");
                        break;
                    }
                    result = receiver.next() => {
                        match result {
                            Some(Ok(event)) => {
                                Self::handle_event(
                                    event,
                                    &topic_name,
                                    node_id,
                                    &topic_owners,
                                    &owned_topics,
                                ).await;
                            }
                            Some(Err(e)) => {
                                error!(topic = %topic_name, error = ?e, "Error receiving message");
                            }
                            None => {
                                // Stream ended
                                debug!(topic = %topic_name, "Message stream ended");
                                break;
                            }
                        }
                    }
                }
            }
        });

        // Store task handle
        {
            let mut handles = self.task_handles.write().await;
            handles.push(handle);
        }

        Ok(())
    }

    /// Handle a gossip event
    async fn handle_event(
        event: Event,
        topic: &str,
        our_node_id: NodeId,
        topic_owners: &Arc<RwLock<HashMap<String, TopicOwnership>>>,
        owned_topics: &Arc<RwLock<Vec<String>>>,
    ) {
        match event {
            Event::Received(msg) => {
                // P0 Fix: Size limit to prevent DoS attacks
                const MAX_MESSAGE_SIZE: usize = 1_048_576; // 1MB max message size
                if msg.content.len() > MAX_MESSAGE_SIZE {
                    warn!(
                        topic = %topic,
                        size = msg.content.len(),
                        max_size = MAX_MESSAGE_SIZE,
                        "Gossip message too large, ignoring"
                    );
                    return;
                }
                
                // Deserialize the message
                match serde_json::from_slice::<GossipMessage>(&msg.content) {
                    Ok(gossip_msg) => {
                        Self::handle_gossip_message(
                            gossip_msg,
                            topic,
                            our_node_id,
                            topic_owners,
                            owned_topics,
                        )
                        .await;
                    }
                    Err(e) => {
                        warn!(
                            topic = %topic,
                            error = %e,
                            from_peer = ?msg.delivered_from,
                            "Failed to deserialize gossip message"
                        );
                    }
                }
            }
            Event::NeighborUp(peer_id) => {
                debug!(
                    topic = %topic,
                    peer_id = %peer_id.fmt_short(),
                    "Neighbor joined topic"
                );
            }
            Event::NeighborDown(peer_id) => {
                debug!(
                    topic = %topic,
                    peer_id = %peer_id.fmt_short(),
                    "Neighbor left topic"
                );
                // Could check if this peer owned topics and clean up
            }
            Event::Lagged => {
                warn!(topic = %topic, "Missed some messages (receiver too slow)");
            }
        }
    }

    /// Handle a gossip message
    async fn handle_gossip_message(
        msg: GossipMessage,
        _topic: &str,
        _our_node_id: NodeId,
        topic_owners: &Arc<RwLock<HashMap<String, TopicOwnership>>>,
        owned_topics: &Arc<RwLock<Vec<String>>>,
    ) {
        match msg {
            GossipMessage::TopicClaim {
                topic: claimed_topic,
                topic_id,
                node_id,
                epoch,
                timestamp,
            } => {
                debug!(
                    claimed_topic = %claimed_topic,
                    node_id = node_id,
                    epoch = epoch,
                    "Received topic claim"
                );

                // P0 Fix: Timestamp sanity check - reject timestamps > 60 seconds in the future
                let now = crate::types::now_nanos();
                let max_future_nanos = Duration::from_secs(60).as_nanos() as u64;
                if timestamp > now.saturating_add(max_future_nanos) {
                    warn!(
                        topic = %claimed_topic,
                        timestamp = timestamp,
                        now = now,
                        "Rejecting claim with future timestamp"
                    );
                    return;
                }

                // Verify topic ID
                let derived_id = topic_id_to_bytes(&derive_iroh_topic_id(&claimed_topic));
                if topic_id != derived_id {
                    warn!("Topic ID mismatch in claim message");
                    return;
                }

                let mut owners = topic_owners.write().await;
                let ownership = owners
                    .entry(claimed_topic.clone())
                    .or_insert_with(|| TopicOwnership::new(node_id));

                // Accept claim if:
                // 1. New claim has higher epoch
                // 2. Same epoch AND same owner (recovery/reclaim)
                // 3. Same epoch AND lower node_id (epoch tiebreaker)
                // Use max for timestamps to handle clock skew
                if epoch > ownership.epoch
                    || (epoch == ownership.epoch && node_id == ownership.owner_node_id)
                    || (epoch == ownership.epoch && node_id < ownership.owner_node_id)
                {
                    ownership.owner_node_id = node_id;
                    ownership.epoch = epoch;
                    ownership.claimed_at = timestamp.max(ownership.claimed_at);
                    ownership.last_heartbeat = timestamp.max(ownership.last_heartbeat);
                    info!(
                        topic = %claimed_topic,
                        owner = node_id,
                        epoch = epoch,
                        "Updated topic ownership from claim"
                    );
                }
                // Reject stale claims (lower epoch) silently
            }
            GossipMessage::TopicRelease {
                topic: released_topic,
                node_id,
                ..
            } => {
                debug!(
                    released_topic = %released_topic,
                    node_id = node_id,
                    "Received topic release"
                );

                let mut owners = topic_owners.write().await;
                if let Some(ownership) = owners.get(&released_topic) {
                    if ownership.owner_node_id == node_id {
                        owners.remove(&released_topic);
                        info!(
                            topic = %released_topic,
                            "Removed topic ownership from release"
                        );
                    }
                }
            }
            GossipMessage::TopicHeartbeat {
                topic: heartbeat_topic,
                node_id,
                epoch,
                timestamp,
                ..
            } => {
                debug!(
                    heartbeat_topic = %heartbeat_topic,
                    node_id = node_id,
                    epoch = epoch,
                    "Received topic heartbeat"
                );

                // P0 Fix: Timestamp sanity check - reject timestamps > 60 seconds in the future
                let now = crate::types::now_nanos();
                let max_future_nanos = Duration::from_secs(60).as_nanos() as u64;
                if timestamp > now.saturating_add(max_future_nanos) {
                    warn!(
                        topic = %heartbeat_topic,
                        timestamp = timestamp,
                        now = now,
                        "Rejecting heartbeat with future timestamp"
                    );
                    return;
                }

                // P0 Fix: Use remote timestamp directly, taking max to handle clock skew
                let mut owners = topic_owners.write().await;
                if let Some(ownership) = owners.get_mut(&heartbeat_topic) {
                    // Only accept heartbeat if epoch matches owner's epoch
                    if ownership.owner_node_id == node_id && epoch == ownership.epoch {
                        ownership.last_heartbeat = timestamp.max(ownership.last_heartbeat);
                    }
                }
            }
            GossipMessage::NodeAnnounce {
                node_id,
                bind_addr,
                owned_topics: announced_topics,
            } => {
                debug!(
                    node_id = node_id,
                    bind_addr = %bind_addr,
                    topics = ?announced_topics,
                    "Received node announcement"
                );

                // Record that this node owns these topics
                let mut owners = topic_owners.write().await;
                for topic_name in announced_topics {
                    owners.insert(topic_name.clone(), TopicOwnership::new(node_id));
                }
            }
            GossipMessage::NodeLeave { node_id } => {
                debug!(node_id = node_id, "Received node leave");

                // Remove all topics owned by this node
                let mut owners = topic_owners.write().await;
                owners.retain(|_, ownership| ownership.owner_node_id != node_id);
            }
            GossipMessage::TopicQuery {
                topic: queried_topic,
                requestor_node_id,
            } => {
                // Check if we own this topic
                let _owners = topic_owners.read().await;
                let owned = owned_topics.read().await;

                if owned.contains(&queried_topic) {
                    debug!(
                        queried_topic = %queried_topic,
                        requestor = requestor_node_id,
                        "Would respond to topic query (we own it)"
                    );
                    // Note: Would need sender to broadcast response
                }
            }
            GossipMessage::TopicResponse {
                topic: response_topic,
                owner_node_id,
            } => {
                debug!(
                    response_topic = %response_topic,
                    owner = ?owner_node_id,
                    "Received topic response"
                );

                if let Some(owner) = owner_node_id {
                    let mut owners = topic_owners.write().await;
                    owners.insert(response_topic.clone(), TopicOwnership::new(owner));
                }
            }
        }
    }

    /// Claim ownership of a topic
    ///
    /// Broadcasts a claim message to all peers in the gossip network.
    pub async fn claim_topic(&self, topic: &str) -> Result<(), GossipError> {
        let topic_id = derive_iroh_topic_id(topic);
        let timestamp = crate::types::now_nanos();

        info!(
            node_id = self.node_id,
            topic = %topic,
            "Claiming topic"
        );

        // Record ownership locally with epoch
        let epoch = {
            let mut owners = self.topic_owners.write().await;
            let ownership = owners.entry(topic.to_string()).or_insert_with(|| TopicOwnership::new(self.node_id));
            
            // If we already own it, increment epoch for reclaim
            // Otherwise use the epoch from new ownership
            if ownership.owner_node_id == self.node_id {
                ownership.epoch = ownership.epoch.saturating_add(1);
                ownership.claimed_at = timestamp;
                ownership.last_heartbeat = timestamp;
            } else {
                // New claim - epoch should be higher than any existing
                let new_epoch = ownership.epoch.saturating_add(1);
                ownership.owner_node_id = self.node_id;
                ownership.epoch = new_epoch;
                ownership.claimed_at = timestamp;
                ownership.last_heartbeat = timestamp;
            }
            ownership.epoch
        };

        // Add to owned topics
        {
            let mut owned = self.owned_topics.write().await;
            if !owned.contains(&topic.to_string()) {
                owned.push(topic.to_string());
            }
        }

        // Broadcast claim message with epoch
        let msg = GossipMessage::TopicClaim {
            topic: topic.to_string(),
            topic_id: topic_id_to_bytes(&topic_id),
            node_id: self.node_id,
            epoch,
            timestamp,
        };

        self.broadcast_message(&topic_id, &msg).await
    }

    /// Release ownership of a topic
    ///
    /// Broadcasts a release message to all peers in the gossip network.
    pub async fn release_topic(&self, topic: &str) -> Result<(), GossipError> {
        let topic_id = derive_iroh_topic_id(topic);

        info!(
            node_id = self.node_id,
            topic = %topic,
            "Releasing topic"
        );

        // Remove from owned topics
        {
            let mut owned = self.owned_topics.write().await;
            owned.retain(|t| t != topic);
        }

        // Remove local ownership
        {
            let mut owners = self.topic_owners.write().await;
            owners.remove(topic);
        }

        // Broadcast release message
        let msg = GossipMessage::TopicRelease {
            topic: topic.to_string(),
            topic_id: topic_id_to_bytes(&topic_id),
            node_id: self.node_id,
        };

        self.broadcast_message(&topic_id, &msg).await
    }

    /// Broadcast a gossip message to a topic
    async fn broadcast_message(
        &self,
        topic_id: &TopicId,
        msg: &GossipMessage,
    ) -> Result<(), GossipError> {
        let senders = self.topic_senders.read().await;

        let sender = senders
            .get(topic_id)
            .ok_or_else(|| {
                GossipError::NotSubscribed(hex::encode(topic_id.as_bytes()))
            })?;

        // Serialize message
        let payload = serde_json::to_vec(msg)?;
        let bytes = Bytes::from(payload);

        // Broadcast
        let sender_guard = sender.lock().await;
        sender_guard
            .broadcast(bytes)
            .await
            .map_err(|e| GossipError::Gossip(format!("Broadcast failed: {}", e)))?;

        debug!(
            node_id = self.node_id,
            topic_id = ?topic_id.as_bytes()[..5],
            "Broadcast message"
        );

        Ok(())
    }

    /// Get the owner of a topic
    ///
    /// Returns the NodeId that owns this topic, or None if unknown/expired.
    pub async fn get_topic_owner(&self, topic: &str) -> Option<NodeId> {
        let owners = self.topic_owners.read().await;
        let ownership = owners.get(topic)?;

        // Check if ownership has timed out using configured timeout value
        let timeout_nanos = Duration::from_secs(self.owner_timeout_secs).as_nanos() as u64;
        if ownership.is_timed_out(timeout_nanos) {
            return None;
        }

        Some(ownership.owner_node_id)
    }

    /// Check if we own a topic
    pub async fn owns_topic(&self, topic: &str) -> bool {
        let owned = self.owned_topics.read().await;
        owned.contains(&topic.to_string())
    }

    /// Set topics that this node owns (for static assignment)
    pub async fn set_owned_topics(&self, topics: Vec<String>) {
        let mut owned = self.owned_topics.write().await;
        *owned = topics;
    }

    /// Set heartbeat interval in seconds
    pub fn set_heartbeat_interval(&mut self, interval_secs: u64) {
        self.heartbeat_interval_secs = interval_secs;
    }

    /// Set owner timeout in seconds
    pub fn set_owner_timeout(&mut self, timeout_secs: u64) {
        self.owner_timeout_secs = timeout_secs;
    }

    /// Get heartbeat interval in seconds
    pub fn heartbeat_interval(&self) -> u64 {
        self.heartbeat_interval_secs
    }

    /// Get owner timeout in seconds
    pub fn owner_timeout(&self) -> u64 {
        self.owner_timeout_secs
    }

    /// Check if this gossip coordinator is functional
    ///
    /// Returns `true` for a real implementation.
    pub fn is_functional(&self) -> bool {
        true
    }

    /// Start heartbeat task for owned topics
    ///
    /// This periodically sends heartbeat messages for topics we own.
    pub async fn start_heartbeat(&self) {
        let owned_topics = self.owned_topics.clone();
        let topic_senders = self.topic_senders.clone();
        let topic_owners = self.topic_owners.clone();
        let node_id = self.node_id;
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        let heartbeat_interval_secs = self.heartbeat_interval_secs;

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(heartbeat_interval_secs));

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        debug!("Heartbeat task shutting down");
                        break;
                    }
                    _ = interval.tick() => {
                        let owned = owned_topics.read().await;
                        let senders = topic_senders.read().await;

                        for topic in owned.iter() {
                            // Get current epoch for this topic
                            let epoch = {
                                let owners = topic_owners.read().await;
                                owners.get(topic).map(|o| o.epoch).unwrap_or(1)
                            };
                            
                            let topic_id = derive_iroh_topic_id(topic);
                            let timestamp = crate::types::now_nanos();

                            let msg = GossipMessage::TopicHeartbeat {
                                topic: topic.clone(),
                                topic_id: topic_id_to_bytes(&topic_id),
                                node_id,
                                epoch,
                                timestamp,
                            };

                            if let Some(sender) = senders.get(&topic_id) {
                                if let Ok(payload) = serde_json::to_vec(&msg) {
                                    let sender_guard = sender.lock().await;
                                    if let Err(e) = sender_guard.broadcast(Bytes::from(payload)).await {
                                        warn!(
                                            topic = %topic,
                                            error = ?e,
                                            "Failed to send heartbeat"
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        let mut heartbeat_handle = self.heartbeat_handle.write().await;
        *heartbeat_handle = Some(handle);
    }

    /// Handle incoming gossip messages (placeholder for global handling)
    ///
    /// This runs until shutdown. Per-topic message handling is done in `join_topic`.
    pub async fn handle_messages(&self) {
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        debug!(
            node_id = self.node_id,
            "Starting message handler"
        );

        // Wait for shutdown signal
        let _ = shutdown_rx.recv().await;

        debug!(
            node_id = self.node_id,
            "Message handler stopped"
        );
    }

    /// Trigger graceful shutdown of gossip operations
    pub async fn shutdown(&self) {
        debug!(
            node_id = self.node_id,
            "Sending shutdown signal to gossip handler"
        );

        // Send shutdown signal
        let _ = self.shutdown_tx.send(());

        // Wait for all task handles to complete
        let handles = self.task_handles.write().await;
        for handle in handles.iter() {
            handle.abort();
        }

        // Stop heartbeat
        let mut heartbeat = self.heartbeat_handle.write().await;
        if let Some(handle) = heartbeat.take() {
            handle.abort();
        }

        debug!(
            node_id = self.node_id,
            "Gossip shutdown complete"
        );
    }

    /// Set the membership event broadcaster (called by RaftClusterNode)
    pub fn set_membership_channel(&mut self, tx: broadcast::Sender<MembershipEvent>) {
        self.membership_tx = Some(tx);
    }

    /// Emit a membership event
    fn emit_event(&self, event: MembershipEvent) {
        if let Some(tx) = &self.membership_tx {
            let _ = tx.send(event);
        }
    }

    /// Called when a new peer joins
    pub async fn peer_joined(&self, node_id: u64, addr: String) {
        // Update peer table
        self.peers.write().await.insert(
            node_id,
            InternalPeerInfo {
                addr: addr.clone(),
                last_seen: SystemTime::now(),
            },
        );

        // Emit event
        self.emit_event(MembershipEvent::node_joined(node_id, addr));
    }

    /// Called when a peer gracefully leaves
    pub async fn peer_left(&self, node_id: u64) {
        // Remove from peer table
        self.peers.write().await.remove(&node_id);

        // Emit event
        self.emit_event(MembershipEvent::node_left(node_id));
    }

    /// Called when a peer times out (heartbeat not received)
    pub async fn peer_failed(&self, node_id: u64) {
        // Remove from peer table
        self.peers.write().await.remove(&node_id);

        // Emit event
        self.emit_event(MembershipEvent::node_failed(node_id));
    }

    /// Update peer last-seen time (called on heartbeat receive)
    pub async fn peer_heartbeat(&self, node_id: u64) {
        if let Some(peer) = self.peers.write().await.get_mut(&node_id) {
            peer.last_seen = SystemTime::now();
        }
    }

    /// Background task to detect failed peers
    pub async fn start_failure_detection(&self, timeout_secs: u64) {
        let peers = self.peers.clone();
        let membership_tx = self.membership_tx.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        debug!("Failure detection shutting down");
                        break;
                    }
                    _ = interval.tick() => {
                        let now = SystemTime::now();
                        let timeout = Duration::from_secs(timeout_secs);

                        // Check all peers for timeout
                        let mut peers_guard = peers.write().await;
                        let mut failed = Vec::new();

                        for (node_id, peer) in peers_guard.iter() {
                            if let Ok(elapsed) = now.duration_since(peer.last_seen) {
                                if elapsed > timeout {
                                    failed.push(*node_id);
                                }
                            }
                        }

                        // Remove failed peers and emit events
                        for node_id in failed {
                            peers_guard.remove(&node_id);

                            warn!(
                                "Node {} failed (heartbeat timeout)",
                                node_id
                            );

                            if let Some(tx) = &membership_tx {
                                let _ = tx.send(MembershipEvent::node_failed(node_id));
                            }
                        }
                    }
                }
            }
        });

        let mut handles = self.task_handles.write().await;
        handles.push(handle);
    }

    /// Get current members
    pub async fn members(&self) -> HashMap<u64, NodeInfo> {
        let peers = self.peers.read().await;
        peers
            .iter()
            .map(|(id, peer)| {
                (
                    *id,
                    NodeInfo {
                        node_id: *id,
                        addr: peer.addr.clone(),
                        last_seen: peer.last_seen,
                        is_leader: false, // Leadership is tracked separately by Raft
                    },
                )
            })
            .collect()
    }

    /// Announce our presence and owned topics
    pub async fn announce_node(&self, bind_addr: String) -> Result<(), GossipError> {
        let owned = self.owned_topics.read().await;

        let msg = GossipMessage::NodeAnnounce {
            node_id: self.node_id,
            bind_addr,
            owned_topics: owned.clone(),
        };

        // Announce to all subscribed topics
        let senders = self.topic_senders.read().await;
        for topic_id in senders.keys() {
            let payload = serde_json::to_vec(&msg)?;
            let bytes = Bytes::from(payload);

            if let Some(sender) = senders.get(topic_id) {
                let sender_guard = sender.lock().await;
                sender_guard
                    .broadcast(bytes.clone())
                    .await
                    .map_err(|e| {
                        GossipError::Gossip(format!("Announce broadcast failed: {}", e))
                    })?;
            }
        }

        debug!(
            node_id = self.node_id,
            topics_count = owned.len(),
            "Announced node presence"
        );

        Ok(())
    }

    /// Start the ownership monitor task for automatic failover
    ///
    /// Monitors topics we own and claims orphaned topics when previous owner times out.
    pub async fn start_ownership_monitor(&self) {
        let topic_owners = self.topic_owners.clone();
        let owned_topics = self.owned_topics.clone();
        let topic_senders = self.topic_senders.clone();
        let node_id = self.node_id;
        let owner_timeout_secs = self.owner_timeout_secs;
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => break,
                    _ = interval.tick() => {
                        // Check for timed-out owners and claim orphaned topics we own
                        let owned = owned_topics.read().await;

                        for topic in owned.iter() {
                            // Check if we need to claim this topic
                            let claim_info = {
                                let owners = topic_owners.read().await;
                                if let Some(ownership) = owners.get(topic) {
                                    let timeout_nanos = Duration::from_secs(owner_timeout_secs).as_nanos() as u64;
                                    if ownership.is_timed_out(timeout_nanos) && ownership.owner_node_id != node_id {
                                        // Need to claim - compute new epoch from existing
                                        Some((ownership.epoch.saturating_add(1), ownership.owner_node_id))
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            };

                            // If we need to claim, do it with write lock
                            if let Some((new_epoch, old_owner)) = claim_info {
                                let now = crate::types::now_nanos();

                                // Update ownership locally with incremented epoch
                                {
                                    let mut owners = topic_owners.write().await;
                                    let new_ownership = TopicOwnership {
                                        owner_node_id: node_id,
                                        epoch: new_epoch,
                                        claimed_at: now,
                                        last_heartbeat: now,
                                    };
                                    owners.insert(topic.clone(), new_ownership);
                                }

                                info!(
                                    topic = %topic,
                                    old_owner = old_owner,
                                    new_epoch = new_epoch,
                                    "Claimed orphaned topic during failover"
                                );

                                // Broadcast claim to other nodes
                                let topic_id = derive_iroh_topic_id(topic);
                                let msg = GossipMessage::TopicClaim {
                                    topic: topic.clone(),
                                    topic_id: topic_id_to_bytes(&topic_id),
                                    node_id,
                                    epoch: new_epoch,
                                    timestamp: now,
                                };

                                // Get sender and broadcast
                                let senders = topic_senders.read().await;
                                if let Some(sender) = senders.get(&topic_id) {
                                    if let Ok(payload) = serde_json::to_vec(&msg) {
                                        let sender_guard = sender.lock().await;
                                        if let Err(e) = sender_guard.broadcast(Bytes::from(payload)).await {
                                            warn!(
                                                topic = %topic,
                                                error = ?e,
                                                "Failed to broadcast failover claim"
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        let mut handles = self.task_handles.write().await;
        handles.push(handle);
    }

    /// Start the task cleanup background task
    ///
    /// Periodically cleans up completed/aborted task handles.
    pub async fn start_task_cleanup(&self) {
        let task_handles = self.task_handles.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => break,
                    _ = interval.tick() => {
                        let mut handles = task_handles.write().await;
                        let before = handles.len();
                        handles.retain(|h| !h.is_finished());
                        let after = handles.len();
                        if before != after {
                            debug!(cleaned = before - after, "Cleaned up finished task handles");
                        }
                    }
                }
            }
        });

        // Add cleanup task to handles
        let mut handles = self.task_handles.write().await;
        handles.push(handle);
    }

    /// Start ownership cache cleanup
    ///
    /// Periodically evicts timed-out ownership entries from the cache.
    pub async fn start_cache_cleanup(&self) {
        let topic_owners = self.topic_owners.clone();
        let owner_timeout_secs = self.owner_timeout_secs;
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => break,
                    _ = interval.tick() => {
                        let timeout_nanos = Duration::from_secs(owner_timeout_secs * 2).as_nanos() as u64;
                        let mut owners = topic_owners.write().await;
                        let before = owners.len();
                        owners.retain(|_, ownership| !ownership.is_timed_out(timeout_nanos));
                        let after = owners.len();
                        if before != after {
                            debug!(evicted = before - after, "Evicted timed-out ownership entries");
                        }
                    }
                }
            }
        });

        let mut handles = self.task_handles.write().await;
        handles.push(handle);
    }

    /// Start topic sender cleanup
    ///
    /// Periodically cleans up senders for topics we no longer own or track.
    pub async fn start_sender_cleanup(&self) {
        let topic_senders = self.topic_senders.clone();
        let owned_topics = self.owned_topics.clone();
        let topic_owners = self.topic_owners.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => break,
                    _ = interval.tick() => {
                        // Get all topics we should keep senders for
                        let owned: std::collections::HashSet<_> = {
                            let owned_guard = owned_topics.read().await;
                            owned_guard.iter().cloned().collect()
                        };

                        let tracked: std::collections::HashSet<String> = {
                            let owners = topic_owners.read().await;
                            owners.keys().cloned().collect()
                        };

                        // Keep topics that are owned or tracked (active)
                        let active_topics: std::collections::HashSet<_> = owned.union(&tracked).collect();

                        // Compute topic_ids for active topics
                        let active_topic_ids: std::collections::HashSet<TopicId> = active_topics
                            .iter()
                            .map(|t| derive_iroh_topic_id(*t))
                            .collect();

                        // Remove senders for inactive topics
                        let mut senders = topic_senders.write().await;
                        let before = senders.len();
                        senders.retain(|topic_id, _| active_topic_ids.contains(topic_id));
                        let after = senders.len();

                        if before != after {
                            debug!(
                                removed = before - after,
                                remaining = after,
                                "Cleaned up inactive topic senders"
                            );
                        }
                    }
                }
            }
        });

        let mut handles = self.task_handles.write().await;
        handles.push(handle);
    }

    /// Query for the owner of a topic
    ///
    /// Broadcasts a query message and waits for responses.
    pub async fn query_topic_owner(&self, topic: &str) -> Result<(), GossipError> {
        let topic_id = derive_iroh_topic_id(topic);

        let msg = GossipMessage::TopicQuery {
            topic: topic.to_string(),
            requestor_node_id: self.node_id,
        };

        self.broadcast_message(&topic_id, &msg).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gossip_coordinator_creation() {
        let result = GossipCoordinator::new(1, "127.0.0.1:0", vec![]).await;

        // Should be able to create coordinator
        assert!(result.is_ok(), "Should be able to create coordinator");

        let coordinator = result.unwrap();
        assert!(coordinator.is_functional());
    }

    #[tokio::test]
    async fn test_gossip_join_topic() {
        let coordinator = GossipCoordinator::new(1, "127.0.0.1:0", vec![])
            .await
            .expect("Failed to create coordinator");

        // Should be able to join a topic
        let result = coordinator.join_topic("test.topic").await;
        assert!(result.is_ok(), "Should be able to join topic");

        // Joining same topic again is idempotent
        let result = coordinator.join_topic("test.topic").await;
        assert!(result.is_ok(), "Should be idempotent");
    }

    #[tokio::test]
    async fn test_gossip_shutdown() {
        let coordinator = GossipCoordinator::new(1, "127.0.0.1:0", vec![])
            .await
            .expect("Failed to create coordinator");

        // Should be able to shutdown cleanly
        coordinator.shutdown().await;

        // And calling shutdown again should be safe
        coordinator.shutdown().await;
    }

    #[tokio::test]
    async fn test_gossip_topic_ownership() {
        let coordinator = GossipCoordinator::new(1, "127.0.0.1:0", vec![])
            .await
            .expect("Failed to create coordinator");

        // Set owned topics
        coordinator
            .set_owned_topics(vec!["topic1".to_string(), "topic2".to_string()])
            .await;

        // Check owns_topic
        assert!(coordinator.owns_topic("topic1").await);
        assert!(coordinator.owns_topic("topic2").await);
        assert!(!coordinator.owns_topic("topic3").await);
    }

    #[tokio::test]
    async fn test_gossip_claim_release() {
        let coordinator = GossipCoordinator::new(1, "127.0.0.1:0", vec![])
            .await
            .expect("Failed to create coordinator");

        // Join topic first
        coordinator
            .join_topic("test.topic")
            .await
            .expect("Failed to join topic");

        // Claim topic
        let result = coordinator.claim_topic("test.topic").await;
        assert!(result.is_ok(), "Should be able to claim topic");

        // Should own it now
        assert!(coordinator.owns_topic("test.topic").await);

        // Release topic
        let result = coordinator.release_topic("test.topic").await;
        assert!(result.is_ok(), "Should be able to release topic");

        // Should not own it now
        assert!(!coordinator.owns_topic("test.topic").await);
    }

    // P0 Fix #2: Test topic validation in join_topic
    #[tokio::test]
    async fn test_gossip_join_topic_validation() {
        let coordinator = GossipCoordinator::new(1, "127.0.0.1:0", vec![])
            .await
            .expect("Failed to create coordinator");

        // Valid topic names should succeed
        assert!(
            coordinator.join_topic("valid.topic").await.is_ok(),
            "Valid topic should succeed"
        );
        assert!(
            coordinator.join_topic("simple").await.is_ok(),
            "Simple topic should succeed"
        );

        // Empty topic name should fail
        let result = coordinator.join_topic("").await;
        assert!(result.is_err(), "Empty topic should be rejected");

        // Topic with control characters should fail
        let result = coordinator.join_topic("bad\0topic").await;
        assert!(result.is_err(), "Control characters should be rejected");

        // Topic with path traversal should fail
        let result = coordinator.join_topic("bad..topic").await;
        assert!(result.is_err(), "Path traversal should be rejected");

        // Very long topic name should fail (> 255 characters)
        let long_topic = "a".repeat(300);
        let result = coordinator.join_topic(&long_topic).await;
        assert!(result.is_err(), "Long topic name should be rejected");
    }

    // P1 Fix #4: Test configurable heartbeat and timeout
    #[tokio::test]
    async fn test_gossip_configurable_intervals() {
        let coordinator = GossipCoordinator::new(1, "127.0.0.1:0", vec![])
            .await
            .expect("Failed to create coordinator");

        // Default values should be used
        assert_eq!(coordinator.heartbeat_interval(), 5);
        assert_eq!(coordinator.owner_timeout(), 30);

        // Create a mutable coordinator to set values
        let mut coordinator2 = GossipCoordinator::new(2, "127.0.0.1:0", vec![])
            .await
            .expect("Failed to create coordinator");

        coordinator2.set_heartbeat_interval(10);
        assert_eq!(coordinator2.heartbeat_interval(), 10);

        coordinator2.set_owner_timeout(60);
        assert_eq!(coordinator2.owner_timeout(), 60);
    }

    #[test]
    fn test_derive_iroh_topic_id() {
        let id1 = derive_iroh_topic_id("test.topic");
        let id2 = derive_iroh_topic_id("test.topic");
        let id3 = derive_iroh_topic_id("other.topic");

        // Same topic should produce same ID
        assert_eq!(id1.as_bytes(), id2.as_bytes());
        // Different topics should produce different IDs
        assert_ne!(id1.as_bytes(), id3.as_bytes());
    }

    // === Synchronous Helper Function Tests ===

    #[test]
    fn test_topic_id_to_bytes() {
        let topic_id = derive_iroh_topic_id("test");
        let bytes = topic_id_to_bytes(&topic_id);

        // Should produce 32 bytes
        assert_eq!(bytes.len(), 32);

        // Should be same as original
        assert_eq!(bytes, *topic_id.as_bytes());
    }

    #[test]
    fn test_bytes_to_topic_id() {
        // Create a topic ID and convert to bytes
        let original_id = derive_iroh_topic_id("my.topic");
        let bytes = topic_id_to_bytes(&original_id);

        // Convert back
        let restored_id = bytes_to_topic_id(bytes);

        // Should be identical
        assert_eq!(original_id.as_bytes(), restored_id.as_bytes());
    }

    #[test]
    fn test_roundtrip_topic_id_conversion() {
        // Test with various topic names
        let topics = vec![
            "simple",
            "dotted.topic",
            "deeply.nested.topic.name",
            "unicode-日本語",
            "emoji-🚀",
            "numbers-123",
            "special-chars_underscore",
        ];

        for topic in topics {
            let id1 = derive_iroh_topic_id(topic);
            let bytes = topic_id_to_bytes(&id1);
            let id2 = bytes_to_topic_id(bytes);

            // Roundtrip should preserve exact bytes
            assert_eq!(id1.as_bytes(), id2.as_bytes(), "Failed for topic: {}", topic);
        }
    }

    #[test]
    fn test_bytes_to_topic_id_all_zeros() {
        // Test with all zeros
        let zeros = [0u8; 32];
        let id = bytes_to_topic_id(zeros);
        assert_eq!(id.as_bytes(), &zeros);
    }

    #[test]
    fn test_bytes_to_topic_id_all_ff() {
        // Test with all 0xFF
        let ones = [0xFFu8; 32];
        let id = bytes_to_topic_id(ones);
        assert_eq!(id.as_bytes(), &ones);
    }

    #[test]
    fn test_topic_id_deterministic() {
        // Same topic name should always produce same ID
        let topic = "deterministic.test";

        for _ in 0..10 {
            let id1 = derive_iroh_topic_id(topic);
            let id2 = derive_iroh_topic_id(topic);
            assert_eq!(id1.as_bytes(), id2.as_bytes());
        }
    }

    #[test]
    fn test_topic_id_different_for_different_topics() {
        // Different topics should (very likely) produce different IDs
        let topics: Vec<String> = (0..100).map(|i| format!("topic.{}", i)).collect();

        let mut ids = std::collections::HashSet::new();
        for topic in topics.iter() {
            let id = derive_iroh_topic_id(topic);
            let bytes = topic_id_to_bytes(&id);
            // Each ID should be unique
            assert!(ids.insert(bytes), "Duplicate ID for topic: {}", topic);
        }
    }

    // === GossipError Display Tests ===

    #[test]
    fn test_gossip_error_io() {
        let err = GossipError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"));
        let display = format!("{}", err);
        assert!(display.contains("IO error"));
        assert!(display.contains("file not found"));
    }

    #[test]
    fn test_gossip_error_serialization() {
        let err = GossipError::Serialization("serde error".to_string());
        let display = format!("{}", err);
        assert!(display.contains("Serialization error"));
        assert!(display.contains("serde error"));
    }

    #[test]
    fn test_gossip_error_gossip() {
        let err = GossipError::Gossip("gossip failure".to_string());
        let display = format!("{}", err);
        assert!(display.contains("Gossip error"));
        assert!(display.contains("gossip failure"));
    }

    #[test]
    fn test_gossip_error_parse() {
        let err = GossipError::Parse("invalid format".to_string());
        let display = format!("{}", err);
        assert!(display.contains("Parse error"));
        assert!(display.contains("invalid format"));
    }

    #[test]
    fn test_gossip_error_topic_not_found() {
        let err = GossipError::TopicNotFound("missing.topic".to_string());
        let display = format!("{}", err);
        assert!(display.contains("Topic not found"));
        assert!(display.contains("missing.topic"));
    }

    #[test]
    fn test_gossip_error_not_subscribed() {
        let err = GossipError::NotSubscribed("topic-abc123".to_string());
        let display = format!("{}", err);
        assert!(display.contains("Not subscribed to topic"));
        assert!(display.contains("topic-abc123"));
    }

    #[test]
    fn test_gossip_error_channel() {
        let err = GossipError::Channel("channel closed".to_string());
        let display = format!("{}", err);
        assert!(display.contains("Channel error"));
        assert!(display.contains("channel closed"));
    }

    #[test]
    fn test_gossip_error_already_shutdown() {
        let err = GossipError::AlreadyShutdown;
        let display = format!("{}", err);
        assert!(display.contains("Already shutdown"));
    }

    #[test]
    fn test_gossip_error_from_serde_json() {
        // Test the From<serde_json::Error> impl
        let json_err = serde_json::from_str::<i32>("not a number");
        assert!(json_err.is_err());
        let gossip_err: GossipError = json_err.unwrap_err().into();
        match gossip_err {
            GossipError::Serialization(msg) => {
                // Just verify the error message exists
                assert!(!msg.is_empty());
            }
            _ => panic!("Expected Serialization variant"),
        }
    }
}