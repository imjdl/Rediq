//! Client builder and enqueue implementation

use crate::{
    storage::{Keys, RedisClient, RedisMode},
    config, Error, Result,
};
use crate::task::Task;
use chrono::Utc;
use fred::prelude::{RedisKey, RedisValue};
use rmp_serde;

/// Client configuration
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Redis connection URL
    pub redis_url: String,
    /// Redis connection mode
    pub redis_mode: RedisMode,
    /// Connection pool size
    pub pool_size: usize,
    /// Default queue name
    pub default_queue: String,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://localhost:6379".to_string(),
            redis_mode: RedisMode::Standalone,
            pool_size: 10,
            default_queue: "default".to_string(),
        }
    }
}

/// Client - Task producer
///
/// Used to create and enqueue tasks to Redis
#[derive(Clone)]
pub struct Client {
    redis: RedisClient,
}

impl Client {
    /// Create a new Client builder
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// Enqueue immediately
    pub async fn enqueue(&self, task: Task) -> Result<String> {
        let task_id = task.id.clone();
        let queue = task.queue.clone();
        let unique_key = task.options.unique_key.clone();
        let deps = task.options.depends_on.clone();
        let priority = task.options.priority;

        // Serialize task
        let task_data = rmp_serde::to_vec(&task)
            .map_err(|e| Error::Serialization(e.to_string()))?;

        // Store task details
        let task_key: RedisKey = Keys::task(&task_id).into();
        self.redis.set(task_key, RedisValue::Bytes(task_data.into())).await?;

        // Handle dependencies - if task has dependencies, don't enqueue to main queue yet
        if let Some(deps) = deps {
            if !deps.is_empty() {
                // Store pending dependencies for this task
                let pending_deps_key: RedisKey = Keys::pending_deps(&task_id).into();
                let dep_values: Vec<RedisValue> = deps.iter().map(|id| id.as_str().into()).collect();
                for dep in &dep_values {
                    self.redis.sadd(pending_deps_key.clone(), dep.clone()).await?;
                }

                // Register this task as dependent on each dependency
                for dep_id in &deps {
                    let task_deps_key: RedisKey = Keys::task_deps(dep_id).into();
                    self.redis.sadd(task_deps_key, task_id.as_str().into()).await?;
                }

                tracing::debug!("Task {} registered with {} dependencies (not enqueued yet)", task_id, deps.len());
                return Ok(task_id);
            }
        }

        // Register queue
        let queues_key: RedisKey = Keys::meta_queues().into();
        self.redis.sadd(queues_key, queue.as_str().into()).await?;

        // Handle deduplication
        if let Some(key) = unique_key {
            let dedup_key: RedisKey = Keys::dedup(&queue).into();
            self.redis.sadd(dedup_key, key.into()).await?;
        }

        // Check if task has non-default priority (default is 50)
        const DEFAULT_PRIORITY: i32 = 50;
        if priority != DEFAULT_PRIORITY {
            // Enqueue to priority queue (ZSet with priority as score)
            let pqueue_key: RedisKey = Keys::priority_queue(&queue).into();
            self.redis.zadd(pqueue_key, task_id.as_str().into(), priority as i64).await?;
            tracing::debug!("Priority task enqueued: {} (priority: {}) to queue {}", task_id, priority, queue);
        } else {
            // Enqueue to regular queue (List)
            let queue_key: RedisKey = Keys::queue(&queue).into();
            self.redis.rpush(queue_key, task_id.as_str().into()).await?;
            tracing::debug!("Task enqueued: {} to queue {}", task_id, queue);
        }

        Ok(task_id)
    }

