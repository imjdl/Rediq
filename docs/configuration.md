# Configuration Guide

Complete guide for configuring Rediq servers and clients.

## Table of Contents

- [Global Configuration](#global-configuration)
- [Client Configuration](#client-configuration)
- [Server Configuration](#server-configuration)
- [Task Configuration](#task-configuration)
- [Redis Configuration](#redis-configuration)
- [Environment Variables](#environment-variables)
- [Configuration Examples](#configuration-examples)

---

## Global Configuration

Rediq provides a global configuration system for system-wide settings that affect task processing behavior.

### Configuration Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `max_payload_size` | `usize` | 512KB | Maximum task payload size in bytes |
| `task_ttl` | `u64` | 86400 (24h) | Default TTL for task details stored in Redis |
| `priority_range` | `(i32, i32)` | (0, 100) | Valid priority range (min, max) |
| `default_max_retry` | `u32` | 3 | Default maximum retry count for tasks |
| `default_timeout_secs` | `u64` | 30 | Default task timeout in seconds |

### Setting Global Configuration

```rust
use rediq::config::{set_config, update_config, RediqConfig};

// Method 1: Create and set a complete configuration
let config = RediqConfig::new()
    .with_max_payload_size(1024 * 1024)  // 1MB
    .with_task_ttl(3600)                  // 1 hour
    .with_priority_range(1, 10)           // 1-10 range
    .with_default_max_retry(5)            // 5 retries
    .with_default_timeout(60);            // 60 seconds

set_config(config);
```

### Updating Existing Configuration

```rust
// Method 2: Update specific values (preserves other settings)
update_config(|config| {
    config
        .with_max_payload_size(2 * 1024 * 1024)  // 2MB
        .with_task_ttl(7200)                       // 2 hours
});
```

### Reading Configuration

```rust
use rediq::config::{
    get_config, get_max_payload_size, get_task_ttl,
    get_priority_range, get_default_max_retry, get_default_timeout_secs
};

// Get full configuration
let config = get_config();
println!("Max payload: {} bytes", config.max_payload_size);

// Or use convenience getters
let max_size = get_max_payload_size();
let ttl = get_task_ttl();
let (min_prio, max_prio) = get_priority_range();
let max_retry = get_default_max_retry();
let timeout = get_default_timeout_secs();
```

### Priority Validation

The global configuration provides priority validation:

```rust
use rediq::config::get_config;

let config = get_config();

// Validate a priority value
match config.validate_priority(50) {
    Ok(_) => println!("Priority is valid"),
    Err(e) => println!("Invalid priority: {}", e),
}

// Clamp a value to the valid range
let clamped = config.clamp_priority(150);  // Returns 100 for default range
```

### When to Use Global Configuration

Global configuration is useful when you need to:

1. **Increase payload limits** for tasks with large data
2. **Adjust task TTL** for longer/shorter retention
3. **Customize priority ranges** for specific use cases
4. **Set default retry/timeout** behavior across all tasks

### Example: Large Payload Configuration

```rust
use rediq::config::set_config;
use rediq::config::RediqConfig;

// Configure for handling large payloads (e.g., file processing)
set_config(RediqConfig::new()
    .with_max_payload_size(10 * 1024 * 1024)  // 10MB
    .with_task_ttl(7 * 86400)                // 7 days
    .with_default_timeout(300)                // 5 minutes
);

// Now all tasks can use these settings
let large_task = Task::builder("file:process")
    .payload(&large_data)  // Up to 10MB
    .build()?;
```

---

## Client Configuration

### Basic Configuration

```rust
use rediq::Client;

let client = Client::builder()
    .redis_url("redis://localhost:6379")
    .build()
    .await?;
```

### Redis Cluster Mode

```rust
let client = Client::builder()
    .redis_url("redis://cluster-node1:6379")
    .cluster_mode()
    .build()
    .await?;
```

### Redis Sentinel Mode

```rust
let client = Client::builder()
    .redis_url("redis://sentinel1:26379")
    .sentinel_mode()
    .build()
    .await?;
```

### Client Configuration Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `redis_url` | `String` | `redis://localhost:6379` | Redis connection URL |
| `redis_mode` | `RedisMode` | `Standalone` | Connection mode |
| `pool_size` | `usize` | `10` | Connection pool size |
| `min_idle` | `Option<usize>` | `None` | Minimum idle connections |
| `connection_timeout` | `Option<u64>` | `None` | Connection timeout (seconds) |
| `idle_timeout` | `Option<u64>` | `None` | Idle timeout (seconds) |
| `max_lifetime` | `Option<u64>` | `None` | Maximum connection lifetime (seconds) |

### Connection Pool Configuration

```rust
use std::time::Duration;

let client = Client::builder()
    .redis_url("redis://localhost:6379")
    .pool_size(20)                              // Maximum pool size
    .min_idle(5)                                // Minimum idle connections
    .connection_timeout(Duration::from_secs(30)) // Connection timeout
    .idle_timeout(Duration::from_secs(600))      // Idle timeout (10 minutes)
    .max_lifetime(Duration::from_secs(1800))     // Max lifetime (30 minutes)
    .build()
    .await?;
```

---

## Server Configuration

### Basic Configuration

```rust
use rediq::server::ServerBuilder;

let state = ServerBuilder::new()
    .redis_url("redis://localhost:6379")
    .queues(&["default"])
    .concurrency(10)
    .build()
    .await?;
```

### Server Configuration Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `redis_url` | `String` | `redis://localhost:6379` | Redis connection URL |
| `redis_mode` | `RedisMode` | `Standalone` | Connection mode |
| `pool_size` | `usize` | `10` | Connection pool size |
| `min_idle` | `Option<usize>` | `None` | Minimum idle connections |
| `connection_timeout` | `Option<u64>` | `None` | Connection timeout (seconds) |
| `idle_timeout` | `Option<u64>` | `None` | Idle timeout (seconds) |
| `max_lifetime` | `Option<u64>` | `None` | Maximum connection lifetime (seconds) |
| `queues` | `Vec<String>` | `["default"]` | Queues to consume from |
| `concurrency` | `usize` | `10` | Number of concurrent workers |
| `heartbeat_interval` | `u64` | `5` | Worker heartbeat interval (seconds) |
| `worker_timeout` | `u64` | `30` | Worker timeout (seconds) |
| `dequeue_timeout` | `u64` | `2` | BLPOP timeout (seconds) |
| `poll_interval` | `u64` | `100` | Queue poll interval (milliseconds) |
| `server_name` | `String` | Auto-generated | Server identifier |
| `enable_scheduler` | `bool` | `true` | Enable built-in scheduler |
| `aggregator_config` | `Option<AggregatorConfig>` | `None` | Task aggregation configuration |
| `janitor_config` | `Option<JanitorConfig>` | `None` | Janitor cleanup configuration |

### Concurrency Guidelines

| Scenario | Recommended Concurrency |
|----------|------------------------|
| CPU-intensive tasks | Number of CPU cores |
| I/O-intensive tasks | 2-3x CPU cores |
| Mixed workload | 1.5x CPU cores |
| Low priority tasks | 5-10 per worker |

### Timeout Configuration

```rust
let state = ServerBuilder::new()
    .redis_url("redis://localhost:6379")
    .heartbeat_interval(10)    // Heartbeat every 10s
    .worker_timeout(60)        // Worker timeout after 60s
    .dequeue_timeout(5)        // Wait 5s for tasks
    .poll_interval(500)        // Poll every 500ms when empty
    .build()
    .await?;
```

### Middleware Configuration

```rust
use rediq::middleware::{MiddlewareChain, LoggingMiddleware, MetricsMiddleware};
use rediq::observability::RediqMetrics;

let metrics = RediqMetrics::new();
let middleware = MiddlewareChain::new()
    .add(LoggingMiddleware::new().with_details())
    .add(MetricsMiddleware::new());

let state = ServerBuilder::new()
    .redis_url("redis://localhost:6379")
    .middleware(middleware)
    .build()
    .await?;
```

### Metrics HTTP Endpoint

```rust
use rediq::Server;

let server = Server::from(state);
server.enable_metrics("0.0.0.0:9090".parse()?);
```

Access metrics at: `http://localhost:9090/metrics`

### Task Aggregation Configuration

Task aggregation allows grouping tasks for batch processing. This is useful for scenarios like:
- Batch email notifications
- Bulk data processing
- Aggregated reporting

```rust
use rediq::server::ServerBuilder;
use rediq::aggregator::AggregatorConfig;
use std::time::Duration;

let state = ServerBuilder::new()
    .redis_url("redis://localhost:6379")
    .aggregator_config(AggregatorConfig::new()
        .max_size(20)                          // Max tasks per group
        .grace_period(Duration::from_secs(60)) // Wait time for partial groups
        .max_delay(Duration::from_secs(300)))  // Max delay from first task
    .build()
    .await?;
```

#### Aggregation Configuration Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `max_size` | `usize` | `10` | Maximum tasks before aggregation triggers |
| `grace_period` | `Duration` | `30s` | Wait time for partial groups |
| `max_delay` | `Duration` | `300s` | Maximum delay from first task |

#### Using Task Groups

```rust
// Create tasks with group assignment
let task = Task::builder("notification:send")
    .payload(&user_data)?
    .group("daily_notifications")  // Assign to group
    .build()?;

client.enqueue(task).await?;
```

### Janitor Configuration

The Janitor automatically cleans up expired task details to prevent Redis memory leaks.

```rust
use rediq::server::{ServerBuilder, JanitorConfig};
use std::time::Duration;

let state = ServerBuilder::new()
    .redis_url("redis://localhost:6379")
    .janitor_config(JanitorConfig::new()
        .interval(Duration::from_secs(60))   // Run every 60 seconds
        .batch_size(100)                     // Delete up to 100 keys per run
        .ttl_threshold(60))                  // Delete keys with TTL < 60s
    .build()
    .await?;
```

#### Janitor Configuration Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `interval` | `Duration` | `60s` | Time between cleanup runs |
| `batch_size` | `usize` | `100` | Maximum keys to delete per run |
| `ttl_threshold` | `i64` | `60` | TTL threshold in seconds |

---

## Task Configuration

### Task Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `queue` | `String` | `"default"` | Target queue name |
| `max_retry` | `u32` | `3` | Maximum retry count |
| `timeout` | `Duration` | `30s` | Task timeout |
| `delay` | `Duration` | `None` | Execution delay |
| `priority` | `i32` | `50` | Priority (0-100) |
| `unique_key` | `String` | `None` | Deduplication key |
| `cron` | `String` | `None` | Cron expression |
| `depends_on` | `Vec<String>` | `None` | Task dependencies |

### Priority Configuration

```rust
// Highest priority
let urgent = Task::builder("urgent:task")
    .priority(0)
    .build()?;

// High priority
let high = Task::builder("high:task")
    .priority(25)
    .build()?;

// Default priority
let normal = Task::builder("normal:task")
    .build()?;

// Low priority
let low = Task::builder("low:task")
    .priority(75)
    .build()?;

// Lowest priority
let bulk = Task::builder("bulk:task")
    .priority(100)
    .build()?;
```

### Retry Configuration

```rust
use std::time::Duration;

let task = Task::builder("external:api")
    .max_retry(5)              // Retry up to 5 times
    .timeout(Duration::from_secs(30))
    .build()?;
```

**Retry Schedule:**

| Retry | Delay |
|-------|-------|
| 1 | 2 seconds |
| 2 | 4 seconds |
| 3 | 8 seconds |
| 4 | 16 seconds |
| 5+ | 60 seconds |

### Delay Configuration

```rust
use std::time::Duration;

// Execute after 5 minutes
let delayed = Task::builder("delayed:task")
    .delay(Duration::from_secs(300))
    .build()?;
```

### Cron Configuration

```rust
// Daily at 2 AM
let daily = Task::builder("daily:task")
    .cron("0 2 * * *")
    .build()?;

// Every hour
let hourly = Task::builder("hourly:task")
    .cron("0 * * * *")
    .build()?;

// Every 30 minutes
let half_hourly = Task::builder("half-hourly:task")
    .cron("*/30 * * * *")
    .build()?;

// Weekdays at 9 AM
let workday = Task::builder("workday:task")
    .cron("0 9 * * 1-5")
    .build()?;
```

### Task Dependencies

```rust
let task_b = Task::builder("process:b")
    .depends_on(&["task-a-id"])
    .build()?;

// Multiple dependencies
let task_c = Task::builder("process:c")
    .depends_on(&["task-a-id", "task-b-id"])
    .build()?;
```

### Unique Tasks (Deduplication)

```rust
// Prevent duplicate tasks
let task = Task::builder("user:update")
    .unique_key("user:123:profile")
    .build()?;

// If another task with the same unique_key is already pending,
// the new task will be rejected.
```

---

## Redis Configuration

### Connection URL Formats

#### Standalone

```
redis://[[username:]password@]host[:port][/db_number]
redis://localhost:6379
redis://user:pass@localhost:6379/0
rediss://localhost:6380  // TLS connection
```

#### Cluster

```
redis://cluster-node1:6379
redis://user:pass@cluster-node1:6379
```

#### Sentinel

```
redis://sentinel1:26379
redis://sentinel1:26379,sentinel2:26379
```

### Redis Configuration Recommendations

#### Memory Management

```conf
# redis.conf
maxmemory 2gb
maxmemory-policy allkeys-lru
```

#### Persistence

```conf
# RDB snapshots
save 900 1
save 300 10
save 60 10000

# AOF (Append Only File)
appendonly yes
appendfsync everysec
```

#### Eviction Policy

| Policy | Description |
|--------|-------------|
| `noeviction` | Don't evict, return errors |
| `allkeys-lru` | LRU for all keys |
| `volatile-lru` | LRU for keys with TTL |
| `allkeys-random` | Random for all keys |
| `volatile-random` | Random for keys with TTL |

### Redis Key TTL

| Key Pattern | TTL |
|-------------|-----|
| `task:{id}` | 24 hours (86400s) |
| `active:{name}` | No TTL |
| `queue:{name}` | No TTL |
| `dead:{name}` | No TTL (manual cleanup) |
| `meta:heartbeat:{id}` | 2x heartbeat interval |

---

## Environment Variables

### Using Environment Variables

```rust
let redis_url = std::env::var("REDIS_URL")
    .unwrap_or_else(|_| "redis://localhost:6379".to_string());

let client = Client::builder()
    .redis_url(&redis_url)
    .build()
    .await?;
```

### Common Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `REDIS_URL` | Redis connection URL | `redis://localhost:6379` |
| `REDIS_PASSWORD` | Redis password | - |
| `SERVER_CONCURRENCY` | Worker concurrency | `10` |
| `SERVER_QUEUES` | Queues to consume | `default` |
| `HEARTBEAT_INTERVAL` | Heartbeat interval (s) | `5` |
| `WORKER_TIMEOUT` | Worker timeout (s) | `30` |

### `.env` File Example

```bash
# .env
REDIS_URL=redis://127.0.0.1:6379
REDIS_PASSWORD=your_password_here
SERVER_CONCURRENCY=20
SERVER_QUEUES=default,critical,high
HEARTBEAT_INTERVAL=10
WORKER_TIMEOUT=60
```

Loading with dotenv:

```rust
// Add to Cargo.toml:
// dotenv = "0.15"

dotenv::dotenv().ok();

let redis_url = std::env::var("REDIS_URL")
    .expect("REDIS_URL must be set");
```

---

## Configuration Examples

### Development Environment

```rust
let state = ServerBuilder::new()
    .redis_url("redis://localhost:6379")
    .queues(&["default"])
    .concurrency(5)
    .heartbeat_interval(5)
    .build()
    .await?;
```

### Production Environment

```rust
let state = ServerBuilder::new()
    .redis_url("redis://redis-cluster:6379")
    .cluster_mode()
    .queues(&["critical", "high", "default", "low"])
    .concurrency(50)
    .heartbeat_interval(10)
    .worker_timeout(120)
    .dequeue_timeout(5)
    .poll_interval(200)
    .server_name("production-worker-1")
    .build()
    .await?;

let server = Server::from(state);
server.enable_metrics("0.0.0.0:9090".parse()?);
```

### Testing Environment

```rust
let state = ServerBuilder::new()
    .redis_url("redis://localhost:6379/15")  // Use DB 15 for tests
    .queues(&["test"])
    .concurrency(2)
    .disable_scheduler()
    .build()
    .await?;
```

### High-Throughput Configuration

```rust
// Redis Cluster with many workers
let state = ServerBuilder::new()
    .redis_url("redis://cluster-node:6379")
    .cluster_mode()
    .queues(&["default"])
    .concurrency(100)
    .heartbeat_interval(15)
    .dequeue_timeout(1)
    .poll_interval(50)
    .build()
    .await?;
```

### Low-Latency Configuration

```rust
let state = ServerBuilder::new()
    .redis_url("redis://localhost:6379")
    .queues(&["urgent", "critical"])
    .concurrency(20)
    .heartbeat_interval(5)
    .dequeue_timeout(1)
    .poll_interval(10)
    .build()
    .await?;
```

### Multi-Queue Configuration

```rust
let state = ServerBuilder::new()
    .redis_url("redis://localhost:6379")
    .queues(&[
        "urgent",      // Highest priority queue
        "high",        // High priority queue
        "default",     // Default queue
        "bulk",        // Low priority queue
    ])
    .concurrency(20)
    .build()
    .await?;

// Workers will poll queues in round-robin order
```

### Complete Production Example

```rust
use rediq::server::{Server, ServerBuilder};
use rediq::processor::Mux;
use rediq::middleware::{MiddlewareChain, LoggingMiddleware, MetricsMiddleware};
use rediq::observability::RediqMetrics;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration from environment
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());

    let queues: Vec<String> = std::env::var("SERVER_QUEUES")
        .unwrap_or_else(|_| "default".to_string())
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    let concurrency: usize = std::env::var("SERVER_CONCURRENCY")
        .unwrap_or_else(|_| "10".to_string())
        .parse()
        .unwrap_or(10);

    // Setup middleware
    let metrics = RediqMetrics::new();
    let middleware = MiddlewareChain::new()
        .add(LoggingMiddleware::new().with_details())
        .add(MetricsMiddleware::new());

    // Build server
    let state = ServerBuilder::new()
        .redis_url(&redis_url)
        .queues(&queues.iter().map(|s| s.as_str()).collect::<Vec<_>>())
        .concurrency(concurrency)
        .heartbeat_interval(10)
        .worker_timeout(60)
        .middleware(middleware)
        .build()
        .await?;

    // Create and configure server
    let server = Server::from(state);
    server.enable_metrics("0.0.0.0:9090".parse()?);

    // Setup handlers
    let mut mux = Mux::new();
    // Register your handlers here...

    // Run server
    server.run(mux).await?;

    Ok(())
}
```
