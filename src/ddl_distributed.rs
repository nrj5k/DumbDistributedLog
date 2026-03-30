//! Distributed DDL implementation
//!
//! Supports three modes:
//! - Standalone: No gossip/Raft, owns all topics (for testing/single-node)
//! - Distributed with Gossip: Uses gossip for node discovery only
//! - Distributed with Raft: Uses Raft for strongly consistent topic ownership

use crate::cluster::membership::MembershipEvent;
use crate::cluster::raft_cluster::RaftClusterNode;
use crate::cluster::types::NodeConfig;
use crate::gossip::GossipCoordinator;
use crate::traits::ddl::{validate_topic, DDL, DdlConfig, DdlError, EntryStream};
use crate::topic_queue::{notify_subscribers, SubscriberInfo, TopicQueue};
use async_trait::async_trait;
use crossbeam::channel::bounded;
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// Distributed DDL implementation
///
/// Supports three modes:
/// - Standalone: No gossip/Raft, owns all topics (for testing/single-node)
/// - Distributed with Gossip: Uses gossip for node discovery only
/// - Distributed with Raft: Uses Raft for strongly consistent topic ownership
pub struct DdlDistributed {
    config: DdlConfig,
    /// Map of topic_name -> TopicQueue
    topics: Arc<DashMap<String, Arc<TopicQueue>>>,
    /// Map of topic_name -> subscription senders with metadata
    subscribers: Arc<DashMap<String, Vec<SubscriberInfo>>>,
    /// Raft cluster for ownership (distributed mode with raft_enabled)
    raft_cluster: Option<Arc<RaftClusterNode>>,
    /// Gossip coordinator for node discovery (distributed mode only)
    gossip: Option<Arc<GossipCoordinator>>,
    /// Handle to the gossip message handler task (None in standalone mode)
    gossip_task_handle: Option<JoinHandle<()>>,
    /// Atomic counter for topic count (avoid DashMap::len() overhead)
    topic_count: AtomicUsize,
}

impl Drop for DdlDistributed {
    fn drop(&mut self) {
        // Cancel the gossip task if it's running
        if let Some(handle) = self.gossip_task_handle.take() {
            handle.abort();
        }
        // Raft shutdown should be async, but Drop can't be async
        // This will be cleaned up when the process exits
    }
}

/// Type alias for backwards compatibility
///
/// InMemoryDdl is now just DdlDistributed in standalone mode.
/// Use `DdlDistributed::new_standalone()` for the same behavior.
pub type InMemoryDdl = DdlDistributed;

impl DdlDistributed {
    /// Create a standalone DDL instance (no gossip, owns all topics)
    ///
    /// Use this for:
    /// - Unit testing
    /// - Single-node deployments
    /// - Development/debugging
    pub fn new_standalone(config: DdlConfig) -> Self {
        let topics = Arc::new(DashMap::new());
        let subscribers = Arc::new(DashMap::new());

        // Pre-create queues for owned topics
        for topic in &config.owned_topics {
            let queue = Arc::new(TopicQueue::new(config.buffer_size));
            topics.insert(topic.clone(), queue);
        }

        // Initialize topic count from owned topics
        let topic_count = AtomicUsize::new(config.owned_topics.len());

        Self {
            config,
            topics,
            subscribers,
            raft_cluster: None,
            gossip: None,
            gossip_task_handle: None,
            topic_count,
        }
    }

    /// Create with default config (standalone mode)
    pub fn default() -> Self {
        Self::new_standalone(DdlConfig::default())
    }

