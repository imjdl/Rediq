# API Reference

Complete API reference for the Rediq task queue framework.

## Table of Contents

- [Client API](#client-api)
- [Task API](#task-api)
- [Server API](#server-api)
- [Handler API](#handler-api)
- [Middleware API](#middleware-api)
- [Error Handling](#error-handling)

---

## Client API

The `Client` is used to enqueue and manage tasks.

### ClientBuilder

```rust
use rediq::Client;
```

#### Methods

| Method | Parameters | Returns | Description |
|--------|------------|---------|-------------|
| `new()` | - | `ClientBuilder` | Create a new client builder |
| `redis_url()` | `url: impl Into<String>` | `Self` | Set Redis connection URL |
| `cluster_mode()` | - | `Self` | Enable Redis Cluster mode |
| `sentinel_mode()` | - | `Self` | Enable Redis Sentinel mode |
| `build()` | - | `impl Future<Output = Result<Client>>` | Build the client |

#### Example

```rust
let client = Client::builder()
    .redis_url("redis://localhost:6379")
    .build()
    .await?;
```

### Client

#### Methods

| Method | Parameters | Returns | Description |
|--------|------------|---------|-------------|
| `enqueue()` | `task: Task` | `impl Future<Output = Result<String>>` | Enqueue a single task |
| `enqueue_batch()` | `tasks: Vec<Task>` | `impl Future<Output = Result<Vec<String>>>` | Enqueue multiple tasks |
| `get_task()` | `id: &str` | `impl Future<Output = Result<Option<TaskInfo>>>` | Get task information |
| `get_task_progress()` | `id: &str` | `impl Future<Output = Result<Option<TaskProgress>>>` | Get task progress |
| `get_tasks()` | `queue: &str, start, size` | `impl Future<Output = Result<Vec<TaskInfo>>>` | List tasks in queue |
| `cancel_task()` | `id: &str` | `impl Future<Output = Result<bool>>` | Cancel a pending task |
| `inspector()` | - | `Inspector` | Get task inspector |
| `flush_queue()` | `queue: &str` | `impl Future<Output = Result<()>>` | Flush all tasks from queue |

#### Example

```rust
let task_id = client.enqueue(task).await?;
let info = client.get_task(&task_id).await?;
```

---

## Task API

### TaskBuilder

```rust
use rediq::Task;
use std::time::Duration;
```

#### Methods

| Method | Parameters | Returns | Description |
|--------|------------|---------|-------------|
| `new()` | `task_type: impl Into<String>` | `TaskBuilder` | Create a new task builder |
| `queue()` | `queue: impl Into<String>` | `Self` | Set target queue name |
| `payload()` | `payload: &T` (Serializable) | `Result<Self>` | Set task payload |
| `raw_payload()` | `payload: Vec<u8>` | `Self` | Set raw binary payload |
| `max_retry()` | `max: u32` | `Self` | Set maximum retry count |
| `timeout()` | `timeout: Duration` | `Self` | Set task timeout |
| `delay()` | `delay: Duration` | `Self` | Set execution delay |
| `cron()` | `cron: impl Into<String>` | `Self` | Set cron expression |
| `unique_key()` | `key: impl Into<String>` | `Self` | Set deduplication key |
| `priority()` | `priority: i32` | `Self` | Set priority (0-100) |
| `depends_on()` | `deps: &[&str]` | `Self` | Set task dependencies |
| `build()` | - | `Result<Task>` | Build the task |

#### Priority Values

| Value | Priority |
|-------|----------|
| 0 | Highest |
| 1-49 | High |
| 50 | Default |
| 51-99 | Low |
| 100 | Lowest |

#### Example

```rust
let task = Task::builder("email:send")
    .queue("emails")
    .payload(&data)?
    .max_retry(5)
    .timeout(Duration::from_secs(30))
    .priority(10)
    .build()?;
```

### Task

#### Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `id()` | `&str` | Task unique ID |
| `task_type()` | `&str` | Task type for routing |
| `queue()` | `&str` | Target queue name |
| `description()` | `String` | Human-readable description |
| `can_retry()` | `bool` | Check if task can be retried |
| `retry_delay()` | `Option<Duration>` | Get retry delay duration |
| `deserialize_payload()` | `Result<T>` | Deserialize payload to type T |

---

## Server API

### ServerBuilder

```rust
use rediq::server::ServerBuilder;
```

#### Methods

| Method | Parameters | Returns | Description |
|--------|------------|---------|-------------|
| `new()` | - | `ServerBuilder` | Create a new server builder |
| `redis_url()` | `url: impl Into<String>` | `Self` | Set Redis connection URL |
| `cluster_mode()` | - | `Self` | Enable Redis Cluster mode |
| `sentinel_mode()` | - | `Self` | Enable Redis Sentinel mode |
| `queues()` | `queues: &[&str]` | `Self` | Set queues to consume from |
| `concurrency()` | `n: usize` | `Self` | Set worker concurrency |
| `heartbeat_interval()` | `seconds: u64` | `Self` | Set heartbeat interval |
| `worker_timeout()` | `seconds: u64` | `Self` | Set worker timeout |
| `dequeue_timeout()` | `seconds: u64` | `Self` | Set dequeue timeout |
| `poll_interval()` | `ms: u64` | `Self` | Set poll interval |
| `server_name()` | `name: impl Into<String>` | `Self` | Set server name |
| `middleware()` | `middleware: M` | `Self` | Add middleware |
| `disable_scheduler()` | - | `Self` | Disable built-in scheduler |
| `build()` | - | `impl Future<Output = Result<ServerState>>` | Build server state |

#### Example

```rust
let state = ServerBuilder::new()
    .redis_url("redis://localhost:6379")
    .queues(&["default", "critical"])
    .concurrency(10)
    .heartbeat_interval(5)
    .build()
    .await?;
```

### Server

#### Methods

| Method | Parameters | Returns | Description |
|--------|------------|---------|-------------|
| `from()` | `state: ServerState` | `Server` | Create server from state |
| `run()` | `mux: Mux` | `impl Future<Output = Result<()>>` | Run the server |
| `enable_metrics()` | `addr: SocketAddr` | `Server` | Enable HTTP metrics endpoint |
| `shutdown()` | - | `()` | Request graceful shutdown |

#### Example

```rust
let server = Server::from(state);
server.enable_metrics("0.0.0.0:9090".parse()?);
server.run(mux).await?;
```

---

## Handler API

### Handler Trait

```rust
use rediq::processor::Handler;
use async_trait::async_trait;
```

#### Definition

```rust
#[async_trait]
pub trait Handler: Send + Sync {
    async fn handle(&self, task: &Task) -> Result<()>;
}
```

#### Implementing a Handler

```rust
#[async_trait]
impl Handler for MyHandler {
    async fn handle(&self, task: &Task) -> Result<()> {
        // Process the task
        Ok(())
    }
}
```

### Mux (Router)

```rust
use rediq::processor::Mux;
```

#### Methods

| Method | Parameters | Returns | Description |
|--------|------------|---------|-------------|
| `new()` | - | `Mux` | Create a new router |
| `handle()` | `task_type: &str, handler: H` | `()` | Register handler for task type |
| `default_handler()` | `handler: H` | `()` | Set default handler |
| `process()` | `task: &Task` | `impl Future<Output = Result<()>>` | Process a task |

#### Example

```rust
let mut mux = Mux::new();
mux.handle("email:send", EmailHandler);
mux.handle("sms:send", SmsHandler);
mux.default_handler(FallbackHandler);
```

---

## Middleware API

### Middleware Trait

```rust
use rediq::middleware::Middleware;
use async_trait::async_trait;
```

#### Definition

```rust
#[async_trait]
pub trait Middleware: Send + Sync {
    async fn before(&self, task: &Task) -> Result<()> {
        Ok(())
    }

    async fn after(&self, task: &Task, result: &Result<()>) -> Result<()> {
        Ok(())
    }
}
```

### MiddlewareChain

```rust
use rediq::middleware::MiddlewareChain;
```

#### Methods

| Method | Parameters | Returns | Description |
|--------|------------|---------|-------------|
| `new()` | - | `MiddlewareChain` | Create a new chain |
| `add()` | `middleware: M` | `Self` | Add middleware to chain |

#### Built-in Middleware

##### LoggingMiddleware

```rust
use rediq::middleware::LoggingMiddleware;

let middleware = LoggingMiddleware::new()
    .with_details();  // Enable detailed logging
```

##### MetricsMiddleware

```rust
use rediq::middleware::MetricsMiddleware;

let middleware = MetricsMiddleware::new();
```

#### Custom Middleware Example

```rust
use rediq::middleware::Middleware;
use async_trait::async_trait;

struct TimingMiddleware;

#[async_trait]
impl Middleware for TimingMiddleware {
    async fn before(&self, task: &Task) -> Result<()> {
        let start = std::time::Instant::now();
        // Store start time in task extensions or context
        Ok(())
    }

    async fn after(&self, task: &Task, result: &Result<()>) -> Result<()> {
        // Calculate and log duration
        Ok(())
    }
}
```

---

## Error Handling

### Error Type

```rust
use rediq::{Error, Result};
```

#### Error Variants

| Variant | Description |
|---------|-------------|
| `Redis(String)` | Redis operation error |
| `Serialization(String)` | Serialization/deserialization error |
| `Validation(String)` | Input validation error |
| `Config(String)` | Configuration error |
| `QueuePaused(String)` | Queue is paused |
| `TaskNotFound(String)` | Task not found |
| `Shutdown` | System is shutting down |
| `Timeout(String)` | Operation timeout |

### Result Type

```rust
pub type Result<T> = std::result::Result<T, Error>;
```

### Error Handling Example

```rust
async fn process_task(task: &Task) -> Result<()> {
    match task.deserialize_payload::<MyData>() {
        Ok(data) => {
            // Process data
            Ok(())
        }
        Err(e) => {
            // Handle error
            Err(Error::Validation(format!("Invalid payload: {}", e)))
        }
    }
}
```

---

## Progress Tracking API

### TaskProgressExt Trait

Extension trait for tasks to access progress reporting during execution.

```rust
use rediq::task::TaskProgressExt;
```

#### Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `progress()` | `Option<ProgressContext>` | Get progress reporter for this task |

#### Example

```rust
#[async_trait]
impl Handler for MyHandler {
    async fn handle(&self, task: &Task) -> Result<()> {
        if let Some(progress) = task.progress() {
            progress.report(50).await?;
            progress.report_with_message(75, Some("Processing...")).await?;
        }
        Ok(())
    }
}
```

### ProgressContext

Progress reporter for task handlers.

#### Methods

| Method | Parameters | Returns | Description |
|--------|------------|---------|-------------|
| `report()` | `current: u32` | `Result<()>` | Report progress (0-100) |
| `report_with_message()` | `current: u32, message: Option<&str>` | `Result<()>` | Report progress with message |
| `increment()` | `delta: u32` | `Result<()>` | Increase progress by delta |
| `current()` | - | `u32` | Get current progress value |

#### Example

```rust
// Report percentage
progress.report(50).await?;

// Report with message
progress.report_with_message(75, Some("Processing file")).await?;

// Increment by 10
progress.increment(10).await?;
```

### TaskProgress

Task progress information structure.

#### Fields

| Field | Type | Description |
|-------|------|-------------|
| `current` | `u32` | Current progress (0-100) |
| `message` | `Option<String>` | Optional progress message |
| `updated_at` | `i64` | Last update timestamp (Unix) |

---

## Utility Types

### TaskStatus

```rust
pub enum TaskStatus {
    Pending,   // Task is waiting to be processed
    Active,    // Task is currently being processed
    Processed, // Task completed successfully
    Retry,     // Task is scheduled for retry
    Dead,      // Task exceeded max retries
}
```

### RedisMode

```rust
pub enum RedisMode {
    Standalone,  // Single Redis instance
    Cluster,     // Redis Cluster
    Sentinel,    // Redis Sentinel
}
```

---

## Type Aliases

```rust
use rediq::{Client, Server, ServerBuilder, Task, Handler, Mux};
```

| Alias | Type |
|-------|------|
| `Client` | Client implementation |
| `Server` | Server implementation |
| `ServerBuilder` | Server builder |
| `ServerState` | Shared server state |
| `Task` | Task model |
| `TaskBuilder` | Task builder |
| `Handler` | Handler trait |
| `Mux` | Task router |
