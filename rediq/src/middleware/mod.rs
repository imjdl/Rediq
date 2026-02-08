//! Middleware module
//!
//! Provides middleware chain and built-in middleware

use crate::{Result};
use crate::task::Task;
use async_trait::async_trait;
use std::sync::Arc;

/// Middleware trait
#[async_trait]
pub trait Middleware: Send + Sync {
    /// Called before task processing
    async fn before(&self, _task: &Task) -> Result<()> {
        Ok(())
    }

    /// Called after task processing
    async fn after(&self, _task: &Task, _result: &Result<()>) -> Result<()> {
        Ok(())
    }
}

/// Type-erased middleware wrapper
type MiddlewareArc = Arc<dyn Middleware>;

/// Middleware chain
///
/// Executes middleware in the order they were added.
#[derive(Default)]
pub struct MiddlewareChain {
    middlewares: Vec<MiddlewareArc>,
}

impl std::fmt::Debug for MiddlewareChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MiddlewareChain")
            .field("count", &self.middlewares.len())
            .finish()
    }
}

impl MiddlewareChain {
    /// Create a new middleware chain
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
        }
    }

    /// Add middleware to the chain
    #[allow(clippy::should_implement_trait)]
    pub fn add<M: Middleware + 'static>(mut self, middleware: M) -> Self {
        self.middlewares.push(Arc::new(middleware));
        self
    }

    /// Execute before hooks
    pub async fn before(&self, task: &Task) -> Result<()> {
        for middleware in &self.middlewares {
            middleware.before(task).await?;
        }
        Ok(())
    }

    /// Execute after hooks
    pub async fn after(&self, task: &Task, result: &Result<()>) -> Result<()> {
        for middleware in &self.middlewares {
            middleware.after(task, result).await?;
        }
        Ok(())
    }

    /// Check if chain is empty
    pub fn is_empty(&self) -> bool {
        self.middlewares.is_empty()
    }
}

/// Logging middleware - logs task processing
#[derive(Debug, Clone)]
pub struct LoggingMiddleware {
    log_details: bool,
}

impl LoggingMiddleware {
    /// Create a new logging middleware
    pub fn new() -> Self {
        Self {
            log_details: false,
        }
    }

    /// Enable detailed logging
    pub fn with_details(mut self) -> Self {
        self.log_details = true;
        self
    }
}

impl Default for LoggingMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Middleware for LoggingMiddleware {
    async fn before(&self, task: &Task) -> Result<()> {
        if self.log_details {
            tracing::info!(
                "Processing task: type={}, id={}, queue={}, retry={}",
                task.task_type,
                task.id,
                task.queue,
                task.retry_cnt
            );
        } else {
            tracing::info!("Processing task: {}", task.id);
        }
        Ok(())
    }

    async fn after(&self, task: &Task, result: &Result<()>) -> Result<()> {
        match result {
            Ok(()) => {
                tracing::info!("Task completed: {}", task.id);
            }
            Err(e) => {
                tracing::error!("Task failed: {} - error: {}", task.id, e);
            }
        }
        Ok(())
    }
}

/// Metrics middleware - collects task processing metrics
#[derive(Debug, Clone, Default)]
pub struct MetricsMiddleware {
    _private: (),
}

impl MetricsMiddleware {
    /// Create a new metrics middleware
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl Middleware for MetricsMiddleware {
    async fn before(&self, task: &Task) -> Result<()> {
        // Record task start time
        tracing::debug!("Task started: {}", task.id);
        Ok(())
    }

    async fn after(&self, task: &Task, result: &Result<()>) -> Result<()> {
        match result {
            Ok(()) => {
                tracing::debug!("Task succeeded: {}", task.id);
            }
            Err(_) => {
                tracing::debug!("Task failed: {}", task.id);
            }
        }
        Ok(())
    }
}
