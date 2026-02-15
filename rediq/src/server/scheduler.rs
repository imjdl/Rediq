//! Scheduler implementation
//!
//! The scheduler handles delayed tasks and retry tasks by moving them
//! from delayed/retry queues to the main queue when they are due.
//!
//! It also manages task dependencies - when a task completes, any tasks
//! that were waiting for it will be checked and enqueued if all dependencies
//! are satisfied.

use crate::{
    storage::{Keys, RedisClient, dependencies},
    Error, Result, Task,
};
use chrono::Utc;
use fred::prelude::{RedisKey, RedisValue};
use rmp_serde;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Scheduler - manages delayed, retry, and dependent tasks
///
/// The scheduler runs in a separate task and periodically checks:
/// 1. Delayed queue - moves tasks whose execution time has arrived to the main queue
/// 2. Retry queue - moves tasks whose retry delay has expired to the main queue
/// 3. Cron queue - creates new instances of periodic tasks
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
    /// It checks retry, delayed, and cron queues at regular intervals.
    pub async fn run(self) -> Result<()> {
        tracing::info!("Scheduler started for queues: {:?}", self.queues);

        let mut tick_count = 0u64;

        loop {
            // Check for shutdown BEFORE doing any work
            if self.shutdown.load(Ordering::Relaxed) {
                tracing::info!("Scheduler stopped after {} ticks", tick_count);
                return Ok(());
            }

            tick_count += 1;

            // Check retry tasks (every 1 second)
            if let Err(e) = self.check_retry_tasks().await {
                tracing::error!("Retry check error: {}", e);
            }

            // Check delayed tasks (every 5 seconds)
            if tick_count % 5 == 0 {
                if let Err(e) = self.check_delayed_tasks().await {
                    tracing::error!("Delayed check error: {}", e);
                }
            }

            // Check cron tasks (every 60 seconds)
            if tick_count % 60 == 0 {
                if let Err(e) = self.check_cron_tasks().await {
                    tracing::error!("Cron check error: {}", e);
                }
            }

            // Sleep for 1 second
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
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

    /// Check and process cron tasks
    ///
    /// This method scans the cron queue for periodic tasks that are due
    /// and creates new task instances for processing.
    async fn check_cron_tasks(&self) -> Result<()> {
        let now = Utc::now().timestamp();

        for queue in &self.queues {
            let cron_key: RedisKey = Keys::cron_queue(queue).into();
            let queue_key: RedisKey = Keys::queue(queue).into();

            // Get cron tasks that are due
            let task_ids = self.redis.zrangebyscore(cron_key.clone(), 0, now).await?;

            for task_id in task_ids {
                // Load the cron task template
                let task_key: RedisKey = Keys::task(&task_id).into();
                if let Some(data) = self.redis.get(task_key).await? {
                    let bytes = data.as_bytes()
                        .ok_or_else(|| Error::Serialization("Task data is not bytes".into()))?;

                    let cron_task: Task = rmp_serde::from_slice(bytes)
                        .map_err(|e| Error::Serialization(e.to_string()))?;

                    // Get the cron expression
                    let cron_expr = cron_task.options.cron.clone()
                        .ok_or_else(|| Error::Validation("Cron task missing cron expression".into()))?;

                    // Remove from cron queue temporarily
                    self.redis.zrem(cron_key.clone(), task_id.as_str().into()).await?;

                    // Create a new task instance (without cron expression, with regular delay)
                    let new_task = match Task::builder(cron_task.task_type.clone())
                        .queue(queue.clone())
                        .max_retry(cron_task.options.max_retry)
                        .timeout(cron_task.options.timeout)
                        .priority(cron_task.options.priority)
                        .raw_payload(cron_task.payload.clone())
                        .build()
                    {
                        Ok(task) => task,
                        Err(e) => {
                            tracing::error!("Failed to create cron task instance for {}: {}", task_id, e);
                            // Re-schedule for 60 seconds later to retry
                            self.redis.zadd(cron_key.clone(), task_id.as_str().into(), now + 60).await?;
                            continue;
                        }
                    };

                    // Store the new task
                    let new_task_key: RedisKey = Keys::task(&new_task.id).into();
                    let new_task_data = rmp_serde::to_vec(&new_task)
                        .map_err(|e| Error::Serialization(e.to_string()))?;
                    self.redis.set(new_task_key, RedisValue::Bytes(new_task_data.into())).await?;

                    // Enqueue the new task instance
                    self.redis.rpush(queue_key.clone(), new_task.id.as_str().into()).await?;

                    tracing::debug!("Cron task {} instantiated and queued", task_id);

                    // Calculate next scheduled time
                    if let Some(next_time) = self.calculate_next_cron_time(&cron_expr, now) {
                        // Re-add the cron template to the cron queue with next scheduled time
                        self.redis.zadd(cron_key.clone(), task_id.as_str().into(), next_time).await?;
                        tracing::debug!("Cron task {} rescheduled for {}", task_id, next_time);
                    } else {
                        tracing::warn!("Could not calculate next time for cron task {}", task_id);
                    }
                }
            }
        }

        Ok(())
    }

    /// Calculate the next scheduled time for a cron expression
    fn calculate_next_cron_time(&self, cron_expr: &str, from_timestamp: i64) -> Option<i64> {
        use cron::Schedule;

        // Parse the cron expression
        let schedule = Schedule::try_from(cron_expr).ok()?;

        // Convert timestamp to DateTime
        let from_datetime = chrono::DateTime::from_timestamp(from_timestamp, 0)?;

        // Get next occurrence using upcoming() iterator
        let timezone = from_datetime.timezone();
        schedule.upcoming(timezone).next().map(|dt| dt.timestamp())
    }

    /// Register a task with dependencies
    ///
    /// Called when a task with dependencies is enqueued.
    /// Sets up the dependency tracking in Redis.
    ///
    /// # Arguments
    /// * `task` - The task with dependencies
    pub async fn register_dependencies(&self, task: &Task) -> Result<()> {
        let deps = match &task.options.depends_on {
            Some(d) if !d.is_empty() => d.clone(),
            _ => return Ok(()),
        };

        dependencies::register(&self.redis, &task.id, &deps).await
    }

    /// Check and enqueue dependent tasks
    ///
    /// Called when a task completes. Checks if any tasks were waiting for
    /// this task and enqueues them if all their dependencies are satisfied.
    ///
    /// # Arguments
    /// * `completed_task_id` - The ID of the completed task
    pub async fn check_dependent_tasks(&self, completed_task_id: &str) -> Result<()> {
        dependencies::check_dependents(&self.redis, completed_task_id).await?;
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
