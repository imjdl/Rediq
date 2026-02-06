//! Rediq CLI - Command line tool for managing distributed task queues

use clap::{Parser, Subcommand};
use rediq::client::{Client, ClientBuilder};

#[derive(Parser)]
#[command(name = "rediq")]
#[command(about = "Rediq CLI - Manage distributed task queues", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Queue operations
    Queue {
        #[command(subcommand)]
        action: QueueAction,
    },
    /// Task operations
    Task {
        #[command(subcommand)]
        action: TaskAction,
    },
    /// Worker operations
    Worker {
        #[command(subcommand)]
        action: WorkerAction,
    },
    /// Statistics
    Stats {
        /// Queue name
        #[arg(long, default_value = "default")]
        queue: String,
    },
}

#[derive(Subcommand)]
enum QueueAction {
    /// List all queues
    List,
    /// Show queue details
    Inspect {
        /// Queue name
        name: String,
    },
    /// Pause queue
    Pause {
        /// Queue name
        name: String,
    },
    /// Resume queue
    Resume {
        /// Queue name
        name: String,
    },
    /// Flush queue
    Flush {
        /// Queue name
        name: String,
    },
}

#[derive(Subcommand)]
enum TaskAction {
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

#[derive(Subcommand)]
enum WorkerAction {
    /// List all workers
    List,
    /// Show worker details
    Inspect {
        /// Worker ID
        id: String,
    },
    /// Stop worker
    Stop {
        /// Worker ID
        id: String,
    },
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    let cli = Cli::parse();
    let redis_url = std::env::var("REDIS_URL").unwrap_or("redis://localhost:6379".to_string());

    match cli.command {
        Commands::Queue { action } => {
            handle_queue_action(&redis_url, action).await?;
        }
        Commands::Task { action } => {
            handle_task_action(&redis_url, action).await?;
        }
        Commands::Worker { action } => {
            handle_worker_action(&redis_url, action).await?;
        }
        Commands::Stats { queue } => {
            show_stats(&redis_url, &queue).await?;
        }
    }

    Ok(())
}

async fn handle_queue_action(redis_url: &str, action: QueueAction) -> color_eyre::Result<()> {
    let client = Client::builder().redis_url(redis_url).build().await?;

    match action {
        QueueAction::List => {
            println!("Queue List");
            let inspector = client.inspector();
            let queues = inspector.list_queues().await?;

            if queues.is_empty() {
                println!("  (No queues)");
            } else {
                for queue in queues {
                    println!("  - {}", queue);
                }
            }
        }
        QueueAction::Inspect { name } => {
            println!("Queue Details: {}", name);
            let inspector = client.inspector();
            let stats = inspector.queue_stats(&name).await?;

            println!("  Pending: {}", stats.pending);
            println!("  Active: {}", stats.active);
            println!("  Delayed: {}", stats.delayed);
            println!("  Retry: {}", stats.retried);
            println!("  Dead: {}", stats.dead);
        }
        QueueAction::Pause { name } => {
            client.pause_queue(&name).await?;
            println!("Queue '{}' paused", name);
        }
        QueueAction::Resume { name } => {
            client.resume_queue(&name).await?;
            println!("Queue '{}' resumed", name);
        }
        QueueAction::Flush { name } => {
            let count = client.flush_queue(&name).await?;
            println!("Queue '{}' flushed, removed {} tasks", name, count);
        }
    }

    Ok(())
}

async fn handle_task_action(redis_url: &str, action: TaskAction) -> color_eyre::Result<()> {
    let client = Client::builder().redis_url(redis_url).build().await?;

    match action {
        TaskAction::List { queue, limit } => {
            println!("Tasks in queue '{}' (showing max {}):", queue, limit);
            let inspector = client.inspector();

            // Get pending tasks
            match inspector.list_tasks(&queue, limit).await {
                Ok(tasks) => {
                    if tasks.is_empty() {
                        println!("  (No tasks in queue)");
                    } else {
                        println!("  Pending tasks:");
                        for task in tasks {
                            println!("    - {} [{}]", task.id, task.task_type);
                            println!("      Status: {}", task.status);
                            println!("      Retry: {}/{}", task.retry_cnt,
                                std::cmp::max(0, 3 - task.retry_cnt as i32));
                        }
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
                Err(e) => {
                    eprintln!("Error: {}", e);
                }
            }
        }
        TaskAction::Inspect { id } => {
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
        }
        TaskAction::Cancel { id, queue } => {
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
        }
        TaskAction::Retry { id, queue } => {
            println!("Retry task: {} (queue: {})", id, queue);
            // For retry, we need to move the task from dead/retry queue back to pending
            // This is a simplified implementation - in production you'd want to check the task status first
            eprintln!("  ! Retry functionality requires additional implementation");
            eprintln!("    For now, you can manually re-enqueue the task");
        }
    }

    Ok(())
}

async fn handle_worker_action(redis_url: &str, action: WorkerAction) -> color_eyre::Result<()> {
    let client = Client::builder().redis_url(redis_url).build().await?;
    let inspector = client.inspector();

    match action {
        WorkerAction::List => {
            println!("Worker List");
            match inspector.list_workers().await {
                Ok(workers) => {
                    if workers.is_empty() {
                        println!("  (No workers registered)");
                    } else {
                        for worker in workers {
                            println!("  - {}", worker.id);
                            println!("    Server: {}", worker.server_name);
                            println!("    Queues: {:?}", worker.queues.join(", "));
                            println!("    Status: {}", worker.status);
                            println!("    Processed: {}", worker.processed_total);
                            println!();
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                }
            }
        }
        WorkerAction::Inspect { id } => {
            println!("Worker Details: {}", id);
            match inspector.get_worker(&id).await {
                Ok(worker) => {
                    println!("  ID: {}", worker.id);
                    println!("  Server: {}", worker.server_name);
                    println!("  Queues: {:?}", worker.queues);
                    println!("  Status: {}", worker.status);
                    println!("  Started at: {}", worker.started_at);
                    println!("  Last heartbeat: {}", worker.last_heartbeat);
                    println!("  Processed total: {}", worker.processed_total);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                }
            }
        }
        WorkerAction::Stop { id } => {
            println!("Stop worker: {}", id);
            match inspector.stop_worker(&id).await {
                Ok(true) => {
                    println!("  ✓ Worker shutdown signal sent");
                    println!("    The worker will finish its current task and exit");
                }
                Ok(false) => {
                    println!("  ! Worker not found");
                    println!("    The worker may have already exited");
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                }
            }
        }
    }

    Ok(())
}

async fn show_stats(redis_url: &str, queue_name: &str) -> color_eyre::Result<()> {
    let client = Client::builder().redis_url(redis_url).build().await?;
    let inspector = client.inspector();

    println!("Statistics");
    let stats = inspector.queue_stats(queue_name).await?;

    println!("  Queue: {}", stats.name);
    println!("  Pending: {}", stats.pending);
    println!("  Active: {}", stats.active);
    println!("  Delayed: {}", stats.delayed);
    println!("  Retry: {}", stats.retried);
    println!("  Dead: {}", stats.dead);

    Ok(())
}
