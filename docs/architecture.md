# Architecture

This document describes the architecture and design of Rediq.

## Overview

Rediq is a distributed task queue system built with Rust and Redis. It follows a producer-consumer pattern where tasks are enqueued by producers and processed asynchronously by workers.

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Application Layer                        │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐  │
│  │ Producer │    │ Producer │    │ API      │    │ Cron     │  │
│  │ (Client) │    │ (Client) │    │ Server   │    │ Scheduler│  │
│  └────┬─────┘    └────┬─────┘    └────┬─────┘    └────┬─────┘  │
│       │               │               │               │          │
└───────┼───────────────┼───────────────┼───────────────┼──────────┘
        │               │               │               │
        ▼               ▼               ▼               ▼
┌─────────────────────────────────────────────────────────────────┐
│                          Redis Layer                             │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐           │
│  │  Queue  │  │Priority │  │ Delayed │  │  Retry  │           │
│  │  List   │  │  ZSet   │  │  ZSet   │  │  ZSet   │  ...      │
│  └─────────┘  └─────────┘  └─────────┘  └─────────┘           │
└─────────────────────────────────────────────────────────────────┘
        │               │               │               │
        ▼               ▼               ▼               ▼
┌─────────────────────────────────────────────────────────────────┐
│                        Worker Layer                              │
│  ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐     │
│  │ Worker  │    │ Worker  │    │ Worker  │    │ Worker  │     │
│  │   #1    │    │   #2    │    │   #3    │    │   #N    │     │
│  └────┬────┘    └────┬────┘    └────┬────┘    └────┬────┘     │
│       │              │              │              │            │
│       ▼              ▼              ▼              ▼            │
│  ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐     │
│  │ Handler │    │ Handler │    │ Handler │    │ Handler │     │
│  │  Mux    │───▶│  Email  │    │  SMS    │    │  Push   │     │
│  └─────────┘    └─────────┘    └─────────┘    └─────────┘     │
└─────────────────────────────────────────────────────────────────┘
```

## Components

### 1. Client (Producer)

The client is responsible for creating and enqueueing tasks to Redis.

**Key responsibilities:**
- Task creation and validation
- Task serialization (MessagePack)
- Redis communication
- Queue management operations

**File:** `rediq/src/client/`

### 2. Server & Workers

The server manages worker processes that consume and process tasks.

**Key responsibilities:**
- Worker lifecycle management
- Task dequeuing
- Task routing to handlers
- Result acknowledgment
- Error handling and retry

**File:** `rediq/src/server/`

### 3. Scheduler

The scheduler handles time-based task execution.

**Key responsibilities:**
- Delayed task scheduling
- Retry task management
- Cron task execution
- Task dependency resolution

**File:** `rediq/src/server/scheduler.rs`

### 4. Processor (Handler)

The processor defines the interface for task handlers.

**Key responsibilities:**
- Handler trait definition
- Task type routing (Mux)
- Handler registration

**File:** `rediq/src/processor/`

### 5. Storage Layer

The storage layer abstracts Redis operations.

**Key responsibilities:**
- Redis client management
- Connection pooling
- Key naming conventions
- Lua script execution

**File:** `rediq/src/storage/`

### 6. Middleware

Middleware provides hooks before and after task processing.

**Key responsibilities:**
- Logging
- Metrics collection
- Custom hooks

**File:** `rediq/src/middleware/`

## Data Structures

### Task Model

```rust
pub struct Task {
    pub id: String,              // Unique task ID (UUID)
    pub task_type: String,       // Task type for routing
    pub queue: String,           // Target queue name
    pub payload: Vec<u8>,        // Serialized payload data
    pub options: TaskOptions,    // Task options
    pub status: TaskStatus,      // Current status
    pub created_at: i64,         // Creation timestamp
    pub enqueued_at: Option<i64>,// Enqueue timestamp
    pub processed_at: Option<i64>,// Process timestamp
    pub retry_cnt: u32,          // Retry count
    pub last_error: Option<String>,// Last error message
}
```

### Task Options

```rust
pub struct TaskOptions {
    pub max_retry: u32,          // Maximum retry count
    pub timeout: Duration,       // Task timeout
    pub delay: Option<Duration>, // Execution delay
    pub priority: i32,           // Priority (0-100)
    pub unique_key: Option<String>,// Deduplication key
    pub cron: Option<String>,    // Cron expression
    pub depends_on: Option<Vec<String>>,// Task dependencies
}
```

## Redis Key Design

All keys use the `rediq:` prefix for namespacing:

| Key Pattern | Type | Purpose |
|-------------|------|---------|
| `rediq:queue:{name}` | List | Pending tasks queue |
| `rediq:pqueue:{name}` | ZSet | Priority queue (score=priority) |
| `rediq:active:{name}` | List | Currently processing tasks |
| `rediq:delayed:{name}` | ZSet | Delayed tasks (score=execute_at) |
| `rediq:retry:{name}` | ZSet | Tasks awaiting retry |
| `rediq:dead:{name}` | List | Dead letter queue |
| `rediq:cron:{name}` | ZSet | Cron tasks (score=next_run) |
| `rediq:dedup:{name}` | Set | Task deduplication |
| `rediq:task:{id}` | Hash | Task details storage |
| `rediq:pending_deps:{id}` | Hash | Pending task dependencies |
| `rediq:task_deps:{id}` | Set | Tasks depending on this task |
| `rediq:meta:queues` | Set | All queue names |
| `rediq:meta:workers` | Set | All worker IDs |
| `rediq:meta:worker:{id}` | Hash | Worker metadata |
| `rediq:meta:heartbeat:{id}` | String | Worker heartbeat |
| `rediq:pause:{name}` | String | Queue pause marker |
| `rediq:stats:{name}` | Hash | Queue statistics |

## Task State Machine

```
                    ┌──────────┐
                    │ Pending  │◀────┐
                    └────┬─────┘     │
                         │           │ (delay/cron)
                         │ (dequeue) │
                         ▼           │
                    ┌──────────┐     │
                    │  Active  │     │
                    └────┬─────┘     │
                         │           │
            ┌────────────┴────────────┐
            │                         │
       (success)                 (failure)
            │                         │
            ▼                         ▼
       ┌──────────┐            ┌──────────┐
       │Processed │            │  Retry   │
       └──────────┘            └────┬─────┘
                                    │ (max retry exceeded)
                                    ▼
                               ┌──────────┐
                               │   Dead   │
                               └──────────┘
