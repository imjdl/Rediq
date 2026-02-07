//! Task progress tracking example
//!
//! This example demonstrates how to use task progress tracking
//! to report execution progress during task processing.
//!
//! Run with: cargo run --example progress_example

use rediq::client::Client;
use rediq::processor::{Handler, Mux};
use rediq::server::{Server, ServerBuilder};
use rediq::task::TaskBuilder;
use rediq::task::TaskProgressExt;
use rediq::Task;
use async_trait::async_trait;
use std::time::Duration;
use tokio::time::sleep;

/// Video processing handler that demonstrates progress reporting
struct VideoProcessingHandler;

#[async_trait]
impl Handler for VideoProcessingHandler {
    async fn handle(&self, task: &Task) -> rediq::Result<()> {
        // Get progress reporter (returns None if not enabled)
        if let Some(progress) = task.progress() {
            tracing::info!("[Task {}] Starting video processing", task.id);

            // Report initial progress
            progress.report_with_message(0, Some("Starting video processing")).await?;

            // Stage 1: Download video
            for i in 1..=5 {
                sleep(Duration::from_millis(300)).await;
                let p = 5 + (i * 3);
                progress.report_with_message(p, Some(&format!("Downloading... {}%", p * 4))).await?;
                tracing::info!("[Task {}] Downloading: {}%", task.id, p * 4);
            }

            // Stage 2: Decode video
            progress.report_with_message(20, Some("Video downloaded")).await?;
            for i in 1..=5 {
                sleep(Duration::from_millis(300)).await;
                let p = 20 + (i * 4);
                progress.report_with_message(p, Some("Decoding...")).await?;
                tracing::info!("[Task {}] Decoding: {}%", task.id, p);
            }

            // Stage 3: Process video frames
            for i in 0..5 {
                sleep(Duration::from_millis(500)).await;
                let current = 45 + (i * 10);
                let msg = format!("Processing frame {}/5", i + 1);
                progress.report_with_message(current, Some(msg.as_str())).await?;
                tracing::info!("[Task {}] {}", task.id, msg);
            }

            // Stage 4: Encode output
            progress.report_with_message(95, Some("Encoding...")).await?;
            sleep(Duration::from_millis(500)).await;

            // Final: Upload and complete
            progress.report_with_message(100, Some("Processing completed")).await?;
            tracing::info!("[Task {}] Video processing completed!", task.id);
        } else {
            tracing::info!("Processing video without progress tracking: {}", task.id);
            sleep(Duration::from_secs(3)).await;
        }

        Ok(())
    }
}

/// Fallback handler for unregistered task types
struct FallbackHandler;

#[async_trait]
impl Handler for FallbackHandler {
    async fn handle(&self, task: &Task) -> rediq::Result<()> {
        tracing::warn!("No handler for task_type '{}', skipping task: {}", task.task_type, task.id);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

    tracing::info!("========================================");
    tracing::info!("   Task Progress Tracking Example");
    tracing::info!("========================================");
    tracing::info!("Redis: {}", redis_url);
    tracing::info!("");

    // Create client
    let client = Client::builder()
        .redis_url(&redis_url)
        .build()
        .await?;

    // Clear the queue first for clean demo
    tracing::info!("Cleaning queue...");
    let _ = client.flush_queue("default").await;
    tracing::info!("");

    // Create a single video processing task
    tracing::info!("Enqueueing video processing task...");
    let task = TaskBuilder::new("video:process")
        .queue("default")
        .payload(&"sample.mp4")?
        .build()?;

    client.enqueue(task).await?;
    tracing::info!("Task enqueued!");
    tracing::info!("");

    tracing::info!("========================================");
    tracing::info!("Starting worker...");
    tracing::info!("You should see progress updates below:");
    tracing::info!("========================================");
    tracing::info!("");

    // Spawn a progress monitor task in the background
    let client_clone = client.clone();
    let monitor_handle = tokio::spawn(async move {
        let mut last_progress = None;
        let mut no_progress_count = 0;

        for _ in 0..100 {
            sleep(Duration::from_millis(200)).await;

            // Get all tasks in queue
            let inspector = client_clone.inspector();
            if let Ok(tasks) = inspector.list_tasks("default", 10).await {
                for task_info in tasks {
                    // Check progress for active/pending tasks
                    if let Ok(Some(progress)) = inspector.get_task_progress(&task_info.id).await {
                        let progress_str = format!("{}% - {}", progress.current,
                            progress.message.as_deref().unwrap_or("Working..."));

                        // Only print when progress changes
                        if last_progress.as_ref() != Some(&progress.current) {
                            match task_info.status {
                                rediq::task::TaskStatus::Active => {
                                    tracing::info!("▶ [{}] Progress: {}", task_info.id, progress_str);
                                }
                                rediq::task::TaskStatus::Processed => {
                                    tracing::info!("✓ [{}] Completed: {}%", task_info.id, progress.current);
                                }
                                _ => {}
                            }
                        }
                        last_progress = Some(progress.current);

                        // Exit if task completed
                        if progress.current >= 100 || task_info.status == rediq::task::TaskStatus::Processed {
                            return;
                        }
                    }
                }
            }

            // Check if queue is empty
            if let Ok(stats) = inspector.queue_stats("default").await {
                if stats.pending == 0 && stats.active == 0 {
                    no_progress_count += 1;
                    if no_progress_count > 5 {
                        tracing::info!("(No more tasks in queue)");
                        return;
                    }
                }
            }
        }
    });

    // Build and start server
    let state = ServerBuilder::new()
        .redis_url(&redis_url)
        .queues(&["default"])
        .concurrency(1)  // Single worker to see sequential progress
        .build()
        .await?;

    let server = Server::from(state);

    // Setup handlers
    let mut mux = Mux::new();
    mux.handle("video:process", VideoProcessingHandler);
    mux.default_handler(FallbackHandler);

    // Run server (processes tasks and exits when queue is empty)
    let server_handle = tokio::spawn(async move {
        let _ = server.run(mux).await;
    });

    // Wait for server or monitor to finish
    tokio::select! {
        _ = server_handle => {},
        _ = monitor_handle => {
            tracing::info!("");
            tracing::info!("========================================");
            tracing::info!("Task completed!");
            tracing::info!("========================================");
        }
    }

    // Show final task status
    let inspector = client.inspector();
    if let Ok(queues) = inspector.list_queues().await {
        for queue in queues {
            if let Ok(stats) = inspector.queue_stats(&queue).await {
                tracing::info!("Queue '{}': completed={}", queue, stats.completed);
            }
        }
    }

    Ok(())
}
