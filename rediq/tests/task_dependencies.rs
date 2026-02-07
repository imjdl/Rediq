//! Task dependencies tests
//!
//! Tests task dependency management:
//! - Dependent tasks wait for dependencies
//! - Task execution order with dependencies
//! - Circular dependency detection

use rediq::{client::Client, task::TaskBuilder};

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_task_execution_with_dependencies() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let queue_name = format!("test-deps-{}", uuid::Uuid::new_v4());

    let client = Client::builder().redis_url(&redis_url).build().await.unwrap();

    // Enqueue task A (no dependencies)
    let task_a = TaskBuilder::new("test:task:a")
        .queue(&queue_name)
        .payload(&serde_json::json!({"data": "A"})).unwrap()
        .build()
        .unwrap();
    let task_a_id = client.enqueue(task_a).await.unwrap();

    // Enqueue task B that depends on A
    let task_b = TaskBuilder::new("test:task:b")
        .queue(&queue_name)
        .depends_on(&[&task_a_id])
        .payload(&serde_json::json!({"data": "B"})).unwrap()
        .build()
        .unwrap();
    let task_b_id = client.enqueue(task_b).await.unwrap();

    // Task A should be in pending queue
    // Task B should NOT be in pending queue (waiting for A)
    let stats = client.inspector().queue_stats(&queue_name).await.unwrap();
    // Only task A should be pending
    assert_eq!(stats.pending, 1, "Only task A should be pending");

    // Verify task B exists but is not enqueued
    let task_b_info = client.inspector().get_task(&task_b_id).await.unwrap();
    assert_eq!(task_b_info.status, rediq::task::TaskStatus::Pending);

    // Clean up
    let _ = client.flush_queue(&queue_name).await;
}

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_multiple_dependencies() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let queue_name = format!("test-multi-deps-{}", uuid::Uuid::new_v4());

    let client = Client::builder().redis_url(&redis_url).build().await.unwrap();

    // Create three independent tasks
    let task_a = TaskBuilder::new("test:task:a")
        .queue(&queue_name)
        .payload(&serde_json::json!({})).unwrap()
        .build()
        .unwrap();
    let task_a_id = client.enqueue(task_a).await.unwrap();

    let task_b = TaskBuilder::new("test:task:b")
        .queue(&queue_name)
        .payload(&serde_json::json!({})).unwrap()
        .build()
        .unwrap();
    let task_b_id = client.enqueue(task_b).await.unwrap();

    let task_c = TaskBuilder::new("test:task:c")
        .queue(&queue_name)
        .payload(&serde_json::json!({})).unwrap()
        .build()
        .unwrap();
    let task_c_id = client.enqueue(task_c).await.unwrap();

    // Task D depends on A, B, and C
    let task_d = TaskBuilder::new("test:task:d")
        .queue(&queue_name)
        .depends_on(&[&task_a_id, &task_b_id, &task_c_id])
        .payload(&serde_json::json!({})).unwrap()
        .build()
        .unwrap();
    let _task_d_id = client.enqueue(task_d).await.unwrap();

    // Only A, B, C should be in pending queue
    let stats = client.inspector().queue_stats(&queue_name).await.unwrap();
    assert_eq!(stats.pending, 3, "Only independent tasks should be pending");

    // Clean up
    let _ = client.flush_queue(&queue_name).await;
}

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_dependency_chain() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let queue_name = format!("test-chain-{}", uuid::Uuid::new_v4());

    let client = Client::builder().redis_url(&redis_url).build().await.unwrap();

    // Create a chain: A -> B -> C -> D
    let task_a = TaskBuilder::new("test:task:a")
        .queue(&queue_name)
        .payload(&serde_json::json!({})).unwrap()
        .build()
        .unwrap();
    let task_a_id = client.enqueue(task_a).await.unwrap();

    let task_b = TaskBuilder::new("test:task:b")
        .queue(&queue_name)
        .depends_on(&[&task_a_id])
        .payload(&serde_json::json!({})).unwrap()
        .build()
        .unwrap();
    let task_b_id = client.enqueue(task_b).await.unwrap();

    let task_c = TaskBuilder::new("test:task:c")
        .queue(&queue_name)
        .depends_on(&[&task_b_id])
        .payload(&serde_json::json!({})).unwrap()
        .build()
        .unwrap();
    let task_c_id = client.enqueue(task_c).await.unwrap();

    let task_d = TaskBuilder::new("test:task:d")
        .queue(&queue_name)
        .depends_on(&[&task_c_id])
        .payload(&serde_json::json!({})).unwrap()
        .build()
        .unwrap();
    let _task_d_id = client.enqueue(task_d).await.unwrap();

    // Only task A should be in pending queue (the root of the chain)
    let stats = client.inspector().queue_stats(&queue_name).await.unwrap();
    assert_eq!(stats.pending, 1, "Only root task should be pending in a chain");

    // Clean up
    let _ = client.flush_queue(&queue_name).await;
}
