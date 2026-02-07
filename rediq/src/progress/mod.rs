//! Task execution progress tracking module
//!
//! Provides progress reporting functionality for task handlers.
//! Progress is stored independently in Redis to avoid impacting Task serialization.

use crate::error::Result;
use crate::storage::{Keys, RedisClient};
use crate::Error;
use chrono::Utc;
use fred::prelude::RedisKey;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Task progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskProgress {
    /// Current progress value (0-100)
    pub current: u32,
    /// Optional progress message
    pub message: Option<String>,
    /// Last update timestamp (Unix timestamp)
    pub updated_at: i64,
}

/// Progress configuration
#[derive(Debug, Clone)]
pub struct ProgressConfig {
    /// Minimum update interval in milliseconds (throttling)
    pub throttle_ms: u64,
    /// Progress data TTL in seconds
    pub ttl_secs: u64,
}

impl Default for ProgressConfig {
    fn default() -> Self {
        Self {
            throttle_ms: 500,
            ttl_secs: 3600,
        }
    }
}

/// Progress reporter context
///
/// This context is created during task execution and allows handlers
/// to report their progress. Updates are throttled to avoid excessive
/// Redis writes.
#[derive(Clone)]
pub struct ProgressContext {
    task_id: Arc<String>,
    redis: RedisClient,
    config: ProgressConfig,
    last_update: Arc<Mutex<Instant>>,
    current_value: Arc<Mutex<u32>>,
}

impl ProgressContext {
    /// Create a new progress context
    pub(crate) fn new(task_id: String, redis: RedisClient, config: ProgressConfig) -> Self {
        Self {
            task_id: Arc::new(task_id),
            redis,
            config,
            last_update: Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1))),
            current_value: Arc::new(Mutex::new(0)),
        }
    }

    /// Get the task ID
    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    /// Report progress (0-100)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The progress value is not in the range 0-100
    /// - Redis write fails
    pub async fn report(&self, current: u32) -> Result<()> {
        self.report_with_message(current, None).await
    }

    /// Report progress with an optional message
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The progress value is not in the range 0-100
    /// - Redis write fails
    pub async fn report_with_message(&self, current: u32, message: Option<&str>) -> Result<()> {
        // Validate range
        if current > 100 {
            return Err(Error::Validation(format!(
                "Progress must be 0-100, got {}",
                current
            )));
        }

        // Throttling check (skip if too soon, unless completing)
        let mut last = self.last_update.lock().await;
        if last.elapsed() < Duration::from_millis(self.config.throttle_ms) && current < 100 {
            return Ok(());
        }
        *last = Instant::now();
        drop(last);

        // Update Redis
        let key: RedisKey = Keys::progress(&self.task_id).into();
        let now = Utc::now().timestamp();

        // Build fields to update
        let mut fields = vec![
            ("current".into(), current.to_string().into()),
            ("updated_at".into(), now.to_string().into()),
        ];

        if let Some(msg) = message {
            fields.push(("message".into(), msg.to_string().into()));
        }

        // Use hset to update the hash
        self.redis.hset(key.clone(), fields).await?;

        // Set TTL
        self.redis.expire(key, self.config.ttl_secs).await?;

        // Update local value
        *self.current_value.lock().await = current;

        Ok(())
    }

    /// Increment progress by a delta amount
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Redis write fails
    pub async fn increment(&self, delta: u32) -> Result<()> {
        let current = *self.current_value.lock().await;
        let new = current.saturating_add(delta).min(100);
        self.report(new).await
    }

    /// Get the current progress value
    pub async fn current(&self) -> u32 {
        *self.current_value.lock().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_config_default() {
        let config = ProgressConfig::default();
        assert_eq!(config.throttle_ms, 500);
        assert_eq!(config.ttl_secs, 3600);
    }

    #[test]
    fn test_task_progress_serialize() {
        let progress = TaskProgress {
            current: 50,
            message: Some("Halfway done".to_string()),
            updated_at: 1234567890,
        };

        let json = serde_json::to_string(&progress).unwrap();
        assert!(json.contains("\"current\":50"));
        assert!(json.contains("Halfway done"));
    }

    #[test]
    fn test_task_progress_deserialize() {
        let json = r#"{"current":75,"message":"Almost done","updated_at":1234567890}"#;
        let progress: TaskProgress = serde_json::from_str(json).unwrap();

        assert_eq!(progress.current, 75);
        assert_eq!(progress.message, Some("Almost done".to_string()));
        assert_eq!(progress.updated_at, 1234567890);
    }
}