    /// Create a distributed DDL instance
    ///
    /// If `raft_enabled` is true, uses Raft for strongly consistent topic ownership.
    /// Otherwise, uses gossip for eventual consistency.
    ///
    /// Use this for:
    /// - Multi-node deployments
    /// - Production distributed systems
    pub async fn new_distributed(config: DdlConfig) -> Result<Self, DdlError> {
        // P1 Fix: Warn if WAL is not enabled in distributed mode
        if !config.wal_enabled {
            warn!(
                "DISTRIBUTED MODE WITHOUT WAL: Data loss may occur on node failure. \
                 Consider enabling WAL with wal_enabled=true"
            );
        }

        let topics = Arc::new(DashMap::new());
        let subscribers = Arc::new(DashMap::new());

        // Create Raft cluster if enabled
        let raft_cluster = if config.raft_enabled {
            let raft = Self::create_raft_cluster(&config).await?;
            
            // Initialize cluster if bootstrap node
            if config.is_bootstrap {
                info!(node_id = config.node_id, "Initializing Raft cluster as bootstrap node");
                raft.initialize()
                    .await
                    .map_err(|e| DdlError::Network(format!("Failed to initialize Raft: {}", e)))?;
                
                // P1 Fix: Wait for leadership with exponential backoff retry
                let max_attempts = 10;
                let mut attempt = 0;
                let base_delay = std::time::Duration::from_millis(100);
                let max_delay = std::time::Duration::from_secs(5);
                
                loop {
                    if raft.is_leader().await {
                        info!(node_id = config.node_id, "Won leadership after {} attempts", attempt + 1);
                        break;
                    }
                    
                    attempt += 1;
                    if attempt >= max_attempts {
                        return Err(DdlError::Network(format!(
                            "Leadership election timeout after {} attempts",
                            max_attempts
                        )));
                    }
                    
                    let delay = std::cmp::min(base_delay * 2u32.pow(attempt as u32), max_delay);
                    debug!(node_id = config.node_id, attempt, "Waiting for leadership, next check in {:?}", delay);
                    tokio::time::sleep(delay).await;
                }
                
                // Now claim topics as the elected leader
                for topic in &config.owned_topics {
                    info!(node_id = config.node_id, topic = %topic, "Claiming topic via Raft");
                    raft.claim_topic(topic)
                        .await
                        .map_err(|e| DdlError::Network(format!("Failed to claim topic {}: {}", topic, e)))?;
                }
            }
            
            Some(raft)
        } else {
            None
        };

        // Create gossip coordinator for node discovery (used in both modes)
        let gossip = if config.gossip_enabled || config.raft_enabled {
            let mut gossip = GossipCoordinator::new(
                    config.node_id,
                    &config.gossip_bind_addr,
                    config.gossip_bootstrap.clone(),
                )
                .await
                .map_err(|e| DdlError::Network(format!("Failed to create gossip: {:?}", e)))?;

            // Set configurable heartbeat and timeout values
            gossip.set_heartbeat_interval(config.heartbeat_interval_secs);
            gossip.set_owner_timeout(config.owner_timeout_secs);

            // Pre-create queues for owned topics
            for topic in &config.owned_topics {
                let queue = Arc::new(TopicQueue::new(config.buffer_size));
                topics.insert(topic.clone(), queue);

                // Join gossip for this topic (for discovery)
                gossip.join_topic(topic).await.map_err(|e| {
                    DdlError::Network(format!("Failed to join gossip for topic {}: {:?}", topic, e))
                })?;
            }

            // Set owned topics in gossip for discovery
            gossip.set_owned_topics(config.owned_topics.clone()).await;

            // Start gossip message handler
            let gossip = Arc::new(gossip);
            let gossip_clone = gossip.clone();
            let gossip_task_handle = tokio::spawn(async move {
                gossip_clone.handle_messages().await;
            });

            Some((gossip, gossip_task_handle))
        } else {
            // Standalone mode - no gossip
            None
        };

        // Initialize topic count from owned topics
        let topic_count = AtomicUsize::new(config.owned_topics.len());

        let (gossip, gossip_task_handle) = match gossip {
            Some((g, h)) => (Some(g), Some(h)),
            None => (None, None),
        };

        Ok(Self {
            config,
            topics,
            subscribers,
            raft_cluster,
            gossip,
            gossip_task_handle,
            topic_count,
        })
    }

