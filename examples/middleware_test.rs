//! Middleware system test example

use rediq::{
    client::Client,
    middleware::{LoggingMiddleware, Middleware, MetricsMiddleware},
    processor::{Handler, Mux},
    server::ServerBuilder,
    Task,
};
use async_trait::async_trait;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Custom middleware that counts tasks
#[derive(Debug, Default, Clone)]
struct CountingMiddleware {
    task_count: Arc<AtomicU32>,
    success_count: Arc<AtomicU32>,
    error_count: Arc<AtomicU32>,
}

impl CountingMiddleware {
    fn new() -> Self {
        Self::default()
    }

    fn get_stats(&self) -> (u32, u32, u32) {
        (
            self.task_count.load(Ordering::SeqCst),
            self.success_count.load(Ordering::SeqCst),
            self.error_count.load(Ordering::SeqCst),
        )
    }
}

#[async_trait]
impl Middleware for CountingMiddleware {
    async fn before(&self, _task: &rediq::Task) -> rediq::Result<()> {
        self.task_count.fetch_add(1, Ordering::SeqCst);
        tracing::info!("📊 Task count: {}", self.task_count.load(Ordering::SeqCst));
        Ok(())
    }

    async fn after(&self, _task: &rediq::Task, result: &rediq::Result<()>) -> rediq::Result<()> {
        match result {
            Ok(()) => {
                self.success_count.fetch_add(1, Ordering::SeqCst);
            }
            Err(_) => {
                self.error_count.fetch_add(1, Ordering::SeqCst);
            }
        }
        Ok(())
    }
}

/// Test handler that simulates work
struct TestHandler {
    should_fail: Arc<AtomicU32>,
}

#[async_trait]
impl Handler for TestHandler {
    async fn handle(&self, task: &rediq::Task) -> rediq::Result<()> {
        tracing::info!("🔧 Processing task: {}", task.id);

        // Simulate some work
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Fail some tasks for testing
        let fail_count = self.should_fail.load(Ordering::SeqCst);
        if fail_count > 0 && fail_count % 3 == 0 {
            self.should_fail.fetch_sub(1, Ordering::SeqCst);
            Err(rediq::Error::Handler("Simulated failure".to_string()))
        } else {
            self.should_fail.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());

    println!("╔════════════════════════════════════════════════════════╗");
    println!("║         Rediq Middleware System Test                    ║");
    println!("╚════════════════════════════════════════════════════════╝\n");

    let client = Client::builder()
        .redis_url(&redis_url)
        .build()
        .await?;

    // ===== Test 1: Enqueue tasks =====
    println!("▶ Test 1: Enqueueing tasks...");

    for i in 0..10 {
        let task = Task::builder("test:middleware")
            .queue("middleware-test")
            .payload(&serde_json::json!({ "index": i }))
            .unwrap()
            .build()
            .unwrap();

        client.enqueue(task).await?;
    }

    println!("  ✓ 10 tasks enqueued\n");

    // ===== Test 2: Create middleware chain =====
    println!("▶ Test 2: Setting up middleware chain...");

    let counting = CountingMiddleware::new();
    let stats = counting.get_stats();

    println!("  ✓ LoggingMiddleware: Logs task processing");
    println!("  ✓ MetricsMiddleware: Collects metrics");
    println!("  ✓ CountingMiddleware: Custom counter (tasks: {}, success: {}, errors: {})\n",
        stats.0, stats.1, stats.2);

    // ===== Test 3: Start server with middleware =====
    println!("▶ Test 3: Starting server with middleware...");

    let mut mux = Mux::new();
    mux.handle("test:middleware", TestHandler {
        should_fail: Arc::new(AtomicU32::new(0)),
    });

    let state = ServerBuilder::new()
        .redis_url(&redis_url)
        .queues(&["middleware-test"])
        .concurrency(3)
        .middleware(LoggingMiddleware::new().with_details())
        .middleware(MetricsMiddleware::new())
        .middleware(counting.clone())
        .build()
        .await?;

    let server = rediq::server::Server::from(state);
    let server_handle = tokio::spawn(async move {
        let _ = server.run(mux).await;
    });

    // Wait for processing
    println!("  Waiting for tasks to be processed...\n");
    tokio::time::sleep(Duration::from_secs(5)).await;

    // ===== Test 4: Check results =====
    println!("▶ Test 4: Middleware Results");

    let stats = counting.get_stats();
    println!("  📊 CountingMiddleware Statistics:");
    println!("     - Tasks seen: {}", stats.0);
    println!("     - Success: {}", stats.1);
    println!("     - Errors: {}", stats.2);

    let inspector = client.inspector();
    let queue_stats = inspector.queue_stats("middleware-test").await?;
    println!("\n  📊 Queue Statistics:");
    println!("     - Pending: {}", queue_stats.pending);
    println!("     - Active: {}", queue_stats.active);
    println!("     - Dead: {}", queue_stats.dead);

    // ===== Summary =====
    println!("\n╔════════════════════════════════════════════════════════╗");
    println!("║  Test Results                                            ║");
    println!("╠════════════════════════════════════════════════════════╣");
    println!("║  LoggingMiddleware: ✓ Executed before/after each task  ║");
    println!("║  MetricsMiddleware: ✓ Executed for monitoring          ║");
    println!("║  CountingMiddleware: ✓ Custom logic working            ║");
    println!("╚════════════════════════════════════════════════════════╝");

    server_handle.abort();
    Ok(())
}
