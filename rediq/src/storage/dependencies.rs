//! Dependency registration utilities
//!
//! Provides common functions for managing task dependencies in Redis.

use crate::{Result, Error};
use crate::storage::{Keys, RedisClient};
use fred::prelude::{RedisKey, RedisValue};

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
///
/// # Reliability Note
///
/// This function includes error recovery: if enqueuing a dependent task fails,
/// it continues processing other dependents rather than aborting entirely.
/// Failed dependents can be recovered by a manual re-run of dependency checking.
pub async fn check_dependents(redis: &RedisClient, completed_task_id: &str) -> Result<usize> {
    // Get tasks that depend on the completed task
    let task_deps_key: RedisKey = Keys::task_deps(completed_task_id).into();
    let dependents = redis.smembers(task_deps_key.clone()).await?;

    if dependents.is_empty() {
        return Ok(0);
    }

    tracing::debug!("Task {} has {} dependent tasks", completed_task_id, dependents.len());

    let mut enqueued_count = 0;
    let mut failed_count = 0;

    for dependent_id in dependents {
        let dependent_id_str = dependent_id.as_str().to_string();

        // Remove the completed task from the pending dependencies
        let pending_deps_key: RedisKey = Keys::pending_deps(&dependent_id_str).into();
        redis.srem(pending_deps_key.clone(), completed_task_id.into()).await?;

        // Check if all dependencies are satisfied
        let remaining_deps: u64 = redis.scard(pending_deps_key.clone()).await?;

        if remaining_deps == 0 {
            // All dependencies satisfied, get queue name and enqueue
            let task_key: RedisKey = Keys::task(&dependent_id_str).into();

            // Get queue name from hash field (more efficient than deserializing entire task)
            match redis.hget(task_key.clone(), "queue".into()).await? {
                Some(queue_value) => {
                    // Get queue name as string slice
                    let queue_name = match queue_value.as_bytes() {
                        Some(bytes) => std::str::from_utf8(bytes)
                            .map_err(|e| Error::Serialization(format!("Invalid queue name: {}", e)))?,
                        None => {
                            tracing::warn!("Task {} has invalid queue field, skipping", dependent_id);
                            failed_count += 1;
                            // Clean up to prevent infinite retry
                            let _ = redis.del(vec![pending_deps_key]).await;
                            continue;
                        }
                    };

                    // Enqueue the task to its queue
                    let queue_key: RedisKey = Keys::queue(queue_name).into();
                    match redis.rpush(queue_key, dependent_id.as_str().into()).await {
                        Ok(_) => {
                            tracing::info!("Task {} enqueued to queue '{}' after all dependencies satisfied", dependent_id, queue_name);

                            // Clean up pending deps key
                            let _ = redis.del(vec![pending_deps_key]).await;

                            enqueued_count += 1;
                        }
                        Err(e) => {
                            tracing::error!("Failed to enqueue dependent task {}: {}", dependent_id, e);
                            failed_count += 1;
                            // Don't clean up pending_deps so this can be retried
                        }
                    }
                }
                None => {
                    tracing::warn!("Task {} not found, skipping", dependent_id);
                    failed_count += 1;
                    // Clean up to prevent infinite retry
                    let _ = redis.del(vec![pending_deps_key]).await;
                }
            }
        } else {
            tracing::debug!("Task {} still has {} pending dependencies", dependent_id, remaining_deps);
        }
    }

    // Clean up the task deps key for the completed task
    redis.del(vec![task_deps_key]).await?;

    if failed_count > 0 {
        tracing::warn!("{} dependent tasks failed to enqueue, may require manual retry", failed_count);
    }

    Ok(enqueued_count)
}

/// Fail all dependent tasks when a task enters the dead letter queue
///
/// This function is called when a task fails permanently (enters dead queue).
/// It finds all tasks that depend on this failed task and moves them to the
/// dead queue as well, preventing dependency deadlocks.
///
/// # Arguments
/// * `redis` - Redis client
/// * `failed_task_id` - The ID of the task that failed permanently
/// * `queue` - The queue name for the dependent tasks
///
/// # Returns
/// Number of dependent tasks that were failed
pub async fn fail_dependents(redis: &RedisClient, failed_task_id: &str, queue: &str) -> Result<usize> {
    // Get tasks that depend on the failed task
    let task_deps_key: RedisKey = Keys::task_deps(failed_task_id).into();
    let dependents = redis.smembers(task_deps_key.clone()).await?;

    if dependents.is_empty() {
        return Ok(0);
    }

    tracing::warn!(
        "Task {} failed permanently, failing {} dependent tasks",
        failed_task_id,
        dependents.len()
    );

    let mut failed_count = 0;

    for dependent_id in &dependents {
        let dependent_id_str = dependent_id.as_str();

        // Get the task data to update its status
        let task_key: RedisKey = Keys::task(dependent_id_str).into();

        match redis.hget(task_key.clone(), "data".into()).await? {
            Some(data) => {
                if let Some(bytes) = data.as_bytes() {
                    match rmp_serde::from_slice::<crate::Task>(bytes) {
                        Ok(mut task) => {
                            // Update task status and error
                            task.status = crate::task::TaskStatus::Dead;
                            task.last_error = Some(format!(
                                "Dependency task {} failed permanently",
                                failed_task_id
                            ));

                            // Serialize updated task
                            match rmp_serde::to_vec(&task) {
                                Ok(new_data) => {
                                    // Update task in Redis
                                    if let Err(e) = redis
                                        .hset(
                                            task_key.clone(),
                                            vec![
                                                ("data".into(), RedisValue::Bytes(new_data.into())),
                                                ("queue".into(), queue.into()),
                                            ],
                                        )
                                        .await
                                    {
                                        tracing::error!(
                                            "Failed to update dependent task {}: {}",
                                            dependent_id_str,
                                            e
                                        );
                                        continue;
                                    }

                                    // Add to dead queue
                                    let dead_key: RedisKey = Keys::dead(queue).into();
                                    if let Err(e) = redis
                                        .lpush(dead_key, dependent_id.as_str().into())
                                        .await
                                    {
                                        tracing::error!(
                                            "Failed to add dependent task {} to dead queue: {}",
                                            dependent_id_str,
                                            e
                                        );
                                        continue;
                                    }

                                    tracing::info!(
                                        "Dependent task {} moved to dead queue due to failed dependency {}",
                                        dependent_id_str,
                                        failed_task_id
                                    );
                                    failed_count += 1;
                                }
                                Err(e) => {
                                    tracing::error!(
                                        "Failed to serialize dependent task {}: {}",
                                        dependent_id_str,
                                        e
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(
                                "Failed to deserialize dependent task {}: {}",
                                dependent_id_str,
                                e
                            );
                        }
                    }
                }
            }
            None => {
                tracing::warn!("Dependent task {} data not found, skipping", dependent_id_str);
            }
        }

        // Clean up pending dependencies for this dependent task
        let pending_deps_key: RedisKey = Keys::pending_deps(dependent_id_str).into();
        let _ = redis.del(vec![pending_deps_key]).await;
    }

    // Clean up the task deps key for the failed task
    let _ = redis.del(vec![task_deps_key]).await;

    Ok(failed_count)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_module_exists() {
        // This test just verifies the module compiles
        assert!(true);
    }
}