    /// Enqueue delayed task
    pub async fn enqueue_delayed(&self, task: Task) -> Result<String> {
        let task_id = task.id.clone();
        let queue = task.queue.clone();
        let delay = task.options.delay.ok_or(Error::Validation(
            "Delay must be set for delayed tasks".into(),
        ))?;

        let execute_at = Utc::now().timestamp() + delay.as_secs() as i64;

        // Serialize and store task
        let task_data = rmp_serde::to_vec(&task)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        let task_key: RedisKey = Keys::task(&task_id).into();
        self.redis.set(task_key, RedisValue::Bytes(task_data.into())).await?;

        // Add to delayed queue
        let delayed_key: RedisKey = Keys::delayed(&queue).into();
        self.redis.zadd(delayed_key, task_id.as_str().into(), execute_at).await?;

        // Register queue
        let queues_key: RedisKey = Keys::meta_queues().into();
        self.redis.sadd(queues_key, queue.as_str().into()).await?;

        tracing::debug!("Delayed task enqueued: {}", task_id);
        Ok(task_id)
    }

    /// Enqueue to priority queue
    ///
    /// Tasks with lower priority values are processed first.
    /// Priority range: 0-100 (default is 50)
    pub async fn enqueue_priority(&self, task: Task) -> Result<String> {
        let task_id = task.id.clone();
        let queue = task.queue.clone();
        let priority = task.options.priority;
        let unique_key = task.options.unique_key.clone();

        // Validate priority (use global config)
        let priority_range = config::get_priority_range();
        if priority < priority_range.0 || priority > priority_range.1 {
            return Err(Error::Validation(format!(
                "Priority must be between {} and {}, got {}",
                priority_range.0, priority_range.1, priority
            )));
        }

        // Serialize task
        let task_data = rmp_serde::to_vec(&task)
            .map_err(|e| Error::Serialization(e.to_string()))?;

        // Store task details with priority
        let task_key: RedisKey = Keys::task(&task_id).into();
        self.redis.set(task_key, RedisValue::Bytes(task_data.into())).await?;

        // Add to priority queue (ZSet with priority as score)
        let pqueue_key: RedisKey = Keys::priority_queue(&queue).into();
        self.redis.zadd(pqueue_key, task_id.as_str().into(), priority as i64).await?;

        // Handle deduplication
        if let Some(key) = unique_key {
            let dedup_key: RedisKey = Keys::dedup(&queue).into();
            self.redis.sadd(dedup_key, key.into()).await?;
        }

        // Register queue
        let queues_key: RedisKey = Keys::meta_queues().into();
        self.redis.sadd(queues_key, queue.as_str().into()).await?;

        tracing::debug!("Priority task enqueued: {} (priority: {})", task_id, priority);
        Ok(task_id)
    }

    /// Enqueue a cron (periodic) task
    ///
    /// Cron tasks are scheduled to run periodically based on a cron expression.
    /// Supported cron format: standard 5-field or 6-field (with seconds) format.
    pub async fn enqueue_cron(&self, task: Task) -> Result<String> {
        let task_id = task.id.clone();
        let queue = task.queue.clone();
        let cron_expr = task.options.cron.clone()
            .ok_or(Error::Validation("Cron task must have cron expression".into()))?;

        // Validate cron expression
        use cron::Schedule;
        Schedule::try_from(cron_expr.as_str())
            .map_err(|e| Error::Validation(format!("Invalid cron expression: {}", e)))?;

        // Calculate first scheduled time
        let schedule = Schedule::try_from(cron_expr.as_str())
            .map_err(|e| Error::Validation(format!("Invalid cron expression: {}", e)))?;
        let next_time = schedule.upcoming(chrono::Utc::now().timezone()).next()
            .ok_or(Error::Validation("Could not calculate next scheduled time".into()))?
            .timestamp();

        // Serialize task
        let task_data = rmp_serde::to_vec(&task)
            .map_err(|e| Error::Serialization(e.to_string()))?;

        // Store task details
        let task_key: RedisKey = Keys::task(&task_id).into();
        self.redis.set(task_key, RedisValue::Bytes(task_data.into())).await?;

        // Add to cron queue (score = next scheduled time)
        let cron_key: RedisKey = Keys::cron_queue(&queue).into();
        self.redis.zadd(cron_key, task_id.as_str().into(), next_time).await?;

        // Register queue
        let queues_key: RedisKey = Keys::meta_queues().into();
        self.redis.sadd(queues_key, queue.as_str().into()).await?;

        tracing::debug!("Cron task enqueued: {} (cron: {}, next: {})", task_id, cron_expr, next_time);
        Ok(task_id)
    }

