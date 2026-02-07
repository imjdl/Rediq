//! Redis Sentinel Example
//!
//! This example demonstrates how to connect to Redis Sentinel
//! for high availability with automatic failover.
//!
//! Run with:
//!   cargo run --example sentinel_example
//!
//! Note: This requires a Redis Sentinel setup running.

use rediq::client::Client;
use rediq::processor::Handler;
use rediq::server::ServerBuilder;
use rediq::Task;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Serialize, Deserialize)]
struct TaskData {
    id: String,
    message: String,
}

#[allow(dead_code)]
struct ExampleHandler;

#[async_trait]
impl Handler for ExampleHandler {
    async fn handle(&self, task: &rediq::Task) -> rediq::Result<()> {
        let payload_str = std::str::from_utf8(&task.payload)
            .map_err(|e| rediq::Error::Serialization(e.to_string()))?;
        let data: TaskData = serde_json::from_str(payload_str)
            .map_err(|e| rediq::Error::Serialization(e.to_string()))?;

        tracing::info!("Processing task {}: {}", data.id, data.message);

        // Simulate task processing
        sleep(Duration::from_millis(100)).await;

        tracing::info!("Task {} completed", data.id);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tracing::info!("Starting Redis Sentinel Example");

    // Example Sentinel configuration
    // Can be overridden via REDIS_SENTINEL_URL environment variable
    let default_sentinel_url = "redis://127.0.0.1:26379";
    let sentinel_url = std::env::var("REDIS_SENTINEL_URL")
        .unwrap_or_else(|_| default_sentinel_url.to_string());

    // Example Sentinel configuration
    // Replace with your actual Sentinel addresses
    let sentinel_addrs = vec![
        &sentinel_url,
        // Additional sentinel addresses can be added here
        // "redis://192.168.1.128:26380",
        // "redis://192.168.1.128:26381",
    ];

    // Use the first Sentinel address (fred will auto-discover the master)
    let sentinel_url = sentinel_addrs[0];

    tracing::info!("==============================================");
    tracing::info!("Redis Sentinel Connection Examples");
    tracing::info!("==============================================");
    tracing::info!("Sentinel addresses: {:?}", sentinel_addrs);
    tracing::info!("Using sentinel: {}", sentinel_url);
    tracing::info!("");

    // Example 1: Create Client in Sentinel mode
    tracing::info!("Example 1: Creating Client in Sentinel mode...");
    let client = match Client::builder()
        .redis_url(sentinel_url)
        .sentinel_mode()
        .build()
        .await
    {
        Ok(c) => {
            tracing::info!("✓ Client connected to Redis Sentinel");
            c
        }
        Err(e) => {
            tracing::warn!("Failed to connect to sentinel: {}", e);
            tracing::info!("Note: This example requires a running Redis Sentinel setup");
            tracing::info!("");
            tracing::info!("To set up Redis Sentinel:");
            tracing::info!("1. Create a sentinel.conf file:");
            tracing::info!("   port 26379");
            tracing::info!("   sentinel monitor mymaster 127.0.0.1 6379 2");
            tracing::info!("   sentinel down-after-milliseconds mymaster 5000");
            tracing::info!("   sentinel failover-timeout mymaster 10000");
            tracing::info!("");
            tracing::info!("2. Start Sentinel: redis-sentinel sentinel.conf");
            tracing::info!("");
            tracing::info!("3. For production, run multiple Sentinel instances");
            return Ok(());
        }
    };

    // Example 2: Create Server in Sentinel mode
    tracing::info!("");
    tracing::info!("Example 2: Creating Server in Sentinel mode...");
    let _server_state = match ServerBuilder::new()
        .redis_url(sentinel_url)
        .sentinel_mode()
        .queues(&["tasks", "default"])
        .concurrency(5)
        .build()
        .await
    {
        Ok(state) => {
            tracing::info!("✓ Server configured for Redis Sentinel");
            state
        }
        Err(e) => {
            tracing::warn!("Failed to create server: {}", e);
            return Ok(());
        }
    };

    // Example 3: Enqueue a task
    tracing::info!("");
    tracing::info!("Example 3: Enqueuing a task...");
    let task_json = serde_json::to_string(&TaskData {
        id: "task-456".to_string(),
        message: "Hello from Redis Sentinel!".to_string(),
    }).unwrap();

    let task = Task::builder("process:task")
        .queue("tasks")
        .raw_payload(task_json.into_bytes())
        .max_retry(3)
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();

    match client.enqueue(task).await {
        Ok(task_id) => {
            tracing::info!("✓ Task enqueued with ID: {}", task_id);
        }
        Err(e) => {
            tracing::warn!("Failed to enqueue task: {}", e);
        }
    }

    // Example 4: Get queue statistics
    tracing::info!("");
    tracing::info!("Example 4: Getting queue statistics...");
    let inspector = client.inspector();
    match inspector.queue_stats("tasks").await {
        Ok(stats) => {
            tracing::info!("✓ Queue 'tasks' statistics:");
            tracing::info!("  - Pending: {}", stats.pending);
            tracing::info!("  - Active: {}", stats.active);
            tracing::info!("  - Delayed: {}", stats.delayed);
            tracing::info!("  - Retried: {}", stats.retried);
            tracing::info!("  - Dead: {}", stats.dead);
        }
        Err(e) => {
            tracing::warn!("Failed to get queue stats: {}", e);
        }
    }

    tracing::info!("");
    tracing::info!("==============================================");
    tracing::info!("Sentinel Examples Completed!");
    tracing::info!("==============================================");
    tracing::info!("");
    tracing::info!("Key Benefits of Redis Sentinel:");
    tracing::info!("1. Automatic failover - if master fails, slave is promoted");
    tracing::info!("2. High availability - system remains operational during failures");
    tracing::info!("3. Configuration provider - clients get current master address");
    tracing::info!("4. Transparent operation - Rediq works seamlessly with Sentinel");
    tracing::info!("");
    tracing::info!("The fred library automatically:");
    tracing::info!("- Discovers the current master from Sentinel");
    tracing::info!("- Reconnects to new master after failover");
    tracing::info!("- Handles connection pooling and retries");

    // If you want to run the server with a handler, uncomment below:
    // let server = Server::from(server_state);
    // let mut mux = Mux::new();
    // mux.handle("process:task", ExampleHandler);
    // server.run(mux).await?;

    Ok(())
}
