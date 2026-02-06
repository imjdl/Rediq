//! End-to-end test for Rediq
//!
//! Tests the complete task lifecycle: enqueue → process → acknowledge

use rediq::{client::Client, processor::{Handler, Mux}, server::ServerBuilder, Task};
use async_trait::async_trait;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Test handler that counts processed tasks
struct TestHandler {
    count: Arc<AtomicU32>,
}

#[async_trait]
impl Handler for TestHandler {
    async fn handle(&self, task: &rediq::Task) -> rediq::Result<()> {
        println!("Processing task: {} (type: {})", task.id, task.task_type);
        self.count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());

    println!("Rediq End-to-End Test");
    println!("Redis URL: {}\n", redis_url);

    let count = Arc::new(AtomicU32::new(0));

    // ===== Part 1: Enqueue tasks =====
    println!("=== Part 1: Enqueueing tasks ===");

    let client = Client::builder()
        .redis_url(&redis_url)
        .build()
        .await?;

    // Enqueue 5 tasks
    for i in 0..5 {
        let task = Task::builder("test:task")
            .queue("e2e-test")
            .payload(&serde_json::json!({ "index": i }))
            .unwrap()
            .build()
            .unwrap();

        let task_id = client.enqueue(task).await?;
        println!("Enqueued task {}: {}", i + 1, task_id);
    }

    println!("\n=== Part 2: Check queue status ===");

    let inspector = client.inspector();
    let stats = inspector.queue_stats("e2e-test").await?;
    println!("Pending tasks: {}", stats.pending);

    // ===== Part 3: Start server and process tasks =====
    println!("\n=== Part 3: Starting server to process tasks ===");

    let mut mux = Mux::new();
    mux.handle("test:task", TestHandler { count: count.clone() });

    // Create server with short timeout
    let server = ServerBuilder::new()
        .redis_url(&redis_url)
        .queues(&["e2e-test"])
        .concurrency(2)
        .heartbeat_interval(5)
        .build()
        .await?;

    // Start server in background
    let server_handle = tokio::spawn(async move {
        let _ = server.run(mux).await;
    });

    // Wait for tasks to be processed
    println!("Waiting for tasks to be processed...");
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Check final count
    let processed = count.load(Ordering::SeqCst);
    println!("\n=== Part 4: Verify results ===");
    println!("Tasks processed: {}", processed);

    // Check queue status again
    let stats = inspector.queue_stats("e2e-test").await?;
    println!("Remaining pending tasks: {}", stats.pending);

    if processed == 5 {
        println!("\n✅ Test PASSED: All tasks processed successfully");
    } else {
        println!("\n❌ Test FAILED: Expected 5 tasks, got {}", processed);
    }

    // Shutdown
    println!("\nShutting down server...");
    server_handle.abort();

    Ok(())
}
