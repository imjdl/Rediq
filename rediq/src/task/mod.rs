//! Task type definitions
//!
//! Provides Task struct and TaskBuilder for building and serializing tasks.

use crate::{Error, Result};
use crate::config;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub mod builder;

pub use builder::TaskBuilder;

/// Task status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    /// Pending to be processed
    Pending,
    /// Currently being processed
    Active,
    /// Processed successfully
    Processed,
    /// Processing failed
    Failed,
    /// Waiting to be retried
    Retry,
    /// In dead letter queue
    Dead,
}

impl Default for TaskStatus {
    fn default() -> Self {
        Self::Pending
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::Active => write!(f, "active"),
            TaskStatus::Processed => write!(f, "processed"),
            TaskStatus::Failed => write!(f, "failed"),
            TaskStatus::Retry => write!(f, "retry"),
            TaskStatus::Dead => write!(f, "dead"),
        }
    }
}

/// Task options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOptions {
    /// Maximum number of retries
    pub max_retry: u32,
    /// Timeout duration
    pub timeout: Duration,
    /// Delay execution time (in seconds)
    pub delay: Option<Duration>,
    /// Cron expression (for periodic tasks)
    pub cron: Option<String>,
    /// Unique key (for deduplication)
    pub unique_key: Option<String>,
    /// Priority (0-100, lower value means higher priority)
    /// 0 is highest priority, 100 is lowest priority, default is 50
    pub priority: i32,
    /// Task dependencies - list of task IDs that must complete before this task runs
    pub depends_on: Option<Vec<String>>,
}

impl Default for TaskOptions {
    fn default() -> Self {
        Self {
            max_retry: 3,
            timeout: Duration::from_secs(30),
            delay: None,
            cron: None,
            unique_key: None,
            priority: 50,
            depends_on: None,
        }
    }
}

/// Task struct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Task ID (UUID)
    pub id: String,
    /// Task type (for routing to different handlers)
    pub task_type: String,
    /// Queue name
    pub queue: String,
    /// Task payload (serialized data)
    pub payload: Vec<u8>,
    /// Task options
    pub options: TaskOptions,
    /// Task status
    pub status: TaskStatus,
    /// Creation time (Unix timestamp, seconds)
    pub created_at: i64,
    /// Enqueue time (Unix timestamp, seconds)
    pub enqueued_at: Option<i64>,
    /// Processing start time (Unix timestamp, seconds)
    pub processed_at: Option<i64>,
    /// Current retry count
    pub retry_cnt: u32,
    /// Last error message
    pub last_error: Option<String>,
}

impl Task {
    /// Create a new task builder
    #[must_use]
    pub fn builder(task_type: impl Into<String>) -> TaskBuilder {
        TaskBuilder::new(task_type)
    }

    /// Validate if the task is valid
    pub fn validate(&self) -> Result<()> {
        if self.task_type.is_empty() {
            return Err(Error::Validation("task_type cannot be empty".into()));
        }

        if self.queue.is_empty() {
            return Err(Error::Validation("queue cannot be empty".into()));
        }

        if self.payload.is_empty() {
            return Err(Error::Validation("payload cannot be empty".into()));
        }

        // Payload size limit (use global config)
        let max_payload_size = config::get_max_payload_size();
        if self.payload.len() > max_payload_size {
            return Err(Error::Validation(format!(
                "payload exceeds {}KB limit (got {}B)",
                max_payload_size / 1024,
                self.payload.len()
            )));
        }

        // Validate priority range (use global config)
        let priority_range = config::get_priority_range();
        if self.options.priority < priority_range.0 || self.options.priority > priority_range.1 {
            return Err(Error::Validation(format!(
                "priority must be between {} and {}, got {}",
                priority_range.0, priority_range.1, self.options.priority
            )));
        }

        // Validate timeout
        if self.options.timeout.is_zero() {
            return Err(Error::Validation("timeout must be greater than 0".into()));
        }

        // Validate cron expression (simple validation)
        if let Some(cron) = &self.options.cron {
            if !cron.contains(' ') && cron != "@always" {
                return Err(Error::Validation(format!(
                    "invalid cron expression: {}",
                    cron
                )));
            }
        }

        Ok(())
    }

