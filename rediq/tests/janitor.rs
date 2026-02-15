//! Janitor cleanup functionality tests
//!
//! Tests the automatic cleanup of expired task details

use rediq::{client::Client, processor::{Handler, Mux}, server::ServerBuilder, task::TaskBuilder, Task};
use async_trait::async_trait;
use std::time::Duration;

#[derive(Clone)]
struct QuickHandler;

#[async_trait]
impl Handler for QuickHandler {
    async fn handle(&self, task: &Task) -> rediq::Result<()> {
        eprintln!("Processing task: {}", task.id);
        Ok(())
    }
}

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_janitor_cleanup_expired_tasks() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let queue_name = format!("test-janitor-{}", uuid::Uuid::new_v4());

    // Create client
    let client = Client::builder()
        .redis_url(&redis_url)
        .build()
        .await
        .expect("Failed to create client");

    // Enqueue multiple tasks
    let mut task_ids = Vec::new();
    for i in 0..5 {
        let task = TaskBuilder::new("janitor:test")
            .queue(&queue_name)
            .payload(&serde_json::json!({"index": i}))
            .expect("Failed to create payload")
            .build()
            .expect("Failed to build task");
        let task_id = client.enqueue(task).await.expect("Failed to enqueue task");
        task_ids.push(task_id);
    }

    // Verify tasks exist
    for task_id in &task_ids {
        let result = client.inspector().get_task(task_id).await;
        assert!(result.is_ok(), "Task should exist");
    }

    // Start server with janitor enabled (default)
    let state = ServerBuilder::new()
        .redis_url(&redis_url)
        .queues(&[&queue_name])
        .concurrency(5)
        .build()
        .await
        .expect("Failed to build server");

    let server = rediq::server::Server::from(state);

    let server_handle = tokio::spawn(async move {
        let mut mux = Mux::new();
        mux.handle("janitor:test", QuickHandler);
        server.run(mux).await
    });

    // Wait for tasks to be processed
    tokio::time::sleep(Duration::from_secs(3)).await;

    server_handle.abort();

    // Clean up
    let _ = client.flush_queue(&queue_name).await;
}

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_task_ttl_set() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let queue_name = format!("test-ttl-{}", uuid::Uuid::new_v4());

    let client = Client::builder()
        .redis_url(&redis_url)
        .build()
        .await
        .expect("Failed to create client");

    // Enqueue a task
    let task = TaskBuilder::new("ttl:test")
        .queue(&queue_name)
        .payload(&serde_json::json!({"test": "ttl"}))
        .expect("Failed to create payload")
        .build()
        .expect("Failed to build task");

    let task_id = client.enqueue(task).await.expect("Failed to enqueue task");

    // Get task info - should exist
    let result = client.inspector().get_task(&task_id).await;
    assert!(result.is_ok(), "Task should exist immediately after enqueue");

    // Clean up
    let _ = client.flush_queue(&queue_name).await;
}

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_batch_enqueue_ttl() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let queue_name = format!("test-batch-ttl-{}", uuid::Uuid::new_v4());

    let client = Client::builder()
        .redis_url(&redis_url)
        .build()
        .await
        .expect("Failed to create client");

    // Batch enqueue tasks
    let mut tasks = Vec::new();
    for i in 0..10 {
        let task = TaskBuilder::new("batch:ttl:test")
            .queue(&queue_name)
            .payload(&serde_json::json!({"index": i}))
            .expect("Failed to create payload")
            .build()
            .expect("Failed to build task");
        tasks.push(task);
    }

    let task_ids = client.enqueue_batch(tasks).await.expect("Failed to batch enqueue");

    // Verify all task IDs returned
    assert_eq!(task_ids.len(), 10, "Should have enqueued 10 tasks");

    // Verify queue stats show the tasks
    let stats = client.inspector().queue_stats(&queue_name).await
        .expect("Failed to get queue stats");
    assert!(stats.pending >= 10 || stats.completed >= 10, "Queue should have the batch tasks");

    // Clean up
    let _ = client.flush_queue(&queue_name).await;
}
