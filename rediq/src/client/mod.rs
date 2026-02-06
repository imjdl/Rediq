//! Client SDK
//!
//! Provides client functionality for task enqueue and inspection.

pub mod builder;

use crate::{
    storage::{Keys, RedisClient},
    task::{TaskStatus, Task},
    Error, Result,
};
use fred::prelude::RedisKey;
use rmp_serde;
use serde::Deserialize;

pub use builder::{Client, ClientBuilder};

/// Task inspector
///
/// Used to query task status and queue statistics
pub struct Inspector {
    redis: RedisClient,
}

impl Inspector {
    /// Create a new inspector
    pub fn new(redis: RedisClient) -> Self {
        Self { redis }
    }

    /// Get task details
    pub async fn get_task(&self, task_id: &str) -> Result<TaskInfo> {
        let key: RedisKey = Keys::task(task_id).into();
        let data = self.redis.get(key).await?.ok_or(Error::TaskNotFound(task_id.to_string()))?;

        // Get task data from Redis
        let bytes = data.as_bytes().ok_or_else(|| {
            Error::Serialization("Task data is not bytes".to_string())
        })?;

        // Deserialize task
        let task: Task = rmp_serde::from_slice(bytes)
            .map_err(|e| Error::Serialization(e.to_string()))?;

        Ok(TaskInfo {
            id: task.id,
            task_type: task.task_type,
            queue: task.queue,
            status: task.status,
            retry_cnt: task.retry_cnt,
            last_error: task.last_error,
            created_at: task.created_at,
            enqueued_at: task.enqueued_at,
            processed_at: task.processed_at,
        })
    }

    /// Get queue statistics
    pub async fn queue_stats(&self, queue_name: &str) -> Result<QueueStats> {
        let queue_key: RedisKey = Keys::queue(queue_name).into();
        let active_key: RedisKey = Keys::active(queue_name).into();
        let delayed_key: RedisKey = Keys::delayed(queue_name).into();
        let retry_key: RedisKey = Keys::retry(queue_name).into();
        let dead_key: RedisKey = Keys::dead(queue_name).into();

        let pending = self.redis.llen(queue_key).await?;
        let active = self.redis.llen(active_key).await?;
        let delayed = self.redis.zrangebyscore(delayed_key, 0, i64::MAX).await?.len() as u64;
        let retry = self.redis.zrangebyscore(retry_key, 0, i64::MAX).await?.len() as u64;
        let dead = self.redis.llen(dead_key).await?;

        Ok(QueueStats {
            name: queue_name.to_string(),
            pending,
            active,
            delayed: delayed as u64,
            retried: retry as u64,
            dead,
        })
    }

    /// List all queues
    pub async fn list_queues(&self) -> Result<Vec<String>> {
        let key: RedisKey = Keys::meta_queues().into();
        let _members = self.redis.get(key).await?;

        // TODO: Implement getting queue list from SMEMBERS
        Ok(Vec::new())
    }
}

/// Task information
#[derive(Debug, Clone, Deserialize)]
pub struct TaskInfo {
    pub id: String,
    pub task_type: String,
    pub queue: String,
    pub status: TaskStatus,
    pub retry_cnt: u32,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub enqueued_at: Option<i64>,
    pub processed_at: Option<i64>,
}

/// Queue statistics
#[derive(Debug, Clone)]
pub struct QueueStats {
    pub name: String,
    pub pending: u64,
    pub active: u64,
    pub delayed: u64,
    pub retried: u64,
    pub dead: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_stats_default() {
        let stats = QueueStats {
            name: "default".to_string(),
            pending: 10,
            active: 2,
            delayed: 5,
            retried: 1,
            dead: 0,
        };

        assert_eq!(stats.name, "default");
        assert_eq!(stats.pending, 10);
    }
}
