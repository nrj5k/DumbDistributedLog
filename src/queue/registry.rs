//! Queue registry implementation
//!
//! Manages queues and their sources in a centralized registry.

use crate::autoqueues_config::Config;
use crate::expression::parser::Parser;
use crate::queue::simple_queue::SimpleQueue;
use crate::queue::source::{AutoQueuesError, ExpressionSource, QueueSource};
use crate::queue::QueueError;
use dashmap::DashMap;
use std::any::Any;
use std::sync::{Arc, RwLock};

/// Type-erased queue entry for the registry
pub trait AnyQueue: Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Clone + Send + Sync + 'static> AnyQueue for Arc<RwLock<SimpleQueue<T>>> {
    fn as_any(&self) -> &dyn Any {
        self
    }
    
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Queue registry for managing queues and sources
pub struct QueueRegistry {
    queues: DashMap<String, Box<dyn AnyQueue>>,
    sources: DashMap<String, Box<dyn Any>>,
    config: Config,
}

impl QueueRegistry {
    /// Create a new queue registry
    pub fn new(config: Config) -> Self {
        Self {
            queues: DashMap::new(),
            sources: DashMap::new(),
            config,
        }
    }
    
    /// Add a queue with its source to the registry
    pub fn add_queue<T: Clone + Send + Sync + 'static>(
        &self,
        name: &str,
        source: impl QueueSource<T> + 'static,
    ) -> Result<(), AutoQueuesError> {
        let queue: Arc<RwLock<SimpleQueue<T>>> = Arc::new(RwLock::new(SimpleQueue::new()));
        
        // Start the source
        source.start(queue.clone(), &self.config);
        
        // Store the queue and source
        self.queues.insert(name.to_string(), Box::new(queue));
        self.sources.insert(name.to_string(), Box::new(source));
        
        Ok(())
    }
    
/// Add an expression-based queue to the registry
    pub fn add_expression_queue(
        &self,
        name: &str,
        expression: &str,
        source_queue: &str,
        trigger_on_push: bool,
        trigger_interval_ms: Option<u64>,
    ) -> Result<(), AutoQueuesError> {
        // Validate the expression
        Parser::parse(expression)
            .map_err(|e| AutoQueuesError::ExpressionError(format!("Invalid expression: {:?}", e)))?;
        
        let source = ExpressionSource::new(
            expression.to_string(),
            source_queue.to_string(),
            trigger_on_push,
            trigger_interval_ms,
        );
        
        // Expressions always evaluate to f64 values
        self.add_queue::<f64>(name, source)
    }
    
    /// Get a queue from the registry
    pub fn get_queue<T: Clone + Send + Sync + 'static>(
        &self,
        name: &str,
    ) -> Result<Arc<RwLock<SimpleQueue<T>>>, AutoQueuesError> {
        if let Some(queue) = self.queues.get(name) {
            if let Some(typed_queue) = queue.as_any().downcast_ref::<Arc<RwLock<SimpleQueue<T>>>>() {
                Ok(typed_queue.clone())
            } else {
                Err(AutoQueuesError::QueueError(QueueError::Other(
                    "Queue type mismatch".to_string(),
                )))
            }
        } else {
            Err(AutoQueuesError::QueueError(QueueError::Other(
                "Queue not found".to_string(),
            )))
        }
    }
    
    /// Start all queues
    pub fn start(&self) {
        // In a real implementation, this would start all queue sources
        // For now, we'll just iterate through the queues
        for queue in self.queues.iter() {
            println!("Starting queue: {}", queue.key());
        }
    }
    
    /// Pause a queue
    pub fn pause(&self, name: &str) {
        if let Some(_source) = self.sources.get(name) {
            // In a real implementation, we would call source.pause()
            println!("Pausing queue: {}", name);
        }
    }
    
    /// Resume a queue
    pub fn resume(&self, name: &str) {
        if let Some(_source) = self.sources.get(name) {
            // In a real implementation, we would call source.resume()
            println!("Resuming queue: {}", name);
        }
    }
    
    /// Remove a queue
    pub fn remove(&self, name: &str) {
        self.queues.remove(name);
        self.sources.remove(name);
        println!("Removed queue: {}", name);
    }
}