    /// Create a Raft cluster node from configuration
    async fn create_raft_cluster(config: &DdlConfig) -> Result<Arc<RaftClusterNode>, DdlError> {
        // Build node configs from peers
        let mut nodes: HashMap<u64, NodeConfig> = HashMap::new();
        
        // Add self to node list
        nodes.insert(
            config.node_id,
            NodeConfig {
                node_id: config.node_id,
                host: "localhost".to_string(), // Would be configured in real deployment
                communication_port: 8080,
                coordination_port: 9090,
            },
        );

        // Add peers to node list
        for (i, peer) in config.peers.iter().enumerate() {
            let peer_node_id = (i + 2) as u64; // Start from 2 since node_id 1 is self
            nodes.insert(
                peer_node_id,
                NodeConfig {
                    node_id: peer_node_id,
                    host: peer.clone(),
                    communication_port: 8080,
                    coordination_port: 9090,
                },
            );
        }

        let raft = RaftClusterNode::new(config.node_id, nodes)
            .await
            .map_err(|e| DdlError::Network(format!("Failed to create Raft cluster: {}", e)))?;

        Ok(Arc::new(raft))
    }

    /// Get or create a topic queue (race-free using entry API and atomic counter)
    fn get_or_create_topic(&self, topic: &str) -> Result<Arc<TopicQueue>, DdlError> {
        // Fast path: check if topic already exists
        if let Some(queue) = self.topics.get(topic) {
            return Ok(queue.clone());
        }

        // Check topic limit using atomic counter (0 means unlimited)
        // Use Ordering::Relaxed for performance - strict ordering not needed
        if self.config.max_topics > 0 && self.topic_count.load(Ordering::Relaxed) >= self.config.max_topics {
            // Double-check with DashMap for race condition
            if let Some(queue) = self.topics.get(topic) {
                return Ok(queue.clone());
            }
            return Err(DdlError::TopicLimitExceeded {
                max: self.config.max_topics,
                current: self.topic_count.load(Ordering::Relaxed),
            });
        }

        // Use entry API for atomic check-and-insert
        // or_insert_with returns the value (either existing or newly created)
        // This handles the race where another thread creates the topic concurrently
        let result = self
            .topics
            .entry(topic.to_string())
            .or_insert_with(|| {
                // Increment counter on new topic creation (use Relaxed ordering for performance)
                self.topic_count.fetch_add(1, Ordering::Relaxed);
                Arc::new(TopicQueue::new(self.config.buffer_size))
            });

        Ok(result.clone())
    }

    /// Check if this node owns a topic (sync version)
    fn owns_topic_internal(&self, topic: &str) -> bool {
        // In standalone mode, we own all topics
        if self.gossip.is_none() && self.raft_cluster.is_none() {
            return true;
        }

        // Check config for statically owned topics
        self.config.owned_topics.contains(&topic.to_string())
    }

    /// Check if this node owns a topic (async version for distributed mode)
    pub async fn owns_topic_async(&self, topic: &str) -> bool {
        // In standalone mode, we own all topics
        if self.gossip.is_none() && self.raft_cluster.is_none() {
            return true;
        }

        // If Raft is enabled, check via Raft for strong consistency
        if let Some(raft) = &self.raft_cluster {
            return raft.owns_topic(topic).await;
        }

        // Check config first for statically owned topics
        if self.config.owned_topics.contains(&topic.to_string()) {
            return true;
        }

        // Check gossip for dynamic ownership (eventual consistency)
        if let Some(gossip) = &self.gossip {
            if let Some(owner) = gossip.get_topic_owner(topic).await {
                return owner == self.config.node_id;
            }
        }

        false
    }

    /// Claim ownership of a topic via Raft (distributed mode only)
    pub async fn claim_topic(&self, topic: &str) -> Result<(), DdlError> {
        if let Some(raft) = &self.raft_cluster {
            raft.claim_topic(topic)
                .await
                .map_err(|e| DdlError::Network(format!("Raft claim failed: {}", e)))
        } else {
            Err(DdlError::Network("Not in distributed Raft mode".to_string()))
        }
    }

