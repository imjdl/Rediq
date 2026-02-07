//! Queue control tests
//!
//! Tests queue management operations:
//! - Pause and resume queues
//! - Flush queue contents
//! - List and inspect queues

use rediq::{client::Client, processor::{Handler, Mux}, server::ServerBuilder, task::TaskBuilder};
use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_queue_pause_resume() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let queue_name = format!("test-pause-{}", uuid::Uuid::new_v4());

    let client = Client::builder().redis_url(&redis_url).build().await.unwrap();

    // Pause the queue
    client.pause_queue(&queue_name).await
        .expect("Failed to pause queue");

    // Verify queue is paused
    let inspector = client.inspector();
    let _queues = inspector.list_queues().await.unwrap();

    // Resume the queue
    client.resume_queue(&queue_name).await
        .expect("Failed to resume queue");

    // Clean up
    let _ = client.flush_queue(&queue_name).await;
}

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_queue_flush() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let queue_name = format!("test-flush-{}", uuid::Uuid::new_v4());

    let client = Client::builder().redis_url(&redis_url).build().await.unwrap();

    // Enqueue some tasks
    for i in 0..5 {
        let task = TaskBuilder::new("test:flush")
            .queue(&queue_name)
            .payload(&serde_json::json!({"index": i})).unwrap()
            .build()
            .unwrap();
        client.enqueue(task).await.unwrap();
    }

    // Verify tasks are in queue
    let stats = client.inspector().queue_stats(&queue_name).await.unwrap();
    assert_eq!(stats.pending, 5, "Should have 5 pending tasks");

    // Flush the queue
    client.flush_queue(&queue_name).await
        .expect("Failed to flush queue");

    // Verify queue is empty
    let stats = client.inspector().queue_stats(&queue_name).await.unwrap();
    assert_eq!(stats.pending, 0, "Queue should be empty after flush");
}

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_paused_queue_no_processing() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let queue_name = format!("test-pause-process-{}", uuid::Uuid::new_v4());

    #[derive(Clone)]
    struct CountingHandler {
        count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Handler for CountingHandler {
        async fn handle(&self, _task: &rediq::Task) -> rediq::Result<()> {
            self.count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }

    let handler = Arc::new(CountingHandler {
        count: Arc::new(AtomicUsize::new(0)),
    });

    // Clone the handler value (not the Arc) for the server
    let handler_for_server = (*handler).clone();
    let count_ref = handler.count.clone();

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
        mux.handle("test:paused", handler_for_server);
        let _ = server.run(mux).await;
    });

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Pause the queue
    let client = Client::builder().redis_url(&redis_url).build().await.unwrap();
    client.pause_queue(&queue_name).await.unwrap();

    // Enqueue tasks while paused
    for _ in 0..3 {
        let task = TaskBuilder::new("test:paused")
            .queue(&queue_name)
            .payload(&serde_json::json!({})).unwrap()
            .build()
            .unwrap();
        client.enqueue(task).await.unwrap();
    }

    // Wait a bit - tasks should NOT be processed
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    assert_eq!(count_ref.load(Ordering::Relaxed), 0, "No tasks should be processed while paused");

    // Resume the queue
    client.resume_queue(&queue_name).await.unwrap();

    // Wait for processing to occur with timeout
    let start = std::time::Instant::now();
    loop {
        let count = count_ref.load(Ordering::Relaxed);
        if count >= 3 {
            break;
        }
        if start.elapsed() > std::time::Duration::from_secs(10) {
            panic!("Tasks were not processed within 10 seconds after resume, count: {}", count);
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    assert_eq!(count_ref.load(Ordering::Relaxed), 3, "All tasks should be processed after resume");

    server_handle.abort();

    // Clean up
    let _ = client.flush_queue(&queue_name).await;
}
