//! Worker command handlers
//!
//! Provides commands for listing workers, inspecting worker details, and stopping workers.

use clap::Subcommand;
use color_eyre::Result;
use rediq::client::Client;

/// Worker actions
#[derive(Subcommand)]
pub enum WorkerAction {
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

/// Handle worker command
///
/// # Arguments
/// * `client` - Rediq client instance
/// * `action` - Worker action to execute
pub async fn handle(client: &Client, action: WorkerAction) -> Result<()> {
    match action {
        WorkerAction::List => list_workers(client).await?,
        WorkerAction::Inspect { id } => inspect_worker(client, id).await?,
        WorkerAction::Stop { id } => stop_worker(client, id).await?,
    }
    Ok(())
}

/// List all workers
async fn list_workers(client: &Client) -> Result<()> {
    println!("Worker List");
    let inspector = client.inspector();

    match inspector.list_workers().await {
        Ok(workers) => {
            if workers.is_empty() {
                println!("  (No workers registered)");
            } else {
                for worker in workers {
                    println!("  - {}", worker.id);
                    println!("    Server: {}", worker.server_name);
                    println!("    Queues: {}", worker.queues.join(", "));
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

    Ok(())
}

/// Show worker details
async fn inspect_worker(client: &Client, id: String) -> Result<()> {
    println!("Worker Details: {}", id);
    let inspector = client.inspector();

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

    Ok(())
}

/// Stop worker
async fn stop_worker(client: &Client, id: String) -> Result<()> {
    println!("Stop worker: {}", id);
    let inspector = client.inspector();

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

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_action_display() {
        // Basic structural test - integration tests cover actual functionality
        let action = WorkerAction::List;
        match action {
            WorkerAction::List => {}
            WorkerAction::Inspect { id } => {
                assert_eq!(id, "test-worker-id");
            }
            WorkerAction::Stop { id } => {
                assert_eq!(id, "test-worker-id");
            }
        }
    }
}