    /// Release ownership of a topic via Raft (distributed mode only)
    pub async fn release_topic(&self, topic: &str) -> Result<(), DdlError> {
        if let Some(raft) = &self.raft_cluster {
            raft.release_topic(topic)
                .await
                .map_err(|e| DdlError::Network(format!("Raft release failed: {}", e)))
        } else {
            Err(DdlError::Network("Not in distributed Raft mode".to_string()))
        }
    }

    /// Get the owner of a topic (linearizable read)
    pub async fn get_topic_owner(&self, topic: &str) -> Result<Option<u64>, DdlError> {
        if let Some(raft) = &self.raft_cluster {
            raft.get_owner(topic)
                .await
                .map_err(|e| DdlError::Network(format!("Raft query failed: {}", e)))
        } else if let Some(gossip) = &self.gossip {
            Ok(gossip.get_topic_owner(topic).await)
        } else {
            // Standalone mode - we own all topics
            Ok(Some(self.config.node_id))
        }
    }

    /// Check if this node is the Raft leader
    pub async fn is_leader(&self) -> bool {
        if let Some(raft) = &self.raft_cluster {
            raft.is_leader().await
        } else {
            // No Raft means standalone or gossip mode
            true
        }
    }

    /// Shutdown the DDL instance and all associated tasks
    pub async fn shutdown(&self) {
        // Release owned topics via Raft
        if let Some(raft) = &self.raft_cluster {
            for topic in &self.config.owned_topics {
                let _ = raft.release_topic(topic).await;
            }
        }
        
        // Shutdown gossip
        if let Some(gossip) = &self.gossip {
            gossip.shutdown().await;
        }
    }

    /// Check if running in standalone mode (no gossip, no Raft)
    pub fn is_standalone(&self) -> bool {
        self.gossip.is_none() && self.raft_cluster.is_none()
    }

    /// Check if running in distributed mode
    pub fn is_distributed(&self) -> bool {
        self.gossip.is_some() || self.raft_cluster.is_some()
    }
    
    /// Check if running in Raft mode (strongly consistent ownership)
    pub fn is_raft_enabled(&self) -> bool {
        self.raft_cluster.is_some()
    }

    /// Subscribe to membership events (Raft mode only)
    ///
    /// Returns a receiver that yields membership events when nodes
    /// join, leave, or fail in the cluster.
    ///
    /// Returns `None` if not in Raft mode (standalone or gossip-only).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut events = ddl.subscribe_membership().await?;
    /// while let Ok(event) = events.recv().await {
    ///     match event.event_type {
    ///         MembershipEventType::NodeFailed { node_id } => {
    ///             // Handle failover
    ///         }
    ///         MembershipEventType::NodeJoined { node_id, addr } => {
    ///             // New node joined
    ///         }
    ///         // ...
    ///     }
    /// }
    /// ```
    pub fn subscribe_membership(&self) -> Option<tokio::sync::broadcast::Receiver<MembershipEvent>> {
        self.raft_cluster.as_ref().map(|rc| rc.subscribe_membership())
    }

    /// Get current cluster membership view (Raft mode only)
    ///
    /// Returns information about all known nodes in the cluster.
    ///
    /// Returns `None` if not in Raft mode (standalone or gossip-only).
    pub fn membership(&self) -> Option<crate::cluster::MembershipView> {
        self.raft_cluster.as_ref().map(|rc| rc.membership())
    }

    /// Get DDL metrics for monitoring (Raft mode only)
    ///
    /// Returns metrics about the cluster state including:
    /// - Active leases
    /// - Membership size
    /// - Raft commit index
    /// - Current leader
    ///
    /// Returns `None` if not in Raft mode (standalone or gossip-only).
    pub fn metrics(&self) -> Option<crate::cluster::DdlMetrics> {
        self.raft_cluster.as_ref().map(|rc| rc.metrics())
    }
}