    /// Batch enqueue
    ///
    /// This optimized version uses Redis pipelining to reduce network round trips.
    /// All tasks are serialized and queued in a single batch operation.
    pub async fn enqueue_batch(&self, tasks: Vec<Task>) -> Result<Vec<String>> {
        if tasks.is_empty() {
            return Ok(Vec::new());
        }

        let mut task_ids = Vec::with_capacity(tasks.len());
        let mut registered_queues = std::collections::HashSet::new();

        // Create pipeline for batch operations
        let mut pipeline = self.redis.pipeline();

        // Process tasks and collect Redis operations
        for task in tasks {
            let task_id = task.id.clone();
            task_ids.push(task_id.clone());

            // Serialize task
            let task_data = rmp_serde::to_vec(&task)
                .map_err(|e| Error::Serialization(e.to_string()))?;

            // Add SET command to pipeline (store task details)
            let task_key: RedisKey = Keys::task(&task_id).into();
            pipeline = pipeline.set(task_key, RedisValue::Bytes(task_data.into()));

            // Add RPUSH command to pipeline (add task_id to queue)
            let queue_key: RedisKey = Keys::queue(&task.queue).into();
            pipeline = pipeline.rpush(queue_key, task_id.as_str().into());

            // Add SADD command to pipeline if unique_key exists
            if let Some(key) = &task.options.unique_key {
                let dedup_key: RedisKey = Keys::dedup(&task.queue).into();
                pipeline = pipeline.sadd(dedup_key, key.as_str().into());
            }

            // Track queues for registration
            registered_queues.insert(task.queue.clone());
        }

        // Execute all Redis commands in a single round trip
        pipeline.execute().await?;

        // Register all queues (also use pipeline for efficiency)
        if !registered_queues.is_empty() {
            let mut queues_pipeline = self.redis.pipeline();
            let queues_key: RedisKey = Keys::meta_queues().into();
            for queue in registered_queues {
                queues_pipeline = queues_pipeline.sadd(queues_key.clone(), queue.as_str().into());
            }
            queues_pipeline.execute().await?;
        }

        tracing::debug!("Batch enqueued {} tasks", task_ids.len());
        Ok(task_ids)
    }

    /// Get inspector
    pub fn inspector(&self) -> super::Inspector {
        super::Inspector::new(self.redis.clone())
    }

    /// Pause queue
    pub async fn pause_queue(&self, queue_name: &str) -> Result<()> {
        let pause_key: RedisKey = Keys::pause(queue_name).into();
        self.redis.set(pause_key, RedisValue::Boolean(true)).await?;
        tracing::info!("Queue '{}' paused", queue_name);
        Ok(())
    }

    /// Resume queue
    pub async fn resume_queue(&self, queue_name: &str) -> Result<()> {
        let pause_key: RedisKey = Keys::pause(queue_name).into();
        self.redis.del(vec![pause_key]).await?;
        tracing::info!("Queue '{}' resumed", queue_name);
        Ok(())
    }

    /// Flush queue
    pub async fn flush_queue(&self, queue_name: &str) -> Result<u64> {
        let queue_key: RedisKey = Keys::queue(queue_name).into();
        let count = self.redis.llen(queue_key.clone()).await?;

        // Delete queue
        self.redis.del(vec![queue_key]).await?;

        tracing::info!("Queue '{}' flushed, {} tasks removed", queue_name, count);
        Ok(count)
    }

