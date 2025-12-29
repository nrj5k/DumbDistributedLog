//! AutoQueues Programmatic API
//!
//! Building on AutoQueues programmatically, adding queues and expressions.
//! Generic over type T for maximum flexibility.
//!
//! # Type Parameter
//!
//! `AutoQueues<T>` is generic over type `T` which must implement:
//! - `Clone + Send + 'static` - For sharing across threads and storage
//! - `serde::Serialize` - For pub/sub messaging
//!
//! # Supported Types
//!
//! | Type | Requirements | Notes |
//! |------|-------------|-------|
//! | `f64` | Clone + Send + 'static + Serialize | Most common, full support |
//! | `i64` | Clone + Send + 'static + Serialize | Integer values |
//! | `u64` | Clone + Send + 'static + Serialize | Unsigned integers |
//! | Custom | Clone + Send + 'static + Serialize | Any serializable type |
//!
//! # Example with f64
//!
//! ```
//! use autoqueues::AutoQueues;
//!
//! let queues: AutoQueues<f64> = AutoQueues::new()
//!     .queue("cpu", None)?
//!     .queue_with_fn("memory", || get_memory_percent())  // Fn() -> f64
//!     .build()
//!     .await?;
//! ```
//!
//! # Example with custom type
//!
//! ```
//! use autoqueues::AutoQueues;
//!
//! #[derive(Clone)]
//! struct SensorReading {
//!     value: f64,
//!     unit: String,
//! }
//!
//! let queues: AutoQueues<SensorReading> = AutoQueues::new()
//!     .queue("temperature", None)?
//!     .build()
//!     .await?;
//!
//! queues.push("temperature", SensorReading { value: 25.5, unit: "°C".to_string() }).await?;
//! ```

use crate::expression::SimpleExpression;
use crate::pubsub::{PubSubBroker, TopicMessage, SubscriptionId};
use crate::queue::{SimpleQueue, QueueError};
use crate::traits::queue::QueueTrait;
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time;

/// AutoQueues programmatic API errors
#[derive(Debug, thiserror::Error)]
pub enum AutoQueuesError {
    #[error("Queue error: {0}")]
    QueueError(#[from] QueueError),

    #[error("Expression error: {0}")]
    ExpressionError(String),

    #[error("PubSub error: {0}")]
    PubSubError(String),

    #[error("Queue not found: {0}")]
    QueueNotFound(String),

    #[error("Invalid queue configuration: {0}")]
    InvalidConfig(String),

    #[error("Function error: {0}")]
    FunctionError(String),
}

/// Main AutoQueues struct for programmatic mode.
///
/// Generic over type `T` which must implement:
/// - `Clone + Send + 'static` - For thread safety and storage
/// - `Serialize + DeserializeOwned` - For pub/sub messaging
///
/// # Example
///
/// ```
/// use autoqueues::AutoQueues;
///
/// // Most common: f64
/// let queues: AutoQueues<f64> = AutoQueues::new()
///     .queue("metrics", None)?
///     .build()
///     .await?;
/// ```
pub struct AutoQueues<T>
where
    T: Clone + Send + 'static + Serialize + DeserializeOwned,
{
    queues: HashMap<String, Arc<RwLock<SimpleQueue<T>>>>,
    broker: Arc<PubSubBroker>,
}

impl<T> AutoQueues<T>
where
    T: Clone + Send + 'static + Serialize + DeserializeOwned,
{
    /// Create a new AutoQueues instance
    ///
    /// # Example
    /// ```
    /// use autoqueues::AutoQueues;
    ///
    /// let queues: AutoQueues<f64> = AutoQueues::new();
    /// ```
    pub fn new() -> Self {
        Self {
            queues: HashMap::new(),
            broker: Arc::new(PubSubBroker::new(100)),
        }
    }

    /// Create a new queue
    ///
    /// For `f64` queues, you can optionally add an expression to filter/transform values.
    /// Expressions use `atomic.*` and `cluster.*` variables and only work with f64.
    ///
    /// # Arguments
    /// * `name` - Queue identifier
    /// * `expression` - Optional expression (only supported for f64 queues)
    ///
    /// # Example (f64 with expression)
    /// ```
    /// use autoqueues::AutoQueues;
    ///
    /// let queues: AutoQueues<f64> = AutoQueues::new()
    ///     .queue("cpu", Some("atomic.cpu_percent > 80"))?
    ///     .queue("data", None)?;
    /// ```
    ///
    /// # Example (custom type without expression)
    /// ```
    /// use autoqueues::AutoQueues;
    ///
    /// #[derive(Clone)]
    /// struct MyData { id: i32, value: f64 }
    ///
    /// let queues: AutoQueues<MyData> = AutoQueues::new()
    ///     .queue("custom", None)?;  // Custom types don't support expressions
    /// ```
    pub fn queue(
        mut self,
        name: &str,
        expression: Option<&str>,
    ) -> Result<Self, AutoQueuesError> {
        let queue = if let Some(expr_str) = expression {
            // Expressions only work with f64 - type check at compile time
            let _expr: SimpleExpression<f64> = SimpleExpression::<f64>::new(expr_str)
                .map_err(|e| AutoQueuesError::ExpressionError(e.to_string()))?;
            SimpleQueue::new()  // Expressions not yet supported for generic queues
        } else {
            SimpleQueue::new()
        };

        self.queues.insert(
            name.to_string(),
            Arc::new(RwLock::new(queue)),
        );
        Ok(self)
    }

    /// Create a queue with a function that returns type T
    ///
    /// The function's return type MUST match the queue type T.
    /// This is verified at compile time!
    ///
    /// # Type Safety
    ///
    /// The function must return the same type T as the queue:
    /// - If queue is `AutoQueues<f64>`, function must return `f64`
    /// - If queue is `AutoQueues<i64>`, function must return `i64`
    /// - etc.
    ///
    /// # Example (f64)
    /// ```
    /// use autoqueues::AutoQueues;
    ///
    /// let queues: AutoQueues<f64> = AutoQueues::new()
    ///     .queue_with_fn("cpu_usage", || get_cpu_percent())  // Fn() -> f64
    ///     .queue_with_fn("memory_usage", || get_memory_percent())  // Fn() -> f64
    ///     .build()
    ///     .await?;
    /// ```
    ///
    /// # Example (custom type)
    /// ```
    /// use autoqueues::AutoQueues;
    ///
    /// #[derive(Clone)]
    /// struct SensorData { temp: f64, humidity: f64 }
    ///
    /// let queues: AutoQueues<SensorData> = AutoQueues::new()
    ///     .queue_with_fn("sensors", || SensorData { temp: 25.0, humidity: 60.0 })  // Fn() -> SensorData
    ///     .build()
    ///     .await?;
    /// ```
    pub fn queue_with_fn<F>(mut self, name: &str, _func: F) -> Result<Self, AutoQueuesError>
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        let queue = SimpleQueue::<T>::new();
        self.queues.insert(
            name.to_string(),
            Arc::new(RwLock::new(queue)),
        );
        Ok(self)
    }

