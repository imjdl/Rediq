//! Server configuration and builder
//!
//! Provides configuration structures for the Rediq server.

use crate::{Error, Result};
use crate::storage::{RedisClient, RedisMode};
use crate::middleware::MiddlewareChain;
use std::sync::Arc;
use uuid::Uuid;

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Redis connection URL
    pub redis_url: String,

    /// Redis connection mode
    pub redis_mode: RedisMode,

    /// Queues to consume from
    pub queues: Vec<String>,

    /// Number of concurrent workers
    pub concurrency: usize,

    /// Heartbeat interval (seconds)
    pub heartbeat_interval: u64,

    /// Worker timeout (seconds) - worker considered dead if no heartbeat
    pub worker_timeout: u64,

    /// Dequeue timeout (seconds) - BLPOP timeout
    pub dequeue_timeout: u64,

    /// Queue poll interval (milliseconds) - wait time when no tasks
    pub poll_interval: u64,

    /// Server name for identification
    pub server_name: String,

    /// Enable scheduler for delayed/retry tasks
    pub enable_scheduler: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://localhost:6379".to_string(),
            redis_mode: RedisMode::Standalone,
            queues: vec!["default".to_string()],
            concurrency: 10,
            heartbeat_interval: 5,
            worker_timeout: 30,
            dequeue_timeout: 2,
            poll_interval: 100,
            server_name: format!("rediq-server-{}", Uuid::new_v4()),
            enable_scheduler: true,
        }
    }
}

/// Server builder
///
/// Provides a fluent interface for configuring and building a Server.
///
/// # Example
///
/// ```rust
/// use rediq::server::ServerBuilder;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let server = ServerBuilder::new()
///     .redis_url("redis://localhost:6379")
///     .queues(&["default", "critical", "low"])
///     .concurrency(20)
///     .heartbeat_interval(10)
///     .build()
///     .await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Default)]
pub struct ServerBuilder {
    config: ServerConfig,
    middleware: MiddlewareChain,
}

impl ServerBuilder {
    /// Create a new server builder with default configuration
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: ServerConfig::default(),
            middleware: MiddlewareChain::new(),
        }
    }

    /// Add middleware to the server
    ///
    /// Middleware will be executed in the order they are added.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rediq::server::ServerBuilder;
    /// use rediq::middleware::LoggingMiddleware;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let server = ServerBuilder::new()
    ///     .middleware(LoggingMiddleware::new())
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn middleware<M: crate::middleware::Middleware + 'static>(mut self, middleware: M) -> Self {
        self.middleware = self.middleware.add(middleware);
        self
    }

    /// Set Redis connection URL
    #[must_use]
    pub fn redis_url(mut self, url: impl Into<String>) -> Self {
        self.config.redis_url = url.into();
        self
    }

    /// Set Redis connection mode to Cluster
    ///
    /// # Example
    ///
    /// ```rust
    /// # use rediq::server::ServerBuilder;
    /// let builder = ServerBuilder::new()
    ///     .redis_url("redis://cluster-node1:6379")
    ///     .cluster_mode()
    ///     .build();
    /// ```
    #[must_use]
    pub fn cluster_mode(mut self) -> Self {
        self.config.redis_mode = RedisMode::Cluster;
        self
    }

    /// Set Redis connection mode to Sentinel
    ///
    /// # Example
    ///
    /// ```rust
    /// # use rediq::server::ServerBuilder;
    /// let builder = ServerBuilder::new()
    ///     .redis_url("redis://sentinel-1:26379")
    ///     .sentinel_mode()
    ///     .build();
    /// ```
    #[must_use]
    pub fn sentinel_mode(mut self) -> Self {
        self.config.redis_mode = RedisMode::Sentinel;
        self
    }

    /// Set the queues to consume from
    ///
    /// # Arguments
    /// * `queues` - Slice of queue names to consume from
    ///
    /// # Example
    ///
    /// ```rust
    /// # use rediq::server::ServerBuilder;
    /// let builder = ServerBuilder::new()
    ///     .queues(&["default", "critical", "low"]);
    /// ```
    #[must_use]
    pub fn queues(mut self, queues: &[&str]) -> Self {
        self.config.queues = queues.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Set the number of concurrent workers
    ///
    /// Each worker will process tasks from the configured queues.
    #[must_use]
    pub fn concurrency(mut self, concurrency: usize) -> Self {
        self.config.concurrency = concurrency;
        self
    }

    /// Set the heartbeat interval (in seconds)
    ///
    /// Workers will send heartbeat to Redis at this interval.
    #[must_use]
    pub fn heartbeat_interval(mut self, seconds: u64) -> Self {
        self.config.heartbeat_interval = seconds;
        self
    }

    /// Set the worker timeout (in seconds)
    ///
    /// A worker is considered dead if no heartbeat is received within this duration.
    #[must_use]
    pub fn worker_timeout(mut self, seconds: u64) -> Self {
        self.config.worker_timeout = seconds;
        self
    }

    /// Set the dequeue timeout (in seconds)
    ///
    /// This is the timeout for BLPOP when waiting for tasks.
    #[must_use]
    pub fn dequeue_timeout(mut self, seconds: u64) -> Self {
        self.config.dequeue_timeout = seconds;
        self
    }

    /// Set the queue poll interval (in milliseconds)
    ///
    /// When a queue is empty, workers will wait this long before polling again.
    #[must_use]
    pub fn poll_interval(mut self, milliseconds: u64) -> Self {
        self.config.poll_interval = milliseconds;
        self
    }

    /// Set the server name for identification
    #[must_use]
    pub fn server_name(mut self, name: impl Into<String>) -> Self {
        self.config.server_name = name.into();
        self
    }

    /// Disable the built-in scheduler
    ///
    /// The scheduler handles delayed and retry tasks automatically.
    #[must_use]
    pub fn disable_scheduler(mut self) -> Self {
        self.config.enable_scheduler = false;
        self
    }

    /// Build the server
    ///
    /// This method connects to Redis and initializes the server.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Redis connection fails
    /// - Invalid configuration is provided
    pub async fn build(self) -> Result<ServerState> {
        // Validate configuration
        if self.config.concurrency == 0 {
            return Err(Error::Config("concurrency must be greater than 0".into()));
        }

        if self.config.queues.is_empty() {
            return Err(Error::Config("at least one queue must be specified".into()));
        }

        if self.config.heartbeat_interval == 0 {
            return Err(Error::Config("heartbeat_interval must be greater than 0".into()));
        }

        // Connect to Redis
        let redis = match self.config.redis_mode {
            RedisMode::Standalone => RedisClient::from_url(&self.config.redis_url).await?,
            RedisMode::Cluster => RedisClient::from_cluster_url(&self.config.redis_url).await?,
            RedisMode::Sentinel => RedisClient::from_sentinel_url(&self.config.redis_url).await?,
        };

        // Ping to verify connection
        redis.ping().await?;

        let mode_str = match self.config.redis_mode {
            RedisMode::Standalone => "Standalone",
            RedisMode::Cluster => "Cluster",
            RedisMode::Sentinel => "Sentinel",
        };
        tracing::info!("Connected to Redis ({}) at {}", mode_str, self.config.redis_url);

        Ok(ServerState {
            config: Arc::new(self.config),
            redis,
            middleware: Arc::new(self.middleware),
        })
    }
}

