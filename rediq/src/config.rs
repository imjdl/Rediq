//! Rediq global configuration module
//!
//! Provides centralized configuration for task limits, timeouts, and other system-wide settings.

use std::sync::RwLock;
use once_cell::sync::Lazy;

/// Rediq global configuration
///
/// Contains system-wide settings for task processing, payload limits, and Redis operations.
#[derive(Debug, Clone)]
pub struct RediqConfig {
    /// Maximum payload size in bytes (default: 512KB)
    pub max_payload_size: usize,

    /// Default task TTL in seconds for task details stored in Redis (default: 24 hours)
    pub task_ttl: u64,

    /// Priority range (min, max) - lower values indicate higher priority (default: 0-100)
    pub priority_range: (i32, i32),

    /// Default maximum retry count for tasks (default: 3)
    pub default_max_retry: u32,

    /// Default timeout for task execution in seconds (default: 30 seconds)
    pub default_timeout_secs: u64,
}

impl Default for RediqConfig {
    fn default() -> Self {
        Self {
            max_payload_size: 512 * 1024,  // 512KB
            task_ttl: 86400,                // 24 hours
            priority_range: (0, 100),
            default_max_retry: 3,
            default_timeout_secs: 30,
        }
    }
}

impl RediqConfig {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum payload size
    #[must_use]
    pub fn with_max_payload_size(mut self, size: usize) -> Self {
        self.max_payload_size = size;
        self
    }

    /// Set task TTL
    #[must_use]
    pub fn with_task_ttl(mut self, ttl: u64) -> Self {
        self.task_ttl = ttl;
        self
    }

    /// Set priority range
    #[must_use]
    pub fn with_priority_range(mut self, min: i32, max: i32) -> Self {
        self.priority_range = (min, max);
        self
    }

    /// Set default max retry count
    #[must_use]
    pub fn with_default_max_retry(mut self, max_retry: u32) -> Self {
        self.default_max_retry = max_retry;
        self
    }

    /// Set default timeout
    #[must_use]
    pub fn with_default_timeout(mut self, timeout_secs: u64) -> Self {
        self.default_timeout_secs = timeout_secs;
        self
    }

    /// Validate priority value against configured range
    pub fn validate_priority(&self, priority: i32) -> Result<(), String> {
        if priority < self.priority_range.0 || priority > self.priority_range.1 {
            Err(format!(
                "Priority must be between {} and {}, got {}",
                self.priority_range.0, self.priority_range.1, priority
            ))
        } else {
            Ok(())
        }
    }

    /// Clamp priority value to configured range
    pub fn clamp_priority(&self, priority: i32) -> i32 {
        priority.clamp(self.priority_range.0, self.priority_range.1)
    }
}

/// Thread-safe global configuration storage
static GLOBAL_CONFIG: Lazy<RwLock<RediqConfig>> =
    Lazy::new(|| RwLock::new(RediqConfig::default()));

/// Get the current global configuration
pub fn get_config() -> RediqConfig {
    GLOBAL_CONFIG
        .read()
        .unwrap_or_else(|e| {
            tracing::error!("Failed to read global config: {}", e);
            std::process::exit(1);
        })
        .clone()
}

/// Set the global configuration
///
/// # Panics
///
/// Panics if the global configuration lock is poisoned (which indicates a serious bug).
pub fn set_config(config: RediqConfig) {
    let mut global = GLOBAL_CONFIG
        .write()
        .unwrap_or_else(|e| {
            tracing::error!("Failed to write global config: {}", e);
            std::process::exit(1);
        });
    *global = config;
    tracing::info!("Global Rediq configuration updated");
}

