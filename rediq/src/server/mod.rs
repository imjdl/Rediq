//! Server module
//!
//! Provides the Rediq server for processing tasks from Redis queues.

pub mod config;
pub mod worker;
pub mod scheduler;

pub use config::{ServerBuilder, ServerConfig, ServerState};
pub use worker::{Worker, WorkerMetadata};
pub use scheduler::Scheduler;

use crate::{Error, Result};
use crate::processor::Mux;
use crate::observability::RediqMetrics;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinSet;

/// Server - manages workers and processes tasks
///
/// The server is responsible for:
/// - Managing multiple workers that process tasks concurrently
/// - Starting the scheduler for delayed/retry tasks
/// - Graceful shutdown handling
///
/// # Example
///
/// ```rust
/// use rediq::server::{Server, ServerBuilder};
/// use rediq::processor::{Handler, Mux};
/// use async_trait::async_trait;
///
/// # struct MyHandler;
/// # #[async_trait]
/// # impl Handler for MyHandler {
/// #     async fn handle(&self, task: &rediq::Task) -> rediq::Result<()> {
/// #         Ok(())
/// #     }
/// # }
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Build server
/// let state = ServerBuilder::new()
///     .redis_url("redis://localhost:6379")
///     .queues(&["default", "critical"])
///     .concurrency(10)
///     .build()
///     .await?;
///
/// let server = Server::from(state);
///
/// // Register handlers
/// let mut mux = Mux::new();
/// mux.handle("email:send", MyHandler);
/// mux.handle("email:welcome", MyHandler);
///
/// // Run server (this will block until Ctrl+C)
/// server.run(mux).await?;
/// # Ok(())
/// # }
/// ```
pub struct Server {
    /// Shared state
    state: Arc<ServerState>,

    /// Shutdown flag
    shutdown: Arc<AtomicBool>,

    /// Active worker count
    worker_count: Arc<AtomicUsize>,

    /// Metrics collector
    metrics: Option<Arc<RediqMetrics>>,

    /// Metrics HTTP bind address
    metrics_bind_address: Option<SocketAddr>,
}

impl Server {
    /// Create a new server from state
    fn new(state: ServerState) -> Self {
        Self {
            state: Arc::new(state),
            shutdown: Arc::new(AtomicBool::new(false)),
            worker_count: Arc::new(AtomicUsize::new(0)),
            metrics: None,
            metrics_bind_address: None,
        }
    }

