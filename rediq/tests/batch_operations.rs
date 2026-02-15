//! Batch operations tests
//!
//! Tests bulk task operations:
//! - Batch enqueue performance
//! - Batch operation atomicity
//! - Bulk status queries

use rediq::{client::Client, task::TaskBuilder};

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_batch_enqueue() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let queue_name = format!("test-batch-{}", uuid::Uuid::new_v4());

    let client = Client::builder().redis_url(&redis_url).build().await.unwrap();

    // Create multiple tasks
    let mut tasks = Vec::new();
    for i in 0..10 {
        let task = TaskBuilder::new("test:batch")
            .queue(&queue_name)
            .payload(&serde_json::json!({"index": i})).unwrap()
            .build()
            .unwrap();
        tasks.push(task);
    }

    // Enqueue all tasks
    let start = std::time::Instant::now();
    for task in tasks {
        client.enqueue(task).await.unwrap();
    }
    let duration = start.elapsed();

    println!("Batch enqueue of 10 tasks took: {:?}", duration);

    // Verify all tasks were enqueued
    let stats = client.inspector().queue_stats(&queue_name).await.unwrap();
    assert_eq!(stats.pending, 10, "All 10 tasks should be pending");

    // Clean up
    let _ = client.flush_queue(&queue_name).await;
}

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_multiple_queues() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());

    let client = Client::builder().redis_url(&redis_url).build().await.unwrap();

    let queue1 = format!("test-queue1-{}", uuid::Uuid::new_v4());
    let queue2 = format!("test-queue2-{}", uuid::Uuid::new_v4());

    // Enqueue to multiple queues
    let task1 = TaskBuilder::new("test:multi")
        .queue(&queue1)
        .payload(&serde_json::json!({"queue": 1})).unwrap()
        .build()
        .unwrap();
    client.enqueue(task1).await.unwrap();

    let task2 = TaskBuilder::new("test:multi")
        .queue(&queue2)
        .payload(&serde_json::json!({"queue": 2})).unwrap()
        .build()
        .unwrap();
    client.enqueue(task2).await.unwrap();

    // List all queues
    let queues = client.inspector().list_queues().await.unwrap();
    assert!(queues.contains(&queue1), "Queue1 should be listed");
    assert!(queues.contains(&queue2), "Queue2 should be listed");

    // Check individual queue stats
    let stats1 = client.inspector().queue_stats(&queue1).await.unwrap();
    assert_eq!(stats1.pending, 1);

    let stats2 = client.inspector().queue_stats(&queue2).await.unwrap();
    assert_eq!(stats2.pending, 1);

    // Clean up
    let _ = client.flush_queue(&queue1).await;
    let _ = client.flush_queue(&queue2).await;
}

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_bulk_task_inspection() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let queue_name = format!("test-bulk-inspect-{}", uuid::Uuid::new_v4());

    let client = Client::builder().redis_url(&redis_url).build().await.unwrap();

    // Enqueue multiple tasks
    let mut task_ids = Vec::new();
    for i in 0..5 {
        let task = TaskBuilder::new("test:inspect")
            .queue(&queue_name)
            .payload(&serde_json::json!({"index": i})).unwrap()
            .build()
            .unwrap();
        let id = client.enqueue(task).await.unwrap();
        task_ids.push(id);
    }

    // Inspect each task
    for task_id in task_ids.iter() {
        let task_info = client.inspector().get_task(task_id).await.unwrap();
        assert_eq!(task_info.id, *task_id);
        assert_eq!(task_info.task_type, "test:inspect");
    }

    // Clean up
    let _ = client.flush_queue(&queue_name).await;
}
