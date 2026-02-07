//! Rediq CLI - Command line tool for managing distributed task queues

mod client;
mod commands;
mod dashboard;
mod utils;

use clap::{Parser, Subcommand};
use client::create_client;
use commands::{queue, task, worker, stats, purge};

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
        action: queue::QueueAction,
    },
    /// Task operations
    Task {
        #[command(subcommand)]
        action: task::TaskAction,
    },
    /// Worker operations
    Worker {
        #[command(subcommand)]
        action: worker::WorkerAction,
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

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    let cli = Cli::parse();
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());

    match cli.command {
        Commands::Queue { action } => {
            let client = create_client(&redis_url).await?;
            queue::handle(&client, action).await?;
        }
        Commands::Task { action } => {
            let client = create_client(&redis_url).await?;
            task::handle(&client, action).await?;
        }
        Commands::Worker { action } => {
            let client = create_client(&redis_url).await?;
            worker::handle(&client, action).await?;
        }
        Commands::Stats { queue } => {
            let client = create_client(&redis_url).await?;
            stats::show(&client, queue).await?;
        }
        Commands::Dash { interval } => {
            let client = create_client(&redis_url).await?;
            if let Err(e) = dashboard::run(&redis_url, interval, &client).await {
                eprintln!("Dashboard error: {}", e);
                return Err(color_eyre::eyre::eyre!(e));
            }
        }
        Commands::Purge { workers, queues, all } => {
            let client = create_client(&redis_url).await?;
            purge::handle(&client, &redis_url, workers, queues, all).await?;
        }
    }

    Ok(())
}
