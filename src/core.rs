//! Core Queue Implementation
//!
//! This module contains the essential Queue<T> implementation extracted from SCORE.
//! It provides autonomous data processing with configurable intervals and AIMD support.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{oneshot, Mutex};
use tokio::time::interval;

use crate::aimd::{AimdConfig, AimdController, AimdStats};
use crate::enums::QueueError;
use crate::server::QueueServerHandle;
use crate::types::{QueueConfig, QueueStats};

pub type Timestamp = u64;
pub type QueueData<T> = (Timestamp, T);

/// Core Queue implementation with autonomous server capabilities
pub struct Queue<T: Clone + Send + 'static> {
    data: Arc<Mutex<VecDeque<QueueData<T>>>>,
    config: QueueConfig<T>,
    aimd_controller: Option<AimdController>,
}

impl<T: Clone + Send + 'static> Queue<T> {
    pub fn new(config: QueueConfig<T>) -> Self {
        Self {
            data: Arc::new(Mutex::new(VecDeque::with_capacity(4096))),
            config,
            aimd_controller: None,
        }
    }

    pub fn with_aimd(config: QueueConfig<T>, aimd_config: AimdConfig) -> Self {
        let aimd_controller = AimdController::from_config(aimd_config).unwrap();

        Self {
            data: Arc::new(Mutex::new(VecDeque::with_capacity(4096))),
            config,
            aimd_controller: Some(aimd_controller),
        }
    }

    pub async fn publish(&mut self, value: T) -> Result<(), QueueError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| QueueError::Other("System time error".to_string()))?
            .as_millis() as Timestamp;

        let mut data = self.data.lock().await;
        data.push_back((timestamp, value.clone()));

        // Update AIMD if available
        if let Some(ref mut aimd) = self.aimd_controller {
            // Simple variance calculation from recent values
            let values: Vec<f64> = data
                .iter()
                .take(10)
                .map(|(_, v)| match v {
                    // Convert different types to f64 for variance calculation
                    // This is a simplified approach - in real usage, you'd have
                    // type-specific conversion logic
                    _ => 0.0,
                })
                .collect();

            if let Err(err) = aimd.update_variance(&values) {
                eprintln!("AIMD variance update failed: {:?}", err);
            }

            if let Err(err) = aimd.adjust_interval() {
                eprintln!("AIMD interval adjustment failed: {:?}", err);
            }
        }

        Ok(())
    }

    pub async fn get_latest(&self) -> Option<QueueData<T>> {
        let data = self.data.lock().await;
        data.back().cloned()
    }

    pub async fn get_data_in_range(&self, start: Timestamp, end: Timestamp) -> Vec<QueueData<T>> {
        let data = self.data.lock().await;
        data.iter()
            .filter(|(timestamp, _)| *timestamp >= start && *timestamp <= end)
            .cloned()
            .collect()
    }

    pub fn get_stats(&self) -> QueueStats {
        let data = self.data.try_lock().unwrap();
        let oldest = data.front().map(|(ts, _)| *ts);
        let newest = data.back().map(|(ts, _)| *ts);

        QueueStats {
            size: data.len(),
            capacity: 4096,
            oldest_timestamp: oldest,
            newest_timestamp: newest,
            mode: self.config.mode,
            model: self.config.model,
            interval_ms: self
                .aimd_controller
                .as_ref()
                .map(|aimd| aimd.get_current_interval())
                .unwrap_or(self.config.queue_type.base_interval),
        }
    }

    pub fn get_config(&self) -> &QueueConfig<T> {
        &self.config
    }

    pub fn get_aimd_stats(&self) -> Option<AimdStats> {
        self.aimd_controller.as_ref().map(|aimd| aimd.get_stats())
    }

    pub fn get_current_interval(&self) -> Timestamp {
        self.aimd_controller
            .as_ref()
            .map(|aimd| aimd.get_current_interval())
            .unwrap_or(self.config.queue_type.base_interval)
    }

    pub fn start_server(self) -> Result<QueueServerHandle, QueueError> {
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        let data = Arc::clone(&self.data);
        let config = self.config.clone();
        let aimd_controller = self.aimd_controller.clone();

        let join_handle = tokio::spawn(async move {
            let interval_ms = aimd_controller
                .as_ref()
                .map(|aimd| aimd.get_current_interval())
                .unwrap_or(config.queue_type.base_interval);

            let mut timer = interval(Duration::from_millis(interval_ms));

            loop {
                tokio::select! {
                    _ = timer.tick() => {
                        // Generate data using function hook
                        if let Ok(value) = (config.hook)(vec![]) {
                            let timestamp = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .map_or_else(|_| 0u64, |d| d.as_millis() as Timestamp);

                            let mut data = data.lock().await;
                            data.push_back((timestamp, value.clone()));

                            // Update AIMD (disabled for now)
                            // if let Some(ref mut aimd) = aimd_controller {
                            //     let values: Vec<f64> = data.iter()
                            //         .take(10)
                            //         .map(|(_, v)| match v {
                            //             _ => 0.0,
                            //         })
                            //         .collect();
                            //
                            //     if let Err(err) = aimd.update_variance(&values) {
                            //         eprintln!("AIMD variance update failed: {:?}", err);
                            //     }
                            //
                            //     if let Err(err) = aimd.adjust_interval() {
                            //         eprintln!("AIMD interval adjustment failed: {:?}", err);
                            //     }
                            // }
                        }
                    }
                    _ = &mut shutdown_rx => {
                        println!("Queue server shutting down gracefully");
                        break;
                    }
                }
            }
        });

        Ok(QueueServerHandle::new(shutdown_tx, join_handle))
    }
}
