//! Processor module
//!
//! Provides Handler trait and multiplexer for task processing.

use async_trait::async_trait;
use crate::{Error, Result, Task};
use std::collections::HashMap;
use std::sync::Arc;

/// Handler trait - Task processor
///
/// Implement this trait to define how tasks are processed.
///
/// # Example
///
/// ```rust
/// use rediq::processor::Handler;
/// use rediq::Task;
/// use async_trait::async_trait;
///
/// struct EmailHandler;
///
/// #[async_trait]
/// impl Handler for EmailHandler {
///     async fn handle(&self, task: &Task) -> rediq::Result<()> {
///         println!("Processing email task: {}", task.task_type);
///         // Process task here...
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait Handler: Send + Sync {
    /// Handle a task
    ///
    /// This method is called when a task of the registered type is dequeued.
    /// Return `Ok(())` if the task was processed successfully.
    /// Return an error if the task failed and should be retried.
    async fn handle(&self, task: &Task) -> Result<()>;
}

/// Multiplexer - Routes tasks to their registered handlers
///
/// The Mux maintains a mapping from task type strings to Handler implementations.
/// When a task is processed, it looks up the handler by task_type and invokes it.
///
/// # Example
///
/// ```rust
/// use rediq::processor::{Handler, Mux};
/// use rediq::Task;
/// use async_trait::async_trait;
///
/// struct EmailHandler;
/// struct SmsHandler;
///
/// #[async_trait]
/// impl Handler for EmailHandler {
///     async fn handle(&self, task: &Task) -> rediq::Result<()> {
///         Ok(())
///     }
/// }
///
/// #[async_trait]
/// impl Handler for SmsHandler {
///     async fn handle(&self, task: &Task) -> rediq::Result<()> {
///         Ok(())
///     }
/// }
///
/// let mut mux = Mux::new();
/// mux.handle("email:send", EmailHandler);
/// mux.handle("sms:send", SmsHandler);
/// ```
pub struct Mux {
    /// Handler mapping: task_type -> Handler
    handlers: HashMap<String, Arc<dyn Handler>>,

    /// Default handler for unknown task types
    default_handler: Option<Arc<dyn Handler>>,
}

impl Mux {
    /// Create a new multiplexer
    #[must_use]
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            default_handler: None,
        }
    }

    /// Register a handler for a specific task type
    ///
    /// # Arguments
    /// * `task_type` - The task type string to match (e.g., "email:send")
    /// * `handler` - The handler implementation
    ///
    /// # Example
    ///
    /// ```rust
    /// mux.handle("email:send", EmailHandler);
    /// ```
    pub fn handle<H: Handler + 'static>(&mut self, task_type: &str, handler: H) {
        self.handlers.insert(task_type.to_string(), Arc::new(handler));
    }

    /// Set a default handler for unknown task types
    ///
    /// The default handler is called when no specific handler is registered
    /// for a task's type.
    pub fn default_handler<H: Handler + 'static>(&mut self, handler: H) {
        self.default_handler = Some(Arc::new(handler));
    }

    /// Process a task by routing it to the appropriate handler
    ///
    /// This method looks up the handler based on the task's type and invokes it.
    /// If no handler is found and no default handler is set, it returns an error.
    pub async fn process(&self, task: &Task) -> Result<()> {
        let handler = self
            .handlers
            .get(&task.task_type)
            .or(self.default_handler.as_ref())
            .ok_or_else(|| {
                Error::Handler(format!("No handler found for task_type: {}", task.task_type))
            })?;

        handler.handle(task).await
    }

    /// Check if a handler exists for the given task type
    pub fn has_handler(&self, task_type: &str) -> bool {
        self.handlers.contains_key(task_type) || self.default_handler.is_some()
    }

    /// Get the number of registered handlers
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }

    /// Get all registered task types
    pub fn registered_types(&self) -> Vec<String> {
        self.handlers.keys().cloned().collect()
    }

    /// Remove a handler for the given task type
    pub fn remove(&mut self, task_type: &str) -> bool {
        self.handlers.remove(task_type).is_some()
    }

    /// Clear all registered handlers
    pub fn clear(&mut self) {
        self.handlers.clear();
        self.default_handler = None;
    }
}

impl Default for Mux {
    fn default() -> Self {
        Self::new()
    }
}

/// A simple handler that logs task information
///
/// Useful for testing and debugging.
pub struct LogHandler {
    /// Prefix for log messages
    pub prefix: String,
}

impl Default for LogHandler {
    fn default() -> Self {
        Self {
            prefix: "Task".to_string(),
        }
    }
}

#[async_trait]
impl Handler for LogHandler {
    async fn handle(&self, task: &Task) -> Result<()> {
        tracing::info!("{}: type={}, queue={}, id={}",
            self.prefix, task.task_type, task.queue, task.id);
        tracing::debug!("Task payload size: {} bytes", task.payload.len());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestHandler {
        should_fail: bool,
    }

    #[async_trait]
    impl Handler for TestHandler {
        async fn handle(&self, task: &Task) -> Result<()> {
            if self.should_fail {
                Err(Error::Handler("Test handler failed".to_string()))
            } else {
                tracing::info!("Handled task: {}", task.task_type);
                Ok(())
            }
        }
    }

    #[tokio::test]
    async fn test_mux_registration() {
        let mut mux = Mux::new();

        mux.handle("test:type", TestHandler { should_fail: false });

        assert!(mux.has_handler("test:type"));
        assert!(!mux.has_handler("unknown:type"));
        assert_eq!(mux.handler_count(), 1);
    }

    #[tokio::test]
    async fn test_mux_routing() {
        let mut mux = Mux::new();

        mux.handle("test:type", TestHandler { should_fail: false });

        let task = Task {
            id: "test-id".to_string(),
            task_type: "test:type".to_string(),
            queue: "default".to_string(),
            payload: vec![1, 2, 3],
            options: Default::default(),
            status: Default::default(),
            created_at: 0,
            enqueued_at: None,
            processed_at: None,
            retry_cnt: 0,
            last_error: None,
        };

        let result = mux.process(&task).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mux_no_handler() {
        let mux = Mux::new();

        let task = Task {
            id: "test-id".to_string(),
            task_type: "unknown:type".to_string(),
            queue: "default".to_string(),
            payload: vec![1, 2, 3],
            options: Default::default(),
            status: Default::default(),
            created_at: 0,
            enqueued_at: None,
            processed_at: None,
            retry_cnt: 0,
            last_error: None,
        };

        let result = mux.process(&task).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mux_default_handler() {
        let mut mux = Mux::new();

        mux.default_handler(TestHandler { should_fail: false });

        let task = Task {
            id: "test-id".to_string(),
            task_type: "unknown:type".to_string(),
            queue: "default".to_string(),
            payload: vec![1, 2, 3],
            options: Default::default(),
            status: Default::default(),
            created_at: 0,
            enqueued_at: None,
            processed_at: None,
            retry_cnt: 0,
            last_error: None,
        };

        let result = mux.process(&task).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_log_handler() {
        let handler = LogHandler {
            prefix: "Test".to_string(),
        };

        let task = Task {
            id: "test-id".to_string(),
            task_type: "test:type".to_string(),
            queue: "default".to_string(),
            payload: vec![1, 2, 3],
            options: Default::default(),
            status: Default::default(),
            created_at: 0,
            enqueued_at: None,
            processed_at: None,
            retry_cnt: 0,
            last_error: None,
        };

        let result = handler.handle(&task).await;
        assert!(result.is_ok());
    }
}
