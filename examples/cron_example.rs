//! Cron Task Example
//!
//! This example demonstrates how to schedule periodic tasks using cron expressions.
//!
//! Run with:
//!   cargo run --example cron_example
//!
//! The cron tasks will be scheduled and executed periodically based on their cron expressions.

use rediq::client::Client;
use rediq::processor::{Handler, Mux};
use rediq::server::{Server, ServerBuilder};
use rediq::Task;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Serialize, Deserialize)]
struct CleanupData {
    target: String,
    description: String,
}

struct CleanupHandler;

#[async_trait]
impl Handler for CleanupHandler {
    async fn handle(&self, task: &rediq::Task) -> rediq::Result<()> {
        let payload_str = std::str::from_utf8(&task.payload)
            .map_err(|e| rediq::Error::Serialization(e.to_string()))?;
        let data: CleanupData = serde_json::from_str(payload_str)
            .map_err(|e| rediq::Error::Serialization(e.to_string()))?;

        tracing::info!("Running cleanup task: {} - {}", data.target, data.description);

        // Simulate cleanup work
        sleep(Duration::from_millis(100)).await;

        tracing::info!("Cleanup completed: {}", data.target);
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ReportData {
    report_type: String,
}

struct ReportHandler;

#[async_trait]
impl Handler for ReportHandler {
    async fn handle(&self, task: &rediq::Task) -> rediq::Result<()> {
        let payload_str = std::str::from_utf8(&task.payload)
            .map_err(|e| rediq::Error::Serialization(e.to_string()))?;
        let data: ReportData = serde_json::from_str(payload_str)
            .map_err(|e| rediq::Error::Serialization(e.to_string()))?;

        tracing::info!("Generating report: {}", data.report_type);

        // Simulate report generation
        sleep(Duration::from_millis(200)).await;

        tracing::info!("Report generated: {}", data.report_type);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tracing::info!("Starting Cron Task Example");

    // Build server state
    let state = ServerBuilder::new()
        .redis_url("redis://192.168.1.128:6379")
        .queues(&["maintenance", "reports"])
        .concurrency(2)
        .build()
        .await?;

    // Create server
    let server = Server::from(state);

    // Register handlers
    let mut mux = Mux::new();
    mux.handle("cleanup:daily", CleanupHandler);
    mux.handle("report:hourly", ReportHandler);

    // Spawn task enqueuer
    let enqueuer = tokio::spawn(async move {
        if let Err(e) = (async move {
            sleep(Duration::from_secs(2)).await;

            let client = Client::builder()
                .redis_url("redis://192.168.1.128:6379")
                .build()
                .await?;

            // Enqueue cron tasks with different schedules
            // Cron format: seconds minutes hours day month weekday
            // Example: "0 */5 * * * *" = every 5 minutes

            // Cleanup task - every 30 seconds for demo (use "0 0 * * * *" for daily)
            let cleanup_json = serde_json::to_string(&CleanupData {
                target: "temp_files".to_string(),
                description: "Clean up temporary files".to_string(),
            }).unwrap();

            let cleanup_task = Task::builder("cleanup:daily")
                .queue("maintenance")
                .cron("0 */30 * * * *")
                .raw_payload(cleanup_json.into_bytes())
                .max_retry(3)
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap();

            client.enqueue_cron(cleanup_task).await?;
            tracing::info!("Enqueued cleanup cron task (every 30 seconds)");

            // Report task - every 45 seconds for demo (use "0 0 * * * *" for daily)
            let report_json = serde_json::to_string(&ReportData {
                report_type: "daily_summary".to_string(),
            }).unwrap();

            let report_task = Task::builder("report:hourly")
                .queue("reports")
                .cron("0 */45 * * * *")
                .raw_payload(report_json.into_bytes())
                .max_retry(3)
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap();

            client.enqueue_cron(report_task).await?;
            tracing::info!("Enqueued report cron task (every 45 seconds)");

            tracing::info!("==============================================");
            tracing::info!("Cron tasks scheduled!");
            tracing::info!("- Cleanup task: every 30 seconds");
            tracing::info!("- Report task: every 45 seconds");
            tracing::info!("==============================================");
            tracing::info!("Watch for periodic task executions...");

            Ok::<(), Box<dyn std::error::Error>>(())
        }).await {
            tracing::error!("Enqueuer error: {}", e);
        }
    });

    tracing::info!("==============================================");
    tracing::info!("Worker is running...");
    tracing::info!("==============================================");
    tracing::info!("Press Ctrl+C to stop the server");

    // Run the server
    tokio::select! {
        _ = server.run(mux) => {},
        r = enqueuer => { r?; },
    }

    Ok(())
}
