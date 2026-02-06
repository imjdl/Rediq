//! Error type definitions
//!
//! Provides all possible error types in the Rediq framework.

use std::time::Duration;

/// Result type alias for Rediq
pub type Result<T> = std::result::Result<T, Error>;

/// Error type for the Rediq framework
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Redis-related errors
    #[error("Redis error: {0}")]
    Redis(#[from] fred::error::RedisError),

    /// Serialization/deserialization errors
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Task validation errors
    #[error("Task validation error: {0}")]
    Validation(String),

    /// Task not found
    #[error("Task not found: {0}")]
    TaskNotFound(String),

    /// Queue not found
    #[error("Queue not found: {0}")]
    QueueNotFound(String),

    /// Queue is paused
    #[error("Queue is paused: {0}")]
    QueuePaused(String),

    /// Handler processing errors
    #[error("Handler error: {0}")]
    Handler(String),

    /// Task timeout
    #[error("Task timeout: {0}")]
    Timeout(String),

    /// Connection errors
    #[error("Connection error: {0}")]
    Connection(String),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Shutdown requested
    #[error("Shutdown requested")]
    Shutdown,

    /// Unknown errors
    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl Error {
    /// Check if the error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Error::Redis(_) | Error::Connection(_) | Error::Timeout(_)
        )
    }

    /// Check if the error is fatal (non-recoverable)
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            Error::Validation(_) | Error::Config(_) | Error::Shutdown
        )
    }

    /// Get the suggested retry delay
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Error::Timeout(_) => Some(Duration::from_secs(60)),
            Error::Connection(_) => Some(Duration::from_secs(5)),
            Error::Redis(_) => Some(Duration::from_secs(1)),
            _ => None,
        }
    }
}

/// Retry strategy configuration
#[derive(Debug, Clone, Copy)]
pub enum RetryStrategy {
    /// Fixed delay retry
    FixedDelay {
        /// Maximum number of retries
        max_retries: u32,
        /// Delay time for each retry
        delay: Duration,
    },
    /// Exponential backoff retry
    ExponentialBackoff {
        /// Maximum number of retries
        max_retries: u32,
        /// Initial delay time
        initial_delay: Duration,
        /// Maximum delay time
        max_delay: Duration,
        /// Delay multiplier
        multiplier: f64,
    },
    /// Linear increment retry
    Linear {
        /// Maximum number of retries
        max_retries: u32,
        /// Increment delay time per retry
        step: Duration,
    },
}

impl RetryStrategy {
    /// Get the maximum number of retries
    pub fn max_retries(&self) -> u32 {
        match self {
            RetryStrategy::FixedDelay { max_retries, .. } => *max_retries,
            RetryStrategy::ExponentialBackoff { max_retries, .. } => *max_retries,
            RetryStrategy::Linear { max_retries, .. } => *max_retries,
        }
    }

    /// Calculate delay time based on current retry count
    pub fn delay_for(&self, retry_count: u32) -> Option<Duration> {
        if retry_count >= self.max_retries() {
            return None;
        }

        match self {
            RetryStrategy::FixedDelay { delay, .. } => Some(*delay),
            RetryStrategy::ExponentialBackoff {
                initial_delay,
                max_delay,
                multiplier,
                ..
            } => {
                let delay = initial_delay.as_millis() as f64 * multiplier.powi(retry_count as i32);
                let delay = Duration::from_millis(delay as u64);
                Some(delay.min(*max_delay))
            }
            RetryStrategy::Linear { step, .. } => {
                Some(Duration::from_millis(step.as_millis() as u64 * (retry_count + 1) as u64))
            }
        }
    }
}

impl Default for RetryStrategy {
    fn default() -> Self {
        RetryStrategy::ExponentialBackoff {
            max_retries: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            multiplier: 2.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retryable_errors() {
        use fred::error::RedisErrorKind;
        assert!(Error::Redis(fred::error::RedisError::new(RedisErrorKind::Unknown, "test")).is_retryable());
        assert!(Error::Connection("test".to_string()).is_retryable());
        assert!(Error::Timeout("test".to_string()).is_retryable());
        assert!(!Error::Validation("test".to_string()).is_retryable());
        assert!(!Error::Config("test".to_string()).is_retryable());
    }

    #[test]
    fn test_exponential_backoff() {
        let strategy = RetryStrategy::ExponentialBackoff {
            max_retries: 5,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            multiplier: 2.0,
        };

        assert_eq!(strategy.delay_for(0), Some(Duration::from_secs(1)));
        assert_eq!(strategy.delay_for(1), Some(Duration::from_secs(2)));
        assert_eq!(strategy.delay_for(2), Some(Duration::from_secs(4)));
        assert_eq!(strategy.delay_for(3), Some(Duration::from_secs(8)));
        assert_eq!(strategy.delay_for(4), Some(Duration::from_secs(16)));
        assert_eq!(strategy.delay_for(5), None); // Exceeds max retries
    }
}
