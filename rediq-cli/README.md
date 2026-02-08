# rediq-cli

CLI tool for [Rediq](https://github.com/imjdl/Rediq) - a distributed task queue framework for Rust based on Redis.

## Installation

```bash
cargo install rediq-cli
```

## Usage

Set the Redis connection URL as an environment variable:

```bash
export REDIS_URL=redis://127.0.0.1
```

### Commands

```bash
# Show real-time dashboard
rediq dash

# List all queues
rediq queue list

# Pause a queue
rediq queue pause <queue_name>

# Resume a queue
rediq queue resume <queue_name>

# List all workers
rediq worker list

# Show task details
rediq task show <task_id>

# Cancel a task
rediq task cancel <task_id> --queue <queue_name>

# Retry a failed task
rediq task retry <task_id> --queue <queue_name>

# Clean up invalid queue metadata
rediq purge queues
```

## Dashboard

The `rediq dash` command provides a real-time monitoring dashboard showing:
- Active workers and their status
- Queue statistics (pending, active, completed, dead)
- Recent task activity

## License

Licensed under either of
- Apache License, Version 2.0 ([LICENSE-Apache-2.0](../LICENSE-Apache-2.0) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
