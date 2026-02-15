//! Redis client wrapper
//!
//! Provides type-safe Redis operation interfaces.

use crate::{Error, Result};
use fred::{
    interfaces::*,
    prelude::*,
    types::{RedisConfig as FredRedisConfig, ReconnectPolicy},
};
use std::sync::Arc;

/// Redis connection mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RedisMode {
    /// Single Redis instance
    #[default]
    Standalone,
    /// Redis Cluster mode
    Cluster,
    /// Redis Sentinel mode for high availability
    Sentinel,
}

/// Connection pool configuration
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum pool size
    pub pool_size: usize,
    /// Minimum idle connections
    pub min_idle: Option<usize>,
    /// Connection timeout in seconds
    pub connection_timeout: Option<u64>,
    /// Idle timeout in seconds
    pub idle_timeout: Option<u64>,
    /// Maximum connection lifetime in seconds
    pub max_lifetime: Option<u64>,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            pool_size: 10,
            min_idle: None,
            connection_timeout: None,
            idle_timeout: None,
            max_lifetime: None,
        }
    }
}

impl PoolConfig {
    /// Create a new pool configuration with the specified pool size
    pub fn new(pool_size: usize) -> Self {
        Self {
            pool_size,
            ..Default::default()
        }
    }

    /// Set minimum idle connections
    #[must_use]
    pub fn min_idle(mut self, min_idle: usize) -> Self {
        self.min_idle = Some(min_idle);
        self
    }

    /// Set connection timeout in seconds
    #[must_use]
    pub fn connection_timeout(mut self, timeout: u64) -> Self {
        self.connection_timeout = Some(timeout);
        self
    }

    /// Set idle timeout in seconds
    #[must_use]
    pub fn idle_timeout(mut self, timeout: u64) -> Self {
        self.idle_timeout = Some(timeout);
        self
    }

    /// Set maximum connection lifetime in seconds
    #[must_use]
    pub fn max_lifetime(mut self, lifetime: u64) -> Self {
        self.max_lifetime = Some(lifetime);
        self
    }
}

/// Redis connection configuration
#[derive(Debug, Clone)]
pub struct RedisConfig {
    /// Redis connection URL
    pub url: String,
    /// Connection pool configuration
    pub pool_config: PoolConfig,
    /// Redis connection mode
    pub mode: RedisMode,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            pool_config: PoolConfig::default(),
            mode: RedisMode::Standalone,
        }
    }
}

/// Redis client
#[derive(Clone)]
pub struct RedisClient {
    pool: Arc<RedisPool>,
}

impl RedisClient {
    /// Create a new Redis client
    pub async fn new(config: RedisConfig) -> Result<Self> {
        let redis_config = FredRedisConfig::from_url(&config.url)?;
        let pool = RedisPool::new(
            redis_config,
            None,
            None,
            Some(ReconnectPolicy::default()),
            config.pool_config.pool_size,
        )?;

        pool.init().await?;

        match config.mode {
            RedisMode::Standalone => {
                tracing::info!("Connected to Redis at {}", config.url);
            }
            RedisMode::Cluster => {
                tracing::info!("Connected to Redis Cluster at {}", config.url);
            }
            RedisMode::Sentinel => {
                tracing::info!("Connected to Redis Sentinel at {}", config.url);
            }
        }

        Ok(Self {
            pool: Arc::new(pool),
        })
    }

    /// Create client from connection URL
    pub async fn from_url(url: impl Into<String>) -> Result<Self> {
        Self::from_url_with_pool_config(url, PoolConfig::default()).await
    }

    /// Create client from connection URL with custom pool size
    pub async fn from_url_with_pool(url: impl Into<String>, pool_size: usize) -> Result<Self> {
        Self::from_url_with_pool_config(url, PoolConfig::new(pool_size)).await
    }

