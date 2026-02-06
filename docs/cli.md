# Rediq CLI Reference

Complete guide for the Rediq command-line interface tool.

## Table of Contents

- [Installation](#installation)
- [Configuration](#configuration)
- [Commands](#commands)
  - [Queue Operations](#queue-operations)
  - [Task Operations](#task-operations)
  - [Worker Operations](#worker-operations)
  - [Statistics](#statistics)
  - [Dashboard](#dashboard)
  - [Purge](#purge)
- [Examples](#examples)
- [Tips and Tricks](#tips-and-tricks)

---

## Installation

The CLI is built automatically with the project:

```bash
# From the project root
cargo build --release -p rediq-cli

# The binary will be at:
# ./target/release/rediq
```

You can also install it globally:

```bash
cargo install --path ./rediq-cli
```

---

## Configuration

The CLI reads the Redis connection URL from the `REDIS_URL` environment variable:

```bash
# Default (if not set)
export REDIS_URL="redis://localhost:6379"

# With password
export REDIS_URL="redis://:password@localhost:6379"

# With database
export REDIS_URL="redis://localhost:6379/0"

# Remote Redis
export REDIS_URL="redis://host:6379"
```

---

## Commands

### Queue Operations

#### List all queues

```bash
rediq queue list
```

**Output:**
```
Queue List
  - default
  - emails
  - reports
```

#### Show queue details

```bash
rediq queue inspect <name>
```

**Example:**
```bash
rediq queue inspect emails
```

**Output:**
```
Queue Details: emails
  Pending: 42
  Active: 3
  Delayed: 0
  Retry: 1
  Dead: 0
```

#### Pause queue

```bash
rediq queue pause <name>
```

Pauses processing of tasks in the queue. Workers will not dequeue new tasks from paused queues.

#### Resume queue

```bash
rediq queue resume <name>
```

Resumes processing of tasks in the queue.

#### Flush queue

```bash
rediq queue flush <name>
```

Removes all tasks (pending, active, delayed, retry, dead) from the queue.

---

### Task Operations

#### List tasks in a queue

```bash
rediq task list <queue> [--limit N]
```

**Example:**
```bash
rediq task list emails --limit 50
```

**Output:**
```
Tasks in queue 'emails' (showing max 50):
  Pending tasks:
    - 550e8f0a-1234-4567-89ab-0123456789ab [email:send]
      Status: pending
      Retry: 0/3
```

#### Show task details

```bash
rediq task inspect <id>
```

**Example:**
```bash
rediq task inspect 550e8f0a-1234-4567-89ab-0123456789ab
```

**Output:**
```
Task Details: 550e8f0a-1234-4567-89ab-0123456789ab
  ID: 550e8f0a-1234-4567-89ab-0123456789ab
  Type: email:send
  Queue: emails
  Status: pending
  Retry count: 0
  Created at: 1678901234
```

#### Cancel task

```bash
rediq task cancel --id <id> --queue <queue>
```

Cancels a pending task. Only works for tasks that are currently in the pending queue.

**Example:**
```bash
rediq task cancel --id 550e8f0a-1234-4567-89ab-0123456789ab --queue emails
```

**Output:**
```
Cancel task: 550e8f0a-1234-4567-89ab-0123456789ab (queue: emails)
  ✓ Task cancelled successfully
```

#### Retry failed task

```bash
rediq task retry --id <id> --queue <queue>
```

Moves a task from the dead/retry queue back to the pending queue.

---

### Worker Operations

#### List all workers

```bash
rediq worker list
```

**Output:**
```
Worker List
  - rediq-server-abc123-worker-0
    Server: app-server-1
    Queues: default, emails
    Status: idle
    Processed: 1523
```

#### Show worker details

```bash
rediq worker inspect <id>
```

**Example:**
```bash
rediq worker inspect rediq-server-abc123-worker-0
```

**Output:**
```
Worker Details: rediq-server-abc123-worker-0
  ID: rediq-server-abc123-worker-0
  Server: app-server-1
  Queues: ["default", "emails"]
  Status: idle
  Started at: 1678901234
  Last heartbeat: 1678904321
  Processed total: 1523
```

#### Stop worker

```bash
rediq worker stop <id>
```

Sends a shutdown signal to the worker. The worker will finish its current task and exit gracefully.

---

### Statistics

#### Show queue statistics

```bash
rediq stats --queue <name>
```

**Example:**
```bash
rediq stats --queue emails
```

**Output:**
```
Statistics
  Queue: emails
  Pending: 42
  Active: 3
  Delayed: 0
  Retry: 1
  Dead: 0
```

---

### Dashboard

#### Start real-time dashboard

```bash
rediq dash [-i INTERVAL]
```

**Options:**
- `-i, --interval` - Refresh interval in milliseconds (default: 500ms)

**Example:**
```bash
rediq dash -i 1000
```

**Dashboard Features:**
- Real-time queue statistics
- Worker list and status
- Total processed tasks
- Operations per second
- Worker health indicator

**Keyboard Controls:**
- `q` or `Esc` - Quit
- `↑` / `↓` or `←` / `→` - Select queue

**Dashboard Display:**
```
╔════════════════════════════════════════════════════════════╗
║                    Rediq Dashboard                      ║
╚════════════════════════════════════════════════════════════╝

┌─────────────────────────────────────────────────────────────┐
│ Queues (3)  │ Workers (5)  │                              │
├──────────────┼──────────────┤                              │
│ Queue    Pen Act Del Ret Dead │ Worker  Server  Status  Proc  │
│ default  42   3   0   1   0  │ w-0     svr1    idle     1523  │
│ emails   10   1   0   0   0  │ w-1     svr1    idle      890  │
│ reports  5    0   0   0   0  │ ...                              │
└──────────────┴──────────────┴──────────────────────────────┘

┌───────────────────────┬─────────────────────────┐
│ Metrics                │ Throughput               │
│ In Queues:       57   │ Ops/sec:    45.2       │
│ ├ Pending:      52   │ Processed:  2413       │
│ ├ Active:        4   │                          │
│ ├ Delayed:       0   │                          │
│ ├ Retry:         1   │                          │
│ └ Dead:          0   │                          │
│                        │ ┌─────────────────────┐ │
│                        │ │ Worker Health        │ │
│                        │ │ ████████████░░ 80% │ │
│                        │ └─────────────────────┘ │
└────────────────────────┴─────────────────────────┘
```

---

### Purge

#### Clean up stale metadata

```bash
rediq purge [--workers] [--queues] [--all]
```

**Options:**
- `--workers` - Show stale workers (no heartbeat in 2+ minutes)
- `--queues` - Show empty queues
- `--all` - Show commands to purge all metadata

**Examples:**
```bash
# List stale workers
rediq purge --workers

# List empty queues
rediq purge --queues

# Show full purge commands
rediq purge --all
```

**Why you might need purge:**

When worker processes exit abnormally (kill, crash, etc.), their metadata may remain in Redis. The `purge` command helps identify and clean up this stale metadata.

---

## Examples

### Common Workflows

#### 1. Monitor Queue Health

```bash
# Quick check of all queues
rediq queue list

# Detailed stats for specific queue
rediq queue inspect emails
rediq stats --queue emails

# Real-time monitoring
rediq dash
```

#### 2. Debug a Specific Task

```bash
# List tasks to find the ID
rediq task list emails --limit 100

# Inspect the task
rediq task inspect <task-id>

# If needed, cancel it
rediq task cancel --id <task-id> --queue emails
```

#### 3. Worker Management

```bash
# Check worker status
rediq worker list

# Check specific worker details
rediq worker inspect <worker-id>

# Gracefully stop a worker
rediq worker stop <worker-id>
```

#### 4. Clean Up After Testing

```bash
# Check for stale workers
rediq purge --workers

# Check for empty queues
rediq purge --queues
```

---

## Tips and Tricks

### Combining with Unix Tools

```bash
# Watch queue stats continuously
watch -n 1 'rediq stats --queue emails'

# Count total pending tasks across all queues
rediq queue list | while read queue; do
  rediq stats --queue "$queue" | grep "Pending"
done

# Find queues with high task count
for queue in $(rediq queue list | tail -n +2); do
  pending=$(rediq stats --queue "$queue" | grep "Pending" | awk '{print $2}')
  if [ "$pending" -gt 100 ]; then
    echo "$queue: $pending pending"
  fi
done
```

### Environment Variables for Convenience

```bash
# Add to ~/.bashrc or ~/.zshrc
alias rediq='REDIS_URL="redis://:password@host:6379/0" /path/to/rediq'

# Common commands
alias rediq-dash='rediq dash -i 1000'
alias rediq-queues='rediq queue list'
alias rediq-workers='rediq worker list'
```

### Scripting with CLI

```bash
#!/bin/bash
# monitor_queues.sh - Alert when queue gets too full

THRESHOLD=1000
QUEUE="emails"

while true; do
  pending=$(rediq stats --queue "$QUEUE" | grep "Pending" | awk '{print $2}')

  if [ "$pending" -gt "$THRESHOLD" ]; then
    echo "⚠️  Queue $QUEUE has $pending pending tasks!"
    # Send alert, log, etc.
  fi

  sleep 60
done
```

---

## Command Reference Summary

| Command | Description |
|---------|-------------|
| `rediq queue list` | List all queues |
| `rediq queue inspect <name>` | Show queue details |
| `rediq queue pause <name>` | Pause queue processing |
| `rediq queue resume <name>` | Resume queue processing |
| `rediq queue flush <name>` | Remove all tasks from queue |
| `rediq task list <queue>` | List tasks in queue |
| `rediq task inspect <id>` | Show task details |
| `rediq task cancel --id <id> --queue <queue>` | Cancel a pending task |
| `rediq task retry --id <id> --queue <queue>` | Retry a failed task |
| `rediq worker list` | List all workers |
| `rediq worker inspect <id>` | Show worker details |
| `rediq worker stop <id>` | Stop a worker |
| `rediq stats --queue <name>` | Show queue statistics |
| `rediq dash [-i interval]` | Real-time dashboard |
| `rediq purge [--workers] [--queues] [--all]` | Clean up stale metadata |

---

## Getting Help

Each command supports the `--help` flag for detailed information:

```bash
rediq --help
rediq queue --help
rediq task --help
rediq worker --help
rediq stats --help
rediq dash --help
rediq purge --help
```
