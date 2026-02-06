//! Client builder and enqueue implementation

use crate::{
    storage::{Keys, RedisClient, RedisMode},
    Error, Result,
};
use crate::task::Task;
use chrono::Utc;
use fred::prelude::{RedisKey, RedisValue};
use rmp_serde;

/// Client configuration
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub redis_url: String,
    pub redis_mode: RedisMode,
    pub pool_size: usize,
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
    config: ClientConfig,
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

        // Add task_id to queue
        let queue_key: RedisKey = Keys::queue(&queue).into();
        self.redis.rpush(queue_key, task_id.as_str().into()).await?;

        // Handle deduplication
        if let Some(key) = unique_key {
            let dedup_key: RedisKey = Keys::dedup(&queue).into();
            self.redis.sadd(dedup_key, key.into()).await?;
        }

        // Register queue
        // TODO: Use SADD to add to meta:queues

        tracing::debug!("Task enqueued: {}", task_id);
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

        // Validate priority
        if priority < 0 || priority > 100 {
            return Err(Error::Validation(format!(
                "Priority must be between 0 and 100, got {}",
                priority
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
        let now = chrono::Utc::now();
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

        tracing::debug!("Cron task enqueued: {} (cron: {}, next: {})", task_id, cron_expr, next_time);
        Ok(task_id)
    }

    /// Batch enqueue
    pub async fn enqueue_batch(&self, tasks: Vec<Task>) -> Result<Vec<String>> {
        let mut task_ids = Vec::with_capacity(tasks.len());

        for task in tasks {
            let task_id = task.id.clone();
            task_ids.push(task_id.clone());

            // Serialize task
            let task_data = rmp_serde::to_vec(&task)
                .map_err(|e| Error::Serialization(e.to_string()))?;

            // Store task details
            let task_key: RedisKey = Keys::task(&task_id).into();
            self.redis.set(task_key, RedisValue::Bytes(task_data.into())).await?;

            // Add task_id to queue
            let queue_key: RedisKey = Keys::queue(&task.queue).into();
            self.redis.rpush(queue_key, task_id.as_str().into()).await?;

            // Handle deduplication
            if let Some(key) = &task.options.unique_key {
                let dedup_key: RedisKey = Keys::dedup(&task.queue).into();
                self.redis.sadd(dedup_key, key.as_str().into()).await?;
            }
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
    /// Attempts to cancel a task by removing it from the queue.
    /// Returns true if the task was found and cancelled, false otherwise.
    pub async fn cancel_task(&self, task_id: &str, queue_name: &str) -> Result<bool> {
        // Try to remove from pending queue
        let queue_key: RedisKey = Keys::queue(queue_name).into();
        let removed = self.redis.lrem(queue_key, task_id.into(), 1).await?;

        if removed > 0 {
            // Task was in pending queue, clean up
            // Delete task details
            let task_key: RedisKey = Keys::task(task_id).into();
            self.redis.del(vec![task_key]).await?;

            tracing::info!("Task '{}' cancelled from queue '{}'", task_id, queue_name);
            Ok(true)
        } else {
            // Task not found in pending queue
            // It might be in active, delayed, or retry queues
            // For now, we only support cancelling pending tasks
            tracing::warn!("Task '{}' not found in pending queue '{}'", task_id, queue_name);
            Ok(false)
        }
    }

    /// Cancel a task with deduplication key cleanup
    ///
    /// Cancels a task and removes its unique key from the deduplication set.
    pub async fn cancel_task_with_unique(&self, task_id: &str, queue_name: &str, unique_key: Option<&str>) -> Result<bool> {
        let cancelled = self.cancel_task(task_id, queue_name).await?;

        if cancelled && unique_key.is_some() {
            // Remove from deduplication set
            let dedup_key: RedisKey = Keys::dedup(queue_name).into();
            self.redis.srem(dedup_key, unique_key.unwrap().into()).await?;
        }

        Ok(cancelled)
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
            config: self.config,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

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
