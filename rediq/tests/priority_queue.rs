//! Priority queue functionality tests
//!
//! Tests priority-based task scheduling:
//! - Higher priority tasks processed first
//! - Priority range validation (0-100)
//! - Mixed normal and priority queues

use rediq::{client::Client, processor::{Handler, Mux}, server::ServerBuilder, task::TaskBuilder};
use async_trait::async_trait;
use std::sync::Arc;
use std::sync::Mutex;

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_priority_ordering() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let queue_name = format!("test-priority-{}", uuid::Uuid::new_v4());

    // Handler that tracks processing order
    pub struct OrderedHandler {
        pub processed: Arc<Mutex<Vec<String>>>,
    }

    impl Clone for OrderedHandler {
        fn clone(&self) -> Self {
            Self {
                processed: Arc::clone(&self.processed),
            }
        }
    }

    #[async_trait]
    impl Handler for OrderedHandler {
        async fn handle(&self, task: &rediq::Task) -> rediq::Result<()> {
            let mut processed = self.processed.lock().unwrap();
            processed.push(task.id.clone());
            Ok(())
        }
    }

    let handler: Arc<OrderedHandler> = Arc::new(OrderedHandler {
        processed: Arc::new(Mutex::new(Vec::new())),
    });

    // Clone the handler value (not the Arc) for the server
    let handler_for_server = (*handler).clone();
    let processed_ref = handler.processed.clone();

    // Start server (scheduler is enabled by default)
    let state = ServerBuilder::new()
        .redis_url(&redis_url)
        .queues(&[&queue_name])
        .build()
        .await
        .unwrap();

    let server = rediq::server::Server::from(state);
    let server_handle = tokio::spawn(async move {
        let mut mux = Mux::new();
        mux.handle("test:priority", handler_for_server);
        let _ = server.run(mux).await;
    });

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Enqueue tasks with different priorities
    let client = Client::builder().redis_url(&redis_url).build().await.unwrap();

    // Lower priority (processed later) - use 80 instead of 50 (50 is default, goes to regular queue)
    let task_low = TaskBuilder::new("test:priority")
        .queue(&queue_name)
        .priority(80)  // Higher number = lower priority
        .payload(&serde_json::json!({"priority": "low"})).unwrap()
        .build()
        .unwrap();
    let id_low = client.enqueue(task_low).await.unwrap();

    // Higher priority (processed first)
    let task_high = TaskBuilder::new("test:priority")
        .queue(&queue_name)
        .priority(10)  // Lower number = higher priority
        .payload(&serde_json::json!({"priority": "high"})).unwrap()
        .build()
        .unwrap();
    let id_high = client.enqueue(task_high).await.unwrap();

    // Wait for processing with timeout
    let start = std::time::Instant::now();
    loop {
        {
            let processed = processed_ref.lock().unwrap().len();
            if processed >= 2 {
                break;
            }
            if start.elapsed() > std::time::Duration::from_secs(30) {
                panic!("Tasks were not processed within 30 seconds, processed count: {}", processed);
            }
        } // Lock released here
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    let processed = processed_ref.lock().unwrap();
    // Higher priority task should be processed first
    if processed.len() >= 2 {
        eprintln!("Processed tasks in order: {:?}", processed);
        eprintln!("Expected: id_high={}, id_low={}", id_high, id_low);
        assert_eq!(processed[0], id_high, "High priority task should be processed first");
        assert_eq!(processed[1], id_low, "Low priority task should be processed second");
    }

    server_handle.abort();

    // Clean up
    let _ = client.flush_queue(&queue_name).await;
}

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_priority_validation() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let queue_name = format!("test-priority-val-{}", uuid::Uuid::new_v4());

    // Note: The current implementation clamps priority values rather than rejecting them.
    // Invalid priorities are clamped to the valid range [0, 100].

    // Test priority clamping (too low -> becomes 0)
    let result = TaskBuilder::new("test:priority")
        .queue(&queue_name)
        .priority(-1)
        .payload(&serde_json::json!({}))
        .unwrap()
        .build();

    assert!(result.is_ok(), "Priority below 0 should be clamped to 0");
    let task = result.unwrap();
    assert_eq!(task.options.priority, 0, "Priority -1 should be clamped to 0");

    // Test priority clamping (too high -> becomes 100)
    let result = TaskBuilder::new("test:priority")
        .queue(&queue_name)
        .priority(101)
        .payload(&serde_json::json!({}))
        .unwrap()
        .build();

    assert!(result.is_ok(), "Priority above 100 should be clamped to 100");
    let task = result.unwrap();
    assert_eq!(task.options.priority, 100, "Priority 101 should be clamped to 100");

    // Clean up
    let client = Client::builder().redis_url(&redis_url).build().await.unwrap();
    let _ = client.flush_queue(&queue_name).await;
}
