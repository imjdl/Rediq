# Troubleshooting Guide

Common issues and solutions for Rediq deployments.

## Table of Contents

- [Connection Issues](#connection-issues)
- [Task Processing Issues](#task-processing-issues)
- [Performance Issues](#performance-issues)
- [Memory Issues](#memory-issues)
- [Worker Issues](#worker-issues)
- [Redis Issues](#redis-issues)
- [Debug Mode](#debug-mode)

---

## Connection Issues

### Cannot Connect to Redis

**Symptoms:**
```
Error: Connection refused
Error: No connection could be made
```

**Solutions:**

1. Verify Redis is running:
```bash
redis-cli ping
# Should return: PONG
```

2. Check connection URL:
```rust
// Correct format
let client = Client::builder()
    .redis_url("redis://127.0.0.1:6379")
    .build()
    .await?;
```

3. Check network connectivity:
```bash
telnet localhost 6379
nc -zv localhost 6379
```

4. Verify firewall rules:
```bash
sudo ufw status
sudo ufw allow 6379/tcp
```

### Connection Timeout

**Symptoms:**
```
Error: Timeout waiting for Redis response
```

**Solutions:**

1. Increase timeout in configuration:
```rust
let state = ServerBuilder::new()
    .dequeue_timeout(10)  // Increase from default 2s
    .build()
    .await?;
```

2. Check Redis server load:
```bash
redis-cli info stats
redis-cli --latency
```

3. Verify network latency:
```bash
ping redis-server
```

### Connection Pool Exhaustion

**Symptoms:**
```
Error: No connections available in pool
```

**Solutions:**

1. Reduce worker concurrency:
```rust
let state = ServerBuilder::new()
    .concurrency(5)  // Reduce from 10
    .build()
    .await?;
```

2. Deploy more worker instances

3. Check for connection leaks:
```bash
redis-cli client list
```

---

## Task Processing Issues

### Tasks Not Being Processed

**Symptoms:**
- Tasks enqueued but never processed
- Queue keeps growing

**Solutions:**

1. Verify worker is running:
```bash
ps aux | grep worker
```

2. Check queue name matches:
```rust
// Producer
let task = Task::builder("email:send")
    .queue("emails")  // Must match worker queue
    .build()?;

// Worker
let state = ServerBuilder::new()
    .queues(&["emails"])  // Must match producer queue
    .build()
    .await?;
```

3. Check if queue is paused:
```bash
rediq queue list
rediq queue resume emails
```

4. Verify handler is registered:
```rust
let mut mux = Mux::new();
mux.handle("email:send", EmailHandler);  // Must match task.task_type()
```

### Tasks Failing Immediately

**Symptoms:**
- Tasks move to dead queue immediately
- All tasks fail with same error

**Solutions:**

1. Check task payload size:
```rust
const MAX_PAYLOAD_SIZE: usize = 512 * 1024;  // 512KB
```

2. Verify task type matches handler:
```rust
// Task type must exactly match handler registration
task.task_type() == "email:send"  // Must be exact
```

3. Check handler error handling:
```rust
#[async_trait]
impl Handler for MyHandler {
    async fn handle(&self, task: &Task) -> Result<()> {
        // Always handle errors, don't panic
        if let Err(e) = process() {
            eprintln!("Error: {}", e);
            return Err(e.into());
        }
        Ok(())
    }
}
```

### Tasks Stuck in Active State

**Symptoms:**
- Tasks in `active:{queue}` but not processing
- Worker appears stuck

**Solutions:**

1. Restart worker gracefully

2. Clear active queue (manual intervention):
```bash
redis-cli LRANGE rediq:active:emails 0 -1
redis-cli LTRIM rediq:active:emails 0 -1
```

3. Check for long-running tasks:
```rust
let task = Task::builder("long:task")
    .timeout(Duration::from_secs(300))  // 5 minute timeout
    .build()?;
```

---

## Performance Issues

### Slow Task Processing

**Symptoms:**
- Tasks taking longer than expected
- Queue backlog growing

**Solutions:**

1. Profile handler performance:
```rust
use std::time::Instant;

#[async_trait]
impl Handler for MyHandler {
    async fn handle(&self, task: &Task) -> Result<()> {
        let start = Instant::now();
        // ... process task ...
        let duration = start.elapsed();
        println!("Task took {:?}", duration);
        Ok(())
    }
}
```

2. Increase worker concurrency:
```rust
let state = ServerBuilder::new()
    .concurrency(20)  // Increase from 10
    .build()
    .await?;
```

3. Deploy more worker instances

4. Check Redis performance:
```bash
redis-cli --latency
redis-cli info stats
```

### High Memory Usage

**Symptoms:**
- Worker memory usage growing
- OOM kills

**Solutions:**

1. Check for large payloads:
```bash
redis-cli STRLEN rediq:task:<task-id>
```

2. Clean up dead letter queue:
```bash
redis-cli LTRIM rediq:dead:emails 0 0
```

3. Reduce task payload size:
```rust
// Store large data externally
let task = Task::builder("process:file")
    .payload(&FileReference {
        path: "/path/to/file".to_string(),
        size: 1024000,
    })?
    .build()?;
```

4. Enable Redis eviction:
```conf
# redis.conf
maxmemory 2gb
maxmemory-policy allkeys-lru
```

### Redis CPU High

**Symptoms:**
- Redis using 100% CPU
- Slow operations

**Solutions:**

1. Use Lua scripts for atomic operations

2. Avoid expensive commands:
```bash
# Avoid: KEYS (blocks Redis)
# Use: SCAN
redis-cli --scan --pattern "rediq:*"
```

3. Monitor slow commands:
```conf
# redis.conf
slowlog-log-slower-than 10000  # Log commands >10ms
slowlog-max-len 128
```

4. Check slow log:
```bash
redis-cli SLOWLOG GET 10
```

---

## Memory Issues

### Redis Out of Memory

**Symptoms:**
```
OOM command not allowed when used memory > 'maxmemory'
```

**Solutions:**

1. Configure Redis maxmemory:
```conf
maxmemory 2gb
maxmemory-policy allkeys-lru
```

2. Clean up old task data:
```bash
# Task data TTL is 24 hours
# Dead letter queue grows indefinitely
redis-cli LTRIM rediq:dead:emails 0 100
```

3. Use shorter TTL for completed tasks

### Worker Out of Memory

**Symptoms:**
- Worker process killed by OOM
- Container restarted

**Solutions:**

1. Increase memory limits:
```yaml
resources:
  limits:
    memory: "1Gi"  # Increase from 512Mi
```

2. Reduce concurrency:
```rust
let state = ServerBuilder::new()
    .concurrency(5)  # Reduce from 10
    .build()
    .await?;
```

3. Profile memory usage:
```bash
# Add to Cargo.toml
# [dependencies]
# memory-stats = "0.1"

# In code
let stats = memory_stats::memory_stats();
println!("Memory: {:?}", stats);
```

---

## Worker Issues

### Worker Not Starting

**Symptoms:**
```
Error: Failed to start worker
```

**Solutions:**

1. Check Redis connectivity first

2. Verify queue configuration:
```rust
let state = ServerBuilder::new()
    .queues(&["default"])  // At least one queue
    .build()
    .await?;
```

3. Check logs for detailed error:
```rust
RUST_LOG=debug cargo run
```

### Worker Keeps Restarting

**Symptoms:**
- Worker exits immediately after starting
- Crash loop

**Solutions:**

1. Check application logs:
```bash
journalctl -u rediq-worker -n 100
```

2. Run in foreground for debugging:
```bash
RUST_BACKTRACE=1 ./worker
```

3. Verify handler doesn't panic:
```rust
#[async_trait]
impl Handler for MyHandler {
    async fn handle(&self, task: &Task) -> Result<()> {
        // Use ? instead of expect()
        let data = task.deserialize_payload::<MyData>()?;
        //               ^ returns error, doesn't panic
        Ok(())
    }
}
```

### Worker Not Processing Tasks

**Symptoms:**
- Worker running but queue growing

**Solutions:**

1. Check if scheduler is enabled:
```rust
let state = ServerBuilder::new()
    // .disable_scheduler()  // Don't disable if using delayed/retry
    .build()
    .await?;
```

2. Verify heartbeat is working:
```bash
redis-cli GET rediq:meta:heartbeat:<worker-id>
```

3. Check worker is in active set:
```bash
redis-cli SMEMBERS rediq:meta:workers
```

---

## Redis Issues

### Redis Connection Limit Reached

**Symptoms:**
```
Error: ERR max number of clients reached
```

**Solutions:**

1. Increase max clients:
```conf
# redis.conf
maxclients 10000
```

2. Reduce number of workers or concurrency

3. Check for connection leaks:
```bash
redis-cli CLIENT LIST | wc -l
```

### Redis Persistence Issues

**Symptoms:**
- Data loss after restart
- Slow performance

**Solutions:**

1. Enable persistence:
```conf
# redis.conf
save 900 1
save 300 10
save 60 10000

appendonly yes
appendfsync everysec
```

2. Check persistence status:
```bash
redis-cli LASTSAVE
redis-cli INFO persistence
```

3. Use AOF for durability:
```conf
appendonly yes
appendfsync always  # Safest but slowest
```

---

## Debug Mode

### Enable Debug Logging

```rust
use tracing_subscriber;

fn init_logging() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();
}
```

Set environment variable:
```bash
export RUST_LOG=debug
export RUST_LOG=rediq=trace  # More verbose
```

### Enable Task Tracing

```rust
use tracing::{info, instrument};

#[async_trait]
impl Handler for MyHandler {
    #[instrument(skip(self))]
    async fn handle(&self, task: &Task) -> Result<()> {
        info!("Processing task: {}", task.id);
        // ... handler logic ...
        Ok(())
    }
}
```

### Query Redis Directly

```bash
# Check queue length
redis-cli LLEN rediq:queue:default

# Check active tasks
redis-cli LRANGE rediq:active:default 0 -1

# Check delayed tasks
redis-cli ZRANGE rediq:delayed:default 0 -1 WITHSCORES

# Check task details
redis-cli HGETALL rediq:task:<task-id>

# Check workers
redis-cli SMEMBERS rediq:meta:workers
```

### Monitor Real-Time Activity

```bash
# Monitor Redis commands
redis-cli MONITOR

# Check Redis stats
redis-cli INFO stats
redis-cli INFO commandstats

# Check queue stats
rediq queue stats default
```

---

## Common Error Messages

| Error | Cause | Solution |
|-------|-------|----------|
| `Connection refused` | Redis not running | Start Redis server |
| `NOAUTH Authentication required` | Redis password required | Add password to URL |
| `Task not found` | Task expired/deleted | Check task TTL |
| `Queue paused` | Queue is paused | Use `queue resume` |
| `Validation: payload cannot be empty` | Empty payload | Set payload before build |
| `Validation: task_type cannot be empty` | Empty task type | Set task_type in builder |
| `Serialization error` | Invalid payload data | Check data types are serializable |
| `Task timed out` | Handler took too long | Increase task timeout |
| `Config: at least one queue must be specified` | No queues configured | Add queues to server builder |

---

## Getting Help

If you're still experiencing issues:

1. Check the [GitHub Issues](https://github.com/example/rediq/issues)
2. Review the [Architecture](architecture.md) documentation
3. Enable debug logging and capture logs
4. Gather Redis statistics: `redis-cli INFO`
5. Include system information: OS, Rust version, Redis version

When reporting issues, please include:
- Rust version: `rustc --version`
- Redis version: `redis-cli --version`
- Rediq version
- Full error message
- Minimal reproduction case
- Relevant logs with `RUST_LOG=debug`
