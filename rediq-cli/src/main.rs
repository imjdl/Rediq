//! Rediq CLI - Command line tool for managing distributed task queues

mod dashboard;

use clap::{Parser, Subcommand};
use rediq::client::Client;

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
    /// Real-time dashboard
    Dash {
        /// Refresh interval in milliseconds
        #[arg(short, long, default_value = "500")]
        interval: u64,
    },
    /// Clean up stale metadata
    Purge {
        /// Remove stale worker metadata
        #[arg(long, default_value_t = false)]
        workers: bool,
        /// Remove empty queue metadata
        #[arg(long, default_value_t = false)]
        queues: bool,
        /// Remove all metadata (use with caution)
        #[arg(long, default_value_t = false)]
        all: bool,
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
        Commands::Dash { interval } => {
            if let Err(e) = dashboard::run(&redis_url, interval).await {
                eprintln!("Dashboard error: {}", e);
                return Err(color_eyre::eyre::eyre!(e));
            }
        }
        Commands::Purge { workers, queues, all } => {
            handle_purge(&redis_url, workers, queues, all).await?;
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

async fn handle_purge(redis_url: &str, workers: bool, queues: bool, all: bool) -> color_eyre::Result<()> {
    let client = Client::builder().redis_url(redis_url).build().await?;
    let inspector = client.inspector();

    if all {
        println!("⚠️  Purging ALL Rediq metadata...");
        println!();
        println!("To purge all metadata, run:");
        println!("  redis-cli -h <host> -p <port> -a <password> --no-auth-warning DEL \"rediq:meta:workers\"");
        println!("  redis-cli -h <host> -p <port> -a <password> --no-auth-warning DEL \"rediq:meta:queues\"");
        println!();
        println!("Or flush the entire database:");
        println!("  redis-cli -h <host> -p <port> -a <password> --no-auth-warning FLUSHDB");
        return Ok(());
    }

    if workers {
        println!("Purging stale worker metadata...");
        let workers = inspector.list_workers().await?;

        let mut stale_count = 0;
        for worker in workers {
            // Check if worker has recent heartbeat (within 2 minutes)
            let now = chrono::Utc::now().timestamp();
            let heartbeat_age = now - worker.last_heartbeat;

            if heartbeat_age > 120 {
                println!("  - Found stale worker: {} (last heartbeat {}s ago)", worker.id, heartbeat_age);
                stale_count += 1;
            }
        }

        if stale_count > 0 {
            println!();
            println!("  To remove stale workers, run:");
            println!("  redis-cli -h <host> -p <port> -a <password> --no-auth-warning DEL \"rediq:meta:workers\"");
            println!("  redis-cli -h <host> -p <port> -a <password> --no-auth-warning --scan --pattern 'rediq:meta:worker:*' | xargs -L 1 redis-cli -h <host> -p <port> -a <password> --no-auth-warning DEL");
        } else {
            println!("  ✓ No stale workers found");
        }
    }

    if queues {
        println!("Purging empty queue metadata...");
        let queues = inspector.list_queues().await?;

        let mut empty_count = 0;
        for queue in queues {
            let stats = inspector.queue_stats(&queue).await?;
            let is_empty = stats.pending == 0 && stats.active == 0
                && stats.delayed == 0 && stats.retried == 0 && stats.dead == 0;

            if is_empty {
                println!("  - Found empty queue: {}", queue);
                empty_count += 1;
            }
        }

        if empty_count > 0 {
            println!();
            println!("  To remove empty queues from metadata, run:");
            println!("  redis-cli -h <host> -p <port> -a <password> --no-auth-warning SREM \"rediq:meta:queues\" \"<queue-name>\"");
        } else {
            println!("  ✓ No empty queues found");
        }
    }

    if !workers && !queues && !all {
        println!("Purge stale metadata from Redis");
        println!();
        println!("Usage: rediq purge [--workers] [--queues] [--all]");
        println!("  --workers   Show stale workers (no heartbeat in 2+ minutes)");
        println!("  --queues    Show empty queues");
        println!("  --all       Show commands to purge all metadata");
        println!();
        println!("Examples:");
        println!("  rediq purge --workers    # List stale workers");
        println!("  rediq purge --queues     # List empty queues");
        println!("  rediq purge --all        # Show purge commands");
    }

    Ok(())
}
