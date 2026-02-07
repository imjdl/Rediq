//! Priority Queue Example
//!
//! This example demonstrates how to use priority queues in Rediq.
//! Tasks with lower priority values are processed first (0-100 range).
//!
//! Run with:
//!   cargo run --example priority_queue_example

use rediq::client::Client;
use rediq::processor::{Handler, Mux};
use rediq::server::{Server, ServerBuilder};
use rediq::task::TaskBuilder;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Serialize, Deserialize)]
struct JobData {
    job_id: usize,
    description: String,
}

struct JobHandler;

#[async_trait]
impl Handler for JobHandler {
    async fn handle(&self, task: &rediq::Task) -> rediq::Result<()> {
        let payload_str = std::str::from_utf8(&task.payload)
            .map_err(|e| rediq::Error::Serialization(e.to_string()))?;
        let data: JobData = serde_json::from_str(payload_str)
            .map_err(|e| rediq::Error::Serialization(e.to_string()))?;

        tracing::info!(
            "Processing job {} (priority: {}): {}",
            data.job_id,
            task.options.priority,
            data.description
        );

        // Simulate processing
        sleep(Duration::from_millis(100)).await;

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tracing::info!("Starting Priority Queue Example");

    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());

    // Build server state
    let state = ServerBuilder::new()
        .redis_url(&redis_url)
        .queues(&["jobs"])
        .concurrency(2)
        .build()
        .await?;

    // Create server
    let server = Server::from(state);

    // Register handler
    let mut mux = Mux::new();
    mux.handle("job:process", JobHandler);

    // Spawn task enqueuer
    let enqueuer = tokio::spawn(async move {
        sleep(Duration::from_secs(2)).await;

        let client = Client::builder()
            .redis_url(&redis_url)
            .build()
            .await
            .unwrap();

        // Enqueue tasks with different priorities
        // Lower priority value = higher priority (processed first)
        let priorities = [10, 50, 90, 30, 70, 5, 100, 20];

        for (i, &priority) in priorities.iter().enumerate() {
            let task = TaskBuilder::new("job:process")
                .queue("jobs")
                .payload(&JobData {
                    job_id: i,
                    description: format!("Task with priority {}", priority),
                })
                .unwrap()
                .priority(priority)
                .build()
                .unwrap();

            client.enqueue_priority(task).await.unwrap();
            tracing::info!("Enqueued task {} with priority {}", i, priority);
        }

        tracing::info!("All tasks enqueued. Expected processing order: 5 -> 10 -> 20 -> 30 -> 50 -> 70 -> 90 -> 100");
    });

    tracing::info!("==============================================");
    tracing::info!("Worker is running...");
    tracing::info!("==============================================");
    tracing::info!("Watch for processing order based on priority!");
    tracing::info!("Press Ctrl+C to stop the server");

    // Run the server
    tokio::select! {
        _ = server.run(mux) => {},
        r = enqueuer => { r?; },
    }

    Ok(())
}
