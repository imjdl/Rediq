//! Task aggregation functionality tests
//!
//! Tests the grouping and aggregation of tasks for batch processing

use rediq::{client::Client, processor::{Handler, Mux}, server::ServerBuilder, task::TaskBuilder, Task};
use rediq::aggregator::{AggregatorConfig, GroupAggregatorFunc};
use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
struct BatchHandler {
    processed: Arc<AtomicUsize>,
}

#[async_trait]
impl Handler for BatchHandler {
    async fn handle(&self, task: &Task) -> rediq::Result<()> {
        self.processed.fetch_add(1, Ordering::Relaxed);
        eprintln!("Batch handler processing task: {}", task.id);
        Ok(())
    }
}

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_task_grouping() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let queue_name = format!("test-aggregator-{}", uuid::Uuid::new_v4());

    let client = Client::builder()
        .redis_url(&redis_url)
        .build()
        .await
        .expect("Failed to create client");

    // Enqueue tasks with the same group
    let group_name = format!("test-group-{}", uuid::Uuid::new_v4());
    for i in 0..5 {
        let task = TaskBuilder::new("notification:send")
            .queue(&queue_name)
            .group(&group_name)
            .payload(&serde_json::json!({"user_id": i}))
            .expect("Failed to create payload")
            .build()
            .expect("Failed to build task");

        client.enqueue(task).await.expect("Failed to enqueue task");
    }

    // Verify tasks are NOT in the main queue (they are in group ZSet)
    // When tasks have a group, they are stored in the group ZSet for aggregation
    // rather than the main queue. The main queue should have 0 pending tasks.
    let stats = client.inspector().queue_stats(&queue_name).await
        .expect("Failed to get queue stats");

    // Tasks with group are stored in group ZSet, not main queue
    // So pending should be 0 (or very low from other tests)
    assert!(stats.pending == 0, "Tasks with group should not be in main queue, they should be in group ZSet");

    // Verify tasks exist by checking we can get task info
    // (This indirectly verifies the tasks were created and stored)
    // Note: In a full integration, the scheduler would aggregate and move tasks to main queue

    // Clean up
    let _ = client.flush_queue(&queue_name).await;
}

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_aggregator_config() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let queue_name = format!("test-agg-config-{}", uuid::Uuid::new_v4());

    let processed = Arc::new(AtomicUsize::new(0));
    let handler = BatchHandler { processed: processed.clone() };

    // Create server with aggregator config
    let aggregator_config = AggregatorConfig::new()
        .max_size(10)
        .grace_period(Duration::from_secs(5))
        .max_delay(Duration::from_secs(30));

    let state = ServerBuilder::new()
        .redis_url(&redis_url)
        .queues(&[&queue_name])
        .aggregator_config(aggregator_config)
        .build()
        .await
        .expect("Failed to build server");

    let server = rediq::server::Server::from(state);
    let handler_clone = handler.clone();

    let server_handle = tokio::spawn(async move {
        let mut mux = Mux::new();
        mux.handle("notification:send", handler_clone);
        server.run(mux).await
    });

    // Enqueue grouped tasks
    let client = Client::builder()
        .redis_url(&redis_url)
        .build()
        .await
        .expect("Failed to create client");

    for i in 0..15 {
        let task = TaskBuilder::new("notification:send")
            .queue(&queue_name)
            .group("batch_group")
            .payload(&serde_json::json!({"index": i}))
            .expect("Failed to create payload")
            .build()
            .expect("Failed to build task");

        client.enqueue(task).await.expect("Failed to enqueue task");
    }

    // Wait for processing
    tokio::time::sleep(Duration::from_secs(5)).await;

    server_handle.abort();

    // Clean up
    let _ = client.flush_queue(&queue_name).await;
}

#[test]
fn test_aggregator_func_creation() {
    // Test that GroupAggregatorFunc can be created
    let _aggregator: GroupAggregatorFunc = GroupAggregatorFunc::new(|_group: &str, _tasks: Vec<Task>| -> rediq::Result<Option<Task>> {
        Ok(None)
    });

    // Test with config
    let config = AggregatorConfig::new()
        .max_size(20)
        .grace_period(Duration::from_secs(10));

    let _aggregator: GroupAggregatorFunc = GroupAggregatorFunc::with_config(
        |_group: &str, _tasks: Vec<Task>| -> rediq::Result<Option<Task>> {
            Ok(None)
        },
        config,
    );
}

#[test]
fn test_aggregator_config_defaults() {
    let config = AggregatorConfig::default();

    // Verify default values are reasonable
    assert!(config.max_size > 0, "max_size should be positive");
    assert!(config.grace_period > Duration::ZERO, "grace_period should be positive");
    assert!(config.max_delay > Duration::ZERO, "max_delay should be positive");
}

#[test]
fn test_aggregator_config_builder() {
    let config = AggregatorConfig::new()
        .max_size(50)
        .grace_period(Duration::from_secs(30))
        .max_delay(Duration::from_secs(120));

    assert_eq!(config.max_size, 50);
    assert_eq!(config.grace_period, Duration::from_secs(30));
    assert_eq!(config.max_delay, Duration::from_secs(120));
}
