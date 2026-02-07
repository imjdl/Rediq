//! Task command handlers
//!
//! Provides commands for listing, inspecting, canceling, and retrying tasks.

use clap::Subcommand;
use color_eyre::Result;
use rediq::client::Client;

/// Task actions
#[derive(Subcommand)]
pub enum TaskAction {
    /// List tasks in queue
    List {
        /// Queue name
        queue: String,
        /// Limit count
        #[arg(long, default_value = "100")]
        limit: usize,
    },
    /// Show task details
    Inspect {
        /// Task ID
        id: String,
    },
    /// Cancel task
    Cancel {
        /// Task ID
        #[arg(short, long)]
        id: String,
        /// Queue name
        #[arg(short, long)]
        queue: String,
    },
    /// Retry failed task
    Retry {
        /// Task ID
        #[arg(short, long)]
        id: String,
        /// Queue name
        #[arg(short, long)]
        queue: String,
    },
}

/// Handle task command
///
/// # Arguments
/// * `client` - Rediq client instance
/// * `action` - Task action to execute
pub async fn handle(client: &Client, action: TaskAction) -> Result<()> {
    match action {
        TaskAction::List { queue, limit } => list_tasks(client, queue, limit).await?,
        TaskAction::Inspect { id } => inspect_task(client, id).await?,
        TaskAction::Cancel { id, queue } => cancel_task(client, id, queue).await?,
        TaskAction::Retry { id, queue } => retry_task(client, id, queue).await?,
    }
    Ok(())
}

/// List tasks in queue
async fn list_tasks(client: &Client, queue: String, limit: usize) -> Result<()> {
    println!("Tasks in queue '{}' (showing max {}):", queue, limit);
    let inspector = client.inspector();

    match inspector.list_tasks(&queue, limit).await {
        Ok(tasks) => {
            if tasks.is_empty() {
                println!("  (No tasks in queue)");
            } else {
                println!("  Pending tasks:");
                for task in tasks {
                    println!("    - {} [{}]", task.id, task.task_type);
                    println!("      Status: {}", task.status);
                    println!("      Retry: {}/{}",
                        task.retry_cnt,
                        std::cmp::max(0, 3 - task.retry_cnt as i32));
                }

                // Show queue summary
                let stats = inspector.queue_stats(&queue).await?;
                println!("\n  Queue Summary:");
                println!("    Pending: {}", stats.pending);
                println!("    Active: {}", stats.active);
                println!("    Delayed: {}", stats.delayed);
                println!("    Retry: {}", stats.retried);
                println!("    Dead: {}", stats.dead);
                println!("\n  Tip: Use 'rediq task inspect <id>' to see task details");
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }

    Ok(())
}

/// Show task details
async fn inspect_task(client: &Client, id: String) -> Result<()> {
    println!("Task Details: {}", id);
    let inspector = client.inspector();

    match inspector.get_task(&id).await {
        Ok(task) => {
            println!("  ID: {}", task.id);
            println!("  Type: {}", task.task_type);
            println!("  Queue: {}", task.queue);
            println!("  Status: {}", task.status);
            println!("  Retry count: {}", task.retry_cnt);
            println!("  Created at: {}", task.created_at);
            if let Some(enqueued) = task.enqueued_at {
                println!("  Enqueued at: {}", enqueued);
            }
            if let Some(processed) = task.processed_at {
                println!("  Processed at: {}", processed);
            }
            if let Some(error) = task.last_error {
                println!("  Last error: {}", error);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }

    Ok(())
}

/// Cancel task
async fn cancel_task(client: &Client, id: String, queue: String) -> Result<()> {
    println!("Cancel task: {} (queue: {})", id, queue);

    match client.cancel_task(&id, &queue).await {
        Ok(true) => {
            println!("  ✓ Task cancelled successfully");
        }
        Ok(false) => {
            println!("  ! Task not found in pending queue");
            println!("    The task may be active, delayed, or retrying");
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }

    Ok(())
}

/// Retry failed task from dead letter queue
///
/// This command moves a task from the dead letter queue back to the pending queue
/// for reprocessing. It resets the task's retry count and status.
async fn retry_task(client: &Client, task_id: String, queue: String) -> Result<()> {
    println!("Retry task: {} (queue: {})", task_id, queue);

    match client.retry_task(&task_id, &queue).await {
        Ok(true) => {
            println!("  ✓ Task '{}' successfully re-queued for processing", task_id);
            println!("    Retry count has been reset to 0");
            println!("    Task status has been reset to Pending");
        }
        Ok(false) => {
            println!("  ! Task not found in dead queue");
            println!("    The task may be in another state (pending, active, retry)");
            println!("    Use 'rediq task inspect {}' to check task status", task_id);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_action_display() {
        // Basic structural test - integration tests cover actual functionality
        let action = TaskAction::Inspect {
            id: "test-task-id".to_string(),
        };
        match action {
            TaskAction::Inspect { id } => {
                assert_eq!(id, "test-task-id");
            }
            _ => {}
        }
    }
}
