# Rediq

A distributed task queue framework for Rust based on Redis, inspired by [Asynq](https://github.com/hibiken/asynq).

## Features

- **Simple API**: Easy-to-use client and server APIs with builder pattern
- **Priority Queues**: Support for high-priority tasks (0-100, lower = higher priority)
- **Task Retries**: Automatic retry with exponential backoff
- **Scheduled Tasks**: Support for delayed and cron-based tasks
- **Task Dependencies**: Execute tasks in dependency order
- **Task Aggregation**: Group tasks for batch processing
- **Progress Tracking**: Report task execution progress (0-100%)
- **Middleware System**: Hook into task processing lifecycle
- **Metrics**: Built-in Prometheus metrics support
- **Redis HA**: Support for standalone, cluster, and sentinel modes
- **Attribute Macros**: Simplified handler registration with `#[task_handler]`

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
rediq = "0.2"
tokio = { version = "1", features = ["full"] }
```

For attribute macros support:

```toml
[dependencies]
rediq = { version = "0.2", features = ["macros"] }
```

## Quick Start

### Producer (Client)

```rust
use rediq::{Client, Task};

let client = Client::builder()
    .redis_url("redis://localhost:6379")
    .build()
    .await?;

let task = Task::builder("email:send")
    .queue("emails")
    .payload(&serde_json::json!({
        "to": "user@example.com",
        "subject": "Welcome!"
    }))?
    .build()?;

client.enqueue(task).await?;
```

### Consumer (Worker)

```rust
use rediq::server::{Server, ServerBuilder};
use rediq::processor::{Handler, Mux};
use rediq::{Task, Result};

struct EmailHandler;

#[async_trait::async_trait]
impl Handler for EmailHandler {
    async fn handle(&self, task: &Task) -> Result<()> {
        println!("Processing email task: {}", task.id);
        // Process the task...
        Ok(())
    }
}

let state = ServerBuilder::new()
    .redis_url("redis://localhost:6379")
    .queues(&["emails"])
    .concurrency(10)
    .build()
    .await?;

let server = Server::from(state);
let mut mux = Mux::new();
mux.handle("email:send", EmailHandler);

server.run(mux).await?;
```

### Attribute Macros (v0.2.0)

Simplify handler registration with `#[task_handler]`:

```rust
use rediq::{Task, Result};
use rediq_macros::{task_handler, register_handlers};

#[task_handler]
async fn send_email(task: &Task) -> Result<()> {
    let payload: EmailData = task.payload_json()?;
    // Process email...
    Ok(())
}

#[task_handler]
async fn send_sms(task: &Task) -> Result<()> {
    let payload: SmsData = task.payload_msgpack()?;
    // Process SMS...
    Ok(())
}

let mux = register_handlers!(
    "email:send" => send_email,
    "sms:send" => send_sms,
);
```

## CLI Tool

```bash
# Install CLI
cargo install rediq-cli

# Real-time dashboard
rediq dash

# Queue operations
rediq queue list
rediq queue pause <name>
rediq queue resume <name>

# Task operations
rediq task show <id>
rediq task cancel <id> --queue <name>
```

## Documentation

For detailed documentation, examples, and API reference, please visit:

- [GitHub Repository](https://github.com/imjdl/rediq)
- [Documentation](https://docs.rs/rediq)

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-Apache-2.0](LICENSE-Apache-2.0) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Contributions are welcome! Please feel free to submit a Pull Request.
