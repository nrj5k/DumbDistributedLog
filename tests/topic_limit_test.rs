//! Test topic limit functionality

use ddl::traits::ddl::{DDL, DdlConfig, DdlError};
use ddl::Ddl;

#[tokio::test]
async fn test_topic_limit_exceeded() {
    // Create config with a small topic limit
    let mut config = DdlConfig::default();
    config.max_topics = 2; // Only allow 2 topics
    
    let ddl = Ddl::new(config);
    
    // Create first topic - should succeed
    let result1 = ddl.push("topic1", vec![1, 2, 3]).await;
    assert!(result1.is_ok());
    
    // Create second topic - should succeed
    let result2 = ddl.push("topic2", vec![4, 5, 6]).await;
    assert!(result2.is_ok());
    
    // Try to create third topic - should fail with TopicLimitExceeded
    let result3 = ddl.push("topic3", vec![7, 8, 9]).await;
    assert!(result3.is_err());
    
    match result3.unwrap_err() {
        DdlError::TopicLimitExceeded { max, current } => {
            assert_eq!(max, 2);
            assert_eq!(current, 2);
        }
        _ => panic!("Expected TopicLimitExceeded error"),
    }
}

#[tokio::test]
async fn test_topic_limit_zero_means_unlimited() {
    // Create config with max_topics = 0 (unlimited)
    let mut config = DdlConfig::default();
    config.max_topics = 0; // Unlimited topics
    
    let ddl = Ddl::new(config);
    
    // Create many topics - should all succeed
    for i in 0..100 {
        let topic_name = format!("topic{}", i);
        let result = ddl.push(&topic_name, vec![i as u8]).await;
        assert!(result.is_ok(), "Failed to create topic {}", topic_name);
    }
}

#[tokio::test]
async fn test_existing_topic_still_works_after_limit() {
    // Create config with a small topic limit
    let mut config = DdlConfig::default();
    config.max_topics = 1; // Only allow 1 topic
    
    let ddl = Ddl::new(config);
    
    // Create the one allowed topic
    let result1 = ddl.push("allowed_topic", vec![1, 2, 3]).await;
    assert!(result1.is_ok());
    
    // Try to create another topic - should fail
    let result2 = ddl.push("blocked_topic", vec![4, 5, 6]).await;
    assert!(result2.is_err());
    
    // But accessing the existing topic should still work
    let result3 = ddl.push("allowed_topic", vec![7, 8, 9]).await;
    assert!(result3.is_ok());
    
    let position = ddl.position("allowed_topic").await;
    assert!(position.is_ok());
}

// Note: We can't easily test DdlDistributed in isolation without setting up
// a full gossip network, so we rely on the fact that both Ddl and DdlDistributed
// share the same underlying get_or_create_topic implementation that we've tested.