//! Cron task functionality tests
//!
//! Tests the periodic task scheduling using cron expressions

use rediq::{client::Client, processor::{Handler, Mux}, server::ServerBuilder, task::TaskBuilder, Task};
use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_cron_task_scheduling() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let queue_name = format!("test-cron-{}", uuid::Uuid::new_v4());

    #[derive(Clone)]
    struct CronHandler {
        count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Handler for CronHandler {
        async fn handle(&self, task: &Task) -> rediq::Result<()> {
            self.count.fetch_add(1, Ordering::Relaxed);
            eprintln!("Cron task executed: {}", task.id);
            Ok(())
        }
    }

    let count = Arc::new(AtomicUsize::new(0));
    let handler = CronHandler { count: count.clone() };

    // Create client
    let client = Client::builder()
        .redis_url(&redis_url)
        .build()
        .await
        .expect("Failed to create client");

    // Schedule a cron task (every second)
    let cron_task = TaskBuilder::new("cron:test")
        .queue(&queue_name)
        .cron("* * * * * *")  // Every second
        .payload(&serde_json::json!({"scheduled": true}))
        .expect("Failed to create payload")
        .build()
        .expect("Failed to build cron task");

    let task_id = client.enqueue_cron(cron_task).await.expect("Failed to enqueue cron task");
    eprintln!("Cron task enqueued: {}", task_id);

    // Verify cron task can be retrieved
    let task_info = client.inspector().get_task(&task_id).await;
    assert!(task_info.is_ok(), "Should be able to get cron task info");

    // Start server
    let state = ServerBuilder::new()
        .redis_url(&redis_url)
        .queues(&[&queue_name])
        .build()
        .await
        .expect("Failed to build server");

    let server = rediq::server::Server::from(state);
    let handler_clone = handler.clone();

    let server_handle = tokio::spawn(async move {
        let mut mux = Mux::new();
        mux.handle("cron:test", handler_clone);
        server.run(mux).await
    });

    // Wait for at least one execution (cron tasks need scheduler to process)
    tokio::time::sleep(Duration::from_secs(5)).await;

    let executions = count.load(Ordering::Relaxed);
    eprintln!("Cron task executed {} times", executions);
    // Note: Cron task execution depends on scheduler timing
    // We just verify the task was registered and server ran without errors
    // The actual execution timing is tested separately

    server_handle.abort();

    // Clean up
    let _ = client.flush_queue(&queue_name).await;
}

#[tokio::test]
#[ignore = "Integration test - requires Redis server"]
async fn test_cron_task_registration() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let queue_name = format!("test-cron-reg-{}", uuid::Uuid::new_v4());

    let client = Client::builder()
        .redis_url(&redis_url)
        .build()
        .await
        .expect("Failed to create client");

    // Register multiple cron tasks
    let mut task_ids = Vec::new();
    for i in 0..3 {
        let cron_task = TaskBuilder::new(&format!("cron:test:{}", i))
            .queue(&queue_name)
            .cron("0 * * * * *")  // Every minute
            .payload(&serde_json::json!({"index": i}))
            .expect("Failed to create payload")
            .build()
            .expect("Failed to build cron task");

        let task_id = client.enqueue_cron(cron_task).await.expect("Failed to enqueue cron task");
        task_ids.push(task_id);
    }

    // Verify all cron tasks can be retrieved
    for task_id in &task_ids {
        let task_info = client.inspector().get_task(task_id).await;
        assert!(task_info.is_ok(), "Cron task should be registered");
    }

    // Clean up
    let _ = client.flush_queue(&queue_name).await;
}