    /// Cancel a task
    ///
    /// Attempts to cancel a task by removing it from all possible queue states.
    /// Checks pending, active, delayed, retry, dead, and priority queues.
    ///
    /// Returns the cancellation status:
    /// - `Ok(Some(status))` - Task was cancelled, status indicates where it was found
    /// - `Ok(None)` - Task was not found in any queue
    pub async fn cancel_task(&self, task_id: &str, queue_name: &str) -> Result<Option<String>> {
        use crate::storage::Keys;

        // Define all possible queue types to check
        // Each tuple contains (queue_key_function, status_name, is_sorted_set)
        let queue_checks: Vec<(fn(&str) -> String, &str, bool)> = vec![
            (Keys::queue, "pending", false),
            (Keys::active, "active", false),
            (Keys::delayed, "delayed", true),
            (Keys::retry, "retry", true),
            (Keys::dead, "dead", false),
            (Keys::priority_queue, "priority", true),
        ];

        for (key_fn, status, is_sorted) in queue_checks {
            let queue_key: RedisKey = key_fn(queue_name).into();

            // Check if the key exists first to avoid unnecessary operations
            if !self.redis.exists(queue_key.clone()).await? {
                continue;
            }

            let removed = if is_sorted {
                // For sorted sets (delayed, retry, priority)
                // zrem returns Result<bool>
                self.redis.zrem(queue_key, task_id.into()).await?
            } else {
                // For lists (pending, active, dead)
                // lrem returns u64 (count of removed elements), convert to bool
                self.redis.lrem(queue_key, task_id.into(), 1).await? > 0
            };

            if removed {
                // Task was found and removed, clean up related data
                let task_key: RedisKey = Keys::task(task_id).into();
                self.redis.del(vec![task_key.clone()]).await?;

                // Clean up dependencies
                let deps_key: RedisKey = Keys::pending_deps(task_id).into();
                self.redis.del(vec![deps_key]).await?;

                let task_deps_key: RedisKey = Keys::task_deps(task_id).into();
                self.redis.del(vec![task_deps_key]).await?;

                // Clean up progress tracking
                let progress_key: RedisKey = Keys::progress(task_id).into();
                self.redis.del(vec![progress_key]).await?;

                tracing::info!(
                    "Task '{}' cancelled from {} queue '{}'",
                    task_id,
                    status,
                    queue_name
                );
                return Ok(Some(status.to_string()));
            }
        }

        // Task not found in any queue
        tracing::debug!(
            "Task '{}' not found in any queue for '{}'",
            task_id,
            queue_name
        );
        Ok(None)
    }

    /// Cancel a task with deduplication key cleanup
    ///
    /// Cancels a task and removes its unique key from the deduplication set.
    ///
    /// Returns the cancellation status (see cancel_task for details).
    pub async fn cancel_task_with_unique(
        &self,
        task_id: &str,
        queue_name: &str,
        unique_key: Option<&str>,
    ) -> Result<Option<String>> {
        let result = self.cancel_task(task_id, queue_name).await?;

        if result.is_some() {
            if let Some(key) = unique_key {
                // Remove from deduplication set
                let dedup_key: RedisKey = Keys::dedup(queue_name).into();
                self.redis.srem(dedup_key, key.into()).await?;
            }
        }

        Ok(result)
    }

