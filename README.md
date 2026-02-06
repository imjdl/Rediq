# Rediq

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

**Rediq** is a distributed task queue framework for Rust, inspired by [Asynq](https://github.com/hibiken/asynq). It provides a robust, production-ready solution for background task processing with Redis as the backend.

## Features

- **Multiple Queue Support** - Process tasks from multiple queues concurrently
- **Priority Queues** - Support for prioritized task execution (0-100, lower = higher priority)
- **Delayed Tasks** - Schedule tasks to run after a specified delay
- **Periodic Tasks** - Cron-style periodic task scheduling
- **Automatic Retry** - Configurable retry mechanism with exponential backoff
- **Dead Letter Queue** - Failed tasks are moved to a dead letter queue
- **Task Dependencies** - Define task execution dependencies
- **Middleware System** - Built-in middleware for logging, metrics, and custom hooks
- **Prometheus Metrics** - Built-in observability with optional HTTP endpoint
- **Redis HA** - Support for Redis Cluster and Sentinel

## Quick Start

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
rediq = "0.1"
tokio = { version = "1", features = ["full"] }
```

### Basic Usage

#### Producer (Enqueue Tasks)

```rust
use rediq::Client;
use rediq::Task;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct EmailPayload {
    to: String,
    subject: String,
    body: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client
    let client = Client::builder()
        .redis_url("redis://localhost:6379")
        .build()
        .await?;

    // Create and enqueue a task
    let task = Task::builder("email:send")
        .queue("default")
        .payload(&EmailPayload {
            to: "user@example.com".to_string(),
            subject: "Welcome".to_string(),
            body: "Hello!".to_string(),
        })?
        .max_retry(5)
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    client.enqueue(task).await?;

    Ok(())
}
```

#### Consumer (Process Tasks)

```rust
use rediq::server::{Server, ServerBuilder};
use rediq::processor::{Handler, Mux};
use async_trait::async_trait;
use rediq::Task;

struct EmailHandler;

#[async_trait]
impl Handler for EmailHandler {
    async fn handle(&self, task: &Task) -> rediq::Result<()> {
        // Process the task
        println!("Processing task: {}", task.id);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Build server state
    let state = ServerBuilder::new()
        .redis_url("redis://localhost:6379")
        .queues(&["default", "critical"])
        .concurrency(10)
        .build()
        .await?;

    // Create server
    let server = Server::from(state);

    // Register handlers
    let mut mux = Mux::new();
    mux.handle("email:send", EmailHandler);

    // Run server
    server.run(mux).await?;

    Ok(())
}
```

## Advanced Features

### Priority Queues

```rust
let task = Task::builder("critical:task")
    .queue("default")
    .priority(0)  // Highest priority
    .payload(&data)?
    .build()?;
```

### Delayed Tasks

```rust
use std::time::Duration;

let task = Task::builder("delayed:task")
    .queue("default")
    .delay(Duration::from_secs(300))  // Run after 5 minutes
    .payload(&data)?
    .build()?;
```

### Periodic Tasks (Cron)

```rust
let task = Task::builder("scheduled:backup")
    .queue("default")
    .cron("0 2 * * *")  // Run at 2 AM daily
    .payload(&data)?
    .build()?;
```

### Task Dependencies

```rust
let task_b = Task::builder("process:b")
    .queue("default")
    .depends_on(&["task-a-id-123"])
    .payload(&data)?
    .build()?;
```

### Middleware

```rust
use rediq::middleware::{MiddlewareChain, LoggingMiddleware};
use rediq::server::ServerBuilder;

let middleware = MiddlewareChain::new()
    .add(LoggingMiddleware::new().with_details());

let state = ServerBuilder::new()
    .redis_url("redis://localhost:6379")
    .middleware(middleware)
    .build()
    .await?;
```

### Redis Cluster

```rust
let client = Client::builder()
    .redis_url("redis://cluster-node:6379")
    .cluster_mode()
    .build()
    .await?;
```

### Redis Sentinel

```rust
let client = Client::builder()
    .redis_url("redis://sentinel:26379")
    .sentinel_mode()
    .build()
    .await?;
```

## Configuration

### Server Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `redis_url` | `String` | `redis://localhost:6379` | Redis connection URL |
| `queues` | `Vec<String>` | `["default"]` | Queues to consume from |
| `concurrency` | `usize` | `10` | Number of concurrent workers |
| `heartbeat_interval` | `u64` | `5` | Worker heartbeat interval (seconds) |
| `worker_timeout` | `u64` | `30` | Worker timeout (seconds) |
| `dequeue_timeout` | `u64` | `2` | BLPOP timeout (seconds) |
| `poll_interval` | `u64` | `100` | Queue poll interval (milliseconds) |

### Task Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `queue` | `String` | `"default"` | Target queue |
| `max_retry` | `u32` | `3` | Maximum retry count |
| `timeout` | `Duration` | `30s` | Task timeout |
| `delay` | `Duration` | `None` | Delay before execution |
| `priority` | `i32` | `50` | Priority (0-100, lower = higher) |
| `unique_key` | `String` | `None` | Unique key for deduplication |
| `depends_on` | `Vec<String>` | `None` | Task dependency IDs |

## CLI Tool

Rediq includes a CLI tool for managing queues:

```bash
# List queues
rediq queue list

# Pause a queue
rediq queue pause default

# Resume a queue
rediq queue resume default

# List workers
rediq worker list

# View task details
rediq task show <task-id>
```

## Architecture

```
┌─────────┐     ┌─────────┐     ┌──────────┐
│ Client  │────▶│  Redis  │────▶│ Worker   │
│ (Producer)    │  Queue  │     │(Consumer)│
└─────────┘     └─────────┘     └──────────┘
                                      │
                                      ▼
                                ┌──────────┐
                                │ Handler  │
                                └──────────┘
```

### Redis Key Structure

| Key Pattern | Type | Description |
|-------------|------|-------------|
| `queue:{name}` | List | Pending tasks |
| `pqueue:{name}` | ZSet | Priority queue |
| `active:{name}` | List | Currently processing |
| `delayed:{name}` | ZSet | Delayed tasks |
| `retry:{name}` | ZSet | Tasks awaiting retry |
| `dead:{name}` | List | Dead letter queue |
| `task:{id}` | Hash | Task details |
| `pending_deps:{id}` | Set | Pending dependencies |
| `task_deps:{id}` | Set | Dependent tasks |

## Project Structure

```
Rediq/
├── rediq/           # Core library
│   ├── client/      # Producer SDK
│   ├── server/      # Server and Worker
│   ├── task/        # Task models
│   ├── processor/   # Handler trait and router
│   ├── middleware/  # Middleware system
│   ├── storage/     # Redis storage layer
│   └── observability/ # Metrics and monitoring
├── rediq-cli/       # CLI tool
├── examples/        # Usage examples
└── scripts/lua/     # Redis Lua scripts
```

## Examples

See the [examples](examples/) directory for more examples:

- `quickstart.rs` - Basic usage
- `priority_queue_example.rs` - Priority queues
- `cron_example.rs` - Periodic tasks
- `dependency_example.rs` - Task dependencies
- `cluster_example.rs` - Redis Cluster
- `sentinel_example.rs` - Redis Sentinel
- `middleware_test.rs` - Middleware usage
- `metrics_http_example.rs` - HTTP metrics endpoint

## Development

```bash
# Build
cargo build

# Run tests
cargo test

# Run examples
cargo run --example quickstart

# Format code
cargo fmt

# Run linter
cargo clippy
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under either of:

- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)

at your option.

## Acknowledgments

Inspired by [Asynq](https://github.com/hibiken/asynq) for Go.