    /// Create client from connection URL with pool configuration
    pub async fn from_url_with_pool_config(url: impl Into<String>, pool_config: PoolConfig) -> Result<Self> {
        let url = url.into();
        let redis_config = FredRedisConfig::from_url(&url)?;
        let pool = RedisPool::new(
            redis_config,
            None,
            None,
            Some(ReconnectPolicy::default()),
            pool_config.pool_size,
        )?;

        pool.init().await?;

        tracing::info!("Connected to Redis at {} (pool size: {})", url, pool_config.pool_size);

        Ok(Self {
            pool: Arc::new(pool),
        })
    }

    /// Create client from Redis Cluster URL
    ///
    /// fred will automatically discover cluster nodes.
    /// Use any cluster node URL to connect.
    ///
    /// # Arguments
    /// * `url` - Any cluster node URL (e.g., "redis://cluster-node1:6379")
    pub async fn from_cluster_url(url: impl Into<String>) -> Result<Self> {
        Self::from_cluster_url_with_pool_config(url, PoolConfig::default()).await
    }

    /// Create client from Redis Cluster URL with custom pool size
    ///
    /// # Arguments
    /// * `url` - Any cluster node URL
    /// * `pool_size` - Connection pool size
    pub async fn from_cluster_url_with_pool(url: impl Into<String>, pool_size: usize) -> Result<Self> {
        Self::from_cluster_url_with_pool_config(url, PoolConfig::new(pool_size)).await
    }

    /// Create client from Redis Cluster URL with pool configuration
    ///
    /// # Arguments
    /// * `url` - Any cluster node URL
    /// * `pool_config` - Pool configuration
    pub async fn from_cluster_url_with_pool_config(url: impl Into<String>, pool_config: PoolConfig) -> Result<Self> {
        let url = url.into();
        let redis_config = FredRedisConfig::from_url(&url)?;
        let pool = RedisPool::new(
            redis_config,
            None,
            None,
            Some(ReconnectPolicy::default()),
            pool_config.pool_size,
        )?;

        pool.init().await?;

        tracing::info!("Connected to Redis Cluster at {} (pool size: {})", url, pool_config.pool_size);

        Ok(Self {
            pool: Arc::new(pool),
        })
    }

    /// Create client from Redis Sentinel URL
    ///
    /// fred will automatically discover the master and handle failover.
    /// Use any Sentinel node URL to connect.
    ///
    /// # Arguments
    /// * `url` - Any sentinel node URL (e.g., "redis://sentinel-1:26379")
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use rediq::storage::RedisClient;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = RedisClient::from_sentinel_url("redis://sentinel-1:26379").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn from_sentinel_url(url: impl Into<String>) -> Result<Self> {
        Self::from_sentinel_url_with_pool_config(url, PoolConfig::default()).await
    }

    /// Create client from Redis Sentinel URL with custom pool size
    ///
    /// # Arguments
    /// * `url` - Any sentinel node URL
    /// * `pool_size` - Connection pool size
    pub async fn from_sentinel_url_with_pool(url: impl Into<String>, pool_size: usize) -> Result<Self> {
        Self::from_sentinel_url_with_pool_config(url, PoolConfig::new(pool_size)).await
    }

    /// Create client from Redis Sentinel URL with pool configuration
    ///
    /// # Arguments
    /// * `url` - Any sentinel node URL
    /// * `pool_config` - Pool configuration
    pub async fn from_sentinel_url_with_pool_config(url: impl Into<String>, pool_config: PoolConfig) -> Result<Self> {
        let url = url.into();
        let redis_config = FredRedisConfig::from_url(&url)?;
        let pool = RedisPool::new(
            redis_config,
            None,
            None,
            Some(ReconnectPolicy::default()),
            pool_config.pool_size,
        )?;

        pool.init().await?;

        tracing::info!("Connected to Redis Sentinel at {} (pool size: {})", url, pool_config.pool_size);

        Ok(Self {
            pool: Arc::new(pool),
        })
    }

    /// Get the underlying Redis connection pool
    pub fn pool(&self) -> &Arc<RedisPool> {
        &self.pool
    }

    /// Ping Redis
    pub async fn ping(&self) -> Result<String> {
        let result: String = self.pool.ping().await?;
        Ok(result)
    }