    /// Get task description
    pub fn description(&self) -> String {
        format!(
            "Task[type={}, queue={}, id={}, status={}]",
            self.task_type, self.queue, self.id, self.status
        )
    }

    /// Check if the task can be retried
    pub fn can_retry(&self) -> bool {
        self.retry_cnt < self.options.max_retry
    }

    /// Calculate the next retry delay (exponential backoff)
    pub fn retry_delay(&self) -> Option<Duration> {
        if !self.can_retry() {
            return None;
        }

        // Exponential backoff: 2^retry_cnt seconds, max 60 seconds
        let delay_secs = 2u64.pow(self.retry_cnt.saturating_add(1).min(6));
        Some(Duration::from_secs(delay_secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn test_task_validation() {
        let task = Task {
            id: Uuid::new_v4().to_string(),
            task_type: "test".to_string(),
            queue: "default".to_string(),
            payload: vec![1, 2, 3],
            options: TaskOptions::default(),
            status: TaskStatus::Pending,
            created_at: Utc::now().timestamp(),
            enqueued_at: None,
            processed_at: None,
            retry_cnt: 0,
            last_error: None,
        };

        assert!(task.validate().is_ok());
    }

    #[test]
    fn test_task_validation_empty_type() {
        let task = Task {
            id: Uuid::new_v4().to_string(),
            task_type: "".to_string(),
            queue: "default".to_string(),
            payload: vec![1, 2, 3],
            options: TaskOptions::default(),
            status: TaskStatus::Pending,
            created_at: Utc::now().timestamp(),
            enqueued_at: None,
            processed_at: None,
            retry_cnt: 0,
            last_error: None,
        };

        assert!(task.validate().is_err());
    }

    #[test]
    fn test_task_validation_large_payload() {
        let task = Task {
            id: Uuid::new_v4().to_string(),
            task_type: "test".to_string(),
            queue: "default".to_string(),
            payload: vec![0u8; 600 * 1024], // 600KB
            options: TaskOptions::default(),
            status: TaskStatus::Pending,
            created_at: Utc::now().timestamp(),
            enqueued_at: None,
            processed_at: None,
            retry_cnt: 0,
            last_error: None,
        };

        assert!(task.validate().is_err());
    }

    #[test]
    fn test_retry_delay() {
        let task = Task {
            id: Uuid::new_v4().to_string(),
            task_type: "test".to_string(),
            queue: "default".to_string(),
            payload: vec![1, 2, 3],
            options: TaskOptions {
                max_retry: 5,
                ..Default::default()
            },
            status: TaskStatus::Pending,
            created_at: Utc::now().timestamp(),
            enqueued_at: None,
            processed_at: None,
            retry_cnt: 0,
            last_error: None,
        };

        // First retry: 2^1 = 2 seconds
        assert_eq!(task.retry_delay(), Some(Duration::from_secs(2)));

        // Second retry: 2^2 = 4 seconds
        let task = Task {
            retry_cnt: 1,
            ..task
        };
        assert_eq!(task.retry_delay(), Some(Duration::from_secs(4)));

        // Third retry: 2^3 = 8 seconds
        let task = Task {
            retry_cnt: 2,
            ..task
        };
        assert_eq!(task.retry_delay(), Some(Duration::from_secs(8)));
    }

    #[test]
    fn test_can_retry() {
        let task = Task {
            id: Uuid::new_v4().to_string(),
            task_type: "test".to_string(),
            queue: "default".to_string(),
            payload: vec![1, 2, 3],
            options: TaskOptions {
                max_retry: 3,
                ..Default::default()
            },
            status: TaskStatus::Pending,
            created_at: Utc::now().timestamp(),
            enqueued_at: None,
            processed_at: None,
            retry_cnt: 0,
            last_error: None,
        };

        assert!(task.can_retry());

        let task = Task {
            retry_cnt: 3,
            ..task
        };
        assert!(!task.can_retry());
    }
}
