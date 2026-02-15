//! Worker implementation
//!
//! Workers are the core processing units that dequeue and handle tasks.

use crate::{
    storage::{Keys, RedisClient, dependencies},
    Error, Result, Task,
    task::TaskStatus,
    progress::{ProgressContext, ProgressConfig},
};
use crate::processor::Mux;
use crate::server::config::ServerState;
use crate::task::progress_ext::set_progress_context;
use chrono::Utc;
use fred::prelude::{RedisKey, RedisValue};
use rmp_serde;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

/// Worker - task processing unit
///
/// Each worker continuously polls queues, dequeues tasks, and processes them
/// through registered handlers.
pub struct Worker {
    /// Unique worker ID
    pub id: String,

    /// Shared server state
    state: Arc<ServerState>,

    /// Shutdown flag
    shutdown: Arc<AtomicBool>,

    /// Task processor router
    mux: Arc<Mutex<Mux>>,

    /// Current queue index for round-robin polling
    queue_index: Arc<AtomicUsize>,

    /// Cached queue references for cheap access (avoids cloning String)
    queues: Vec<Arc<String>>,
}

impl Worker {
    /// Create a new worker
    pub fn new(
        id: String,
        state: Arc<ServerState>,
        shutdown: Arc<AtomicBool>,
        mux: Arc<Mutex<Mux>>,
    ) -> Self {
        // Cache queue references to avoid cloning in hot path
        let queues: Vec<Arc<String>> = state.config.queues
            .iter()
            .map(|s| Arc::new(s.clone()))
            .collect();

        Self {
            id,
            state,
            shutdown,
            mux,
            queue_index: Arc::new(AtomicUsize::new(0)),
            queues,
        }
    }

    /// Run the worker
    ///
    /// This method:
    /// 1. Registers the worker in Redis
    /// 2. Starts the heartbeat task
    /// 3. Enters the task processing loop
    /// 4. Unregisters the worker on shutdown
    pub async fn run(self) -> Result<()> {
        tracing::info!("Worker {} starting", self.id);

        // Register worker
        self.register().await?;

        // Start heartbeat task
        let heartbeat = self.start_heartbeat();

        // Task processing loop
        let result = self.task_loop().await;

        // Unregister worker
        if let Err(e) = self.unregister().await {
            tracing::error!("Failed to unregister worker: {}", e);
        }

        // Cancel heartbeat
        heartbeat.abort();

        tracing::info!("Worker {} stopped", self.id);
        result
    }

    /// Register worker in Redis
    async fn register(&self) -> Result<()> {
        let metadata = WorkerMetadata {
            id: self.id.clone(),
            server_name: self.state.config.server_name.clone(),
            queues: self.state.config.queues.clone(),
            started_at: Utc::now().timestamp(),
            last_heartbeat: Utc::now().timestamp(),
            processed_total: 0,
            status: "idle".to_string(),
        };

        // Store metadata
        let data = rmp_serde::to_vec(&metadata)
            .map_err(|e| Error::Serialization(e.to_string()))?;

        let worker_key: RedisKey = Keys::meta_worker(&self.id).into();
        self.state.redis.set(worker_key, RedisValue::Bytes(data.into())).await?;

        // Add to workers set
        let workers_key: RedisKey = Keys::meta_workers().into();
        self.state.redis.sadd(workers_key, self.id.as_str().into()).await?;

        // Add queues to meta:queues set
        let queues_key: RedisKey = Keys::meta_queues().into();
        for queue in &self.state.config.queues {
            self.state.redis.sadd(queues_key.clone(), queue.as_str().into()).await?;
        }

        // Initial heartbeat
        self.update_heartbeat().await?;

        tracing::debug!("Worker {} registered", self.id);
        Ok(())
    }

