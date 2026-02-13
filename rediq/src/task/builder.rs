//! Task builder
//!
//! Provides fluent API for building tasks.

use crate::{Error, Result};
use crate::config;
use super::{Task, TaskOptions, TaskStatus};
use chrono::Utc;
use serde::Serialize;
use std::time::Duration;
use uuid::Uuid;

/// Task builder
///
/// # Examples
///
/// ```rust
/// use rediq::Task;
/// use serde::{Deserialize, Serialize};
/// use std::time::Duration;
///
/// #[derive(Serialize, Deserialize)]
/// struct EmailPayload {
///     to: String,
///     subject: String,
/// }
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let task = Task::builder("email:send")
///     .queue("default")
///     .payload(&EmailPayload {
///         to: "user@example.com".to_string(),
///         subject: "Hello".to_string(),
///     })?
///     .max_retry(5)
///     .timeout(Duration::from_secs(30))
///     .delay(Duration::from_secs(60))
///     .unique_key("email:user@example.com:hello")
///     .priority(80)
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct TaskBuilder {
    task_type: String,
    queue: String,
    payload: Vec<u8>,
    options: TaskOptions,
}

impl TaskBuilder {
    /// Create a new task builder
    #[must_use]
    pub fn new(task_type: impl Into<String>) -> Self {
        Self {
            task_type: task_type.into(),
            queue: "default".to_string(),
            payload: Vec::new(),
            options: TaskOptions::default(),
        }
    }

    /// Set queue name
    #[must_use]
    pub fn queue(mut self, queue: impl Into<String>) -> Self {
        self.queue = queue.into();
        self
    }

    /// Set task payload (serialized)
    pub fn payload<T: Serialize>(mut self, payload: &T) -> Result<Self> {
        self.payload = rmp_serde::to_vec(payload)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        Ok(self)
    }

    /// Set raw binary payload
    #[must_use]
    pub fn raw_payload(mut self, payload: Vec<u8>) -> Self {
        self.payload = payload;
        self
    }

    /// Set maximum retry count
    #[must_use]
    pub fn max_retry(mut self, max: u32) -> Self {
        self.options.max_retry = max;
        self
    }

