//! Common test utilities
//!
//! Shared helper functions and fixtures for integration tests.

use rediq::{client::Client, server::{Server, ServerBuilder}, processor::Mux};
use std::time::Duration;
use tokio::task::JoinHandle;

/// Test setup structure
///
/// Manages Redis connection and cleanup for integration tests.
pub struct TestSetup {
    pub redis_url: String,
    pub queue_name: String,
    client: Option<Client>,
}

impl TestSetup {
    /// Create a new test setup
    ///
    /// # Arguments
    /// * `test_name` - Name of the test (used for queue naming)
    pub async fn new(test_name: &str) -> Self {
        let redis_url = std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://localhost:6379".to_string());

        let queue_name = format!("test-{}-{}", test_name, uuid::Uuid::new_v4());

        let client = Client::builder()
            .redis_url(&redis_url)
            .build()
            .await
            .expect("Failed to create client");

        Self {
            redis_url,
            queue_name,
            client: Some(client),
        }
    }

    /// Get the client
    pub async fn client(&self) -> Client {
        Client::builder()
            .redis_url(&self.redis_url)
            .build()
            .await
            .expect("Failed to create client")
    }

    /// Consume and return the client
    pub fn into_client(mut self) -> Client {
        self.client.take()
            .expect("Client already consumed")
    }

    /// Clean up test data
    ///
    /// Removes the test queue from Redis.
    pub async fn cleanup(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(client) = &self.client {
            // Flush the test queue
            let _ = client.flush_queue(&self.queue_name).await;

            // Clean up metadata
            let inspector = client.inspector();
            let _ = inspector.list_queues().await;
        }
        Ok(())
    }
}

impl Drop for TestSetup {
    fn drop(&mut self) {
        // Note: We can't do async cleanup in Drop
        // Users should call cleanup() explicitly
    }
}

/// Server handle for managing a test server
pub struct TestServer {
    handle: Option<JoinHandle<()>>,
}

impl TestServer {
    /// Create a new test server
    ///
    /// # Arguments
    /// * `redis_url` - Redis connection URL
    /// * `queues` - List of queues to process
    /// * `mux` - Task processor router
    pub async fn new(redis_url: &str, queues: Vec<String>, mux: Mux) -> Result<Self, Box<dyn std::error::Error>> {
        // Convert Vec<String> to Vec<&str>
        let queue_refs: Vec<&str> = queues.iter().map(|s| s.as_str()).collect();

        let state = ServerBuilder::new()
            .redis_url(redis_url)
            .queues(&queue_refs)
            .build()
            .await?;

        let server = Server::from(state);

        // Run server in background
        let handle = tokio::spawn(async move {
            let _ = server.run(mux).await;
        });

        // Wait a bit for server to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok(Self {
            handle: Some(handle),
        })
    }

    /// Stop the test server
    pub async fn stop(mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
            let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

/// Create a test task
///
/// # Arguments
/// * `task_type` - Type of the task
/// * `queue` - Queue name
///
/// # Returns
/// A new Task instance
pub fn create_test_task(task_type: &str, queue: &str) -> rediq::Task {
    rediq::task::TaskBuilder::new(task_type)
        .queue(queue)
        .payload(&serde_json::json!({"test": "data"}))
        .expect("Failed to create payload")
        .build()
        .expect("Failed to build task")
}

/// Create a test task with custom payload
///
/// # Arguments
/// * `task_type` - Type of the task
/// * `queue` - Queue name
/// * `payload` - JSON payload
///
/// # Returns
/// A new Task instance
pub fn create_test_task_with_payload(
    task_type: &str,
    queue: &str,
    payload: &serde_json::Value,
) -> rediq::Task {
    rediq::task::TaskBuilder::new(task_type)
        .queue(queue)
        .payload(payload)
        .expect("Failed to serialize payload")
        .build()
        .expect("Failed to build task")
}

/// Wait for a condition to be true
///
/// # Arguments
/// * `condition` - Function that returns true when condition is met
/// * `timeout` - Maximum time to wait
///
/// # Returns
/// Ok(()) if condition was met, Err if timeout occurred
pub async fn wait_for<F>(
    mut condition: F,
    timeout: Duration,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnMut() -> bool,
{
    let start = std::time::Instant::now();
    let check_interval = Duration::from_millis(50);

    while start.elapsed() < timeout {
        if condition() {
            return Ok(());
        }
        tokio::time::sleep(check_interval).await;
    }

    Err(format!("Condition not met after {:?}", timeout).into())
}

/// Assert queue stats match expected values
///
/// # Arguments
/// * `client` - Rediq client
/// * `queue` - Queue name
/// * `expected_pending` - Expected pending count
/// * `expected_active` - Expected active count
/// * `expected_completed` - Expected completed count
pub async fn assert_queue_stats(
    client: &Client,
    queue: &str,
    expected_pending: Option<u64>,
    expected_active: Option<u64>,
    expected_completed: Option<u64>,
) -> Result<(), Box<dyn std::error::Error>> {
    let stats = client.inspector().queue_stats(queue).await?;

    if let Some(pending) = expected_pending {
        assert_eq!(stats.pending, pending, "Pending count mismatch");
    }
    if let Some(active) = expected_active {
        assert_eq!(stats.active, active, "Active count mismatch");
    }
    if let Some(completed) = expected_completed {
        assert_eq!(stats.completed, completed, "Completed count mismatch");
    }

    Ok(())
}

/// Mock handlers for testing
pub mod mock_handlers {
    use async_trait::async_trait;
    use rediq::processor::Handler;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::sync::Mutex;

    /// Handler that always succeeds
    #[derive(Clone)]
    pub struct SuccessHandler;

    #[async_trait]
    impl Handler for SuccessHandler {
        async fn handle(&self, _task: &rediq::Task) -> rediq::Result<()> {
            Ok(())
        }
    }

    /// Handler that tracks processing order
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

    /// Handler that counts invocations
    pub struct CountingHandler {
        pub count: Arc<AtomicUsize>,
    }

    impl Clone for CountingHandler {
        fn clone(&self) -> Self {
            Self {
                count: Arc::clone(&self.count),
            }
        }
    }

    #[async_trait]
    impl Handler for CountingHandler {
        async fn handle(&self, _task: &rediq::Task) -> rediq::Result<()> {
            self.count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }
}
