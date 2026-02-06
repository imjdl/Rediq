//! Task Dependency Example
//!
//! This example demonstrates how to use task dependencies.
//! Task B will only execute after Task A completes successfully.
//!
//! Run with:
//!   cargo run --example dependency_example

use rediq::client::Client;
use rediq::processor::{Handler, Mux};
use rediq::server::{Server, ServerBuilder};
use rediq::Task;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Serialize, Deserialize)]
struct TaskAData {
    message: String,
}

struct TaskAHandler;

#[async_trait]
impl Handler for TaskAHandler {
    async fn handle(&self, task: &rediq::Task) -> rediq::Result<()> {
        let payload_str = std::str::from_utf8(&task.payload)
            .map_err(|e| rediq::Error::Serialization(e.to_string()))?;
        let data: TaskAData = serde_json::from_str(payload_str)
            .map_err(|e| rediq::Error::Serialization(e.to_string()))?;

        tracing::info!("Task A processing: {}", data.message);

        // Simulate Task A work
        sleep(Duration::from_millis(500)).await;

        tracing::info!("Task A completed successfully");
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct TaskBData {
    message: String,
}

struct TaskBHandler;

#[async_trait]
impl Handler for TaskBHandler {
    async fn handle(&self, task: &rediq::Task) -> rediq::Result<()> {
        let payload_str = std::str::from_utf8(&task.payload)
            .map_err(|e| rediq::Error::Serialization(e.to_string()))?;
        let data: TaskBData = serde_json::from_str(payload_str)
            .map_err(|e| rediq::Error::Serialization(e.to_string()))?;

        tracing::info!("Task B processing: {}", data.message);
        tracing::info!("Task B confirms that Task A has completed (it was a dependency)");

        // Simulate Task B work
        sleep(Duration::from_millis(300)).await;

        tracing::info!("Task B completed successfully");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tracing::info!("Starting Task Dependency Example");
    tracing::info!("=====================================");

    // Build server state
    let state = ServerBuilder::new()
        .redis_url("redis://192.168.1.128:6379")
        .queues(&["tasks"])
        .concurrency(2)
        .build()
        .await?;

    // Create server
    let server = Server::from(state);

    // Register handlers
    let mut mux = Mux::new();
    mux.handle("task:a", TaskAHandler);
    mux.handle("task:b", TaskBHandler);

    // Spawn task enqueuer
    let enqueuer = tokio::spawn(async move {
        if let Err(e) = (async move {
            sleep(Duration::from_secs(2)).await;

            let client = Client::builder()
                .redis_url("redis://192.168.1.128:6379")
                .build()
                .await?;

            // Enqueue Task A (the dependency)
            let task_a_json = serde_json::to_string(&TaskAData {
                message: "I am Task A, I must complete first!".to_string(),
            }).unwrap();

            let task_a = Task::builder("task:a")
                .queue("tasks")
                .raw_payload(task_a_json.into_bytes())
                .max_retry(3)
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap();

            let task_a_id = client.enqueue(task_a).await?;
            tracing::info!("Enqueued Task A with ID: {}", task_a_id);

            // Wait a moment for Task A to be enqueued
            sleep(Duration::from_millis(100)).await;

            // Enqueue Task B (depends on Task A)
            let task_b_json = serde_json::to_string(&TaskBData {
                message: "I am Task B, I wait for Task A!".to_string(),
            }).unwrap();

            let task_b = Task::builder("task:b")
                .queue("tasks")
                .raw_payload(task_b_json.into_bytes())
                .depends_on(&[&task_a_id])  // Task B depends on Task A
                .max_retry(3)
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap();

            let task_b_id = client.enqueue(task_b).await?;
            tracing::info!("Enqueued Task B with ID: {} (depends on Task A: {})", task_b_id, task_a_id);

            tracing::info!("");
            tracing::info!("=====================================");
            tracing::info!("Dependency Flow:");
            tracing::info!("1. Task A is enqueued first");
            tracing::info!("2. Task B is enqueued with dependency on Task A");
            tracing::info!("3. Task B will NOT run until Task A completes");
            tracing::info!("4. When Task A completes, Task B is automatically enqueued");
            tracing::info!("=====================================");
            tracing::info!("");
            tracing::info!("Watch for:");
            tracing::info!("- Task A being processed first");
            tracing::info!("- Task B being processed AFTER Task A completes");

            Ok::<(), Box<dyn std::error::Error>>(())
        }).await {
            tracing::error!("Enqueuer error: {}", e);
        }
    });

    tracing::info!("=====================================");
    tracing::info!("Worker is running...");
    tracing::info!("=====================================");
    tracing::info!("Press Ctrl+C to stop the server");

    // Run the server
    tokio::select! {
        _ = server.run(mux) => {},
        r = enqueuer => { r?; },
    }

    Ok(())
}
