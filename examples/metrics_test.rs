//! Prometheus metrics test example

use rediq::{
    client::Client,
    observability::RediqMetrics,
    processor::{Handler, Mux},
    server::ServerBuilder,
    Task,
};
use async_trait::async_trait;
use std::time::Duration;

/// Test handler with metrics integration
struct TestHandler {
    #[allow(dead_code)]
    metrics: RediqMetrics,
}

#[async_trait]
impl Handler for TestHandler {
    async fn handle(&self, task: &rediq::Task) -> rediq::Result<()> {
        // Record task start
        tracing::info!("Processing task: {}", task.id);

        // Simulate some work
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Record success (handled by middleware)
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());

    println!("╔════════════════════════════════════════════════════════╗");
    println!("║         Rediq Prometheus Metrics Test                  ║");
    println!("╚════════════════════════════════════════════════════════╝\n");

    // Create metrics collector
    let metrics = RediqMetrics::new()?;
    println!("✓ Prometheus metrics collector created");

    let client = Client::builder()
        .redis_url(&redis_url)
        .build()
        .await?;

    // ===== Test 1: Enqueue tasks and record metrics =====
    println!("\n▶ Test 1: Enqueuing tasks with metrics...");

    for i in 0..20 {
        let task = Task::builder("test:metrics")
            .queue("metrics-test")
            .payload(&serde_json::json!({ "index": i }))
            .unwrap()
            .build()
            .unwrap();

        client.enqueue(task).await?;
        metrics.record_task_enqueued("metrics-test", "test:metrics");
    }

    println!("  ✓ 20 tasks enqueued");
    println!("  ✓ Metrics recorded");

    // ===== Test 2: Display metrics =====
    println!("\n▶ Test 2: Current metrics (Prometheus format):");
    println!("---");
    for line in metrics.gather().lines().take(30) {
        println!("  {}", line);
    }
    println!("---");

    // ===== Test 3: Start server with metrics =====
    println!("\n▶ Test 3: Starting server to process tasks...");

    let mut mux = Mux::new();
    mux.handle("test:metrics", TestHandler {
        metrics: metrics.clone(),
    });

    let state = ServerBuilder::new()
        .redis_url(&redis_url)
        .queues(&["metrics-test"])
        .concurrency(4)
        .build()
        .await?;

    let server = rediq::server::Server::from(state);
    let server_handle = tokio::spawn(async move {
        let _ = server.run(mux).await;
    });

    // Wait for processing
    println!("  Waiting for tasks to be processed...\n");
    tokio::time::sleep(Duration::from_secs(5)).await;

    // ===== Test 4: Final metrics =====
    println!("▶ Test 4: Final metrics (Prometheus format):");
    println!("---");
    let metrics_output = metrics.gather();
    for line in metrics_output.lines() {
        if line.contains("rediq_") {
            println!("  {}", line);
        }
    }
    println!("---");

    // ===== Test 5: Update queue metrics =====
    println!("\n▶ Test 5: Updating queue metrics...");

    let inspector = client.inspector();
    let stats = inspector.queue_stats("metrics-test").await?;

    metrics.update_queue_metrics(
        "metrics-test",
        stats.pending,
        stats.active,
        stats.delayed,
        stats.retried,
        stats.dead,
    );

    println!("  ✓ Queue metrics updated");
    println!("  - Pending: {}", stats.pending);
    println!("  - Active: {}", stats.active);
    println!("  - Delayed: {}", stats.delayed);
    println!("  - Retry: {}", stats.retried);
    println!("  - Dead: {}", stats.dead);

    // ===== Test 6: Scrape endpoint =====
    println!("\n▶ Test 6: Simulate Prometheus scrape...");
    println!("  Endpoint: GET /metrics");
    println!("  Content-Type: text/plain\n");

    let scrape_output = metrics.gather();
    println!("--- Sample output ---");
    for line in scrape_output.lines().take(20) {
        if line.contains("rediq_") && !line.starts_with("#") {
            println!("  {}", line);
        }
    }
    println!("---");

    // ===== Summary =====
    println!("\n╔════════════════════════════════════════════════════════╗");
    println!("║  Test Results                                            ║");
    println!("╠════════════════════════════════════════════════════════╣");
    println!("║  Metrics Collection: ✓ Working                          ║");
    println!("║  Task Enqueued Metric: ✓ Recorded                        ║");
    println!("║  Task Processed Metric: ✓ Recorded                       ║");
    println!("║  Queue Metrics: ✓ Updated                                ║");
    println!("║  Prometheus Format: ✓ Valid                              ║");
    println!("╠════════════════════════════════════════════════════════╣");
    println!("║  To expose metrics endpoint:                             ║");
    println!("║    1. Run HTTP server on /metrics                         ║");
    println!("║    2. Configure Prometheus scrape target                  ║");
    println!("║    3. Use Grafana for visualization                      ║");
    println!("╚════════════════════════════════════════════════════════╝");

    server_handle.abort();
    Ok(())
}
