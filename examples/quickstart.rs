//! Rediq quickstart example
//!
//! Demonstrates how to use Rediq to create and process tasks

use rediq::{client::Client, Task};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Email task payload
#[derive(Debug, Serialize, Deserialize)]
struct EmailPayload {
    to: String,
    subject: String,
    body: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Create client
    let client = Client::builder()
        .redis_url("redis://localhost:6379")
        .build()
        .await?;

    println!("Rediq Quickstart Example\n");

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

    println!("Example completed!");

    Ok(())
}

/// Server example code (not run in main)
///
/// To run this example, you need:
/// 1. Ensure Redis is running
/// 2. Implement complete Server/Worker functionality
#[allow(dead_code)]
async fn server_example() -> Result<(), Box<dyn std::error::Error>> {
    use rediq::processor::{Handler, Mux};

    // Custom handler
    struct EmailHandler;

    #[async_trait::async_trait]
    impl Handler for EmailHandler {
        async fn handle(&self, task: &rediq::Task) -> rediq::Result<()> {
            println!("Processing email task: {}", task.task_type);
            // Implement email sending logic here
            Ok(())
        }
    }

    // // Create server
    // let mut server = Server::builder()
    //     .redis_url("redis://localhost:6379")
    //     .queues(&["default", "batch"])
    //     .concurrency(10)
    //     .build()
    //     .await?;

    // Register handler
    let mut mux = Mux::new();
    mux.handle("email:send", EmailHandler);
    mux.handle("email:welcome", EmailHandler);
    mux.handle("notification:push", EmailHandler);

    // Run server
    // server.run(mux).await?;

    Ok(())
}
