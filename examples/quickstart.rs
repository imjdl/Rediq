//! Rediq quickstart example
//!
//! Demonstrates how to use Rediq to create and process tasks

use rediq::{client::Client, processor::{Handler, Mux}, server::ServerBuilder, Task};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Email task payload
#[derive(Debug, Serialize, Deserialize)]
struct EmailPayload {
    to: String,
    subject: String,
    body: String,
}

/// Email handler
struct EmailHandler;

#[async_trait::async_trait]
impl Handler for EmailHandler {
    async fn handle(&self, task: &rediq::Task) -> rediq::Result<()> {
        // Deserialize payload
        let payload: EmailPayload = rmp_serde::from_slice(&task.payload)
            .map_err(|e| rediq::Error::Serialization(e.to_string()))?;

        println!("Processing email task: {}", task.task_type);
        println!("  To: {}", payload.to);
        println!("  Subject: {}", payload.subject);
        println!("  Body: {}", payload.body);

        // Simulate email sending
        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok(())
    }
}

/// Batch handler
struct BatchHandler;

#[async_trait::async_trait]
impl Handler for BatchHandler {
    async fn handle(&self, task: &rediq::Task) -> rediq::Result<()> {
        println!("Processing batch task: {}", task.id);

        // Simulate batch processing
        tokio::time::sleep(Duration::from_millis(50)).await;

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("Rediq Quickstart Example\n");

    // Get Redis URL from environment or use default
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());

    // ===== PART 1: Client - Enqueue tasks =====
    println!("=== PART 1: Client - Creating tasks ===\n");

    let client = Client::builder()
        .redis_url(&redis_url)
        .build()
        .await?;

    // 1. Create simple task
    println!("1. Creating simple email task...");
    let task = Task::builder("email:send")
        .queue("default")
        .payload(&EmailPayload {
            to: "user@example.com".to_string(),
            subject: "Welcome to Rediq!".to_string(),
            body: "Thank you for using Rediq distributed task queue!".to_string(),
        })?
        .max_retry(3)
        .timeout(Duration::from_secs(30))
        .build()?;

    let task_id = client.enqueue(task).await?;
    println!("   Task enqueued, ID: {}\n", task_id);

    // 2. Create delayed task
    println!("2. Creating delayed task (60 seconds)...");
    let delayed_task = Task::builder("notification:push")
        .queue("default")
        .payload(&EmailPayload {
            to: "user@example.com".to_string(),
            subject: "Delayed Notification".to_string(),
            body: "This is a delayed message".to_string(),
        })?
        .delay(Duration::from_secs(60))
        .build()?;

    let delayed_id = client.enqueue_delayed(delayed_task).await?;
    println!("   Delayed task enqueued, ID: {}\n", delayed_id);

    // 3. Create task with unique key (deduplication)
    println!("3. Creating task with unique key (prevent duplicates)...");
    let unique_task = Task::builder("email:welcome")
        .queue("default")
        .payload(&EmailPayload {
            to: "newuser@example.com".to_string(),
            subject: "Welcome!".to_string(),
            body: "Welcome to join us!".to_string(),
        })?
        .unique_key("email:newuser@example.com:welcome")
        .build()?;

    // Try to enqueue duplicate - will be deduplicated
    let _ = client.enqueue(unique_task.clone()).await?;
    let _ = client.enqueue(unique_task).await?;
    println!("   Unique key task enqueued, duplicates are automatically deduplicated\n");

    // 4. Batch enqueue
    println!("4. Batch enqueuing 10 tasks...");
    let mut batch_tasks = Vec::new();
    for i in 0..10 {
        let task = Task::builder("batch:process")
            .queue("batch")
            .payload(&serde_json::json!({ "index": i }))
            .unwrap()
            .build()
            .unwrap();
        batch_tasks.push(task);
    }

    let task_ids = client.enqueue_batch(batch_tasks).await?;
    println!("   Batch enqueue completed, {} tasks\n", task_ids.len());

    // 5. Query queue status
    println!("5. Querying queue status...");
    let inspector = client.inspector();
    let default_stats = inspector.queue_stats("default").await?;
    println!("   default queue: pending={}, active={}", default_stats.pending, default_stats.active);

    let batch_stats = inspector.queue_stats("batch").await?;
    println!("   batch queue: pending={}, active={}\n", batch_stats.pending, batch_stats.active);

    println!("=== PART 1 Complete: Tasks enqueued ===\n");

    // ===== PART 2: Server - Process tasks =====
    println!("=== PART 2: Server - Starting to process tasks ===");
    println!("Press Ctrl+C to stop the server\n");

    // Register handlers
    let mut mux = Mux::new();
    mux.handle("email:send", EmailHandler);
    mux.handle("email:welcome", EmailHandler);
    mux.handle("notification:push", EmailHandler);
    mux.handle("batch:process", BatchHandler);

    // Create and run server
    let state = ServerBuilder::new()
        .redis_url(&redis_url)
        .queues(&["default", "batch"])
        .concurrency(3)
        .heartbeat_interval(5)
        .build()
        .await?;

    let server = rediq::server::Server::from(state);
    server.run(mux).await?;

    println!("Server stopped gracefully");

    Ok(())
}