#[async_trait]
impl DDL for DdlDistributed {
    async fn push(&self, topic: &str, payload: Vec<u8>) -> Result<u64, DdlError> {
        // Validate topic name (defense-in-depth)
        validate_topic(topic)?;

        // Check ownership in distributed mode
        if !self.owns_topic_async(topic).await {
            return Err(DdlError::NotOwner(topic.to_string()));
        }

        // Convert topic to Arc<str> once for zero-copy sharing
        let topic_arc: Arc<str> = topic.into();

        let queue = self.get_or_create_topic(topic)?;
        let id = queue.push(topic_arc, payload)?;

        // Get entry to send to subscribers
        if let Some(entry) = queue.read(id) {
            // Use shared notification function
            notify_subscribers(
                &self.subscribers,
                topic,
                &entry,
                self.config.subscription_backpressure,
            )?;
        }

        Ok(id)
    }

    async fn push_batch(&self, topic: &str, payloads: Vec<Vec<u8>>) -> Result<u64, DdlError> {
        // Validate topic name (defense-in-depth)
        validate_topic(topic)?;

        if payloads.is_empty() {
            return Err(DdlError::BufferFull(topic.to_string()));
        }

        // Check ownership once (not per-payload!)
        if !self.owns_topic_async(topic).await {
            return Err(DdlError::NotOwner(topic.to_string()));
        }

        // Convert topic to Arc<str> once for zero-copy sharing
        let topic_arc: Arc<str> = topic.into();

        let queue = self.get_or_create_topic(topic)?;
        let mut last_id = 0;

        // Push all entries and notify subscribers
        for payload in payloads {
            let id = queue.push(topic_arc.clone(), payload)?;
            last_id = id;

            // Get entry and send to subscribers
            if let Some(entry) = queue.read(id) {
                notify_subscribers(
                    &self.subscribers,
                    topic,
                    &entry,
                    self.config.subscription_backpressure,
                )?;
            }
        }

        Ok(last_id)
    }

    async fn subscribe(&self, topic: &str) -> Result<EntryStream, DdlError> {
        // Validate topic name (defense-in-depth)
        validate_topic(topic)?;

        // Ensure topic exists (create if needed)
        self.get_or_create_topic(topic)?;

        let buffer_size = self.config.subscription_buffer_size;
        let (sender, receiver) = bounded(buffer_size);

        // Add to subscribers with metadata
        // Use created_at only in distributed mode for tracking subscriber age
        let created_at = if self.gossip.is_some() {
            Some(Instant::now())
        } else {
            None
        };

        self.subscribers
            .entry(topic.to_string())
            .or_insert_with(Vec::new)
            .push(SubscriberInfo {
                sender,
                created_at,
            });

        // Create stream with backpressure mode
        Ok(EntryStream::with_backpressure(
            receiver,
            self.config.subscription_backpressure,
        ))
    }

    async fn ack(&self, topic: &str, entry_id: u64) -> Result<(), DdlError> {
        if let Some(queue) = self.topics.get(topic) {
            queue.ack(entry_id);
            Ok(())
        } else {
            Err(DdlError::TopicNotFound(topic.to_string()))
        }
    }

    async fn position(&self, topic: &str) -> Result<u64, DdlError> {
        if let Some(queue) = self.topics.get(topic) {
            Ok(queue.position())
        } else {
            Err(DdlError::TopicNotFound(topic.to_string()))
        }
    }