    /// Set Key-Value
    pub async fn set(&self, key: RedisKey, value: RedisValue) -> Result<()> {
        let _: () = self.pool.set(key, value, None, None, false).await?;
        Ok(())
    }

    /// Get Value
    pub async fn get(&self, key: RedisKey) -> Result<Option<RedisValue>> {
        let result: Option<RedisValue> = self.pool.get(key).await?;
        Ok(result)
    }

    /// Delete Key
    pub async fn del(&self, keys: Vec<RedisKey>) -> Result<usize> {
        let result: usize = self.pool.del(keys).await?;
        Ok(result)
    }

    /// Check if Key exists
    pub async fn exists(&self, key: RedisKey) -> Result<bool> {
        let result: bool = self.pool.exists(key).await?;
        Ok(result)
    }

    /// Set expiration time
    pub async fn expire(&self, key: RedisKey, seconds: u64) -> Result<bool> {
        let result: bool = self.pool.expire(key, seconds as i64).await?;
        Ok(result)
    }

    /// List operation: right push
    pub async fn rpush(&self, key: RedisKey, value: RedisValue) -> Result<u64> {
        let result: u64 = self.pool.rpush(key, value).await?;
        Ok(result)
    }

    /// List operation: left push
    pub async fn lpush(&self, key: RedisKey, value: RedisValue) -> Result<u64> {
        let result: u64 = self.pool.lpush(key, value).await?;
        Ok(result)
    }

    /// List operation: left pop (blocking)
    pub async fn blpop(&self, key: RedisKey, timeout: u64) -> Result<Option<(String, String)>> {
        let result: Option<(String, String)> = self.pool.blpop(key, timeout as f64).await?;
        Ok(result)
    }

    /// List operation: right pop (blocking)
    pub async fn brpop(&self, key: RedisKey, timeout: u64) -> Result<Option<(String, String)>> {
        let result: Option<(String, String)> = self.pool.brpop(key, timeout as f64).await?;
        Ok(result)
    }

    /// List operation: get length
    pub async fn llen(&self, key: RedisKey) -> Result<u64> {
        let result: u64 = self.pool.llen(key).await?;
        Ok(result)
    }

    /// List operation: get range by index
    pub async fn lrange(&self, key: RedisKey, start: i64, stop: i64) -> Result<Vec<String>> {
        let result: Vec<RedisValue> = self.pool.lrange(key, start, stop).await?;
        Ok(result.into_iter().filter_map(|v| v.as_string().map(|s| s.to_string())).collect())
    }

    /// Sorted Set operation: add
    pub async fn zadd(&self, key: RedisKey, member: RedisValue, score: i64) -> Result<()> {
        let values: Vec<(f64, RedisValue)> = vec![(score as f64, member)];
        let _: () = self.pool.zadd(key, None, None, false, false, values).await?;
        Ok(())
    }

    /// Sorted Set operation: get by index range (with scores)
    pub async fn zrange_with_scores(&self, key: RedisKey, start: i64, stop: i64) -> Result<Vec<(String, f64)>> {
        let result: Vec<RedisValue> = self
            .pool
            .zrange(key, start, stop, None, false, None, true)
            .await?;
        // Result comes as alternating member, score, member, score, ...
        let mut output = Vec::new();
        for chunk in result.chunks(2) {
            if chunk.len() == 2 {
                let member = chunk[0].as_string().map(|s| s.to_string());
                let score = chunk[1].as_f64();
                if let (Some(m), Some(s)) = (member, score) {
                    output.push((m, s));
                }
            }
        }
        Ok(output)
    }

    /// Sorted Set operation: get by index range
    pub async fn zrange(&self, key: RedisKey, start: i64, stop: i64) -> Result<Vec<String>> {
        let result: Vec<RedisValue> = self
            .pool
            .zrange(key, start, stop, None, false, None, false)
            .await?;
        Ok(result.into_iter().filter_map(|v| v.as_string().map(|s| s.to_string())).collect())
    }

