#[cfg(test)]
mod helper_tests {
    use super::*;
    use crate::types::{QueueConfig, QueueType};
    use crate::enums::{Mode, Model, QueueValue};
    use std::sync::Arc;

    /// Test processor for our examples
    fn test_processor(data: Vec<i32>) -> Result<i32, QueueError> {
        if data.is_empty() {
            return Ok(50);
        }
        let avg = data.iter().sum::<i32>() / data.len() as i32;
        Ok(avg)
    }

    #[tokio::test]
    async fn test_get_latest_value_empty() {
        let queue_type = QueueType::new(
            QueueValue::NodeLoad,
            100,
            2,
            "test.log".to_string(),
            "test_var".to_string(),
        );

        let config = QueueConfig::new(
            Mode::Sensor,
            Arc::new(test_processor),
            Model::Linear,
            queue_type,
        );

        let queue = Queue::new(config);
        
        // Test empty queue
        let latest = queue.get_latest_value().await;
        assert!(latest.is_none());
    }

    #[tokio::test] 
    async fn test_get_latest_value_with_data() {
        let queue_type = QueueType::new(
            QueueValue::NodeLoad,
            100,
            2,
            "test.log".to_string(),
            "test_var".to_string(),
        );

        let config = QueueConfig::new(
            Mode::Sensor,
            Arc::new(test_processor),
            Model::Linear,
            queue_type,
        );

        let mut queue = Queue::new(config);
        
        // Add test data
        queue.publish(42).await.unwrap();
        queue.publish(55).await.unwrap();
        queue.publish(67).await.unwrap();
        
        // Test latest value (should be 67)
        let latest = queue.get_latest_value().await;
        assert_eq!(latest, Some(67));
    }

    #[tokio::test]
    async fn test_get_latest_n_values_empty() {
        let queue_type = QueueType::new(
            QueueValue::NodeLoad,
            100,
            2,
            "test.log".to_string(),
            "test_var".to_string(),
        );

        let config = QueueConfig::new(
            Mode::Sensor,
            Arc::new(test_processor),
            Model::Linear,
            queue_type,
        );

        let queue = Queue::new(config);
        
        // Test empty queue with various N values
        assert_eq!(queue.get_latest_n_values(0).await, Vec::<i32>::new());
        assert_eq!(queue.get_latest_n_values(1).await, Vec::<i32>::new());
        assert_eq!(queue.get_latest_n_values(10).await, Vec::<i32>::new());
    }

    #[tokio::test]
    async fn test_get_latest_n_values_with_data() {
        let queue_type = QueueType::new(
            QueueValue::NodeLoad,
            100,
            2,
            "test.log".to_string(),
            "test_var".to_string(),
        );

        let config = QueueConfig::new(
            Mode::Sensor,
            Arc::new(test_processor),
            Model::Linear,
            queue_type,
        );

        let mut queue = Queue::new(config);
        
        // Add test data: [10, 20, 30, 40, 50]
        for i in 1..=5 {
            queue.publish(i * 10).await.unwrap();
        }
        
        // Test various N values
        assert_eq!(queue.get_latest_n_values(1).await, vec![50]);
        assert_eq!(queue.get_latest_n_values(3).await, vec![50, 40, 30]);
        assert_eq!(queue.get_latest_n_values(5).await, vec![50, 40, 30, 20, 10]);
        assert_eq!(queue.get_latest_n_values(10).await, vec![50, 40, 30, 20, 10]); // N > data
    }

    #[tokio::test]
    async fn test_has_data_empty() {
        let queue_type = QueueType::new(
            QueueValue::NodeLoad,
            100,
            2,
            "test.log".to_string(),
            "test_var".to_string(),
        );

        let config = QueueConfig::new(
            Mode::Sensor,
            Arc::new(test_processor),
            Model::Linear,
            queue_type,
        );

        let queue = Queue::new(config);
        
        assert!(!queue.has_data().await);
    }

    #[tokio::test]
    async fn test_has_data_with_data() {
        let queue_type = QueueType::new(
            QueueValue::NodeLoad,
            100,
            2,
            "test.log".to_string(),
            "test_var".to_string(),
        );

        let config = QueueConfig::new(
            Mode::Sensor,
            Arc::new(test_processor),
            Model::Linear,
            queue_type,
        );

        let mut queue = Queue::new(config);
        
        assert!(!queue.has_data().await);
        
        queue.publish(42).await.unwrap();
        assert!(queue.has_data().await);
        
        queue.publish(55).await.unwrap();
        assert!(queue.has_data().await);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let queue_type = QueueType::new(
            QueueValue::NodeLoad,
            100,
            2,
            "test.log".to_string(),
            "test_var".to_string(),
        );

        let config = QueueConfig::new(
            Mode::Sensor,
            Arc::new(test_processor),
            Model::Linear,
            queue_type,
        );

        let mut queue = Queue::new(config);
        
        // Setup concurrent access
        let queue_clone = Arc::new(Mutex::new(queue));
        
        // Spawn multiple tasks to access queue concurrently
        let mut handles = vec![];
        
        for i in 0..10 {
            let queue = queue_clone.clone();
            let handle = tokio::spawn(async move {
                let mut queue = queue.lock().await;
                queue.publish(i).await.unwrap();
                
                // Check various helper methods
                let latest = queue.get_latest_value().await;
                let recent = queue.get_latest_n_values(3).await;
                let has_any = queue.has_data().await;
                
                (latest, recent.len(), has_any)
            });
            handles.push(handle);
        }
        
        // Wait for all tasks and verify results
        let results = futures::future::join_all(handles).await;
        
        for result in results {
            let (latest, count, has_any) = result.unwrap();
            assert!(latest.is_some());
            assert!(count > 0);
            assert!(has_any);
        }
    }
}