    /// Unregister worker from Redis
    async fn unregister(&self) -> Result<()> {
        // Remove from workers set
        let workers_key: RedisKey = Keys::meta_workers().into();
        self.state.redis.srem(workers_key, self.id.as_str().into()).await?;

        // Delete metadata
        let worker_key: RedisKey = Keys::meta_worker(&self.id).into();
        self.state.redis.del(vec![worker_key]).await?;

        // Delete heartbeat
        let heartbeat_key: RedisKey = Keys::meta_heartbeat(&self.id).into();
        self.state.redis.del(vec![heartbeat_key]).await?;

        tracing::debug!("Worker {} unregistered", self.id);
        Ok(())
    }

    /// Start heartbeat task
    fn start_heartbeat(&self) -> JoinHandle<()> {
        let id = self.id.clone();
        let redis = self.state.redis.clone();
        let interval = Duration::from_secs(self.state.config.heartbeat_interval);
        let worker_timeout = self.state.config.worker_timeout;
        let ttl_multiplier = self.state.config.heartbeat_ttl_multiplier;
        let shutdown = self.shutdown.clone();

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);

            while !shutdown.load(Ordering::Relaxed) {
                ticker.tick().await;

                if let Err(e) = Self::update_heartbeat_for(&id, &redis, worker_timeout, ttl_multiplier).await {
                    tracing::error!("Heartbeat update failed: {}", e);
                }
            }
        })
    }

    /// Update heartbeat for this worker
    async fn update_heartbeat(&self) -> Result<()> {
        Self::update_heartbeat_for(
            &self.id,
            &self.state.redis,
            self.state.config.worker_timeout,
            self.state.config.heartbeat_ttl_multiplier,
        ).await
    }

    /// Static method to update heartbeat for any worker
    ///
    /// # Important
    ///
    /// The heartbeat TTL is calculated based on worker_timeout and ttl_multiplier
    /// to ensure that workers are not incorrectly marked as dead due to network delays
    /// or temporary processing slowdowns.
    async fn update_heartbeat_for(worker_id: &str, redis: &RedisClient, worker_timeout: u64, ttl_multiplier: f64) -> Result<()> {
        let heartbeat_key: RedisKey = Keys::meta_heartbeat(worker_id).into();
        let now = Utc::now().timestamp();

        redis.set(heartbeat_key.clone(), now.to_string().into()).await?;

        // Set expiration based on worker_timeout and multiplier
        // Use multiplier to provide a safety margin for network issues
        // This ensures healthy workers aren't incorrectly marked as dead
        let ttl = (worker_timeout as f64 * ttl_multiplier) as u64;
        redis.expire(heartbeat_key, ttl).await?;

        tracing::trace!("Heartbeat updated for worker {}, TTL: {}s (multiplier: {})", worker_id, ttl, ttl_multiplier);

        Ok(())
    }

    /// Main task processing loop
    async fn task_loop(&self) -> Result<()> {
        while !self.shutdown.load(Ordering::Relaxed) {
            // Get next queue (round-robin)
            let queue = self.next_queue();

            match self.dequeue_task_any(&queue).await {
                Ok(Some(task)) => {
                    // Process task
                    let result = self.process_task(task).await;

                    // Update status
                    match result {
                        Ok(_) => {
                            tracing::debug!("Task processed successfully");
                        }
                        Err(e) => {
                            tracing::error!("Task processing failed: {}", e);
                        }
                    }
                }
                Ok(None) => {
                    // No task available, wait before polling again
                    tokio::time::sleep(Duration::from_millis(self.state.config.poll_interval)).await;
                }
                Err(Error::QueuePaused(_)) => {
                    // Queue paused, wait longer
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                Err(Error::Shutdown) => {
                    break;
                }
                Err(e) => {
                    // Check if this is a timeout (normal when queue is empty)
                    let error_msg = e.to_string();
                    if error_msg.contains("Timeout") || error_msg.contains("timed out") {
                        // Timeout is normal - queue is empty, just wait and retry
                        tracing::debug!("Queue empty, waiting for tasks...");
                    } else {
                        tracing::warn!("Dequeue error: {}", e);
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }

        Ok(())
    }

    /// Get next queue using round-robin
    ///
    /// This method uses Arc<String> to avoid expensive String clones.
    /// Cloning an Arc is much cheaper than cloning a String.
    fn next_queue(&self) -> Arc<String> {
        let index = self.queue_index.fetch_add(1, Ordering::Relaxed) % self.queues.len();
        Arc::clone(&self.queues[index])
    }

    /// Dequeue a task from the specified queue
    async fn dequeue_task(&self, queue: &str) -> Result<Option<Task>> {
        // Check if queue is paused
        let pause_key: RedisKey = Keys::pause(queue).into();
        if self.state.redis.exists(pause_key).await? {
            return Err(Error::QueuePaused(queue.to_string()));
        }

        // Try to get task from queue
        let queue_key: RedisKey = Keys::queue(queue).into();
        let timeout = self.state.config.dequeue_timeout;

        match self.state.redis.blpop(queue_key, timeout).await? {
            Some((_, task_id)) => {
                // Move to active queue
                let active_key: RedisKey = Keys::active(queue).into();
                self.state.redis.lpush(active_key, task_id.as_str().into()).await?;

                // Load full task data
                self.load_task(&task_id).await.map(Some)
            }
            None => Ok(None),
        }
    }

    /// Dequeue a task from priority queue
    ///
    /// Priority queues use ZSet where lower score = higher priority
    async fn dequeue_task_priority(&self, queue: &str) -> Result<Option<Task>> {
        let pqueue_key: RedisKey = Keys::priority_queue(queue).into();
        let active_key: RedisKey = Keys::active(queue).into();
        let pause_key: RedisKey = Keys::pause(queue).into();
        // Use a dummy key prefix for the task (the script constructs the actual task key)
        let dummy_key: RedisKey = "rediq:task:*".to_string().into();
        let task_ttl = crate::config::get_task_ttl() as usize;

        // Use pdequeue.lua for atomic dequeue (check pause + zrange + zrem + lpush)
        match self.state.redis.pdequeue_lua(
            pqueue_key,
            active_key,
            pause_key,
            dummy_key,
            task_ttl,
        ).await? {
            task_id if !task_id.is_empty() => {
                // Task was dequeued successfully, now load the full task data
                self.load_task(&task_id).await.map(Some)
            }
            _ => Ok(None), // No task available (timeout or queue empty)
        }
    }

    /// Dequeue task - tries priority queue first, then regular queue
    ///
    /// This method attempts to dequeue from the priority queue first without
    /// checking its size beforehand. This avoids a race condition where another
    /// worker could take the last task between the check and the dequeue operation.
    ///
    /// If the priority queue is empty (returns no task), it falls back to the
    /// regular queue.
    async fn dequeue_task_any(&self, queue: &str) -> Result<Option<Task>> {
        // Try priority queue first without pre-checking size
        // This avoids race condition: zcard > 0 but another worker takes the task
        tracing::debug!("Attempting to dequeue from priority queue {}", queue);
        match self.dequeue_task_priority(queue).await {
            Ok(Some(task)) => {
                tracing::debug!("Successfully dequeued from priority queue {}", queue);
                return Ok(Some(task));
            }
            Ok(None) => {
                // Priority queue is empty, fall back to regular queue
                tracing::debug!("Priority queue {} is empty, trying regular queue", queue);
            }
            Err(Error::QueuePaused(_)) => {
                // Queue is paused, propagate the error
                return Err(Error::QueuePaused(queue.to_string()));
            }
            Err(e) => {
                // Log other errors but continue to regular queue
                tracing::warn!("Priority queue dequeue failed: {}, trying regular queue", e);
            }
        }

        // Fall back to regular queue
        self.dequeue_task(queue).await
    }

    /// Load task data from Redis
    async fn load_task(&self, task_id: &str) -> Result<Task> {
        let task_key: RedisKey = Keys::task(task_id).into();
        // Get task data from hash field (stored as 'data' field)
        let data = self.state.redis.hget(task_key.clone(), "data".into()).await?
            .ok_or_else(|| Error::TaskNotFound(task_id.to_string()))?;

        let bytes = data.as_bytes()
            .ok_or_else(|| Error::Serialization("Task data is not bytes".into()))?;

        let mut task: Task = rmp_serde::from_slice(bytes)
            .map_err(|e| Error::Serialization(e.to_string()))?;

        // Update status to active
        task.status = TaskStatus::Active;
        task.processed_at = Some(Utc::now().timestamp());

        // Update task status in Redis
        let current_timestamp = Utc::now().timestamp();
        self.state.redis.hset(
            task_key.clone(),
            vec![
                ("status".into(), current_timestamp.to_string().into()),
            ],
        ).await?;

        Ok(task)
    }

    /// Process a task
    async fn process_task(&self, mut task: Task) -> Result<()> {
        tracing::debug!("Processing task: {}", task.description());

        // Setup progress context before execution
        let progress_config = ProgressConfig::default();
        let progress_ctx = ProgressContext::new(
            task.id.clone(),
            self.state.redis.clone(),
            progress_config,
        );

        // Set progress context for this task
        set_progress_context(Some(progress_ctx.clone()));

        // Initialize progress to 0
        let _ = progress_ctx.report(0).await;

        // Execute middleware before hooks
        if !self.state.middleware.is_empty() {
            self.state.middleware.before(&task).await?;
        }

        // Get handler and process with timeout
        let mux = self.mux.lock().await;
        let handler = mux.process(&task);

        // Apply timeout
        let result = tokio::time::timeout(task.options.timeout, handler).await;

        let process_result = match result {
            Ok(r) => r,
            Err(_) => {
                task.last_error = Some("Task timed out".to_string());
                Err(Error::Timeout(format!("Task {} timed out after {:?}", task.id, task.options.timeout)))
            }
        };

        drop(mux); // Release lock after handler completes

        // Execute middleware after hooks
        if !self.state.middleware.is_empty() {
            let _ = self.state.middleware.after(&task, &process_result).await;
        }

        // Clear progress context
        set_progress_context(None);

        // Update task status based on result
        match &process_result {
            Ok(_) => {
                // Set final progress to 100% on success
                let _ = progress_ctx.report(100).await;
                self.ack_task(&task, TaskStatus::Processed, None).await?;
            }
            Err(e) => {
                task.last_error = Some(e.to_string());
                task.retry_cnt += 1;

                if task.can_retry() {
                    self.ack_task(&task, TaskStatus::Retry, Some(e)).await?;
                    self.schedule_retry(&task).await?;
                } else {
                    self.ack_task(&task, TaskStatus::Dead, Some(e)).await?;
                    // Fail dependent tasks when this task enters dead queue
                    if let Err(dep_err) = self.fail_dependent_tasks(&task.id, &task.queue).await {
                        tracing::error!("Failed to propagate failure to dependent tasks: {}", dep_err);
                    }
                }
            }
        }

        process_result
    }

    /// Acknowledge task completion
    async fn ack_task(&self, task: &Task, status: TaskStatus, error: Option<&Error>) -> Result<()> {
        let active_key: RedisKey = Keys::active(&task.queue).into();
        let task_key: RedisKey = Keys::task(&task.id).into();

        // Remove from active queue
        self.state.redis.lrem(active_key, task.id.as_str().into(), 1).await?;

        // Update task in Redis (store in hash fields)
        let mut task_data = task.clone();
        task_data.status = status;
        task_data.last_error = error.map(|e| e.to_string());

        let data = rmp_serde::to_vec(&task_data)
            .map_err(|e| Error::Serialization(e.to_string()))?;

        // Store as separate hash fields
        self.state.redis.hset(
            task_key.clone(),
            vec![
                ("data".into(), RedisValue::Bytes(data.into())),
                ("queue".into(), task.queue.as_str().into()),
            ],
        ).await?;

        // Update TTL
        let task_ttl = crate::config::get_task_ttl() as u64;
        self.state.redis.expire(task_key, task_ttl).await?;

        // Update worker processed_total on successful completion
        if status == TaskStatus::Processed {
            self.increment_processed().await?;
            // Update queue statistics
            let stats_key: RedisKey = Keys::stats(&task.queue).into();
            let field_key: RedisKey = "processed".into();
            let _ = self.state.redis.hincrby(stats_key, field_key, 1).await;
        }

        // If task was successfully processed, check for dependent tasks
        if status == TaskStatus::Processed {
            if let Err(e) = self.check_dependent_tasks(&task.id).await {
                tracing::error!("Failed to check dependent tasks: {}", e);
            }
        }

        Ok(())
    }

    /// Increment worker processed count
    async fn increment_processed(&self) -> Result<()> {
        // Get current metadata
        let worker_key: RedisKey = Keys::meta_worker(&self.id).into();
        let data = self.state.redis.get(worker_key.clone()).await?
            .ok_or_else(|| Error::Validation("Worker metadata not found".into()))?;

        let bytes = data.as_bytes()
            .ok_or_else(|| Error::Serialization("Worker data is not bytes".into()))?;

        let mut metadata: WorkerMetadata = rmp_serde::from_slice(bytes)
            .map_err(|e| Error::Serialization(e.to_string()))?;

        // Increment and update
        metadata.processed_total += 1;
        metadata.last_heartbeat = Utc::now().timestamp();

        let new_data = rmp_serde::to_vec(&metadata)
            .map_err(|e| Error::Serialization(e.to_string()))?;

        self.state.redis.set(worker_key, RedisValue::Bytes(new_data.into())).await?;

        Ok(())
    }

    /// Check and enqueue dependent tasks
    ///
    /// Called when a task completes successfully. Checks if any tasks were waiting for
    /// this task and enqueues them if all their dependencies are satisfied.
    async fn check_dependent_tasks(&self, completed_task_id: &str) -> Result<()> {
        dependencies::check_dependents(&self.state.redis, completed_task_id).await?;
        Ok(())
    }

    /// Fail dependent tasks when a task enters dead queue
    ///
    /// Called when a task fails permanently. Finds all tasks that depend on this
    /// failed task and moves them to the dead queue to prevent dependency deadlocks.
    async fn fail_dependent_tasks(&self, failed_task_id: &str, queue: &str) -> Result<()> {
        let count = dependencies::fail_dependents(&self.state.redis, failed_task_id, queue).await?;
        if count > 0 {
            tracing::warn!(
                "Task {} failed, {} dependent tasks moved to dead queue",
                failed_task_id,
                count
            );
        }
        Ok(())
    }

    /// Schedule task for retry
    async fn schedule_retry(&self, task: &Task) -> Result<()> {
        let delay = task.retry_delay()
            .ok_or_else(|| Error::Validation("No retry delay available".into()))?;

        let execute_at = Utc::now().timestamp() + delay.as_secs() as i64;
        let retry_key: RedisKey = Keys::retry(&task.queue).into();

        self.state.redis.zadd(
            retry_key,
            task.id.as_str().into(),
            execute_at,
        ).await?;

        tracing::debug!("Task {} scheduled for retry in {:?}", task.id, delay);
        Ok(())
    }
}

/// Worker metadata stored in Redis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerMetadata {
    /// Worker ID
    pub id: String,

    /// Server name
    pub server_name: String,

    /// Queues being processed
    pub queues: Vec<String>,

    /// Start time (Unix timestamp)
    pub started_at: i64,

    /// Last heartbeat time (Unix timestamp)
    pub last_heartbeat: i64,

    /// Total tasks processed
    pub processed_total: u64,

    /// Current status
    pub status: String,
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_next_queue_round_robin() {
        // This is a basic structural test
        // Full worker tests require Redis
    }
}
