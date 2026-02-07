# Rediq

A distributed task queue framework for Rust based on Redis, inspired by [Asynq](https://github.com/hibiken/asynq).

## Features

- **Simple API**: Easy-to-use client and server APIs
- **Priority Queues**: Support for high-priority tasks
- **Task Retries**: Automatic retry with exponential backoff
- **Scheduled Tasks**: Support for delayed and cron-based tasks
- **Task Dependencies**: Execute tasks in dependency order
- **Middleware System**: Hook into task processing lifecycle
- **Metrics**: Built-in Prometheus metrics support
- **Redis Cluster**: Support for standalone, cluster, and sentinel modes

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
rediq = "0.1"
```

## Quick Start

### Producer (Client)

```rust
use rediq::client::Client;
use rediq::task::Task;
use serde::Serialize;

#[derive(Serialize)]
struct EmailData {
    to: String,
    subject: String,
    body: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::from_url("redis://127.0.0.1")?;

    let task = Task::builder("email:send")
        .payload(&EmailData {
            to: "user@example.com".to_string(),
            subject: "Welcome".to_string(),
            body: "Hello!".to_string(),
        })?
        .queue("default")
        .build()?;

    client.enqueue(&task).await?;

    Ok(())
}
```

### Consumer (Worker)

```rust
use rediq::server::{Server, ServerBuilder};
use rediq::processor::{Handler, Mux};
use rediq::task::Task;
use rediq::error::Result;

struct EmailHandler;

#[async_trait::async_trait]
impl Handler for EmailHandler {
    async fn handle(&self, task: &Task) -> Result<()> {
        println!("Processing email task: {}", task.id);
        // Process the task...
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut mux = Mux::new();
    mux.handle("email:send", EmailHandler);

    let server = ServerBuilder::new()
        .concurrency(10)
        .queues(["default"])
        .handler(mux)
        .build()?;

    server.run().await?;

    Ok(())
}
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
