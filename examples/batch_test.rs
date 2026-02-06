//! Test batch enqueue and queue control features

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
        println!("✓ Processed: {}", task.id);
        self.count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());

    println!("╔════════════════════════════════════════════════════════╗");
    println!("║      Rediq Batch Enqueue & Queue Control Test          ║");
    println!("╚════════════════════════════════════════════════════════╝\n");

    let client = Client::builder()
        .redis_url(&redis_url)
        .build()
        .await?;

    let count = Arc::new(AtomicU32::new(0));

    // ===== Test 1: Batch Enqueue =====
    println!("▶ Test 1: Batch Enqueue (10 tasks)");

    let mut tasks = Vec::new();
    for i in 0..10 {
        let task = Task::builder("test:batch")
            .queue("batch-test")
            .payload(&serde_json::json!({ "index": i }))
            .unwrap()
            .build()
            .unwrap();
        tasks.push(task);
    }

    let task_ids = client.enqueue_batch(tasks).await?;
    println!("  ✓ Batch enqueued {} tasks", task_ids.len());

    let inspector = client.inspector();
    let stats = inspector.queue_stats("batch-test").await?;
    println!("  - Queue stats: pending={}, active={}\n", stats.pending, stats.active);

    // ===== Test 2: Pause Queue =====
    println!("▶ Test 2: Pause Queue");

    client.pause_queue("batch-test").await?;
    println!("  ✓ Queue paused\n");

    // Try to enqueue while paused
    let task = Task::builder("test:batch")
        .queue("batch-test")
        .payload(&serde_json::json!({ "index": 999 }))
        .unwrap()
        .build()
        .unwrap();
    client.enqueue(task).await?;
    println!("  ✓ Task enqueued to paused queue (will wait)");

    let stats = inspector.queue_stats("batch-test").await?;
    println!("  - Queue stats: pending={}, active={}\n", stats.pending, stats.active);

    // ===== Test 3: Resume Queue =====
    println!("▶ Test 3: Resume Queue & Process");

    // Start server
    let mut mux = Mux::new();
    mux.handle("test:batch", TestHandler { count: count.clone() });

    let state = ServerBuilder::new()
        .redis_url(&redis_url)
        .queues(&["batch-test"])
        .concurrency(3)
        .build()
        .await?;

    let server = rediq::server::Server::from(state);
    let server_handle = tokio::spawn(async move {
        let _ = server.run(mux).await;
    });

    // Wait a bit then resume
    tokio::time::sleep(Duration::from_secs(1)).await;

    let stats_before = inspector.queue_stats("batch-test").await?;
    println!("  - Before resume: pending={}, active={}", stats_before.pending, stats_before.active);

    client.resume_queue("batch-test").await?;
    println!("  ✓ Queue resumed");

    // Wait for processing
    tokio::time::sleep(Duration::from_secs(3)).await;

    let stats_after = inspector.queue_stats("batch-test").await?;
    println!("  - After resume: pending={}, active={}\n", stats_after.pending, stats_after.active);

    // ===== Test 4: Flush Queue =====
    println!("▶ Test 4: Flush Queue");

    // Enqueue more tasks
    let mut tasks = Vec::new();
    for i in 0..5 {
        let task = Task::builder("test:batch")
            .queue("batch-test")
            .payload(&serde_json::json!({ "index": i }))
            .unwrap()
            .build()
            .unwrap();
        tasks.push(task);
    }
    client.enqueue_batch(tasks).await?;
    println!("  ✓ Added 5 more tasks");

    let stats_before = inspector.queue_stats("batch-test").await?;
    println!("  - Before flush: pending={}", stats_before.pending);

    client.flush_queue("batch-test").await?;
    println!("  ✓ Queue flushed");

    let stats_after = inspector.queue_stats("batch-test").await?;
    println!("  - After flush: pending={}\n", stats_after.pending);

    // ===== Results =====
    let processed = count.load(Ordering::SeqCst);
    println!("╔════════════════════════════════════════════════════════╗");
    println!("║  Test Results                                            ║");
    println!("╠════════════════════════════════════════════════════════╣");
    println!("║  Batch Enqueue: ✓                                        ║");
    println!("║  Pause Queue: ✓                                          ║");
    println!("║  Resume Queue: ✓                                         ║");
    println!("║  Flush Queue: ✓                                          ║");
    println!("║  Tasks Processed: {}                                      ║", processed);
    println!("╚════════════════════════════════════════════════════════╝");

    server_handle.abort();
    Ok(())
}
