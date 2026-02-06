# Deployment Guide

Guide for deploying Rediq in production environments.

## Table of Contents

- [Pre-Deployment Checklist](#pre-deployment-checklist)
- [Deployment Strategies](#deployment-strategies)
- [Docker Deployment](#docker-deployment)
- [Kubernetes Deployment](#kubernetes-deployment)
- [Systemd Service](#systemd-service)
- [Monitoring](#monitoring)
- [Scaling](#scaling)
- [Security](#security)

---

## Pre-Deployment Checklist

### Redis Setup

- [ ] Redis server installed and configured
- [ ] Redis persistence enabled (RDB/AOF)
- [ ] Redis memory limits configured
- [ ] Redis eviction policy set
- [ ] Redis connection tested
- [ ] Cluster/Sentinel configured (if using HA)

### Application Setup

- [ ] Environment variables configured
- [ ] Connection strings verified
- [ ] Queue names defined
- [ ] Task handlers implemented
- [ ] Error handling tested
- [ ] Logging configured
- [ ] Metrics enabled

### Infrastructure

- [ ] Sufficient CPU/Memory allocated
- [ ] Network connectivity verified
- [ ] Firewall rules configured
- [ ] Load balancer configured (if multiple workers)
- [ ] Monitoring setup
- [ ] Alert rules configured

---

## Deployment Strategies

### Single Instance Deployment

Simple deployment for low-volume applications.

```
┌──────────────┐
│  Application │
│  (Producer)  │
└───────┬──────┘
        │
        ▼
┌──────────────┐
│    Redis     │
└───────┬──────┘
        │
        ▼
┌──────────────┐
│   Worker     │
│   Process    │
└──────────────┘
```

**Pros:** Simple, cost-effective
**Cons:** Single point of failure, limited scalability

### Multi-Instance Deployment

Multiple workers for high availability.

```
┌──────────────┐
│  Application │
│  (Producer)  │
└───────┬──────┘
        │
        ▼
┌──────────────┐
│    Redis     │
└───────┬──────┘
        │
    ┌───┴───┬────────┐
    │       │        │
    ▼       ▼        ▼
┌─────┐ ┌─────┐ ┌─────┐
│Wkr 1│ │Wkr 2│ │Wkr N│
└─────┘ └─────┘ └─────┘
```

**Pros:** High availability, scalable
**Cons:** More complex, higher cost

### Dedicated Scheduler Deployment

Run scheduler separately from workers.

```
┌──────────────┐
│    Redis     │
└───────┬──────┘
        │
    ┌───┴────────┐
    │            │
    ▼            ▼
┌─────────┐  ┌────────┐
│Scheduler│  │Workers │
└─────────┘  └────────┘
```

**Pros:** Better separation of concerns
**Cons:** Additional deployment complexity

---

## Docker Deployment

### Dockerfile

```dockerfile
# Dockerfile
FROM rust:1.70 as builder

WORKDIR /app
COPY . .

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary
COPY --from=builder /app/target/release/your-worker /usr/local/bin/worker

# Create a non-root user
RUN useradd -m -u 1000 worker
USER worker

ENTRYPOINT ["worker"]
```

### Build and Run

```bash
# Build image
docker build -t rediq-worker:latest .

# Run container
docker run -d \
  --name rediq-worker \
  -e REDIS_URL=redis://redis:6379 \
  -e SERVER_QUEUES=default,critical \
  -e SERVER_CONCURRENCY=10 \
  rediq-worker:latest
```

### Docker Compose

```yaml
version: '3.8'

services:
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis-data:/data
    command: redis-server --appendonly yes

  worker:
    build: .
    environment:
      - REDIS_URL=redis://redis:6379
      - SERVER_QUEUES=default,critical,high
      - SERVER_CONCURRENCY=10
    depends_on:
      - redis
    restart: unless-stopped

volumes:
  redis-data:
```

---

## Kubernetes Deployment

### Deployment YAML

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: rediq-worker
spec:
  replicas: 3
  selector:
    matchLabels:
      app: rediq-worker
  template:
    metadata:
      labels:
        app: rediq-worker
    spec:
      containers:
      - name: worker
        image: rediq-worker:latest
        env:
        - name: REDIS_URL
          value: "redis://redis-service:6379"
        - name: SERVER_QUEUES
          value: "default,critical,high"
        - name: SERVER_CONCURRENCY
          value: "10"
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "512Mi"
            cpu: "500m"
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 5
---
apiVersion: v1
kind: Service
metadata:
  name: rediq-worker
spec:
  selector:
    app: rediq-worker
  ports:
  - port: 8080
    name: http
  - port: 9090
    name: metrics
```

### ConfigMap

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: rediq-config
data:
  REDIS_URL: "redis://redis-service:6379"
  SERVER_QUEUES: "default,critical,high,bulk"
  SERVER_CONCURRENCY: "10"
  HEARTBEAT_INTERVAL: "10"
  WORKER_TIMEOUT: "60"
```

### Horizontal Pod Autoscaler

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: rediq-worker-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: rediq-worker
  minReplicas: 3
  maxReplicas: 20
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
```

---

## Systemd Service

### Service File

```ini
# /etc/systemd/system/rediq-worker.service
[Unit]
Description=Rediq Worker
After=network.target redis.service
Requires=redis.service

[Service]
Type=simple
User=rediq
Group=rediq
WorkingDirectory=/opt/rediq
Environment="REDIS_URL=redis://localhost:6379"
Environment="SERVER_QUEUES=default,critical"
Environment="SERVER_CONCURRENCY=10"
Environment="RUST_LOG=info"
ExecStart=/opt/rediq/worker
Restart=always
RestartSec=10

# Resource limits
MemoryMax=1G
CPUQuota=200%

# Security
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true

[Install]
WantedBy=multi-user.target
```

### Enable and Start

```bash
# Copy service file
sudo cp rediq-worker.service /etc/systemd/system/

# Reload systemd
sudo systemctl daemon-reload

# Enable service
sudo systemctl enable rediq-worker

# Start service
sudo systemctl start rediq-worker

# Check status
sudo systemctl status rediq-worker

# View logs
sudo journalctl -u rediq-worker -f
```

---

## Monitoring

### Prometheus Metrics

Rediq exposes Prometheus metrics on port 9090.

**Available Metrics:**

| Metric | Type | Description |
|--------|------|-------------|
| `rediq_tasks_enqueued_total` | Counter | Total tasks enqueued |
| `rediq_tasks_processed_total` | Counter | Total tasks processed |
| `rediq_tasks_failed_total` | Counter | Total tasks failed |
| `rediq_tasks_retry_total` | Counter | Total task retries |
| `rediq_tasks_dead_total` | Counter | Total dead tasks |
| `rediq_task_duration_seconds` | Histogram | Task processing duration |
| `rediq_queue_length` | Gauge | Current queue length |
| `rediq_active_workers` | Gauge | Number of active workers |

### Prometheus Configuration

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'rediq'
    static_configs:
      - targets: ['worker1:9090', 'worker2:9090', 'worker3:9090']
    scrape_interval: 15s
```

### Grafana Dashboard

**Recommended Queries:**

- Task processing rate: `rate(rediq_tasks_processed_total[5m])`
- Task error rate: `rate(rediq_tasks_failed_total[5m])`
- Average task duration: `rate(rediq_task_duration_seconds_sum[5m]) / rate(rediq_task_duration_seconds_count[5m])`
- Queue length: `rediq_queue_length`
- Active workers: `rediq_active_workers`

### Health Checks

```rust
// Add health check endpoint
use warp::Filter;

async fn health() -> impl warp::Reply {
    warp::reply::json(&serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

// In server setup
let health_route = warp::path("health")
    .and(warp::get())
    .then(health);

let routes = health_route
    .or(metrics_route);
```

---

## Scaling

### Vertical Scaling

Increase resources per worker:

```yaml
resources:
  requests:
    memory: "512Mi"   # Increased from 256Mi
    cpu: "500m"       # Increased from 250m
  limits:
    memory: "1Gi"
    cpu: "1000m"
```

Adjust concurrency accordingly:

```rust
let concurrency = std::env::var("SERVER_CONCURRENCY")
    .unwrap_or_else(|_| "20".to_string())  // Increased from 10
    .parse()
    .unwrap_or(20);
```

### Horizontal Scaling

Add more worker instances:

```bash
# Kubernetes
kubectl scale deployment rediq-worker --replicas=10

# Docker
docker-compose up --scale worker=10
```

### Scaling Guidelines

| Queue Size | Workers | Concurrency |
|------------|--------|-------------|
| < 1,000 | 2-3 | 5-10 |
| 1,000 - 10,000 | 3-5 | 10-20 |
| 10,000 - 100,000 | 5-10 | 20-50 |
| > 100,000 | 10+ | 50+ |

---

## Security

### Redis Security

#### Password Authentication

```conf
# redis.conf
requirepass your_strong_password
```

```rust
let client = Client::builder()
    .redis_url("redis://:password@localhost:6379")
    .build()
    .await?;
```

#### TLS/SSL

```rust
let client = Client::builder()
    .redis_url("rediss://localhost:6380")  // Note: rediss://
    .build()
    .await?;
```

### Network Security

```yaml
# Kubernetes network policy
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: rediq-worker-policy
spec:
  podSelector:
    matchLabels:
      app: rediq-worker
  policyTypes:
  - Egress
  egress:
  - to:
    - podSelector:
        matchLabels:
          app: redis
    ports:
    - protocol: TCP
      port: 6379
```

### Secrets Management

```yaml
# Kubernetes Secret
apiVersion: v1
kind: Secret
metadata:
  name: rediq-secrets
type: Opaque
stringData:
  REDIS_URL: "redis://:password@redis:6379"
```

```yaml
# Deployment with secrets
env:
  - name: REDIS_URL
    valueFrom:
      secretKeyRef:
        name: rediq-secrets
        key: REDIS_URL
```

### Resource Limits

Prevent resource exhaustion:

```yaml
resources:
  requests:
    memory: "256Mi"
    cpu: "250m"
  limits:
    memory: "512Mi"
    cpu: "500m"
```

---

## Rollout Strategy

### Rolling Update

```bash
# Kubernetes
kubectl set image deployment/rediq-worker \
  worker=rediq-worker:v2.0.0

# With automatic rollout
kubectl rollout undo deployment/rediq-worker
```

### Blue-Green Deployment

```bash
# Deploy new version
kubectl apply -f deployment-green.yaml

# Switch traffic
kubectl apply -f service-green.yaml
```

### Canary Deployment

```yaml
# Deploy 10% canary
apiVersion: apps/v1
kind: Deployment
metadata:
  name: rediq-worker-canary
spec:
  replicas: 1  # 10% of total
  selector:
    matchLabels:
      app: rediq-worker
      version: v2.0.0
  template:
    metadata:
      labels:
        app: rediq-worker
        version: v2.0.0
    # ... spec
```

---

## Disaster Recovery

### Redis Backup

```bash
# RDB snapshot
cp /var/lib/redis/dump.rdb /backup/dump-$(date +%Y%m%d).rdb

# AOF backup
cp /var/lib/redis/appendonly.aof /backup/appendonly-$(date +%Y%m%d).aof
```

### Task Recovery

Tasks persist in Redis until processed. After Redis restart:

- Pending tasks remain in queues
- Active tasks may need manual intervention
- Use `rediq task list` to check task states

### Worker Recovery

Workers automatically:
- Reconnect to Redis
- Resume processing tasks
- Continue from where they left off

No manual intervention required for worker recovery.
