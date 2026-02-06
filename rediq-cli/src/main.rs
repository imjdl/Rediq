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
    // Parse Redis URL to extract connection details (do this first for --all case)
    let conn_info = parse_redis_url(redis_url);

    if all {
        println!("⚠️  Purging ALL Rediq metadata...");
        println!();
        println!("To purge all metadata, run:");
        print_redis_command(&conn_info, "DEL \"rediq:meta:workers\"");
        print_redis_command(&conn_info, "DEL \"rediq:meta:queues\"");
        println!();
        println!("Or flush the entire database:");
        print_redis_command(&conn_info, "FLUSHDB");
        return Ok(());
    }

    // For other options, we need to connect to Redis to inspect data
    let client = Client::builder().redis_url(redis_url).build().await?;
    let inspector = client.inspector();

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
            print_redis_command_indented(&conn_info, 2, "DEL \"rediq:meta:workers\"");
            println!("  {}redis-cli {} --scan --pattern 'rediq:meta:worker:*' | xargs -L 1 redis-cli {} DEL",
                "  ".repeat(2),
                build_redis_cli_args(&conn_info),
                build_redis_cli_args(&conn_info)
            );
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
            print_redis_command_indented(&conn_info, 2, "SREM \"rediq:meta:queues\" \"<queue-name>\"");
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

/// Parse Redis URL and extract connection details
fn parse_redis_url(url: &str) -> RedisConnectionInfo {
    // Default values
    let mut host = "localhost".to_string();
    let mut port = 6379;
    let mut password = None;
    let mut db = 0;

    // Remove protocol prefix
    let url = url.strip_prefix("redis://").unwrap_or(url);

    // Parse format: [:password@]host[:port][/db]
    let parts: Vec<&str> = url.split('@').collect();

    if parts.len() > 1 {
        // Has authentication: [password@]host[:port][/db]
        let auth_part = parts[0];
        let remaining = parts[1];

        // Check if password exists (format: :password or just @ for no password)
        if !auth_part.is_empty() {
            password = Some(auth_part.strip_prefix(':').unwrap_or(auth_part).to_string());
        }

        // Parse host:port/db from remaining part
        parse_host_port_db(remaining, &mut host, &mut port, &mut db);
    } else {
        // No authentication: host[:port][/db]
        parse_host_port_db(parts[0], &mut host, &mut port, &mut db);
    }

    RedisConnectionInfo { host, port, password, db }
}

/// Parse host, port, and database from URL part
fn parse_host_port_db(s: &str, host: &mut String, port: &mut u16, db: &mut i64) {
    // Split by / to separate [host:port] and [db]
    let parts: Vec<&str> = s.split('/').collect();

    let addr_part = parts[0];

    // Parse host:port
    if let Some(colon_pos) = addr_part.rfind(':') {
        if let Some(parsed_port) = addr_part[colon_pos + 1..].parse::<u16>().ok() {
            *port = parsed_port;
            *host = addr_part[..colon_pos].to_string();
        } else {
            *host = addr_part.to_string();
        }
    } else {
        *host = addr_part.to_string();
    }

    // Parse database number
    if parts.len() > 1 {
        if let Some(parsed_db) = parts[1].parse::<i64>().ok() {
            *db = parsed_db;
        }
    }
}

/// Build redis-cli arguments from connection info
fn build_redis_cli_args(info: &RedisConnectionInfo) -> String {
    let mut args = vec![
        format!("-h {}", info.host),
        format!("-p {}", info.port),
    ];

    if let Some(ref pwd) = info.password {
        args.push(format!("-a \"{}\"", pwd));
    }

    args.push("--no-auth-warning".to_string());

    if info.db > 0 {
        args.push(format!("-n {}", info.db));
    }

    args.join(" ")
}

/// Print a redis-cli command with connection info
fn print_redis_command(info: &RedisConnectionInfo, cmd: &str) {
    println!("  redis-cli {} {}", build_redis_cli_args(info), cmd);
}

/// Print a redis-cli command with connection info and indentation
fn print_redis_command_indented(info: &RedisConnectionInfo, indent: usize, cmd: &str) {
    let indent_str = " ".repeat(indent);
    println!("{}redis-cli {} {}", indent_str, build_redis_cli_args(info), cmd);
}

/// Redis connection information extracted from URL
#[derive(Debug, Clone)]
struct RedisConnectionInfo {
    host: String,
    port: u16,
    password: Option<String>,
    db: i64,
}
