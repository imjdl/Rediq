//! HTTP Metrics Endpoint Example
//!
//! This example demonstrates how to enable the HTTP metrics endpoint
//! for Prometheus scraping.
//!
//! Run with:
//!   cargo run --example metrics_http_example
//!
//! Then access the metrics at:
//!   http://localhost:9090/metrics
//!
//! Or use curl:
//!   curl http://localhost:9090/metrics

use rediq::client::Client;
use rediq::processor::{Handler, Mux};
use rediq::server::ServerBuilder;
use rediq::task::TaskBuilder;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Serialize, Deserialize)]
struct EmailData {
    to: String,
    subject: String,
    body: String,
}

struct EmailHandler;

#[async_trait]
impl Handler for EmailHandler {
    async fn handle(&self, task: &rediq::Task) -> rediq::Result<()> {
        // Convert payload bytes to string, then parse JSON
        let payload_str = std::str::from_utf8(&task.payload)
            .map_err(|e| rediq::Error::Serialization(e.to_string()))?;
        let data: EmailData = serde_json::from_str(payload_str)
            .map_err(|e| rediq::Error::Serialization(e.to_string()))?;

        tracing::info!("Sending email to {}: {}", data.to, data.subject);

        // Simulate email sending
        sleep(Duration::from_millis(100)).await;

        Ok(())
    }
}

struct ImageHandler;

#[async_trait]
impl Handler for ImageHandler {
    async fn handle(&self, task: &rediq::Task) -> rediq::Result<()> {
        tracing::info!("Processing image: {}", task.id);

        // Simulate image processing (slower)
        sleep(Duration::from_millis(500)).await;

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tracing::info!("Starting HTTP Metrics Example");

    // Build server state
    let state = ServerBuilder::new()
        .redis_url("redis://192.168.1.128:6379")
        .queues(&["default", "critical"])
        .concurrency(5)
        .build()
        .await?;

    // Create server and enable metrics
    let mut server = rediq::server::Server::from(state);
    server.enable_metrics("0.0.0.0:9090".parse::<std::net::SocketAddr>().unwrap())?;

    // Register handlers
    let mut mux = Mux::new();
    mux.handle("email:send", EmailHandler);
    mux.handle("image:process", ImageHandler);

    // Spawn a task to enqueue some test data
    let enqueuer = tokio::spawn(async move {
        sleep(Duration::from_secs(2)).await;

        let client = Client::builder()
            .redis_url("redis://192.168.1.128:6379")
            .build()
            .await
            .unwrap();

        for i in 0..10 {
            let task = TaskBuilder::new("email:send")
                .queue("default")
                .payload(&EmailData {
                    to: format!("user{}@example.com", i),
                    subject: format!("Test Email {}", i),
                    body: "Hello from Rediq!".to_string(),
                })
                .unwrap()
                .build()
                .unwrap();

            client.enqueue(task).await.unwrap();
            tracing::info!("Enqueued email task {}", i);
        }

        // Enqueue some image processing tasks (slower)
        for i in 0..3 {
            let task = TaskBuilder::new("image:process")
                .queue("critical")
                .payload(&serde_json::json!({"image_id": i}))
                .unwrap()
                .build()
                .unwrap();

            client.enqueue(task).await.unwrap();
            tracing::info!("Enqueued image task {}", i);
        }
    });

    tracing::info!("==============================================");
    tracing::info!("Metrics HTTP server is running!");
    tracing::info!("==============================================");
    tracing::info!("Access metrics at: http://localhost:9090/metrics");
    tracing::info!("Or use: curl http://localhost:9090/metrics");
    tracing::info!("==============================================");
    tracing::info!("Press Ctrl+C to stop the server");

    // Run the server (this will block until Ctrl+C)
    tokio::select! {
        _ = server.run(mux) => {},
        r = enqueuer => { r?; },
    }

    Ok(())
}
