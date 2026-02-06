//! Automated end-to-end test
//!
//! This test runs the complete flow and automatically stops after processing.

use rediq::{client::Client, processor::{Handler, Mux}, server::ServerBuilder, Task};
use async_trait::async_trait;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Test handler
struct TestHandler {
    count: Arc<AtomicU32>,
}

#[async_trait]
impl Handler for TestHandler {
    async fn handle(&self, task: &rediq::Task) -> rediq::Result<()> {
        println!("✓ Processed task: {} (type: {})", task.id, task.task_type);
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
    println!("║         Rediq End-to-End Automated Test                 ║");
    println!("╚════════════════════════════════════════════════════════╝");
    println!("Redis URL: {}\n", redis_url);

    let count = Arc::new(AtomicU32::new(0));

    // ===== Part 1: Enqueue tasks =====
    println!("▶ Part 1: Enqueuing tasks...");

    let client = Client::builder()
        .redis_url(&redis_url)
        .build()
        .await?;

    // Enqueue test tasks
    for i in 0..5 {
        let task = Task::builder("test:task")
            .queue("auto-test")
            .payload(&serde_json::json!({ "index": i }))
            .unwrap()
            .build()
            .unwrap();

        let task_id = client.enqueue(task).await?;
        println!("  ✓ Enqueued task {}: {}", i + 1, task_id);
    }

    // ===== Part 2: Check queue status =====
    println!("\n▶ Part 2: Checking queue status...");

    let inspector = client.inspector();
    let stats = inspector.queue_stats("auto-test").await?;
    println!("  - Pending tasks: {}", stats.pending);

    // ===== Part 3: Start server =====
    println!("\n▶ Part 3: Starting server...");

    let mut mux = Mux::new();
    mux.handle("test:task", TestHandler { count: count.clone() });

    let state = ServerBuilder::new()
        .redis_url(&redis_url)
        .queues(&["auto-test"])
        .concurrency(2)
        .heartbeat_interval(5)
        .build()
        .await?;

    let server = rediq::server::Server::from(state);

    // Run server in background
    let server_handle = tokio::spawn(async move {
        let _ = server.run(mux).await;
    });

    // ===== Part 4: Wait for processing =====
    println!("\n▶ Part 4: Waiting for tasks to be processed...");

    for i in 1..=10 {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let processed = count.load(Ordering::SeqCst);
        print!("\r  Progress: {}/10 seconds | Processed: {}/5", i, processed);
        use std::io::Write;
        std::io::stdout().flush().unwrap();

        if processed >= 5 {
            println!("\n");
            break;
        }
    }

    // ===== Part 5: Verify results =====
    println!("\n▶ Part 5: Final verification...");

    let processed = count.load(Ordering::SeqCst);
    let stats = inspector.queue_stats("auto-test").await?;
    println!("  - Tasks processed: {}", processed);
    println!("  - Remaining pending: {}", stats.pending);

    // ===== Results =====
    println!("\n╔════════════════════════════════════════════════════════╗");
    if processed == 5 {
        println!("║  ✅ TEST PASSED: All tasks processed successfully!       ║");
    } else {
        println!("║  ❌ TEST FAILED: Expected 5, got {}                        ║", processed);
    }
    println!("╚════════════════════════════════════════════════════════╝");

    // Shutdown server
    println!("\nShutting down server...");
    server_handle.abort();

    Ok(())
}