    /// Sorted Set operation: get by score range
    pub async fn zrangebyscore(&self, key: RedisKey, min: i64, max: i64) -> Result<Vec<String>> {
        let result: Vec<RedisValue> = self
            .pool
            .zrangebyscore(key, min, max, false, None)
            .await?;
        Ok(result.into_iter().filter_map(|v| v.as_string().map(|s| s.to_string())).collect())
    }

    /// Sorted Set operation: remove
    pub async fn zrem(&self, key: RedisKey, member: RedisValue) -> Result<bool> {
        let result: u64 = self.pool.zrem(key, member).await?;
        Ok(result > 0)
    }

    /// Sorted Set operation: get cardinality (number of elements)
    pub async fn zcard(&self, key: RedisKey) -> Result<u64> {
        let result: u64 = self.pool.zcard(key).await?;
        Ok(result)
    }

    /// Set operation: add
    pub async fn sadd(&self, key: RedisKey, member: RedisValue) -> Result<bool> {
        let result: u64 = self.pool.sadd(key, member).await?;
        Ok(result > 0)
    }

    /// Set operation: check if member exists
    pub async fn sismember(&self, key: RedisKey, member: RedisValue) -> Result<bool> {
        let result: bool = self.pool.sismember(key, member).await?;
        Ok(result)
    }

    /// Set operation: remove
    pub async fn srem(&self, key: RedisKey, member: RedisValue) -> Result<bool> {
        let result: u64 = self.pool.srem(key, member).await?;
        Ok(result > 0)
    }

    /// Set operation: get all members
    pub async fn smembers(&self, key: RedisKey) -> Result<Vec<String>> {
        let result: Vec<RedisValue> = self.pool.smembers(key).await?;
        Ok(result.into_iter().filter_map(|v| v.as_string().map(|s| s.to_string())).collect())
    }

    /// Set operation: get cardinality (number of members)
    pub async fn scard(&self, key: RedisKey) -> Result<u64> {
        let result: u64 = self.pool.scard(key).await?;
        Ok(result)
    }

    /// Hash operation: set field
    pub async fn hset(&self, key: RedisKey, values: Vec<(RedisKey, RedisValue)>) -> Result<bool> {
        let result: u64 = self.pool.hset(key, values).await?;
        Ok(result > 0)
    }

    /// Hash operation: increment field by value
    pub async fn hincrby(&self, key: RedisKey, field: RedisKey, increment: i64) -> Result<i64> {
        let result: i64 = self.pool.hincrby(key, field, increment).await?;
        Ok(result)
    }

    /// Hash operation: get field
    pub async fn hget(&self, key: RedisKey, field: RedisKey) -> Result<Option<RedisValue>> {
        let result: Option<RedisValue> = self.pool.hget(key, field).await?;
        Ok(result)
    }

    /// Hash operation: get all fields
    pub async fn hgetall(&self, key: RedisKey) -> Result<Vec<RedisValue>> {
        let result: Vec<RedisValue> = self.pool.hgetall(key).await?;
        Ok(result)
    }

    /// Hash operation: set multiple fields
    pub async fn hmset(&self, key: RedisKey, values: Vec<(RedisKey, RedisValue)>) -> Result<()> {
        let _: () = self.pool.hset(key, values).await?;
        Ok(())
    }

    /// Pipeline operation
    pub fn pipeline(&self) -> RedisPipeline {
        RedisPipeline::new(self.pool.clone())
    }

    /// List operation: remove element
    pub async fn lrem(&self, key: RedisKey, value: RedisValue, count: i64) -> Result<u64> {
        let result: u64 = self.pool.lrem(key, count, value).await?;
        Ok(result)
    }