```

## Concurrency Model

### Worker Pool

```
┌──────────────────────────────────────────────────────┐
│                    Server                            │
│  ┌────────────────────────────────────────────┐     │
│  │            Worker Pool                       │     │
│  │  ┌────────┐ ┌────────┐ ┌────────┐           │     │
│  │  │Worker 1│ │Worker 2│ │Worker N│ ...       │     │
│  │  └───┬────┘ └───┬────┘ └───┬────┘           │     │
│  │      │          │          │                  │     │
│  │      ▼          ▼          ▼                  │     │
│  │  ┌────────────────────────────────┐          │     │
│  │  │           Mux (Router)          │          │     │
│  │  └────────────────────────────────┘          │     │
│  │                │                              │     │
│  │       ┌────────┴────────┐                    │     │
│  │       │                 │                    │     │
│  │  ┌────▼────┐      ┌────▼────┐                 │     │
│  │  │Handler A│      │Handler B│ ...            │     │
│  │  └─────────┘      └─────────┘                 │     │
│  └────────────────────────────────────────────┘     │
│                                                     │
│  ┌────────────────────────────────────────────┐     │
│  │            Scheduler (Background)            │     │
│  │  • Delayed tasks → Queue                    │     │
│  │  • Retry tasks → Queue                      │     │
│  │  • Cron tasks → Queue                        │     │
│  └────────────────────────────────────────────┘     │
└──────────────────────────────────────────────────────┘
```

### Round-Robin Queue Polling

Workers poll queues in round-robin fashion to ensure fair task distribution:

```rust
queue_index = (queue_index + 1) % queues.len()
queue = queues[queue_index]
```

## Middleware Chain

Middleware allows pre/post processing hooks:

```
┌────────────────────────────────────────────────────┐
│              Middleware Chain                      │
│                                                     │
│  Task ──▶ [Middleware 1] ──▶ [Middleware 2] ──▶ ... │
│             before()           before()             │
│                                                     │
│  Handler ──▶ Process Task ──▶ ... ──▶ Result       │
│                                                     │
│  Result ──▶ [Middleware 2] ──▶ [Middleware 1] ──▶ ...│
│              after()            after()             │
└────────────────────────────────────────────────────┘
```

## Atomic Operations

Lua scripts ensure atomicity for critical operations:

| Script | Purpose |
|--------|---------|
| `enqueue.lua` | Atomic task enqueue with deduplication |
| `penqueue.lua` | Atomic priority task enqueue |
| `dequeue.lua` | Atomic task dequeue with pause check |
| `pdequeue.lua` | Atomic priority task dequeue |
| `ack.lua` | Atomic task acknowledgment |

## Scalability

### Horizontal Scaling

- **Multiple Workers**: Run multiple worker instances
- **Worker Isolation**: Different workers can handle different queues
- **Redis Cluster**: Use Redis Cluster for high throughput

### Vertical Scaling

- **Concurrency**: Increase worker concurrency per instance
- **Connection Pooling**: Redis connection pool size

## Reliability

### Task Durability

- Tasks are persisted in Redis before acknowledgment
- Task details stored in hash with 24-hour TTL
- Failed tasks moved to dead letter queue

### Worker Resilience

- Graceful shutdown handling
- Automatic reconnection to Redis
- Worker heartbeat monitoring

### Retry Strategy

Exponential backoff: `delay = 2^n` seconds (max 60s)

```
Retry 1: 2 seconds
Retry 2: 4 seconds
Retry 3: 8 seconds
Retry 4: 16 seconds
Retry 5+: 60 seconds
```
