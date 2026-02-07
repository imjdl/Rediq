//! Metrics and monitoring tests
//!
//! Tests Prometheus metrics collection:
//! - Task processing metrics
//! - Queue statistics
//! - Custom metrics registration

use rediq::{client::Client, processor::{Handler, Mux}, server::ServerBuilder, task::TaskBuilder};
use async_trait::async_trait;

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_queue_metrics() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let queue_name = format!("test-metrics-{}", uuid::Uuid::new_v4());

    let client = Client::builder().redis_url(&redis_url).build().await.unwrap();

    // Enqueue some tasks
    for i in 0..3 {
        let task = TaskBuilder::new("test:metrics")
            .queue(&queue_name)
            .payload(&serde_json::json!({"index": i})).unwrap()
            .build()
            .unwrap();
        client.enqueue(task).await.unwrap();
    }

    // Check queue stats
    let stats = client.inspector().queue_stats(&queue_name).await.unwrap();
    assert_eq!(stats.pending, 3);
    assert_eq!(stats.active, 0);
    assert_eq!(stats.completed, 0);

    // Clean up
    let _ = client.flush_queue(&queue_name).await;
}

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_processing_increments_metrics() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let queue_name = format!("test-process-metrics-{}", uuid::Uuid::new_v4());

    #[derive(Clone)]
    struct TestHandler;

    #[async_trait]
    impl Handler for TestHandler {
        async fn handle(&self, _task: &rediq::Task) -> rediq::Result<()> {
            Ok(())
        }
    }

    let handler = TestHandler;

    // Start server
    let state = ServerBuilder::new()
        .redis_url(&redis_url)
        .queues(&[&queue_name])
        .build()
        .await
        .unwrap();

    let server = rediq::server::Server::from(state);
    let server_handle = tokio::spawn(async move {
        let mut mux = Mux::new();
        mux.handle("test:metrics", handler);
        let _ = server.run(mux).await;
    });

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Enqueue tasks
    let client = Client::builder().redis_url(&redis_url).build().await.unwrap();
    for _ in 0..5 {
        let task = TaskBuilder::new("test:metrics")
            .queue(&queue_name)
            .payload(&serde_json::json!({})).unwrap()
            .build()
            .unwrap();
        client.enqueue(task).await.unwrap();
    }

    // Wait for processing with timeout
    let start = std::time::Instant::now();
    loop {
        let stats = client.inspector().queue_stats(&queue_name).await.unwrap();
        if stats.completed >= 1 {
            break;
        }
        if start.elapsed() > std::time::Duration::from_secs(10) {
            panic!("No tasks were completed within 10 seconds");
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    // Check that completed count increased
    let stats = client.inspector().queue_stats(&queue_name).await.unwrap();
    assert!(stats.completed >= 1, "At least one task should be completed");

    server_handle.abort();

    // Clean up
    let _ = client.flush_queue(&queue_name).await;
}