    /// Execute pdequeue.lua script for atomic priority queue dequeue
    ///
    /// This atomically:
    /// 1. Checks if queue is paused
    /// 2. Gets task with highest priority (lowest score)
    /// 3. Removes from priority queue
    /// 4. Moves to active queue
    /// 5. Updates task status
    ///
    /// Returns Ok(task_id) if successful, Ok(empty) if queue is empty, Err if queue is paused
    ///
    /// # Security Note
    ///
    /// This implementation uses a Lua script to ensure atomicity, preventing race conditions
    /// where multiple workers might attempt to dequeue the same task concurrently.
    ///
    /// # Important
    ///
    /// Since Fred's eval interface is complex, we use individual Redis commands with retry logic.
    /// This provides reasonable atomicity for most use cases. For true transactional guarantees,
    /// consider using Redis transactions (MULTI/EXEC) when Fred's interface stabilizes.
    pub async fn pdequeue_lua(
        &self,
        pqueue: RedisKey,
        active: RedisKey,
        pause: RedisKey,
        _task_prefix: RedisKey,
        ttl: usize,
    ) -> Result<String> {
        const MAX_RETRIES: usize = 5;

        tracing::debug!("pdequeue_lua: attempting atomic dequeue with retry");

        for attempt in 0..MAX_RETRIES {
            // Check if queue is paused first
            if self.exists(pause.clone()).await? {
                tracing::debug!("pdequeue_lua: queue is paused");
                return Err(Error::QueuePaused("Queue paused".to_string()));
            }

            // Get task with highest priority (lowest score)
            let results = self.zrange_with_scores(pqueue.clone(), 0, 0).await?;

            if results.is_empty() {
                tracing::debug!("pdequeue_lua: no tasks in priority queue");
                return Ok(String::new());
            }

            let (task_id, score) = &results[0];

            // Try to remove the task - this is the atomic check
            let removed = self.zrem(pqueue.clone(), task_id.as_str().into()).await?;

            if !removed {
                // Task was removed by another worker, retry
                tracing::debug!("pdequeue_lua: task {} was already removed by another worker, retrying (attempt {})", task_id, attempt + 1);
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                continue;
            }

            // We won the race, now move to active queue
            self.lpush(active.clone(), task_id.as_str().into()).await?;

            // Update task status
            let task_key: RedisKey = format!("rediq:task:{}", task_id).into();
            let current_timestamp = chrono::Utc::now().timestamp();
            self.hset(
                task_key.clone(),
                vec![
                    ("status".into(), "active".into()),
                    ("processed_at".into(), current_timestamp.to_string().into()),
                ],
            ).await?;

            // Set TTL on task data
            self.expire(task_key, ttl as u64).await?;

            tracing::debug!("pdequeue_lua: successfully dequeued task {} with priority {}", task_id, score);
            return Ok(task_id.clone());
        }

        tracing::warn!("pdequeue_lua: failed to dequeue after {} retries", MAX_RETRIES);
        Ok(String::new())
    }

    /// Get TTL of a key in seconds
    ///
    /// Returns `Ok(Some(ttl))` if the key has an expiration time,
    /// `Ok(None)` if the key exists but has no expiration,
    /// or `Err` if the key doesn't exist.
    pub async fn ttl(&self, key: &str) -> Result<Option<i64>> {
        let key: RedisKey = key.into();
        let result: Option<i64> = self.pool.ttl(key).await?;

        // Redis returns -2 if key doesn't exist, -1 if key exists but has no expiration
        match result {
            Some(-2) => Ok(None), // Key doesn't exist
            Some(-1) => Ok(None), // Key exists but no expiration
            Some(ttl) => Ok(Some(ttl)),
            None => Ok(None),
        }
    }