/// Update the global configuration with a modifier function
///
/// This is useful for making partial changes to the configuration.
///
/// # Example
///
/// ```rust
/// use rediq::config::update_config;
///
/// update_config(|config| {
///     config.with_max_payload_size(1024 * 1024) // 1MB
/// });
/// ```
///
/// # Implementation Note
///
/// This function takes the write lock directly to avoid potential deadlock scenarios
/// that could occur with the previous implementation which acquired read and write
/// locks separately. The modifier now receives a mutable reference instead of
/// consuming and returning a RediqConfig.
pub fn update_config<F>(modifier: F)
where
    F: FnOnce(&mut RediqConfig),
{
    let mut global = GLOBAL_CONFIG
        .write()
        .unwrap_or_else(|e| {
            tracing::error!("Failed to write global config: {}", e);
            std::process::exit(1);
        });

    // Modify the config in place, avoiding the clone
    modifier(&mut global);

    tracing::info!("Global Rediq configuration updated");
}

/// Get a specific configuration value
pub fn get_max_payload_size() -> usize {
    get_config().max_payload_size
}

/// Get the task TTL
pub fn get_task_ttl() -> u64 {
    get_config().task_ttl
}

/// Get the priority range
pub fn get_priority_range() -> (i32, i32) {
    get_config().priority_range
}

/// Get the default max retry count
pub fn get_default_max_retry() -> u32 {
    get_config().default_max_retry
}

/// Get the default timeout in seconds
pub fn get_default_timeout_secs() -> u64 {
    get_config().default_timeout_secs
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Mutex to ensure tests that modify global config run serially
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_default_config() {
        let config = RediqConfig::default();
        assert_eq!(config.max_payload_size, 512 * 1024);
        assert_eq!(config.task_ttl, 86400);
        assert_eq!(config.priority_range, (0, 100));
        assert_eq!(config.default_max_retry, 3);
        assert_eq!(config.default_timeout_secs, 30);
    }

    #[test]
    fn test_config_builder() {
        let config = RediqConfig::new()
            .with_max_payload_size(1024 * 1024)
            .with_task_ttl(3600)
            .with_priority_range(1, 10)
            .with_default_max_retry(5)
            .with_default_timeout(60);

        assert_eq!(config.max_payload_size, 1024 * 1024);
        assert_eq!(config.task_ttl, 3600);
        assert_eq!(config.priority_range, (1, 10));
        assert_eq!(config.default_max_retry, 5);
        assert_eq!(config.default_timeout_secs, 60);
    }

    #[test]
    fn test_validate_priority() {
        let config = RediqConfig::new().with_priority_range(0, 100);

        assert!(config.validate_priority(50).is_ok());
        assert!(config.validate_priority(0).is_ok());
        assert!(config.validate_priority(100).is_ok());
        assert!(config.validate_priority(-1).is_err());
        assert!(config.validate_priority(101).is_err());
    }

    #[test]
    fn test_clamp_priority() {
        let config = RediqConfig::new().with_priority_range(10, 90);

        assert_eq!(config.clamp_priority(50), 50);
        assert_eq!(config.clamp_priority(5), 10);
        assert_eq!(config.clamp_priority(95), 90);
    }

    #[test]
    fn test_global_config() {
        let _lock = TEST_LOCK.lock().unwrap();
        // Save original config
        let original = get_config();

        // Set new config
        let new_config = RediqConfig::new()
            .with_max_payload_size(2048);
        set_config(new_config.clone());

        // Verify it was set
        assert_eq!(get_config().max_payload_size, 2048);

        // Update with modifier
        update_config(|c| {
            c.task_ttl = 7200;
        });
        assert_eq!(get_config().task_ttl, 7200);
        assert_eq!(get_config().max_payload_size, 2048); // Should be preserved

        // Restore original
        set_config(original);
    }

    #[test]
    fn test_config_getters() {
        let _lock = TEST_LOCK.lock().unwrap();
        let original = get_config();

        set_config(RediqConfig::new()
            .with_max_payload_size(123456)
            .with_task_ttl(12345)
            .with_priority_range(5, 95)
            .with_default_max_retry(7)
            .with_default_timeout(45));

        assert_eq!(get_max_payload_size(), 123456);
        assert_eq!(get_task_ttl(), 12345);
        assert_eq!(get_priority_range(), (5, 95));
        assert_eq!(get_default_max_retry(), 7);
        assert_eq!(get_default_timeout_secs(), 45);

        // Restore original
        set_config(original);
    }
}
