//! Redis Cluster Example
//!
//! This example demonstrates how to connect to Redis Cluster
//! and use the Client and Server in cluster mode.
//!
//! Run with:
//!   cargo run --example cluster_example
//!
//! Note: This requires a Redis Cluster running on the specified nodes.

use rediq::client::Client;
use rediq::processor::Handler;
use rediq::server::ServerBuilder;
use rediq::Task;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Serialize, Deserialize)]
struct JobData {
    job_id: String,
    message: String,
}

#[allow(dead_code)]
struct JobHandler;

#[async_trait]
impl Handler for JobHandler {
    async fn handle(&self, task: &rediq::Task) -> rediq::Result<()> {
        let payload_str = std::str::from_utf8(&task.payload)
            .map_err(|e| rediq::Error::Serialization(e.to_string()))?;
        let data: JobData = serde_json::from_str(payload_str)
            .map_err(|e| rediq::Error::Serialization(e.to_string()))?;

        tracing::info!("Processing job {}: {}", data.job_id, data.message);

        // Simulate job processing
        sleep(Duration::from_millis(100)).await;

        tracing::info!("Job {} completed", data.job_id);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tracing::info!("Starting Redis Cluster Example");

    // Example cluster node URLs
    // Can be overridden via REDIS_CLUSTER_URL environment variable
    let default_cluster_url = "redis://127.0.0.1:7000";
    let cluster_url = std::env::var("REDIS_CLUSTER_URL")
        .unwrap_or_else(|_| default_cluster_url.to_string());

    // Example cluster node URLs
    // Replace with your actual cluster node addresses
    let cluster_nodes = vec![
        &cluster_url,
        // Additional cluster nodes can be added here
        // "redis://192.168.1.128:7001",
        // "redis://192.168.1.128:7002",
    ];

    // Use the first node for connection (fred will auto-discover other nodes)
    let cluster_url = cluster_nodes[0];

    tracing::info!("==============================================");
    tracing::info!("Redis Cluster Connection Examples");
    tracing::info!("==============================================");
    tracing::info!("Cluster nodes: {:?}", cluster_nodes);
    tracing::info!("Using node: {}", cluster_url);
    tracing::info!("");

    // Example 1: Create Client in Cluster mode
    tracing::info!("Example 1: Creating Client in Cluster mode...");
    let client = match Client::builder()
        .redis_url(cluster_url)
        .cluster_mode()
        .build()
        .await
    {
        Ok(c) => {
            tracing::info!("✓ Client connected to Redis Cluster");
            c
        }
        Err(e) => {
            tracing::warn!("Failed to connect to cluster: {}", e);
            tracing::info!("Note: This example requires a running Redis Cluster");
            tracing::info!("You can create a test cluster using: redis-cli --cluster create ...");
            return Ok(());
        }
    };

    // Example 2: Create Server in Cluster mode
    tracing::info!("");
    tracing::info!("Example 2: Creating Server in Cluster mode...");
    let _server_state = match ServerBuilder::new()
        .redis_url(cluster_url)
        .cluster_mode()
        .queues(&["jobs", "default"])
        .concurrency(5)
        .build()
        .await
    {
        Ok(state) => {
            tracing::info!("✓ Server configured for Redis Cluster");
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
    let job_json = serde_json::to_string(&JobData {
        job_id: "job-123".to_string(),
        message: "Hello from Redis Cluster!".to_string(),
    }).unwrap();

    let task = Task::builder("process:job")
        .queue("jobs")
        .raw_payload(job_json.into_bytes())
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
    match inspector.queue_stats("jobs").await {
        Ok(stats) => {
            tracing::info!("✓ Queue 'jobs' statistics:");
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
    tracing::info!("Cluster Examples Completed!");
    tracing::info!("==============================================");
    tracing::info!("");
    tracing::info!("Key Points:");
    tracing::info!("1. Use .cluster_mode() to enable cluster support");
    tracing::info!("2. Provide any cluster node URL (auto-discovery)");
    tracing::info!("3. All Rediq features work transparently in cluster mode");
    tracing::info!("");
    tracing::info!("For a complete working example, set up a Redis Cluster:");
    tracing::info!("  redis-cli --cluster create 127.0.0.1:7000 127.0.0.1:7001 \\");
    tracing::info!("    127.0.0.1:7002 127.0.0.1:7003 127.0.0.1:7004 127.0.0.1:7005 \\");
    tracing::info!("    --cluster-replicas 1");

    // If you want to run the server with a handler, uncomment below:
    // let server = Server::from(server_state);
    // let mut mux = Mux::new();
    // mux.handle("process:job", JobHandler);
    // server.run(mux).await?;

    Ok(())
}
