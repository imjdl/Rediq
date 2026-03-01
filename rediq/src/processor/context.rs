//! Handler context module
//!
//! Provides context information to task handlers, including access to Redis,
//! progress reporting, and cancellation state.

use crate::storage::RedisClient;
use crate::progress::ProgressContext;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Context provided to task handlers during execution
///
/// HandlerContext provides handlers with access to:
/// - The task ID being processed
/// - Redis client for additional operations
/// - Progress reporting context
/// - Cancellation state
///
/// # Example
///
/// ```rust
/// use rediq::processor::{Handler, HandlerContext};
/// use rediq::Task;
/// use async_trait::async_trait;
///
/// struct MyHandler;
///
/// #[async_trait]
/// impl Handler for MyHandler {
///     async fn handle(&self, task: &Task, ctx: &HandlerContext) -> rediq::Result<()> {
///         // Report progress
///         ctx.report_progress(50).await?;
///
///         // Check if cancelled
///         if ctx.is_cancelled() {
///             return Err(rediq::Error::Cancelled("Task was cancelled".into()));
///         }
///
///         Ok(())
///     }
/// }
/// ```
#[derive(Clone)]
pub struct HandlerContext {
    /// Task ID being processed
    task_id: String,

    /// Redis client for additional operations
    redis: RedisClient,

    /// Progress context for reporting progress
    progress: Option<ProgressContext>,

    /// Cancellation flag
    cancelled: Arc<AtomicBool>,
}

impl HandlerContext {
    /// Create a new handler context
    pub fn new(
        task_id: String,
        redis: RedisClient,
        progress: Option<ProgressContext>,
        cancelled: Arc<AtomicBool>,
    ) -> Self {
        Self {
            task_id,
            redis,
            progress,
            cancelled,
        }
    }

    /// Get the task ID being processed
    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    /// Get the Redis client for additional operations
    ///
    /// This allows handlers to perform additional Redis operations
    /// such as querying other data, updating state, etc.
    pub fn redis(&self) -> &RedisClient {
        &self.redis
    }

    /// Get the progress context if available
    ///
    /// Returns None if progress tracking is not enabled for this task
    pub fn progress(&self) -> Option<&ProgressContext> {
        self.progress.as_ref()
    }

    /// Check if the task has been cancelled
    ///
    /// Handlers should periodically check this flag during long-running
    /// operations and abort if the task has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// Report progress for the task
    ///
    /// This is a convenience method that delegates to the progress context.
    /// Returns Ok(()) if progress tracking is not enabled.
    ///
    /// # Arguments
    /// * `percentage` - Progress percentage (0-100)
    ///
    /// # Example
    ///
    /// ```rust
    /// # use rediq::processor::HandlerContext;
    /// # async fn example(ctx: &HandlerContext) -> rediq::Result<()> {
    /// // Report 25% progress
    /// ctx.report_progress(25).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn report_progress(&self, percentage: u8) -> crate::Result<()> {
        if let Some(progress) = &self.progress {
            progress.report(percentage as u32).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires Redis server"]
    async fn test_handler_context_creation() {
        let redis = crate::storage::RedisClient::from_url("redis://localhost:6379")
            .await
            .unwrap();

        let ctx = HandlerContext::new(
            "test-task-id".to_string(),
            redis,
            None,
            Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(ctx.task_id(), "test-task-id");
        assert!(!ctx.is_cancelled());
    }

    #[test]
    fn test_cancellation_flag() {
        let cancelled = Arc::new(AtomicBool::new(false));

        // Test initial state
        assert!(!cancelled.load(Ordering::Relaxed));

        // Cancel the task
        cancelled.store(true, Ordering::Relaxed);

        assert!(cancelled.load(Ordering::Relaxed));
    }
}