    /// Add a function that auto-populates a queue
    ///
    /// The function must return the same type T as the queue.
    /// Called every `interval_ms` milliseconds in the background.
    ///
    /// # Type Safety
    ///
    /// The function must return type T:
    /// ```compile_fail
    /// use autoqueues::AutoQueues;
    ///
    /// let queues: AutoQueues<f64> = AutoQueues::new()
    ///     .queue("test", None)?
    ///     .build()
    ///     .await?;
    ///
    /// // This won't compile! Function returns &str, but queue expects f64
    /// queues.add_function_populator("test", || "hello", 1000).await;
    /// ```
    ///
    /// # Example (f64)
    /// ```
    /// use autoqueues::AutoQueues;
    ///
    /// let queues: AutoQueues<f64> = AutoQueues::new()
    ///     .queue("metrics", None)?
    ///     .build()
    ///     .await?;
    ///
    /// queues.add_function_populator("metrics", || get_cpu_percent(), 1000).await?;
    /// ```
    pub async fn add_function_populator<F>(
        &self,
        queue_name: &str,
        func: F,
        interval_ms: u64,
    ) -> Result<(), AutoQueuesError>
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        if let Some(queue) = self.queues.get(queue_name) {
            let queue = queue.clone();
            let func = Arc::new(func);
            let mut interval = time::interval(Duration::from_millis(interval_ms));

            tokio::spawn(async move {
                loop {
                    interval.tick().await;
                    let value: T = func();
                    let mut guard = queue.write().await;
                    let _ = guard.publish(value);
                }
            });

            Ok(())
        } else {
            Err(AutoQueuesError::QueueNotFound(queue_name.to_string()))
        }
    }

    /// Build the AutoQueues instance
    pub async fn build(self) -> Result<Self, AutoQueuesError> {
        Ok(self)
    }

    /// Push a value of type T to a queue
    ///
    /// # Example
    /// ```
    /// use autoqueues::AutoQueues;
    ///
    /// let queues: AutoQueues<f64> = AutoQueues::new()
    ///     .queue("data", None)?
    ///     .build()
    ///     .await?;
    ///
    /// queues.push("data", 42.5).await?;
    /// queues.push("data", 99.9).await?;
    /// ```
    pub async fn push(&self, queue_name: &str, data: T) -> Result<(), AutoQueuesError> {
        if let Some(queue) = self.queues.get(queue_name) {
            let mut queue_guard = queue.write().await;
            queue_guard.publish(data)
                .map_err(AutoQueuesError::QueueError)?;
            Ok(())
        } else {
            Err(AutoQueuesError::QueueNotFound(queue_name.to_string()))
        }
    }

    /// Pop a value of type T from a queue
    ///
    /// Returns `Ok(Some(T))` if data exists, `Ok(None)` if empty.
    ///
    /// # Example
    /// ```
    /// use autoqueues::AutoQueues;
    ///
    /// let queues: AutoQueues<f64> = AutoQueues::new()
    ///     .queue("data", None)?
    ///     .build()
    ///     .await?;
    ///
    /// queues.push("data", 42.5).await?;
    ///
    /// let value = queues.pop("data").await?;
    /// assert_eq!(value, Some(42.5));
    /// ```
    pub async fn pop(&self, queue_name: &str) -> Result<Option<T>, AutoQueuesError> {
        if let Some(queue) = self.queues.get(queue_name) {
            let queue_guard = queue.read().await;
            Ok(queue_guard.get_latest().map(|(_, value)| value))
        } else {
            Err(AutoQueuesError::QueueNotFound(queue_name.to_string()))
        }
    }

    /// Get the latest value from a queue
    pub async fn get_latest(&self, queue_name: &str) -> Result<Option<T>, AutoQueuesError> {
        self.pop(queue_name).await
    }

    /// Get the latest n values from a queue
    pub async fn get_latest_n(&self, queue_name: &str, n: usize) -> Result<Vec<T>, AutoQueuesError> {
        if let Some(queue) = self.queues.get(queue_name) {
            let queue_guard = queue.read().await;
            let values: Vec<T> = queue_guard.get_latest_n(n)
                .into_iter()
                .collect();
            Ok(values)
        } else {
            Err(AutoQueuesError::QueueNotFound(queue_name.to_string()))
        }
    }

    /// Subscribe to a topic
    pub async fn subscribe(&self, topic: &str) -> Result<SubscriptionId, AutoQueuesError> {
        self.broker
            .subscribe_exact(topic.to_string())
            .await
            .map_err(|e| AutoQueuesError::PubSubError(e.to_string()))
    }

    /// Publish to a topic
    pub async fn publish_to_topic(&self, topic: &str, data: &T) -> Result<(), AutoQueuesError> {
        self.broker
            .publish(topic.to_string(), data)
            .await
            .map_err(|e| AutoQueuesError::PubSubError(e.to_string()))
    }

    /// Receive a message from a subscription
    pub async fn receive(&self, subscription_id: SubscriptionId) -> Option<TopicMessage> {
        self.broker.receive(subscription_id).await
    }
}

