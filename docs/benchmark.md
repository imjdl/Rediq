# Benchmark Guide

Complete guide for benchmarking Rediq performance.

## Table of Contents

- [Quick Start](#quick-start)
- [Command Options](#command-options)
- [Benchmark Scenarios](#benchmark-scenarios)
- [Understanding Results](#understanding-results)
- [Performance Tuning](#performance-tuning)
- [Monitoring During Tests](#monitoring-during-tests)

---

## Quick Start

### Basic Benchmark

```bash
# Set Redis URL
export REDIS_URL="redis://:password@host:6379"

# Run basic benchmark (1000 tasks, 10 workers)
cargo run --release --example benchmark -- --tasks 1000 --workers 10
```

### High Throughput Test

```bash
# Test with 100K tasks, 50 workers
cargo run --release --example benchmark -- \
  --tasks 100000 \
  --workers 50 \
  --batch-size 100
```

### Low Latency Test

```bash
# Test with fast tasks
cargo run --release --example benchmark -- \
  --tasks 1000 \
  --workers 10 \
  --task-duration 1
```

---

## Command Options

| Option | Default | Description |
|--------|---------|-------------|
| `--tasks` | 1000 | Number of tasks to enqueue |
| `--workers` | 10 | Number of concurrent workers |
| `--batch-size` | 10 | Batch size for enqueueing |
| `--task-duration` | 10 | Simulated task duration (ms) |
| `--queues` | 1 | Number of queues |
| `--priority` | false | Use priority queue |
| `--payload-size` | 1 | Task payload size (KB) |
| `--redis-url` | - | Redis connection URL (reads from `REDIS_URL` env var) |

### Examples

```bash
# Set task duration to 50ms
--task-duration 50

# Use 3 queues
--queues 3

# Enable priority queue
--priority

# Set payload to 10KB
--payload-size 10

# Explicit Redis URL
--redis-url "redis://:password@localhost:6379/0"
```

---

## Benchmark Scenarios

### 1. Throughput Test

Measure maximum throughput:

```bash
cargo run --release --example benchmark -- \
  --tasks 100000 \
  --workers 50 \
  --batch-size 100 \
  --task-duration 1
```

**Goal**: Find maximum tasks/second the system can handle.

### 2. Latency Test

Measure end-to-end latency:

```bash
cargo run --release --example benchmark -- \
  --tasks 1000 \
  --workers 10 \
  --task-duration 10
```

**Goal**: Measure average task processing time.

### 3. Priority Queue Test

Test priority queue performance:

```bash
cargo run --release --example benchmark -- \
  --tasks 10000 \
  --workers 20 \
  --priority
```

**Goal**: Compare priority queue vs regular queue performance.

### 4. Multi-Queue Test

Test with multiple queues to verify load distribution:

```bash
cargo run --release --example benchmark -- \
  --tasks 30000 \
  --workers 30 \
  --queues 3
```

**Output with multiple queues:**
```
Configuration:
  Tasks:          30000
  Workers:        30
  ...
  Queues:         3
    └ Queue distribution:
       - queue-0: 10000 tasks (ID: 0-9999)
       - queue-1: 10000 tasks (ID: 10000-19999)
       - queue-2: 10000 tasks (ID: 20000-29999)
  ...
```

**At the end, per-queue statistics are shown:**
```
╔════════════════════════════════════════════════════════════╗
║                    Per-Queue Statistics                    ║
╚════════════════════════════════════════════════════════════╝

  Queue: queue-0
    Pending:    0
    Active:     0
    Delayed:    0
    Retry:      0
    Dead:       0

  Queue: queue-1
    ...
```

**Goal**: Verify load distribution across queues and compare single vs multi-queue performance.

**Multi-Queue Benefits:**
- Better load balancing across workers
- Reduced contention on single queue
- Ability to prioritize different task types
- Improved isolation between workloads

### 5. Priority Queue Test

Test priority queue behavior:

```bash
cargo run --release --example benchmark -- \
  --tasks 10000 \
  --workers 10 \
  --queues 1 \
  --priority
```

**Goal**: Verify that lower priority values (higher priority) are processed first.

### 6. Large Payload Test

Test with large task payloads:

```bash
cargo run --release --example benchmark -- \
  --tasks 1000 \
  --workers 10 \
  --payload-size 100
```

**Goal**: Measure impact of payload size on performance.

---

## Understanding Results

### Output Breakdown

```
╔════════════════════════════════════════════════════════════╗
║                    Rediq Benchmark Tool                      ║
╚════════════════════════════════════════════════════════════╝

Configuration:
  Tasks:          100000
  Workers:        50
  Batch size:     100
  Task duration:  10ms
  ...

▶ Enqueueing 100000 tasks...
  Enqueued: 100000/100000
  Time:     2.34s
  Throughput: 42735 tasks/sec

⏳ Waiting for tasks to be processed...
  Processed: 100000/100000
  Time:        24.56s
  Throughput:  4071 tasks/sec

╔════════════════════════════════════════════════════════════╗
║                         Summary                            ║
╚════════════════════════════════════════════════════════════╝

  Total tasks:        100000
  Successfully:       100000
  Failed:             0

  Enqueue time:       2.34s
  Process time:       24.56s
  Total time:         26.90s

  Enqueue throughput: 42735 tasks/sec    ← How fast tasks can be queued
  Process throughput:  4071 tasks/sec    ← How fast tasks are processed
  Overall throughput:  3717 tasks/sec    ← End-to-end throughput

  Avg latency:        ~15ms             ← Average task completion time
```

### Key Metrics

| Metric | Description | Good Range |
|--------|-------------|------------|
| **Enqueue Throughput** | Tasks queued per second | >10,000 |
| **Process Throughput** | Tasks processed per second | >1,000 |
| **Overall Throughput** | End-to-end throughput | Depends on task duration |
| **Avg Latency** | Average task completion time | task_duration + overhead |

### Calculating Expected Throughput

For a given configuration:

```
Expected Process Throughput = (workers × 1000) / task_duration

Example:
- workers = 50
- task_duration = 10ms
- Expected = (50 × 1000) / 10 = 5000 tasks/sec
```

If actual throughput is significantly lower, check:
- Network latency to Redis
- Redis server performance
- System resource limits

---

## Performance Tuning

### 1. Batch Size

Larger batch sizes improve enqueue throughput:

```bash
# Test different batch sizes
--batch-size 10    # Default, good balance
--batch-size 100   # Higher throughput
--batch-size 1000  # Maximum throughput, higher memory usage
```

**Trade-off**: Larger batches = higher throughput but more memory.

### 2. Worker Count

Match workers to CPU cores:

```bash
# CPU-intensive tasks
--workers <number of CPU cores>

# I/O-intensive tasks
--workers <2-3x number of CPU cores>
```

### 3. Multiple Queues

Distribute load across queues for better performance:

```bash
--queues 1   # Single queue, baseline
--queues 3   # 3 queues, ~30% better throughput
--queues 5   # 5 queues, diminishing returns
```

**When to use multiple queues:**

| Scenario | Recommendation | Reason |
|----------|----------------|--------|
| High throughput needed | Use 3-5 queues | Reduces lock contention |
| Different task priorities | Use separate queues per priority | Isolates workloads |
| Different task types | Use dedicated queues | Better organization |
| Single task type | 1 queue is sufficient | No benefit from splitting |

**Multi-Queue vs Single Queue:**

```bash
# Compare single vs multi-queue
# Single queue test
cargo run --release --example benchmark -- --tasks 100000 --workers 50

# Multi-queue test (typically 20-40% better throughput)
cargo run --release --example benchmark -- --tasks 100000 --workers 50 --queues 3
```

**Note**: Workers process queues in round-robin fashion, so tasks are evenly distributed.

### 4. Payload Size

Minimize payload size:

```bash
--payload-size 1  # 1KB, default
--payload-size 10 # 10KB, ~10% slower
```

---

## Monitoring During Tests

### Terminal 1: Run Benchmark

```bash
export REDIS_URL="redis://:password@host:6379"
cargo run --release --example benchmark -- --tasks 100000 --workers 50
```

### Terminal 2: Monitor Redis Stats

```bash
# Real-time operations per second
watch -n 1 'redis-cli -h host -p 6379 -a password --no-auth-warning \
  INFO stats | grep instantaneous_ops_per_sec'
```

### Terminal 3: Monitor Queue Length

```bash
# Watch queue length
watch -n 1 'redis-cli -h host -p 6379 -a password --no-auth-warning \
  LLEN rediq:queue:default'
```

### Terminal 4: Monitor Latency

```bash
# Redis latency monitoring
redis-cli -h host -p 6379 -a password --latency-history
```

---

## Benchmark Results Reference

### Expected Performance by Task Duration

| Task Duration | Workers | Expected Throughput |
|--------------|---------|-------------------|
| 1ms | 10 | ~10,000 tasks/sec |
| 10ms | 10 | ~1,000 tasks/sec |
| 10ms | 50 | ~5,000 tasks/sec |
| 100ms | 10 | ~100 tasks/sec |

### Performance Factors

| Factor | Impact |
|--------|--------|
| Network latency | High (adds to each operation) |
| Redis CPU | Medium |
| Payload size | Medium (serialization overhead) |
| Batch size | High (enqueue throughput) |
| Worker count | High (process throughput) |
| **Queue count** | **Medium (3-5 queues can improve 20-40%)** |

---

## Troubleshooting

### Low Throughput

**Symptoms**: Process throughput much lower than expected.

**Solutions**:
1. Check Redis latency: `redis-cli --latency`
2. Increase worker count
3. Reduce task duration
4. Check network connectivity

### High Enqueue Errors

**Symptoms**: Tasks failing to enqueue.

**Solutions**:
1. Check Redis memory: `redis-cli INFO memory`
2. Reduce batch size
3. Reduce payload size
4. Check Redis connection limits

### Timeout During Processing

**Symptoms**: Benchmark times out waiting for tasks.

**Solutions**:
1. Increase worker count
2. Reduce task duration
3. Check Redis performance
4. Check system resources

---

## Advanced Benchmarking

### Custom Workload

Create custom benchmark scenarios:

```bash
# Web application workload
--tasks 50000 --workers 20 --task-duration 50 --batch-size 50

# Batch processing workload
--tasks 10000 --workers 5 --task-duration 500 --batch-size 10

# Real-time processing workload
--tasks 100000 --workers 100 --task-duration 5 --batch-size 200
```

### Comparative Testing

Compare configurations:

```bash
# Test 1: Baseline
cargo run --release --example benchmark -- --tasks 10000 --workers 10

# Test 2: More workers
cargo run --release --example benchmark -- --tasks 10000 --workers 20

# Test 3: Batch enqueue
cargo run --release --example benchmark -- --tasks 10000 --workers 10 --batch-size 100
```

### Multi-Queue Comparative Testing

Compare single queue vs multiple queues:

```bash
# Single queue baseline
cargo run --release --example benchmark -- \
  --tasks 100000 --workers 50 --batch-size 100 --queues 1

# Compare with 3 queues
cargo run --release --example benchmark -- \
  --tasks 100000 --workers 50 --batch-size 100 --queues 3

# Compare with 5 queues
cargo run --release --example benchmark -- \
  --tasks 100000 --workers 50 --batch-size 100 --queues 5

# Compare with 10 queues (diminishing returns expected)
cargo run --release --example benchmark -- \
  --tasks 100000 --workers 50 --batch-size 100 --queues 10
```

**Real-World Multi-Queue Scenarios:**

```bash
# E-commerce: Separate queues for different operations
cargo run --release --example benchmark -- \
  --tasks 50000 --workers 30 --queues 4
# Queues: orders, payments, shipping, notifications

# Data processing: Separate queues for pipeline stages
cargo run --release --example benchmark -- \
  --tasks 100000 --workers 50 --queues 5
# Queues: ingest, parse, validate, transform, load

# Microservices: Separate queue per service
cargo run --release --example benchmark -- \
  --tasks 30000 --workers 20 --queues 6
# Queues: auth, user, content, search, analytics, email
```

### Stress Testing

Find system limits:

```bash
# Gradually increase load
for workers in 10 20 50 100; do
  echo "Testing with $workers workers..."
  cargo run --release --example benchmark -- \
    --tasks 100000 --workers $workers --batch-size 100
done
```

---

## Best Practices

1. **Run multiple iterations**: Results can vary between runs
2. **Monitor system resources**: Check CPU, memory, network during tests
3. **Test with realistic payloads**: Use payload sizes matching production
4. **Document configuration**: Save benchmark parameters for reproducibility
5. **Test in production-like environment**: Network and Redis setup matter

---

## Example Benchmark Script

```bash
#!/bin/bash

# benchmark.sh - Run comprehensive benchmarks

REDIS_URL="${REDIS_URL:-redis://localhost:6379}"
TASKS=10000

echo "Rediq Benchmark Suite"
echo "===================="
echo ""

run_benchmark() {
  local name=$1
  shift
  echo "Running: $name"
  cargo run --release --example benchmark -- \
    --redis-url "$REDIS_URL" \
    --tasks $TASKS \
    "$@"
  echo ""
}

# Baseline
run_benchmark "Baseline" --workers 10 --task-duration 10

# High throughput
run_benchmark "High Throughput" --workers 50 --task-duration 1 --batch-size 100

# Large payload
run_benchmark "Large Payload" --workers 10 --task-duration 10 --payload-size 10

# Priority queue
run_benchmark "Priority Queue" --workers 10 --task-duration 10 --priority

# Multi-queue tests
echo "===================="
echo "Multi-Queue Tests"
echo "===================="
echo ""

run_benchmark "Single Queue" --workers 30 --queues 1
run_benchmark "Three Queues" --workers 30 --queues 3
run_benchmark "Five Queues" --workers 30 --queues 5

echo "Benchmark complete!"
```

Usage:
```bash
chmod +x benchmark.sh
./benchmark.sh
```
