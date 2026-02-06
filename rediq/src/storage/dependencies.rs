//! Dependency registration utilities
//!
//! Provides common functions for managing task dependencies in Redis.

use crate::{Result, Error};
use crate::storage::{Keys, RedisClient};
use fred::prelude::RedisKey;

/// Register task dependencies in Redis
///
/// This function sets up the dependency tracking data structures for a task.
/// It creates two keys in Redis:
/// 1. `pending_deps:{task_id}` - Set of all dependency task IDs the task is waiting for
/// 2. `task_deps:{dep_id}` - Set of all task IDs that depend on this task
///
/// # Arguments
/// * `redis` - Redis client
/// * `task_id` - The task ID that has dependencies
/// * `deps` - List of dependency task IDs
///
/// # Example
/// ```rust,no_run
/// # use rediq::storage::{RedisClient, dependencies};
/// # async fn example(redis: RedisClient) -> Result<(), Box<dyn std::error::Error>> {
/// let task_id = "task-b-123";
/// let deps = vec!["task-a-001".to_string(), "task-a-002".to_string()];
/// dependencies::register(&redis, task_id, &deps).await?;
/// # Ok(())
/// # }
/// ```
pub async fn register(redis: &RedisClient, task_id: &str, deps: &[String]) -> Result<()> {
    if deps.is_empty() {
        return Ok(());
    }

    let pending_deps_key: RedisKey = Keys::pending_deps(task_id).into();

    // Store all pending dependencies and register the task as dependent on each
    for dep_id in deps {
        redis.sadd(pending_deps_key.clone(), dep_id.as_str().into()).await?;

        // Register this task as dependent on each dependency
        let task_deps_key: RedisKey = Keys::task_deps(dep_id).into();
        redis.sadd(task_deps_key, task_id.into()).await?;
    }

    tracing::debug!("Task {} registered with {} dependencies", task_id, deps.len());
    Ok(())
}

/// Check and enqueue dependent tasks after a task completes
///
/// This function is called when a task completes successfully. It checks
/// if any tasks were waiting for this task and enqueues them if all their
/// dependencies are satisfied.
///
/// # Arguments
/// * `redis` - Redis client
/// * `completed_task_id` - The ID of the completed task
///
/// # Returns
/// Number of dependent tasks that were enqueued
pub async fn check_dependents(redis: &RedisClient, completed_task_id: &str) -> Result<usize> {
    // Get tasks that depend on the completed task
    let task_deps_key: RedisKey = Keys::task_deps(completed_task_id).into();
    let dependents = redis.smembers(task_deps_key.clone()).await?;

    if dependents.is_empty() {
        return Ok(0);
    }

    tracing::debug!("Task {} has {} dependent tasks", completed_task_id, dependents.len());

    let mut enqueued_count = 0;

    for dependent_id in dependents {
        let dependent_id_str = dependent_id.as_str().to_string();

        // Remove the completed task from the pending dependencies
        let pending_deps_key: RedisKey = Keys::pending_deps(&dependent_id_str).into();
        redis.srem(pending_deps_key.clone(), completed_task_id.into()).await?;

        // Check if all dependencies are satisfied
        let remaining_deps: u64 = redis.scard(pending_deps_key.clone()).await?;

        if remaining_deps == 0 {
            // All dependencies satisfied, load and enqueue the task
            if let Some(task_data) = redis.get(Keys::task(&dependent_id_str).into()).await? {
                let bytes = task_data.as_bytes()
                    .ok_or_else(|| Error::Serialization("Task data is not bytes".into()))?;

                let task: crate::Task = rmp_serde::from_slice(bytes)
                    .map_err(|e| Error::Serialization(e.to_string()))?;

                // Enqueue the task to its queue
                let queue_key: RedisKey = Keys::queue(&task.queue).into();
                redis.rpush(queue_key, dependent_id.as_str().into()).await?;

                tracing::info!("Task {} enqueued after all dependencies satisfied", dependent_id);

                // Clean up pending deps key
                redis.del(vec![pending_deps_key]).await?;

                enqueued_count += 1;
            }
        } else {
            tracing::debug!("Task {} still has {} pending dependencies", dependent_id, remaining_deps);
        }
    }

    // Clean up the task deps key for the completed task
    redis.del(vec![task_deps_key]).await?;

    Ok(enqueued_count)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_module_exists() {
        // This test just verifies the module compiles
        assert!(true);
    }
}