    /// Set timeout duration
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.options.timeout = timeout;
        self
    }

    /// Set delayed execution time
    #[must_use]
    pub fn delay(mut self, delay: Duration) -> Self {
        self.options.delay = Some(delay);
        self
    }

    /// Set cron expression (for periodic tasks)
    #[must_use]
    pub fn cron(mut self, cron: impl Into<String>) -> Self {
        self.options.cron = Some(cron.into());
        self
    }

    /// Set unique key (for deduplication)
    #[must_use]
    pub fn unique_key(mut self, key: impl Into<String>) -> Self {
        self.options.unique_key = Some(key.into());
        self
    }

    /// Set priority (0-100, lower value means higher priority)
    /// Priority 0 is highest, 100 is lowest. Default is 50.
    #[must_use]
    pub fn priority(mut self, priority: i32) -> Self {
        self.options.priority = priority.clamp(0, 100);
        self
    }

    /// Set task dependencies
    ///
    /// The task will only execute after all specified dependency tasks have completed successfully.
    ///
    /// # Arguments
    /// * `deps` - Slice of task IDs that this task depends on
    ///
    /// # Example
    ///
    /// ```rust
    /// use rediq::Task;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let task_b = Task::builder("process:b")
    ///     .queue("default")
    ///     .raw_payload(b"data".to_vec())
    ///     .depends_on(&["task-a-id-123"])
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn depends_on(mut self, deps: &[&str]) -> Self {
        self.options.depends_on = Some(deps.iter().map(|s| s.to_string()).collect());
        self
    }

    /// Set group name for task aggregation
    ///
    /// Tasks with the same group name will be aggregated together before processing.
    /// The aggregation is triggered by one of the following conditions:
    /// - The group reaches its maximum size
    /// - The grace period expires
    /// - The maximum delay from the first task is exceeded
    ///
    /// # Arguments
    /// * `group` - The group name for aggregation
    ///
    /// # Example
    ///
    /// ```rust
    /// use rediq::Task;
    /// use serde::Serialize;
    ///
    /// #[derive(Serialize)]
    /// struct UserData {
    ///     user_id: String,
    /// }
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let user_data = UserData { user_id: "123".to_string() };
    /// let task = Task::builder("notification:send")
    ///     .queue("default")
    ///     .payload(&user_data)?
    ///     .group("daily_notifications")
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.options.group = Some(group.into());
        self
    }

    /// Build the task
    pub fn build(self) -> Result<Task> {
        let now = Utc::now().timestamp();

        // Validate task type
        if self.task_type.is_empty() {
            return Err(Error::Validation("task_type cannot be empty".into()));
        }

        // Validate payload
        if self.payload.is_empty() {
            return Err(Error::Validation("payload cannot be empty".into()));
        }

        // Validate payload size (use global config)
        let max_payload_size = config::get_max_payload_size();
        if self.payload.len() > max_payload_size {
            return Err(Error::Validation(format!(
                "payload exceeds {}KB limit (got {}B)",
                max_payload_size / 1024,
                self.payload.len()
            )));
        }

        let task = Task {
            id: Uuid::new_v4().to_string(),
            task_type: self.task_type,
            queue: self.queue,
            payload: self.payload,
            options: self.options,
            status: TaskStatus::Pending,
            created_at: now,
            enqueued_at: None,
            processed_at: None,
            retry_cnt: 0,
            last_error: None,
        };

        // Validate task
        task.validate()?;

        Ok(task)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Serialize, Deserialize};

    #[derive(Serialize, Deserialize)]
    struct TestPayload {
        message: String,
    }

    #[test]
    fn test_builder_basic() {
        let task = Task::builder("test:task")
            .payload(&TestPayload {
                message: "Hello".to_string(),
            })
            .unwrap()
            .build()
            .unwrap();

        assert_eq!(task.task_type, "test:task");
        assert_eq!(task.queue, "default");
        assert!(!task.payload.is_empty());
        assert_eq!(task.status, TaskStatus::Pending);
    }

    #[test]
    fn test_builder_with_options() {
        let task = Task::builder("test:task")
            .queue("high")
            .payload(&TestPayload {
                message: "Hello".to_string(),
            })
            .unwrap()
            .max_retry(5)
            .timeout(Duration::from_secs(60))
            .delay(Duration::from_secs(30))
            .priority(80)
            .build()
            .unwrap();

        assert_eq!(task.queue, "high");
        assert_eq!(task.options.max_retry, 5);
        assert_eq!(task.options.timeout, Duration::from_secs(60));
        assert_eq!(task.options.delay, Some(Duration::from_secs(30)));
        assert_eq!(task.options.priority, 80);
    }

    #[test]
    fn test_builder_empty_type() {
        let result = Task::builder("")
            .payload(&TestPayload {
                message: "Hello".to_string(),
            })
            .unwrap()
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_builder_empty_payload() {
        let result = Task::builder("test:task").build();

        assert!(result.is_err());
    }

    #[test]
    fn test_builder_priority_clamp() {
        let task = Task::builder("test:task")
            .payload(&TestPayload {
                message: "Hello".to_string(),
            })
            .unwrap()
            .priority(150)
            .build()
            .unwrap();

        assert_eq!(task.options.priority, 100); // Clamped to max value
    }

    #[test]
    fn test_builder_with_unique_key() {
        let task = Task::builder("test:task")
            .payload(&TestPayload {
                message: "Hello".to_string(),
            })
            .unwrap()
            .unique_key("unique-key-123")
            .build()
            .unwrap();

        assert_eq!(task.options.unique_key, Some("unique-key-123".to_string()));
    }

    #[test]
    fn test_builder_with_cron() {
        let task = Task::builder("test:task")
            .payload(&TestPayload {
                message: "Hello".to_string(),
            })
            .unwrap()
            .cron("0 0 * * *")
            .build()
            .unwrap();

        assert_eq!(task.options.cron, Some("0 0 * * *".to_string()));
    }

    #[test]
    fn test_builder_with_dependencies() {
        let task = Task::builder("test:task")
            .payload(&TestPayload {
                message: "Hello".to_string(),
            })
            .unwrap()
            .depends_on(&["task-1", "task-2", "task-3"])
            .build()
            .unwrap();

        assert_eq!(task.options.depends_on, Some(vec!["task-1".to_string(), "task-2".to_string(), "task-3".to_string()]));
    }

    #[test]
    fn test_builder_with_group() {
        let task = Task::builder("test:task")
            .payload(&TestPayload {
                message: "Hello".to_string(),
            })
            .unwrap()
            .group("daily_notifications")
            .build()
            .unwrap();

        assert_eq!(task.options.group, Some("daily_notifications".to_string()));
    }
}
