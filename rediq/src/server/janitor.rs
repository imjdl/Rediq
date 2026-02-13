//! Janitor - Task cleanup module
//!
//! Automatically cleans up expired task details to prevent Redis memory leaks.

use crate::storage::{Keys, RedisClient};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::time::interval;

/// Configuration for the janitor
#[derive(Debug, Clone)]
pub struct JanitorConfig {
    /// Interval between cleanup runs
    pub interval: Duration,
    /// Maximum number of keys to delete per run
    pub batch_size: usize,
    /// TTL threshold (seconds) - keys with TTL below this will be deleted
    pub ttl_threshold: i64,
}

impl Default for JanitorConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(60),
            batch_size: 100,
            ttl_threshold: 60,
        }
    }
}

impl JanitorConfig {
    /// Create a new janitor configuration
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the cleanup interval
    #[must_use]
    pub fn interval(mut self, duration: Duration) -> Self {
        self.interval = duration;
        self
    }

    /// Set the batch size
    #[must_use]
    pub fn batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    /// Set the TTL threshold
    #[must_use]
    pub fn ttl_threshold(mut self, seconds: i64) -> Self {
        self.ttl_threshold = seconds;
        self
    }
}

/// Janitor - Cleans up expired tasks
pub struct Janitor {
    redis: RedisClient,
    config: JanitorConfig,
    shutdown: Arc<AtomicBool>,
}

impl Janitor {
    /// Create a new janitor
    pub fn new(redis: RedisClient, config: JanitorConfig) -> Self {
        Self {
            redis,
            config,
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get a handle to the shutdown flag
    pub fn shutdown_handle(&self) -> Arc<AtomicBool> {
        self.shutdown.clone()
    }

    /// Start the janitor cleanup loop
    pub async fn run(&self) {
        let mut timer = interval(self.config.interval);
        timer.tick().await; // Skip first immediate tick

        tracing::info!("Janitor started (interval: {:?}, batch_size: {})",
            self.config.interval, self.config.batch_size);

        while !self.shutdown.load(Ordering::Relaxed) {
            timer.tick().await;

            if self.shutdown.load(Ordering::Relaxed) {
                break;
            }

            match self.cleanup().await {
                Ok(count) => {
                    if count > 0 {
                        tracing::debug!("Janitor cleaned up {} expired tasks", count);
                    }
                }
                Err(e) => {
                    tracing::error!("Janitor cleanup failed: {}", e);
                }
            }
        }

        tracing::info!("Janitor stopped");
    }

    /// Stop the janitor
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }

    /// Perform cleanup of expired tasks
    ///
    /// This method scans for task keys with low TTL and deletes them.
    async fn cleanup(&self) -> Result<usize, crate::Error> {
        let mut deleted_count = 0;
        let pattern = format!("{}:*", Keys::task(""));

        // Use SCAN to iterate over task keys
        let mut cursor = 0u64;
        let mut keys_to_delete = Vec::new();

        loop {
            // Scan for keys matching the pattern
            let (next_cursor, keys) = self.redis.scan_match(cursor, &pattern, 100).await?;

            cursor = next_cursor;

            // Check TTL of each key
            for key in keys {
                if deleted_count + keys_to_delete.len() >= self.config.batch_size {
                    break;
                }

                match self.redis.ttl(&key).await {
                    Ok(Some(ttl)) => {
                        if ttl < self.config.ttl_threshold {
                            keys_to_delete.push(key);
                        }
                    }
                    Ok(None) => {
                        // Key has no TTL, skip
                    }
                    Err(e) => {
                        tracing::warn!("Failed to get TTL for key {}: {}", key, e);
                    }
                }
            }

            // Delete accumulated keys
            if !keys_to_delete.is_empty() {
                let keys: Vec<fred::prelude::RedisKey> = keys_to_delete
                    .iter()
                    .map(|k| k.as_str().into())
                    .collect();

                match self.redis.del(keys).await {
                    Ok(count) => {
                        deleted_count += count;
                        tracing::debug!("Janitor deleted {} keys", count);
                    }
                    Err(e) => {
                        tracing::error!("Failed to delete keys: {}", e);
                    }
                }

                keys_to_delete.clear();
            }

            // Check if we've processed enough keys
            if deleted_count >= self.config.batch_size {
                break;
            }

            // Check if scan is complete
            if cursor == 0 {
                break;
            }
        }

        Ok(deleted_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_janitor_config() {
        let config = JanitorConfig::new()
            .interval(Duration::from_secs(30))
            .batch_size(200)
            .ttl_threshold(120);

        assert_eq!(config.interval, Duration::from_secs(30));
        assert_eq!(config.batch_size, 200);
        assert_eq!(config.ttl_threshold, 120);
    }

    #[test]
    fn test_janitor_config_default() {
        let config = JanitorConfig::default();
        assert_eq!(config.interval, Duration::from_secs(60));
        assert_eq!(config.batch_size, 100);
        assert_eq!(config.ttl_threshold, 60);
    }
}
