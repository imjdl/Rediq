//! Queue command handlers
//!
//! Provides commands for listing, inspecting, pausing, resuming, and flushing queues.

use clap::{Subcommand};
use color_eyre::Result;
use rediq::client::Client;

/// Queue actions
#[derive(Subcommand)]
pub enum QueueAction {
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

/// Handle queue command
///
/// # Arguments
/// * `client` - Rediq client instance
/// * `action` - Queue action to execute
pub async fn handle(client: &Client, action: QueueAction) -> Result<()> {
    match action {
        QueueAction::List => list_queues(client).await?,
        QueueAction::Inspect { name } => inspect_queue(client, name).await?,
        QueueAction::Pause { name } => pause_queue(client, name).await?,
        QueueAction::Resume { name } => resume_queue(client, name).await?,
        QueueAction::Flush { name } => flush_queue(client, name).await?,
    }
    Ok(())
}

/// List all queues
async fn list_queues(client: &Client) -> Result<()> {
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

    Ok(())
}

/// Show queue details
async fn inspect_queue(client: &Client, name: String) -> Result<()> {
    println!("Queue Details: {}", name);
    let inspector = client.inspector();
    let stats = inspector.queue_stats(&name).await?;

    println!("  Pending: {}", stats.pending);
    println!("  Active: {}", stats.active);
    println!("  Delayed: {}", stats.delayed);
    println!("  Retry: {}", stats.retried);
    println!("  Dead: {}", stats.dead);
    println!("  Completed: {}", stats.completed);

    Ok(())
}

/// Pause queue
async fn pause_queue(client: &Client, name: String) -> Result<()> {
    client.pause_queue(&name).await?;
    println!("Queue '{}' paused", name);
    Ok(())
}

/// Resume queue
async fn resume_queue(client: &Client, name: String) -> Result<()> {
    client.resume_queue(&name).await?;
    println!("Queue '{}' resumed", name);
    Ok(())
}

/// Flush queue
async fn flush_queue(client: &Client, name: String) -> Result<()> {
    let count = client.flush_queue(&name).await?;
    println!("Queue '{}' flushed, removed {} tasks", name, count);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_action_display() {
        // Basic structural test - integration tests cover actual functionality
        let action = QueueAction::List;
        match action {
            QueueAction::List => {}
            QueueAction::Inspect { name } => {
                assert_eq!(name, "test");
            }
            _ => {}
        }
    }
}