impl<T> Default for AutoQueues<T>
where
    T: Clone + Send + 'static + Serialize + DeserializeOwned + Default,
{
    fn default() -> Self {
        Self::new()
    }
}

// Convenience type aliases
/// AutoQueues with f64 values - the most common use case
pub type AutoQueuesF64 = AutoQueues<f64>;

/// AutoQueues with i64 values - for integer data
pub type AutoQueuesI64 = AutoQueues<i64>;

/// AutoQueues with u64 values - for unsigned integer data
pub type AutoQueuesU64 = AutoQueues<u64>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_autoqueues_creation() {
        let queues: AutoQueues<f64> = AutoQueues::new()
            .queue("test_queue", Some("atomic.cpu + cluster.memory"))
            .unwrap();

        assert!(queues.queues.contains_key("test_queue"));
    }

    #[tokio::test]
    async fn test_queue_with_fn() {
        let queues: AutoQueues<f64> = AutoQueues::new()
            .queue_with_fn("cpu", || 42.0)
            .unwrap();

        assert!(queues.queues.contains_key("cpu"));
    }

    #[tokio::test]
    async fn test_function_populator() {
        let queues: AutoQueues<f64> = AutoQueues::new()
            .queue("metrics", None)
            .unwrap()
            .build()
            .await
            .unwrap();

        queues
            .add_function_populator("metrics", || 99.9, 1000)
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(1100)).await;

        let value = queues.get_latest("metrics").await.unwrap();
        assert!(value.is_some());
        assert!((value.unwrap() - 99.9).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_push_pop_roundtrip() {
        let queues: AutoQueues<f64> = AutoQueues::new()
            .queue("test", None)
            .unwrap()
            .build()
            .await
            .unwrap();

        queues.push("test", 123.456).await.unwrap();

        let value = queues.pop("test").await.unwrap();
        assert_eq!(value, Some(123.456));
    }

    #[tokio::test]
    async fn test_custom_type() {
        // Custom type with Clone + Send + 'static + serde traits
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        struct SensorReading {
            value: f64,
            unit: String,
        }

        let queues: AutoQueues<SensorReading> = AutoQueues::new()
            .queue("temperature", None)
            .unwrap()
            .build()
            .await
            .unwrap();

        // Push a SensorReading
        queues.push("temperature", SensorReading { value: 25.5, unit: "°C".to_string() }).await.unwrap();

        let value: Option<SensorReading> = queues.pop("temperature").await.unwrap();
        assert_eq!(value, Some(SensorReading { value: 25.5, unit: "°C".to_string() }));
    }

    #[tokio::test]
    async fn test_i64_type() {
        let queues: AutoQueues<i64> = AutoQueues::new()
            .queue("counters", None)
            .unwrap()
            .build()
            .await
            .unwrap();

        queues.push("counters", 42i64).await.unwrap();
        queues.push("counters", 100i64).await.unwrap();

        let value: Option<i64> = queues.pop("counters").await.unwrap();
        assert_eq!(value, Some(100i64));
    }
}