/// Server state shared across workers
///
/// This struct contains the runtime state that is shared across
/// all workers in the server.
#[derive(Clone)]
pub struct ServerState {
    /// Server configuration
    pub config: Arc<ServerConfig>,

    /// Redis client
    pub redis: RedisClient,

    /// Middleware chain
    pub middleware: Arc<MiddlewareChain>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ServerConfig::default();
        assert_eq!(config.redis_url, "redis://localhost:6379");
        assert_eq!(config.queues, vec!["default"]);
        assert_eq!(config.concurrency, 10);
        assert_eq!(config.heartbeat_interval, 5);
        assert_eq!(config.worker_timeout, 30);
        assert_eq!(config.dequeue_timeout, 2);
        assert_eq!(config.poll_interval, 100);
        assert!(config.enable_scheduler);
    }

    #[test]
    fn test_builder() {
        let builder = ServerBuilder::new()
            .redis_url("redis://localhost:6380")
            .queues(&["critical", "low"])
            .concurrency(20)
            .heartbeat_interval(10)
            .dequeue_timeout(5)
            .poll_interval(200)
            .server_name("test-server");

        assert_eq!(builder.config.redis_url, "redis://localhost:6380");
        assert_eq!(builder.config.queues, vec!["critical", "low"]);
        assert_eq!(builder.config.concurrency, 20);
        assert_eq!(builder.config.heartbeat_interval, 10);
        assert_eq!(builder.config.dequeue_timeout, 5);
        assert_eq!(builder.config.poll_interval, 200);
        assert_eq!(builder.config.server_name, "test-server");
    }

    #[test]
    fn test_builder_disable_scheduler() {
        let builder = ServerBuilder::new().disable_scheduler();
        assert!(!builder.config.enable_scheduler);
    }

    #[tokio::test]
    #[ignore = "Requires Redis server"]
    async fn test_build_server() {
        let redis_url = std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://localhost:6379".to_string());
        let state = ServerBuilder::new()
            .redis_url(&redis_url)
            .queues(&["default"])
            .concurrency(5)
            .build()
            .await
            .unwrap();

        assert_eq!(state.config.queues.len(), 1);
        assert_eq!(state.config.concurrency, 5);
    }
}
