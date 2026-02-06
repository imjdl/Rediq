//! Redis client wrapper
//!
//! Provides type-safe Redis operation interfaces.

use crate::Result;
use fred::{
    interfaces::*,
    prelude::*,
    types::{RedisConfig as FredRedisConfig, ReconnectPolicy},
};
use std::sync::Arc;

/// Redis connection configuration
#[derive(Debug, Clone)]
pub struct RedisConfig {
    /// Redis connection URL
    pub url: String,
    /// Connection pool size
    pub pool_size: usize,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            pool_size: 10,
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
            config.pool_size,
        )?;

        pool.init().await?;

        Ok(Self {
            pool: Arc::new(pool),
        })
    }

    /// Create client from connection URL
    pub async fn from_url(url: impl Into<String>) -> Result<Self> {
        let url = url.into();
        let redis_config = FredRedisConfig::from_url(&url)?;
        let pool = RedisPool::new(
            redis_config,
            None,
            None,
            Some(ReconnectPolicy::default()),
            10,
        )?;

        pool.init().await?;

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
            .zrange(key, start, stop, None, true, None, false)
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

    /// Hash operation: set field
    pub async fn hset(&self, key: RedisKey, values: Vec<(RedisKey, RedisValue)>) -> Result<bool> {
        let result: u64 = self.pool.hset(key, values).await?;
        Ok(result > 0)
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
        RedisPipeline {
            pool: self.pool.clone(),
            commands: Vec::new(),
        }
    }

    /// List operation: remove element
    pub async fn lrem(&self, key: RedisKey, value: RedisValue, count: i64) -> Result<u64> {
        let result: u64 = self.pool.lrem(key, count, value).await?;
        Ok(result)
    }
}

/// Redis Pipeline
pub struct RedisPipeline {
    pool: Arc<RedisPool>,
    commands: Vec<String>,
}

impl RedisPipeline {
    /// Add SET command
    pub fn set(mut self, key: RedisKey, value: RedisValue) -> Self {
        self.commands.push(format!("SET {:?} {:?}", key, value));
        self
    }

    /// Add RPUSH command
    pub fn rpush(mut self, key: RedisKey, value: RedisValue) -> Self {
        self.commands.push(format!("RPUSH {:?} {:?}", key, value));
        self
    }

    /// Execute all commands
    pub async fn execute(self) -> Result<Vec<RedisValue>> {
        // TODO: Implement real pipeline execution
        // For now, return empty result
        Ok(Vec::new())
    }
}

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
