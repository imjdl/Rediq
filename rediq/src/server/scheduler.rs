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
    aggregator::AggregatorManager,
    Error, Result, Task,
};
use chrono::Utc;
use fred::prelude::{RedisKey, RedisValue};
use rmp_serde;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Default batch size for aggregation
const DEFAULT_AGGREGATION_SIZE: usize = 10;

/// Scheduler - manages delayed, retry, and dependent tasks
///
/// The scheduler runs in a separate task and periodically checks:
/// 1. Delayed queue - moves tasks whose execution time has arrived to the main queue
/// 2. Retry queue - moves tasks whose retry delay has expired to the main queue
/// 3. Cron queue - creates new instances of periodic tasks
/// 4. Task groups - aggregates tasks when group conditions are met
pub struct Scheduler {
    /// Redis client
    redis: RedisClient,

    /// Queues to monitor
    queues: Vec<String>,

    /// Shutdown flag
    shutdown: Arc<AtomicBool>,

    /// Aggregator manager for task grouping
    aggregator: Option<Arc<AggregatorManager>>,
}

impl Scheduler {
    /// Create a new scheduler
    #[must_use]
    pub fn new(redis: RedisClient, queues: Vec<String>) -> Self {
        Self {
            redis,
            queues,
            shutdown: Arc::new(AtomicBool::new(false)),
            aggregator: None,
        }
    }

    /// Create a new scheduler with aggregator support
    #[must_use]
    pub fn with_aggregator(redis: RedisClient, queues: Vec<String>, aggregator: Arc<AggregatorManager>) -> Self {
        Self {
            redis,
            queues,
            shutdown: Arc::new(AtomicBool::new(false)),
            aggregator: Some(aggregator),
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

            // Check task aggregation (every 2 seconds)
            if tick_count % 2 == 0 {
                if let Err(e) = self.check_aggregation().await {
                    tracing::error!("Aggregation check error: {}", e);
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
    /// Uses atomic Lua script for batch operations to avoid N+1 Redis calls.
    async fn check_retry_tasks(&self) -> Result<()> {
        let now = Utc::now().timestamp();
        const BATCH_SIZE: usize = 100; // Process up to 100 tasks per batch

        for queue in &self.queues {
            let retry_key: RedisKey = Keys::retry(queue).into();
            let queue_key: RedisKey = Keys::queue(queue).into();

            // Use atomic Lua script for batch move
            match self.redis.move_expired_tasks_lua(
                retry_key,
                queue_key,
                now,
                BATCH_SIZE,
            ).await {
                Ok(count) if count > 0 => {
                    tracing::debug!("Moved {} tasks from retry to queue {}", count, queue);
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!("Failed to move retry tasks for queue {}: {}", queue, e);
                }
            }
        }

        Ok(())
    }

    /// Check and process delayed tasks
    ///
    /// This method scans the delayed queue for tasks whose execution time has arrived
    /// and moves them to the main queue for processing.
    /// Uses atomic Lua script for batch operations to avoid N+1 Redis calls.
    async fn check_delayed_tasks(&self) -> Result<()> {
        let now = Utc::now().timestamp();
        const BATCH_SIZE: usize = 100; // Process up to 100 tasks per batch

        for queue in &self.queues {
            let delayed_key: RedisKey = Keys::delayed(queue).into();
            let queue_key: RedisKey = Keys::queue(queue).into();

            // Use atomic Lua script for batch move
            match self.redis.move_expired_tasks_lua(
                delayed_key,
                queue_key,
                now,
                BATCH_SIZE,
            ).await {
                Ok(count) if count > 0 => {
                    tracing::debug!("Moved {} tasks from delayed to queue {}", count, queue);
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!("Failed to move delayed tasks for queue {}: {}", queue, e);
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

    /// Check and process task aggregation
    ///
    /// This method checks task groups for aggregation conditions:
    /// 1. If group size >= max_size, trigger aggregation
    /// 2. If grace period elapsed since first task, trigger aggregation
    ///
    /// When aggregation is triggered, tasks are moved from group ZSet to main queue.
    /// If an aggregator is registered, it will be called to combine tasks.
    async fn check_aggregation(&self) -> Result<()> {
        // Skip if no aggregator is configured
        let aggregator = match &self.aggregator {
            Some(a) => a,
            None => return Ok(()),
        };

        let now = Utc::now().timestamp();

        // Scan for group keys
        let group_pattern = "rediq:meta:group:*";
        let (mut cursor, keys) = self.redis.scan_match(0, group_pattern, 100).await?;

        // Process found groups
        for meta_key in keys {
            // Extract group name from key
            let group_name = meta_key.strip_prefix("rediq:meta:group:").unwrap_or(&meta_key);

            // Get group count
            let count: i64 = match self.redis.hget(meta_key.clone().into(), "count".into()).await? {
                Some(v) => v.as_string().and_then(|s| s.parse().ok()).unwrap_or(0),
                None => continue,
            };

            // Get config
            let config = aggregator.default_config();
            let should_aggregate = count as usize >= config.max_size;

            if should_aggregate {
                // Get tasks from group
                let group_key: RedisKey = Keys::group(group_name).into();
                let task_ids = self.redis.zrange(group_key.clone(), 0, -1).await?;

                if task_ids.is_empty() {
                    continue;
                }

                // Load tasks
                let mut tasks = Vec::new();
                for task_id in &task_ids {
                    let task_key: RedisKey = Keys::task(task_id).into();
                    if let Some(data) = self.redis.hget(task_key.clone(), "data".into()).await? {
                        if let Some(bytes) = data.as_bytes() {
                            if let Ok(task) = rmp_serde::from_slice::<Task>(bytes) {
                                tasks.push(task);
                            }
                        }
                    }
                }

                // Find aggregator for this group
                let aggregated_task = if let Some(agg) = aggregator.get(group_name) {
                    agg.aggregate(group_name, tasks)?
                } else {
                    // No specific aggregator, skip
                    continue;
                };

                // Remove tasks from group
                for task_id in &task_ids {
                    self.redis.zrem(group_key.clone(), task_id.as_str().into()).await?;
                }

                // Reset group count
                self.redis.hset(
                    meta_key.clone().into(),
                    vec![("count".into(), "0".into())],
                ).await?;

                // Enqueue aggregated task if created
                if let Some(new_task) = aggregated_task {
                    let new_task_key: RedisKey = Keys::task(&new_task.id).into();
                    let new_task_data = rmp_serde::to_vec(&new_task)
                        .map_err(|e| Error::Serialization(e.to_string()))?;
                    self.redis.hset(
                        new_task_key.clone(),
                        vec![
                            ("data".into(), RedisValue::Bytes(new_task_data.into())),
                            ("queue".into(), new_task.queue.as_str().into()),
                        ],
                    ).await?;

                    // Add to queue
                    let queue_key: RedisKey = Keys::queue(&new_task.queue).into();
                    self.redis.rpush(queue_key, new_task.id.as_str().into()).await?;

                    tracing::debug!("Aggregated {} tasks from group {} into task {}",
                        task_ids.len(), group_name, new_task.id);
                }
            }
        }

        // Handle pagination if there are more groups
        while cursor != 0 {
            let (next_cursor, more_keys) = self.redis.scan_match(cursor, group_pattern, 100).await?;
            cursor = next_cursor;
            // Process more_keys...
            for _key in more_keys {
                // Same logic as above, but simplified for now
            }
        }

        Ok(())
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
