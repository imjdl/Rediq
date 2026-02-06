# Getting Started

This guide will help you get started with Rediq - a distributed task queue framework for Rust.

## Prerequisites

- Rust 1.70 or higher
- Redis server (2.8+ or higher)
- Cargo package manager

## Installation

### Adding to Your Project

Add the following to your `Cargo.toml`:

```toml
[dependencies]
rediq = "0.1"
tokio = { version = "1", features = ["full"] }
```

### Installing Redis

#### On Ubuntu/Debian

```bash
sudo apt-get update
sudo apt-get install redis-server
sudo systemctl start redis
```

#### On macOS

```bash
brew install redis
brew services start redis
```

#### Using Docker

```bash
docker run -d -p 6379:6379 redis:7-alpine
```

## Your First Task Queue

### Step 1: Create a Task Producer

Create `producer.rs`:

```rust
use rediq::{Client, Task};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct WelcomeEmail {
    user_id: String,
    email: String,
    username: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to Redis
    let client = Client::builder()
        .redis_url("redis://127.0.0.1:6379")
        .build()
        .await?;

    // Create a task
    let task = Task::builder("email:welcome")
        .queue("emails")
        .payload(&WelcomeEmail {
            user_id: "user-123".to_string(),
            email: "newuser@example.com".to_string(),
            username: "alice".to_string(),
        })?
        .build()?;

    // Enqueue the task
    let task_id = client.enqueue(task).await?;
    println!("Task enqueued: {}", task_id);

    Ok(())
}
```

### Step 2: Create a Task Consumer

Create `consumer.rs`:

```rust
use rediq::server::{Server, ServerBuilder};
use rediq::processor::{Handler, Mux};
use rediq::Task;
use async_trait::async_trait;
use serde::Deserialize;

#[derive(Deserialize)]
struct WelcomeEmail {
    user_id: String,
    email: String,
    username: String,
}

struct WelcomeEmailHandler;

#[async_trait]
impl Handler for WelcomeEmailHandler {
    async fn handle(&self, task: &Task) -> rediq::Result<()> {
        // Deserialize payload
        let email: WelcomeEmail = task.deserialize_payload()?;

        // Process the task
        println!("Sending welcome email to: {}", email.email);
        println!("Welcome, {}!", email.username);

        // Your email sending logic here
        // send_email(&email.email, "Welcome!", &format!("Hi {}", email.username)).await?;

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure the server
    let state = ServerBuilder::new()
        .redis_url("redis://127.0.0.1:6379")
        .queues(&["emails"])
        .concurrency(5)
        .build()
        .await?;

    // Create server
    let server = Server::from(state);

    // Register handlers
    let mut mux = Mux::new();
    mux.handle("email:welcome", WelcomeEmailHandler);

    // Start processing tasks
    println!("Worker started, waiting for tasks...");
    server.run(mux).await?;

    Ok(())
}
```

### Step 3: Run Your Tasks

**Terminal 1 - Start the worker:**

```bash
cargo run --bin consumer
```

**Terminal 2 - Enqueue a task:**

```bash
cargo run --bin producer
```

You should see the worker process the task and print the welcome message.

## Advanced Features

### Task with Priority

```rust
let urgent_task = Task::builder("urgent:notification")
    .queue("notifications")
    .priority(0)  // Highest priority
    .payload(&data)?
    .build()?;
```

### Delayed Task

```rust
use std::time::Duration;

let delayed_task = Task::builder("scheduled:report")
    .queue("reports")
    .delay(Duration::from_secs(3600))  // Run after 1 hour
    .payload(&data)?
    .build()?;
```

### Periodic Task

```rust
let daily_task = Task::builder("daily:cleanup")
    .queue("maintenance")
    .cron("0 2 * * *")  // Run at 2 AM daily
    .payload(&data)?
    .build()?;
```

### Task with Retry

```rust
let resilient_task = Task::builder("external:api_call")
    .queue("api")
    .max_retry(5)
    .timeout(Duration::from_secs(30))
    .payload(&data)?
    .build()?;
```

### Unique Tasks (Deduplication)

```rust
let unique_task = Task::builder("user:profile:update")
    .queue("updates")
    .unique_key("user:123:profile")  // Prevents duplicate tasks
    .payload(&data)?
    .build()?;
```

## CLI Usage

Rediq includes a CLI tool for managing queues:

```bash
# List all queues
rediq queue list

# Get queue statistics
rediq queue stats emails

# Pause a queue
rediq queue pause emails

# Resume a queue
rediq queue resume emails

# Purge all tasks from a queue
rediq queue purge emails

# List active workers
rediq worker list

# View task details
rediq task show <task-id>

# Cancel a pending task
rediq task cancel <task-id>
```

## Next Steps

- Read the [Architecture](architecture.md) guide to understand the system design
- Check the [API Reference](api-reference.md) for detailed API documentation
- See the [Configuration](configuration.md) guide for all configuration options
- Review the [Deployment](deployment.md) guide for production deployment

## Examples

More examples are available in the `examples/` directory:

```bash
# Basic usage
cargo run --example quickstart

# Priority queues
cargo run --example priority_queue_example

# Cron tasks
cargo run --example cron_example

# Task dependencies
cargo run --example dependency_example

# Middleware
cargo run --example middleware_test

# Redis Cluster
cargo run --example cluster_example

# Redis Sentinel
cargo run --example sentinel_example
```

## Troubleshooting

### Worker Not Processing Tasks

1. Check Redis connection: `redis-cli ping`
2. Verify queue name matches between producer and consumer
3. Check worker logs for errors

### Tasks Not Being Enqueued

1. Verify Redis is running: `redis-cli info server`
2. Check connection URL in producer code
3. Ensure task payload is serializable

### High Memory Usage

1. Tasks with large payloads (>512KB) are rejected
2. Dead letter queue may need periodic cleaning
3. Consider using `unique_key` to prevent duplicate tasks

For more help, see the [Troubleshooting](troubleshooting.md) guide.