    /// Retry a failed task from the dead letter queue
    ///
    /// Moves a task from the dead letter queue back to the pending queue for reprocessing.
    /// Resets the task's retry count and status.
    ///
    /// # Arguments
    /// * `task_id` - The ID of the task to retry
    /// * `queue_name` - The name of the queue
    ///
    /// # Returns
    /// * `Ok(true)` - Task was found and successfully re-queued
    /// * `Ok(false)` - Task was not found in the dead queue
    /// * `Err(_)` - An error occurred during the operation
    pub async fn retry_task(&self, task_id: &str, queue_name: &str) -> Result<bool> {
        use crate::task::TaskStatus;

        // Try to find the task in dead queue
        let dead_key: RedisKey = Keys::dead(queue_name).into();
        let dead_tasks = self.redis.lrange(dead_key.clone(), 0, -1).await?;

        let mut task_found = false;
        let mut task_data: Option<Task> = None;

        // Search for the task in dead queue
        for task_id_str in dead_tasks {
            if task_id_str == task_id {
                task_found = true;

                // Load the task details
                let task_key: RedisKey = Keys::task(task_id).into();
                if let Some(data) = self.redis.get(task_key).await? {
                    let bytes = data.as_bytes()
                        .ok_or_else(|| Error::Serialization("Task data is not bytes".into()))?;

                    task_data = Some(rmp_serde::from_slice(bytes)
                        .map_err(|e| Error::Serialization(e.to_string()))?);
                }
                break;
            }
        }

        if !task_found {
            tracing::warn!("Task '{}' not found in dead queue '{}'", task_id, queue_name);
            return Ok(false);
        }

        let mut task = task_data.ok_or_else(|| Error::Validation("Failed to load task data".into()))?;

        // Reset task state for retry
        task.status = TaskStatus::Pending;
        task.retry_cnt = 0;
        task.last_error = None;
        task.processed_at = None;

        // Update task in Redis
        let task_key: RedisKey = Keys::task(task_id).into();
        let new_data = rmp_serde::to_vec(&task)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        self.redis.set(task_key, RedisValue::Bytes(new_data.into())).await?;

        // Remove from dead queue
        self.redis.lrem(dead_key, task_id.into(), 1).await?;

        // Add to pending queue
        let queue_key: RedisKey = Keys::queue(queue_name).into();
        self.redis.rpush(queue_key, task_id.into()).await?;

        tracing::info!("Task '{}' re-queued for processing in queue '{}'", task_id, queue_name);
        Ok(true)
    }
}

/// Client builder
#[derive(Debug, Default)]
pub struct ClientBuilder {
    config: ClientConfig,
}

impl ClientBuilder {
    /// Set Redis URL
    #[must_use]
    pub fn redis_url(mut self, url: impl Into<String>) -> Self {
        self.config.redis_url = url.into();
        self
    }

    /// Set Redis connection mode to Cluster
    ///
    /// # Example
    ///
    /// ```rust
    /// use rediq::client::Client;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::builder()
    ///     .redis_url("redis://cluster-node1:6379")
    ///     .cluster_mode()
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn cluster_mode(mut self) -> Self {
        self.config.redis_mode = RedisMode::Cluster;
        self
    }

    /// Set Redis connection mode to Sentinel
    ///
    /// # Example
    ///
    /// ```rust
    /// use rediq::client::Client;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::builder()
    ///     .redis_url("redis://sentinel-1:26379")
    ///     .sentinel_mode()
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn sentinel_mode(mut self) -> Self {
        self.config.redis_mode = RedisMode::Sentinel;
        self
    }

    /// Set connection pool size
    #[must_use]
    pub fn pool_size(mut self, size: usize) -> Self {
        self.config.pool_size = size;
        self
    }

    /// Set default queue
    #[must_use]
    pub fn default_queue(mut self, queue: impl Into<String>) -> Self {
        self.config.default_queue = queue.into();
        self
    }

    /// Build Client
    pub async fn build(self) -> Result<Client> {
        let redis = match self.config.redis_mode {
            RedisMode::Standalone => RedisClient::from_url(&self.config.redis_url).await?,
            RedisMode::Cluster => RedisClient::from_cluster_url(&self.config.redis_url).await?,
            RedisMode::Sentinel => RedisClient::from_sentinel_url(&self.config.redis_url).await?,
        };
        Ok(Client {
            redis,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_builder() {
        let builder = Client::builder()
            .redis_url("redis://localhost:6380")
            .pool_size(20)
            .default_queue("custom");

        assert_eq!(builder.config.redis_url, "redis://localhost:6380");
        assert_eq!(builder.config.pool_size, 20);
        assert_eq!(builder.config.default_queue, "custom");
    }

    #[test]
    fn test_client_config_default() {
        let config = ClientConfig::default();
        assert_eq!(config.redis_url, "redis://localhost:6379");
        assert_eq!(config.pool_size, 10);
        assert_eq!(config.default_queue, "default");
    }
}
