//! Task progress tracking functionality tests
//!
//! Tests the progress reporting and tracking during task execution

use rediq::{client::Client, processor::{Handler, Mux}, server::ServerBuilder, task::TaskBuilder, Task};
use rediq::task::TaskProgressExt;
use async_trait::async_trait;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
struct ProgressHandler {
    max_progress: Arc<AtomicU32>,
}

#[async_trait]
impl Handler for ProgressHandler {
    async fn handle(&self, task: &Task) -> rediq::Result<()> {
        // Simulate progress reporting
        for i in 1..=10 {
            let progress = i * 10;
            if let Some(ctx) = task.progress() {
                let _ = ctx.report(progress).await;
                self.max_progress.fetch_max(progress, Ordering::Relaxed);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Ok(())
    }
}

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_progress_tracking() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let queue_name = format!("test-progress-{}", uuid::Uuid::new_v4());

    let max_progress = Arc::new(AtomicU32::new(0));
    let handler = ProgressHandler { max_progress: max_progress.clone() };

    // Create client
    let client = Client::builder()
        .redis_url(&redis_url)
        .build()
        .await
        .expect("Failed to create client");

    // Enqueue task
    let task = TaskBuilder::new("progress:test")
        .queue(&queue_name)
        .payload(&serde_json::json!({"test": "progress"}))
        .expect("Failed to create payload")
        .build()
        .expect("Failed to build task");

    let task_id = client.enqueue(task).await.expect("Failed to enqueue task");
    eprintln!("Task enqueued: {}", task_id);

    // Start server
    let state = ServerBuilder::new()
        .redis_url(&redis_url)
        .queues(&[&queue_name])
        .concurrency(1)
        .build()
        .await
        .expect("Failed to build server");

    let server = rediq::server::Server::from(state);
    let handler_clone = handler.clone();

    let server_handle = tokio::spawn(async move {
        let mut mux = Mux::new();
        mux.handle("progress:test", handler_clone);
        server.run(mux).await
    });

    // Wait for task to be processed
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Check max progress reported
    let final_progress = max_progress.load(Ordering::Relaxed);
    eprintln!("Max progress reported: {}", final_progress);
    assert!(final_progress > 0, "Progress should have been reported");

    server_handle.abort();

    // Clean up
    let _ = client.flush_queue(&queue_name).await;
}

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_progress_query() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let queue_name = format!("test-progress-query-{}", uuid::Uuid::new_v4());

    let client = Client::builder()
        .redis_url(&redis_url)
        .build()
        .await
        .expect("Failed to create client");

    // Enqueue task
    let task = TaskBuilder::new("progress:query:test")
        .queue(&queue_name)
        .payload(&serde_json::json!({"test": "query"}))
        .expect("Failed to create payload")
        .build()
        .expect("Failed to build task");

    let task_id = client.enqueue(task).await.expect("Failed to enqueue task");

    // Query progress (may be None if not yet started)
    let progress = client.inspector().get_task_progress(&task_id).await
        .expect("Failed to get task progress");
    // Progress may be None before task starts
    eprintln!("Initial progress: {:?}", progress);

    // Clean up
    let _ = client.flush_queue(&queue_name).await;
}

#[test]
fn test_progress_context_none_initially() {
    // Test that progress context is None initially
    let task = TaskBuilder::new("test:no:progress")
        .queue("test")
        .payload(&serde_json::json!({"test": true}))
        .expect("Failed to create payload")
        .build()
        .expect("Failed to build task");

    // Progress should be None outside of handler execution
    assert!(task.progress().is_none(), "Progress should be None initially");
    assert!(!task.has_progress(), "has_progress should be false initially");
}

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_multiple_tasks_progress() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let queue_name = format!("test-multi-progress-{}", uuid::Uuid::new_v4());

    let client = Client::builder()
        .redis_url(&redis_url)
        .build()
        .await
        .expect("Failed to create client");

    // Enqueue multiple tasks
    let mut task_ids = Vec::new();
    for i in 0..3 {
        let task = TaskBuilder::new("multi:progress:test")
            .queue(&queue_name)
            .payload(&serde_json::json!({"task_index": i}))
            .expect("Failed to create payload")
            .build()
            .expect("Failed to build task");

        let task_id = client.enqueue(task).await.expect("Failed to enqueue task");
        task_ids.push(task_id);
    }

    // Verify all tasks exist
    for task_id in &task_ids {
        let result = client.inspector().get_task(task_id).await;
        assert!(result.is_ok(), "Task should exist");
    }

    // Clean up
    let _ = client.flush_queue(&queue_name).await;
}