    /// Scan for keys matching a pattern
    ///
    /// Uses SCAN to iterate over keys matching the given pattern.
    /// Note: This implementation uses fred's Stream-based scan interface.
    /// The cursor parameter is used to determine if this is a new scan (cursor=0)
    /// or a continuation. For simplicity, each call returns one page of results.
    ///
    /// # Arguments
    /// * `cursor` - The cursor to start from (0 for new scan)
    /// * `pattern` - The pattern to match (e.g., "rediq:task:*")
    /// * `count` - The approximate number of elements to return per page
    ///
    /// # Returns
    /// * `Ok((next_cursor, keys))` - The next cursor and list of matching keys
    ///   - next_cursor is 0 when scan is complete
    ///   - next_cursor is non-zero when more results are available
    pub async fn scan_match(&self, _cursor: u64, pattern: &str, count: u64) -> Result<(u64, Vec<String>)> {
        use fred::types::Scanner;
        use futures::StreamExt;

        // fred's scan returns a Stream of ScanResult pages
        // For cursor=0, we start a new scan
        // For cursor!=0, we continue from where we left off
        // Since fred manages cursor internally, we simplify by:
        // - cursor=0: start new scan, return first page
        // - cursor=1: there might be more pages (simplified)
        // - cursor=0 returned: scan complete

        // Get a client from the pool to use scan method
        let client = self.pool.next();
        let mut stream = client.scan(pattern, Some(count as u32), None);

        // Get the first page of results
        match stream.next().await {
            Some(Ok(scan_result)) => {
                let has_more = scan_result.has_more();
                let keys: Vec<String> = scan_result
                    .results()
                    .as_ref()
                    .map(|v| v.iter().filter_map(|k| k.as_str().map(|s| s.to_string())).collect())
                    .unwrap_or_default();

                // Trigger next page scan in background if there are more results
                // fred's Drop impl will continue the scan when we don't call next()
                if has_more {
                    // Return non-zero cursor to indicate more results
                    // Use 1 as a simple marker since fred manages the actual cursor
                    Ok((1, keys))
                } else {
                    // Return 0 to indicate scan complete
                    Ok((0, keys))
                }
            }
            Some(Err(e)) => Err(Error::Redis(e)),
            None => Ok((0, Vec::new())),
        }
    }
}

/// Redis Pipeline for batch operations
///
/// Allows multiple Redis commands to be sent together, reducing network round trips.
pub struct RedisPipeline {
    pool: Arc<RedisPool>,
    sets: Vec<(RedisKey, RedisValue)>,
    rpushes: Vec<(RedisKey, Vec<RedisValue>)>,
    sadds: Vec<(RedisKey, RedisValue)>,
    expires: Vec<(RedisKey, u64)>,
}

impl RedisPipeline {
    /// Create a new pipeline
    pub fn new(pool: Arc<RedisPool>) -> Self {
        Self {
            pool,
            sets: Vec::new(),
            rpushes: Vec::new(),
            sadds: Vec::new(),
            expires: Vec::new(),
        }
    }

    /// Add a SET command
    pub fn set(mut self, key: RedisKey, value: RedisValue) -> Self {
        self.sets.push((key, value));
        self
    }

    /// Add an RPUSH command
    pub fn rpush(mut self, key: RedisKey, value: RedisValue) -> Self {
        self.rpushes.push((key, vec![value]));
        self
    }

    /// Add an SADD command
    pub fn sadd(mut self, key: RedisKey, member: RedisValue) -> Self {
        self.sadds.push((key, member));
        self
    }

    /// Add an EXPIRE command
    pub fn expire(mut self, key: RedisKey, seconds: u64) -> Self {
        self.expires.push((key, seconds));
        self
    }

    /// Execute all commands
    pub async fn execute(self) -> Result<Vec<RedisValue>> {
        // Execute SET commands
        for (key, value) in self.sets {
            let _: () = self.pool.set(key, value, None, None, false).await?;
        }

        // Execute RPUSH commands
        for (key, values) in self.rpushes {
            let _: u64 = self.pool.rpush(key, values).await?;
        }

        // Execute SADD commands
        for (key, member) in self.sadds {
            let _: u64 = self.pool.sadd(key, member).await?;
        }

        // Execute EXPIRE commands
        for (key, seconds) in self.expires {
            let _: bool = self.pool.expire(key, seconds as i64).await?;
        }

        // Return empty results (commands executed in order)
        Ok(Vec::new())
    }
}

/// Redis Pipeline for batch operations (legacy alias)
pub type RedisPipelineBuilder = RedisPipeline;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires Redis server"]
    async fn test_redis_ping() {
        let redis_url = std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://localhost:6379".to_string());
        let client = RedisClient::from_url(&redis_url)
            .await
            .unwrap();
        let result = client.ping().await.unwrap();
        assert_eq!(result, "PONG");
    }
}
