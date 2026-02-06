//! Scheduler implementation
//!
//! The scheduler handles delayed tasks and retry tasks by moving them
//! from delayed/retry queues to the main queue when they are due.

use crate::{
    storage::{Keys, RedisClient},
    Error, Result,
};
use chrono::Utc;
use fred::prelude::RedisKey;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Scheduler - manages delayed and retry tasks
///
/// The scheduler runs in a separate task and periodically checks:
/// 1. Delayed queue - moves tasks whose execution time has arrived to the main queue
/// 2. Retry queue - moves tasks whose retry delay has expired to the main queue
pub struct Scheduler {
    /// Redis client
    redis: RedisClient,

    /// Queues to monitor
    queues: Vec<String>,

    /// Shutdown flag
    shutdown: Arc<AtomicBool>,
}

impl Scheduler {
    /// Create a new scheduler
    #[must_use]
    pub fn new(redis: RedisClient, queues: Vec<String>) -> Self {
        Self {
            redis,
            queues,
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Run the scheduler loop
    ///
    /// This method runs continuously until shutdown is requested.
    /// It checks both delayed and retry queues at regular intervals.
    pub async fn run(self) -> Result<()> {
        tracing::info!("Scheduler started for queues: {:?}", self.queues);

        let mut retry_interval = tokio::time::interval(Duration::from_secs(1));
        let mut delayed_interval = tokio::time::interval(Duration::from_secs(5));

        loop {
            tokio::select! {
                _ = retry_interval.tick() => {
                    if let Err(e) = self.check_retry_tasks().await {
                        tracing::error!("Retry check error: {}", e);
                    }
                }
                _ = delayed_interval.tick() => {
                    if let Err(e) = self.check_delayed_tasks().await {
                        tracing::error!("Delayed check error: {}", e);
                    }
                }
            }

            if self.shutdown.load(Ordering::Relaxed) {
                break;
            }
        }

        tracing::info!("Scheduler stopped");
        Ok(())
    }

    /// Request graceful shutdown
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }

    /// Check and process retry tasks
    ///
    /// This method scans the retry queue for tasks whose retry delay has expired
    /// and moves them to the main queue for processing.
    async fn check_retry_tasks(&self) -> Result<()> {
        let now = Utc::now().timestamp();

        for queue in &self.queues {
            let retry_key: RedisKey = Keys::retry(queue).into();
            let queue_key: RedisKey = Keys::queue(queue).into();

            // Get tasks that are due for retry
            let task_ids = self.redis.zrangebyscore(retry_key.clone(), 0, now).await?;

            for task_id in task_ids {
                // Remove from retry queue
                let removed = self.redis.zrem(retry_key.clone(), task_id.as_str().into()).await?;

                if removed {
                    // Add to main queue
                    self.redis.rpush(queue_key.clone(), task_id.as_str().into()).await?;
                    tracing::debug!("Task {} moved from retry to queue {}", task_id, queue);
                }
            }
        }

        Ok(())
    }

    /// Check and process delayed tasks
    ///
    /// This method scans the delayed queue for tasks whose execution time has arrived
    /// and moves them to the main queue for processing.
    async fn check_delayed_tasks(&self) -> Result<()> {
        let now = Utc::now().timestamp();

        for queue in &self.queues {
            let delayed_key: RedisKey = Keys::delayed(queue).into();
            let queue_key: RedisKey = Keys::queue(queue).into();

            // Get tasks that are due for execution
            let task_ids = self.redis.zrangebyscore(delayed_key.clone(), 0, now).await?;

            for task_id in task_ids {
                // Remove from delayed queue
                let removed = self.redis.zrem(delayed_key.clone(), task_id.as_str().into()).await?;

                if removed {
                    // Add to main queue
                    self.redis.rpush(queue_key.clone(), task_id.as_str().into()).await?;
                    tracing::debug!("Task {} moved from delayed to queue {}", task_id, queue);
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires Redis server"]
    async fn test_scheduler_creation() {
        let redis_url = std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://localhost:6379".to_string());
        let redis = RedisClient::from_url(&redis_url)
            .await
            .unwrap();

        let scheduler = Scheduler::new(redis, vec!["default".to_string()]);
        assert_eq!(scheduler.queues.len(), 1);
    }
}