    /// Enable metrics collection with HTTP endpoint
    ///
    /// This enables Prometheus metrics collection and starts an HTTP server
    /// on the specified address for the `/metrics` endpoint.
    ///
    /// # Arguments
    /// * `bind_address` - Address to bind the metrics server (e.g., "0.0.0.0:9090")
    ///
    /// # Example
    ///
    /// ```rust
    /// use rediq::server::{Server, ServerBuilder};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let state = ServerBuilder::new()
    ///     .redis_url("redis://localhost:6379")
    ///     .queues(&["default"])
    ///     .build()
    ///     .await?;
    ///
    /// let mut server = Server::from(state);
    /// server.enable_metrics_on("0.0.0.0:9090")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn enable_metrics(&mut self, bind_address: impl Into<SocketAddr>) -> Result<()> {
        let metrics = RediqMetrics::new()
            .map_err(|e| Error::Metrics(e.to_string()))?;
        self.metrics = Some(Arc::new(metrics));
        let addr = bind_address.into();
        self.metrics_bind_address = Some(addr);
        tracing::info!("Metrics enabled on http://{}", addr);
        tracing::info!("Metrics endpoint: http://{}/metrics", addr);
        Ok(())
    }

    /// Enable metrics collection with HTTP endpoint (accepts string address)
    ///
    /// This is a convenience method that accepts a string address and parses it internally.
    ///
    /// # Arguments
    /// * `bind_address` - Address string to bind the metrics server (e.g., "0.0.0.0:9090")
    ///
    /// # Example
    ///
    /// ```rust
    /// use rediq::server::{Server, ServerBuilder};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let state = ServerBuilder::new()
    ///     .redis_url("redis://localhost:6379")
    ///     .queues(&["default"])
    ///     .build()
    ///     .await?;
    ///
    /// let mut server = Server::from(state);
    /// server.enable_metrics_on("0.0.0.0:9090")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn enable_metrics_on(&mut self, bind_address: impl Into<String>) -> Result<()> {
        let addr_str = bind_address.into();
        let addr = addr_str.parse::<SocketAddr>()
            .map_err(|e| Error::Config(format!("Invalid metrics address '{}': {}", addr_str, e)))?;
        self.enable_metrics(addr)
    }

    /// Run the server
    ///
    /// This method:
    /// 1. Starts the scheduler (if enabled)
    /// 2. Creates and starts the configured number of workers
    /// 3. Waits for Ctrl+C signal
    /// 4. Initiates graceful shutdown
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Worker initialization fails
    /// - Scheduler fails to start
    pub async fn run(self, mux: Mux) -> Result<()> {
        tracing::info!("Starting Rediq Server: {}", self.state.config.server_name);
        tracing::info!("Queues: {:?}", self.state.config.queues);
        tracing::info!("Concurrency: {}", self.state.config.concurrency);

        let mux = Arc::new(tokio::sync::Mutex::new(mux));
        let mut join_set = JoinSet::new();

        // Start metrics HTTP server if enabled
        #[cfg(feature = "metrics-http")]
        if let (Some(metrics), Some(bind_address)) = (self.metrics.clone(), self.metrics_bind_address) {
            use crate::observability::http_server::MetricsServer;
            let metrics_server = MetricsServer::new(metrics, bind_address);
            let metrics_server_shutdown = self.shutdown.clone();

            join_set.spawn(async move {
                let result = metrics_server.run().await;
                // Signal shutdown if metrics server exits
                metrics_server_shutdown.store(true, Ordering::SeqCst);
                result
            });

            tracing::info!("Metrics HTTP server started on http://{}", bind_address);
        }

        // Start scheduler if enabled
        if self.state.config.enable_scheduler {
            let scheduler = Scheduler::new(
                self.state.redis.clone(),
                self.state.config.queues.clone(),
            );

            let scheduler_shutdown = self.shutdown.clone();
            tokio::spawn(async move {
                let result = scheduler.run().await;
                // Signal shutdown on scheduler exit
                scheduler_shutdown.store(true, Ordering::SeqCst);
                result
            });

            // Track scheduler for graceful shutdown
            // Note: We don't add scheduler to join_set as it manages its own lifecycle
            tracing::info!("Scheduler started");
        }

        // Create and start workers
        for i in 0..self.state.config.concurrency {
            let worker = self.create_worker(i, mux.clone())?;
            let _shutdown = self.shutdown.clone();
            let count = self.worker_count.clone();

            count.fetch_add(1, Ordering::Relaxed);

            join_set.spawn(async move {
                let result = worker.run().await;
                count.fetch_sub(1, Ordering::Relaxed);
                result
            });
        }

        tracing::info!("Started {} workers", self.state.config.concurrency);

        // Wait for shutdown signal
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Received shutdown signal");
                self.shutdown.store(true, Ordering::SeqCst);
            }
        }

        // Graceful shutdown
        self.graceful_shutdown(&mut join_set).await?;

        tracing::info!("Server stopped");
        Ok(())
    }

    /// Create a new worker
    fn create_worker(&self, index: usize, mux: Arc<Mutex<Mux>>) -> Result<Worker> {
        let worker_id = format!(
            "{}-worker-{}",
            self.state.config.server_name,
            index
        );

        Ok(Worker::new(
            worker_id,
            self.state.clone(),
            self.shutdown.clone(),
            mux,
        ))
    }

    /// Graceful shutdown
    ///
    /// Waits for workers to finish their current tasks or timeout.
    async fn graceful_shutdown(&self, join_set: &mut JoinSet<Result<()>>) -> Result<()> {
        tracing::info!("Initiating graceful shutdown");

        let timeout = Duration::from_secs(30);
        let start = std::time::Instant::now();
        let initial_count = self.worker_count.load(Ordering::Relaxed);

        while initial_count > 0 && start.elapsed() < timeout {
            if let Some(result) = join_set.join_next().await {
                if let Err(e) = result {
                    tracing::error!("Worker error during shutdown: {}", e);
                }
            } else {
                break;
            }
        }

        // Force shutdown remaining workers
        let remaining = self.worker_count.load(Ordering::Relaxed);
        if remaining > 0 {
            tracing::warn!("Force shutting down {} workers", remaining);
        }

        while let Some(result) = join_set.join_next().await {
            if let Err(e) = result {
                tracing::error!("Worker error: {}", e);
            }
        }

        Ok(())
    }

    /// Get server statistics
    pub fn stats(&self) -> ServerStats {
        ServerStats {
            server_name: self.state.config.server_name.clone(),
            active_workers: self.worker_count.load(Ordering::Relaxed),
            queues: self.state.config.queues.clone(),
        }
    }
}

/// Server statistics
#[derive(Debug, Clone)]
pub struct ServerStats {
    /// Server name
    pub server_name: String,

    /// Number of active workers
    pub active_workers: usize,

    /// Queues being processed
    pub queues: Vec<String>,
}

impl From<ServerState> for Server {
    fn from(state: ServerState) -> Self {
        Self::new(state)
    }
}

/// Convenience function to create and run a server
///
/// # Example
///
/// ```rust
/// use rediq::server::run_server;
/// use rediq::processor::{Handler, Mux};
/// use async_trait::async_trait;
///
/// # struct MyHandler;
/// # #[async_trait]
/// # impl Handler for MyHandler {
/// #     async fn handle(&self, task: &rediq::Task) -> rediq::Result<()> {
/// #         Ok(())
/// #     }
/// # }
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut mux = Mux::new();
/// mux.handle("test", MyHandler);
///
/// run_server(
///     "redis://localhost:6379",
///     &["default"],
///     mux,
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn run_server(
    redis_url: impl Into<String>,
    queues: &[&str],
    mux: Mux,
) -> Result<()> {
    let state = ServerBuilder::new()
        .redis_url(redis_url)
        .queues(queues)
        .build()
        .await?;

    let server = Server::from(state);
    server.run(mux).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires Redis server"]
    async fn test_server_creation() {
        let redis_url = std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://localhost:6379".to_string());
        let state = ServerBuilder::new()
            .redis_url(&redis_url)
            .queues(&["default"])
            .concurrency(5)
            .build()
            .await
            .unwrap();

        let server = Server::new(state);
        let stats = server.stats();

        assert_eq!(stats.queues, vec!["default"]);
    }
}
