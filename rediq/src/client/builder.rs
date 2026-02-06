//! Client builder and enqueue implementation

use crate::{
    storage::{Keys, RedisClient},
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
    pub pool_size: usize,
    pub default_queue: String,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://localhost:6379".to_string(),
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

        // Serialize task
        let task_data = rmp_serde::to_vec(&task)
            .map_err(|e| Error::Serialization(e.to_string()))?;

        // Store task details
        let task_key: RedisKey = Keys::task(&task_id).into();
        self.redis.set(task_key, RedisValue::Bytes(task_data.into())).await?;

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
        let redis = RedisClient::from_url(&self.config.redis_url).await?;
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
