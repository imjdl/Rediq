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

    /// List tasks in a queue
    pub async fn list_tasks(&self, queue_name: &str, limit: usize) -> Result<Vec<TaskInfo>> {
        let queue_key: RedisKey = Keys::queue(queue_name).into();
        let queue_len = self.redis.llen(queue_key.clone()).await?;

        let actual_limit = queue_len.min(limit as u64) as usize;
        if actual_limit == 0 {
            return Ok(Vec::new());
        }

        // Get task IDs from queue
        let task_ids = self.redis.lrange(queue_key, 0, actual_limit as i64 - 1).await?;

        let mut tasks = Vec::new();
        for task_id in task_ids {
            if let Ok(task_info) = self.get_task(&task_id).await {
                tasks.push(task_info);
            }
        }

        Ok(tasks)
    }

    /// List all workers
    pub async fn list_workers(&self) -> Result<Vec<WorkerInfo>> {
        let key: RedisKey = Keys::meta_workers().into();
        let members = self.redis.smembers(key).await?;

        let mut workers = Vec::new();
        for worker_id in members {
            if let Ok(info) = self.get_worker(&worker_id).await {
                workers.push(info);
            }
        }

        Ok(workers)
    }

    /// Get worker details
    pub async fn get_worker(&self, worker_id: &str) -> Result<WorkerInfo> {
        let worker_key: RedisKey = Keys::meta_worker(worker_id).into();
        let data = self.redis.get(worker_key).await?;

        if let Some(data) = data {
            let bytes = data.as_bytes()
                .ok_or_else(|| Error::Serialization("Worker data is not bytes".into()))?;

            let metadata: crate::server::worker::WorkerMetadata = rmp_serde::from_slice(bytes)
                .map_err(|e| Error::Serialization(e.to_string()))?;

            Ok(WorkerInfo {
                id: metadata.id,
                server_name: metadata.server_name,
                queues: metadata.queues,
                started_at: metadata.started_at,
                last_heartbeat: metadata.last_heartbeat,
                processed_total: metadata.processed_total,
                status: metadata.status,
            })
        } else {
            Err(Error::TaskNotFound(format!("Worker {} not found", worker_id)))
        }
    }

    /// Stop a worker by deleting its heartbeat
    ///
    /// This signals the worker to shut down gracefully.
    pub async fn stop_worker(&self, worker_id: &str) -> Result<bool> {
        let heartbeat_key: RedisKey = Keys::meta_heartbeat(worker_id).into();
        let exists = self.redis.exists(heartbeat_key.clone()).await?;

        if exists {
            // Delete heartbeat to signal shutdown
            self.redis.del(vec![heartbeat_key]).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

/// Task information
#[derive(Debug, Clone, Deserialize)]
pub struct TaskInfo {
    /// Task ID
    pub id: String,
    /// Task type
    pub task_type: String,
    /// Queue name
    pub queue: String,
    /// Task status
    pub status: TaskStatus,
    /// Retry count
    pub retry_cnt: u32,
    /// Last error message
    pub last_error: Option<String>,
    /// Creation timestamp
    pub created_at: i64,
    /// Enqueue timestamp
    pub enqueued_at: Option<i64>,
    /// Processed timestamp
    pub processed_at: Option<i64>,
}

/// Queue statistics
#[derive(Debug, Clone)]
pub struct QueueStats {
    /// Queue name
    pub name: String,
    /// Pending tasks count
    pub pending: u64,
    /// Active tasks count
    pub active: u64,
    /// Delayed tasks count
    pub delayed: u64,
    /// Retry tasks count
    pub retried: u64,
    /// Dead tasks count
    pub dead: u64,
}

/// Worker information
#[derive(Debug, Clone)]
pub struct WorkerInfo {
    /// Worker ID
    pub id: String,
    /// Server name
    pub server_name: String,
    /// Assigned queues
    pub queues: Vec<String>,
    /// Start timestamp
    pub started_at: i64,
    /// Last heartbeat timestamp
    pub last_heartbeat: i64,
    /// Total processed tasks
    pub processed_total: u64,
    /// Worker status
    pub status: String,
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