    fn owns_topic(&self, topic: &str) -> bool {
        self.owns_topic_internal(topic)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ddl::BackpressureMode;

    #[tokio::test]
    async fn test_standalone_ddl_creation() {
        let config = DdlConfig::default();
        let ddl = DdlDistributed::new_standalone(config);

        assert!(ddl.is_standalone());
        assert!(!ddl.is_distributed());
        assert!(ddl.owns_topic("any_topic")); // Standalone owns all topics
    }

    #[tokio::test]
    async fn test_standalone_push_and_subscribe() {
        let ddl = DdlDistributed::new_standalone(DdlConfig::default());

        // Subscribe first
        let mut stream = ddl.subscribe("test").await.unwrap();

        // Push data
        let id = ddl.push("test", vec![1, 2, 3]).await.unwrap();
        assert_eq!(id, 0);

        // Receive
        let entry = stream.next().unwrap();
        assert_eq!(entry.id, 0);
        assert_eq!(&*entry.payload, &[1, 2, 3]);
    }

    #[tokio::test]
    async fn test_standalone_ack() {
        let ddl = DdlDistributed::new_standalone(DdlConfig::default());

        let id = ddl.push("test", vec![1]).await.unwrap();
        ddl.ack("test", id).await.unwrap();

        // Position should advance
        let pos = ddl.position("test").await.unwrap();
        assert!(pos >= 1);
    }

    #[tokio::test]
    async fn test_standalone_batch_push() {
        let ddl = DdlDistributed::new_standalone(DdlConfig::default());
        let mut stream = ddl.subscribe("test").await.unwrap();

        // Push batch
        let last_id = ddl
            .push_batch("test", vec![vec![1], vec![2], vec![3]])
            .await
            .unwrap();

        assert_eq!(last_id, 2); // IDs are 0, 1, 2

        // Should receive all entries
        for i in 0..3 {
            let entry = stream.next().unwrap();
            assert_eq!(entry.id, i);
            assert_eq!(&*entry.payload, &[i as u8 + 1]);
        }
    }

    #[tokio::test]
    async fn test_standalone_drop_oldest_backpressure() {
        // Configure with DropOldest (default) and small buffer
        let mut config = DdlConfig::default();
        config.subscription_buffer_size = 2;
        config.subscription_backpressure = BackpressureMode::DropOldest;

        let ddl = DdlDistributed::new_standalone(config);
        let stream = ddl.subscribe("test").await.unwrap();

        // Push more messages than buffer can hold
        for i in 0..5 {
            ddl.push("test", vec![i]).await.unwrap();
        }

        // Should be able to read some messages
        let mut count = 0;
        while let Some(_) = stream.try_next() {
            count += 1;
        }

        // We should get at most 2 messages (buffer size)
        assert!(count <= 2);
    }

    #[tokio::test]
    async fn test_standalone_error_backpressure() {
        let mut config = DdlConfig::default();
        config.subscription_buffer_size = 2;
        config.subscription_backpressure = BackpressureMode::Error;

        let ddl = DdlDistributed::new_standalone(config);
        let _stream = ddl.subscribe("test").await.unwrap();

        // Fill the buffer
        ddl.push("test", vec![1]).await.unwrap();
        ddl.push("test", vec![2]).await.unwrap();

        // Third push should fail with buffer full error
        let result = ddl.push("test", vec![3]).await;
        assert!(result.is_err());
        match result {
            Err(DdlError::SubscriberBufferFull(topic)) => assert_eq!(topic, "test"),
            _ => panic!("Expected SubscriberBufferFull error"),
        }
    }

    #[tokio::test]
    async fn test_standalone_drop_newest_backpressure() {
        let mut config = DdlConfig::default();
        config.subscription_buffer_size = 2;
        config.subscription_backpressure = BackpressureMode::DropNewest;

        let ddl = DdlDistributed::new_standalone(config);
        let _stream = ddl.subscribe("test").await.unwrap();

        // Push messages - should succeed without errors
        for i in 0..5 {
            ddl.push("test", vec![i]).await.unwrap();
        }

        // No assertions needed - just verifying no errors are thrown
        // In DropNewest mode, when buffer is full, messages are silently dropped
    }

    #[tokio::test]
    async fn test_standalone_block_backpressure_slow_consumer() {
        let mut config = DdlConfig::default();
        config.subscription_buffer_size = 1;
        config.subscription_backpressure = BackpressureMode::Block;

        let ddl = DdlDistributed::new_standalone(config);
        let stream = ddl.subscribe("test").await.unwrap();

        // Push one message - should succeed immediately
        ddl.push("test", vec![1]).await.unwrap();

        // Buffer is now full (size 1)
        // Next push should block in Block mode, but we can't easily test blocking
        // in a unit test without causing deadlock. We'll just verify the config works.

        let entry = stream.try_next().unwrap();
        assert_eq!(&*entry.payload, &[1]);
    }

    #[tokio::test]
    async fn test_standalone_multiple_subscribers_same_topic() {
        let ddl = DdlDistributed::new_standalone(DdlConfig::default());

        let stream1 = ddl.subscribe("test").await.unwrap();
        let stream2 = ddl.subscribe("test").await.unwrap();

        ddl.push("test", vec![42]).await.unwrap();

        // Both subscribers should receive the message
        let entry1 = stream1.try_next().unwrap();
        let entry2 = stream2.try_next().unwrap();

        assert_eq!(&*entry1.payload, &[42]);
        assert_eq!(&*entry2.payload, &[42]);
    }

    #[tokio::test]
    async fn test_standalone_backpressure_mode_in_stream() {
        let mut config = DdlConfig::default();
        config.subscription_backpressure = BackpressureMode::Block;

        let ddl = DdlDistributed::new_standalone(config);
        let stream = ddl.subscribe("test").await.unwrap();

        assert_eq!(stream.backpressure_mode(), BackpressureMode::Block);

        let mut config2 = DdlConfig::default();
        config2.subscription_backpressure = BackpressureMode::Error;

        let ddl2 = DdlDistributed::new_standalone(config2);
        let stream2 = ddl2.subscribe("test").await.unwrap();

        assert_eq!(stream2.backpressure_mode(), BackpressureMode::Error);
    }

    #[tokio::test]
    async fn test_standalone_owns_all_topics() {
        let ddl = DdlDistributed::new_standalone(DdlConfig::default());

        // In standalone mode, we own all topics
        assert!(ddl.owns_topic("any_topic"));
        assert!(ddl.owns_topic("metrics.cpu"));
        assert!(ddl.owns_topic("logs.app"));

        // Async version should also return true
        assert!(ddl.owns_topic_async("another_topic").await);
    }

    #[tokio::test]
    async fn test_distributed_ddl_creation() {
        let mut config = DdlConfig::default();
        config.gossip_enabled = true;
        config.gossip_bind_addr = "127.0.0.1:0".to_string();
        config.owned_topics = vec!["test".to_string()];

        let ddl = DdlDistributed::new_distributed(config)
            .await
            .expect("Failed to create distributed DDL");

        assert!(ddl.is_distributed());
        assert!(!ddl.is_standalone());
        assert!(ddl.owns_topic("test"));
        assert!(!ddl.owns_topic("other"));
    }

    #[tokio::test]
    async fn test_standalone_with_owned_topics() {
        let mut config = DdlConfig::default();
        config.owned_topics = vec!["topic1".to_string(), "topic2".to_string()];

        let ddl = DdlDistributed::new_standalone(config);

        // Even in standalone mode with owned_topics set, we own all topics
        assert!(ddl.owns_topic("topic1"));
        assert!(ddl.owns_topic("topic2"));
        assert!(ddl.owns_topic("other_topic")); // Still owns all in standalone
    }

    #[tokio::test]
    async fn test_standalone_topic_limit() {
        let mut config = DdlConfig::default();
        config.max_topics = 2;

        let ddl = DdlDistributed::new_standalone(config);

        // Create two topics - should succeed
        ddl.push("topic1", vec![1]).await.unwrap();
        ddl.push("topic2", vec![2]).await.unwrap();

        // Third topic should fail
        let result = ddl.push("topic3", vec![3]).await;
        match result {
            Err(DdlError::TopicLimitExceeded { max, current }) => {
                assert_eq!(max, 2);
                assert!(current >= 2);
            }
            _ => panic!("Expected TopicLimitExceeded error, got {:?}", result),
        }
    }
}