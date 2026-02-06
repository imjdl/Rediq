# Configuration Guide

Complete guide for configuring Rediq servers and clients.

## Table of Contents

- [Client Configuration](#client-configuration)
- [Server Configuration](#server-configuration)
- [Task Configuration](#task-configuration)
- [Redis Configuration](#redis-configuration)
- [Environment Variables](#environment-variables)
- [Configuration Examples](#configuration-examples)

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
| `queues` | `Vec<String>` | `["default"]` | Queues to consume from |
| `concurrency` | `usize` | `10` | Number of concurrent workers |
| `heartbeat_interval` | `u64` | `5` | Worker heartbeat interval (seconds) |
| `worker_timeout` | `u64` | `30` | Worker timeout (seconds) |
| `dequeue_timeout` | `u64` | `2` | BLPOP timeout (seconds) |
| `poll_interval` | `u64` | `100` | Queue poll interval (milliseconds) |
| `server_name` | `String` | Auto-generated | Server identifier |
| `enable_scheduler` | `bool` | `true` | Enable built-in scheduler |

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
