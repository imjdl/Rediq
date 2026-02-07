//! Basic queue functionality tests
//!
//! Tests the fundamental queue operations:
//! - Task enqueue/dequeue
//! - Queue registration
//! - Basic task processing

use rediq::{client::Client, processor::{Handler, Mux}, server::ServerBuilder, task::TaskBuilder};
use async_trait::async_trait;

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_basic_enqueue() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let queue_name = format!("test-basic-{}", uuid::Uuid::new_v4());

    let client = Client::builder()
        .redis_url(&redis_url)
        .build()
        .await
        .expect("Failed to create client");

    // Create a simple task
    let task = TaskBuilder::new("test:basic")
        .queue(&queue_name)
        .payload(&serde_json::json!({"message": "hello"}))
        .expect("Failed to create payload")
        .build()
        .expect("Failed to build task");

    let task_id = client.enqueue(task).await
        .expect("Failed to enqueue task");

    assert!(!task_id.is_empty());

    // Verify task was enqueued
    let stats = client.inspector().queue_stats(&queue_name).await
        .expect("Failed to get queue stats");
    assert_eq!(stats.pending, 1);

    // Clean up
    let _ = client.flush_queue(&queue_name).await;
}

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_task_processing() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let queue_name = format!("test-processing-{}", uuid::Uuid::new_v4());

    #[derive(Clone)]
    struct TestHandler {
        processed: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    #[async_trait]
    impl Handler for TestHandler {
        async fn handle(&self, task: &rediq::Task) -> rediq::Result<()> {
            eprintln!("Handler called for task: {}", task.id);
            self.processed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        }
    }

    let handler = std::sync::Arc::new(TestHandler {
        processed: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    });

    // Clone the handler value (not the Arc) for the server
    let handler_for_server = (*handler).clone();

    // Start server
    let state = ServerBuilder::new()
        .redis_url(&redis_url)
        .queues(&[&queue_name])
        .build()
        .await
        .expect("Failed to build server");

    let server = rediq::server::Server::from(state);

    // Spawn server in a separate task
    let server_handle = tokio::spawn(async move {
        eprintln!("Server starting...");
        let mut mux = Mux::new();
        mux.handle("test:process", handler_for_server);
        let result = server.run(mux).await;
        eprintln!("Server ended: {:?}", result);
        result
    });

    // Give server more time to start
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    eprintln!("Server started, enqueuing task...");

    // Enqueue task
    let client = Client::builder()
        .redis_url(&redis_url)
        .build()
        .await
        .expect("Failed to create client");

    let task = TaskBuilder::new("test:process")
        .queue(&queue_name)
        .payload(&serde_json::json!({"data": "test"})).expect("Failed to create payload")
        .build().expect("Failed to build task");

    let task_id = client.enqueue(task).await.expect("Failed to enqueue");
    eprintln!("Task enqueued: {}", task_id);

    // Check queue stats
    let stats = client.inspector().queue_stats(&queue_name).await.unwrap();
    eprintln!("Queue stats: pending={}, active={}, completed={}", stats.pending, stats.active, stats.completed);

    // Wait for processing with timeout check
    let start = std::time::Instant::now();
    loop {
        let count = handler.processed.load(std::sync::atomic::Ordering::Relaxed);
        eprintln!("Processed count: {}", count);

        // Also check queue stats
        let stats = client.inspector().queue_stats(&queue_name).await.unwrap();
        eprintln!("Queue stats: pending={}, active={}, completed={}", stats.pending, stats.active, stats.completed);

        if count >= 1 {
            break;
        }
        if start.elapsed() > std::time::Duration::from_secs(15) {
            panic!("Task was not processed within 15 seconds, processed count: {}", count);
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    assert_eq!(handler.processed.load(std::sync::atomic::Ordering::Relaxed), 1);

    server_handle.abort();

    // Clean up
    let _ = client.flush_queue(&queue_name).await;
}
