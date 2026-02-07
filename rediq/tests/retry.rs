//! Retry mechanism tests
//!
//! Tests the retry behavior when tasks fail:
//! - Exponential backoff strategy
//! - Retry count limits
//! - Dead letter queue transfer

use rediq::{client::Client, processor::{Handler, Mux}, server::ServerBuilder, task::TaskBuilder};
use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_task_retry_on_failure() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let queue_name = format!("test-retry-{}", uuid::Uuid::new_v4());

    // Handler that fails first time, succeeds second time
    #[derive(Clone)]
    struct FailOnceHandler {
        fail_count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Handler for FailOnceHandler {
        async fn handle(&self, _task: &rediq::Task) -> rediq::Result<()> {
            let count = self.fail_count.fetch_add(1, Ordering::Relaxed);
            if count == 0 {
                Err(rediq::Error::Handler("First attempt fails".to_string()))
            } else {
                Ok(())
            }
        }
    }

    let handler = Arc::new(FailOnceHandler {
        fail_count: Arc::new(AtomicUsize::new(0)),
    });

    // Clone the handler value (not the Arc) for the server
    let handler_for_server = (*handler).clone();

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
        mux.handle("test:retry", handler_for_server);
        let _ = server.run(mux).await;
    });

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Enqueue task with retry enabled
    let client = Client::builder().redis_url(&redis_url).build().await.unwrap();
    let task = TaskBuilder::new("test:retry")
        .queue(&queue_name)
        .max_retry(3)
        .payload(&serde_json::json!({})).unwrap()
        .build()
        .unwrap();
    client.enqueue(task).await.unwrap();

    // Wait for task to be processed (after retry)
    // Note: Allow sufficient time for retry delay and processing
    let start = std::time::Instant::now();
    loop {
        let count = handler.fail_count.load(Ordering::Relaxed);
        if count >= 2 {
            break;
        }
        if start.elapsed() > std::time::Duration::from_secs(60) {
            panic!("Task was not retried within 60 seconds, fail_count: {}", count);
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    assert!(handler.fail_count.load(Ordering::Relaxed) >= 2, "Task should be processed after retry");

    server_handle.abort();

    // Clean up
    let _ = client.flush_queue(&queue_name).await;
}
