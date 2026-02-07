# Rediq User Guide: E-Commerce Order Processing System

**Version**: 0.1.1
**Updated**: 2026-02-07

---

## Table of Contents

1. [Project Overview](#1-project-overview)
2. [Business Scenario Analysis](#2-business-scenario-analysis)
3. [System Architecture Design](#3-system-architecture-design)
4. [Environment Setup](#4-environment-setup)
5. [Project Initialization](#5-project-initialization)
6. [Core Feature Implementation](#6-core-feature-implementation)
7. [Advanced Feature Applications](#7-advanced-feature-applications)
8. [Monitoring & Operations](#8-monitoring--operations)
9. [Troubleshooting](#9-troubleshooting)
10. [Best Practices](#10-best-practices)

---

## 1. Project Overview

### 1.1 Background

This guide uses an **e-commerce order processing system** as a case study to demonstrate how to build a reliable asynchronous task processing system using Rediq.

### 1.2 Business Scenario

A typical e-commerce order involves the following asynchronous processing steps:

```
Order Creation → Inventory Deduction → Payment Processing → Shipping Notification → Shipping
              ↓                  ↓                ↓                  ↓
          Inventory Rollback  Payment Retry    SMS Retry        Shipping Update
```

### 1.3 Why Choose Rediq?

| Requirement | Rediq Solution |
|-------------|----------------|
| High-concurrency order processing | ✅ Multiple queues + priorities |
| Payment failure retry | ✅ Auto retry + dead letter queue |
| Scheduled tasks (timeout cancel) | ✅ Cron periodic tasks |
| Dependencies (shipping requires payment) | ✅ Task dependencies |
| Progress tracking (batch import) | ✅ Progress reporting |
| Real-time monitoring | ✅ Prometheus + Dashboard |
| HA deployment | ✅ Redis Cluster/Sentinel |

---

## 2. Business Scenario Analysis

### 2.1 Task Type Design

| Task Type | Queue | Priority | Retry | Description |
|-----------|-------|----------|-------|-------------|
| `order:inventory` | `inventory` | 0 (high) | 3 | Inventory deduction |
| `order:payment` | `payment` | 0 (high) | 5 | Payment processing |
| `order:sms` | `notifications` | 50 (medium) | 3 | SMS notification |
| `order:shipping` | `shipping` | 30 (medium-high) | 3 | Shipping processing |
| `order:cancel` | `cancellations` | 0 (high) | 1 | Order cancellation |
| `order:timeout` | - | - | - | Scheduled timeout order check |
| `report:daily` | `reports` | 100 (low) | 1 | Daily report generation |

### 2.2 Task Dependencies

```
order:create (user places order)
    │
    ├─→ order:inventory (deduct inventory)
    │       │
    │       └─→ order:payment (process payment)
    │               │
    │               ├─→ order:sms (notify user)
    │               └─→ order:shipping (ship order)
    │
    └─→ order:timeout (check unpaid order after 24h)
```

---

## 3. System Architecture Design

### 3.1 Overall Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    E-commerce Frontend/APP                       │
└─────────────────────────────┬───────────────────────────────────┘
                              │ HTTP API
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Order Service (Rust)                        │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                   Business Logic Layer                     │  │
│  │  - Order validation - Price calculation - Coupon handling │  │
│  └───────────────────────────────────────────────────────────┘  │
│                              │                                   │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                   Rediq Client                            │  │
│  │  - Task creation - Enqueue operations - Status queries    │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────┬───────────────────────────────────┘
                              │ Redis Protocol
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Redis Cluster                              │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐           │
│  │  Node 1  │ │  Node 2  │ │  Node 3  │ │  Node 4  │           │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘           │
│                                                                  │
│  Stored Data:                                                    │
│  - Queue data (queue:*, active:*, retry:*, dead:*)             │
│  - Task details (task:*)                                        │
│  - Schedule data (delayed:*, cron:*)                            │
│  - Statistics (stats:*, meta:*)                                 │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Worker Service Pool                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │  Worker 1   │  │  Worker 2   │  │  Worker N   │             │
│  │  ┌────────┐ │  │  ┌────────┐ │  │  ┌────────┐ │             │
│  │  │Scheduler│ │  │  │Scheduler│ │  │  │Scheduler│             │
│  │  └───┬────┘ │  │  └───┬────┘ │  │  └───┬────┘ │             │
│  │      │      │  │      │      │  │      │      │             │
│  │  ┌───┴────┐ │  │  ┌───┴────┐ │  │  ┌───┴────┐ │             │
│  │  │Worker  │ │  │  │Worker  │ │  │  │Worker  │ │             │
│  │  │- 10 thr │ │  │  │- 10 thr │ │  │  │- 10 thr │ │             │
│  │  │- Mux   │ │  │  │- Mux   │ │  │  │- Mux   │ │             │
│  │  └────────┘ │  │  └────────┘ │  │  └────────┘ │             │
│  └─────────────┘  └─────────────┘  └─────────────┘             │
│                                                                  │
│  Queue Assignment:                                               │
│  - Worker 1: inventory, payment (high priority)                │
│  - Worker 2: shipping, cancellations                            │
│  - Worker N: notifications, reports (low priority)              │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      External Service Integration                 │
│  - Inventory Service - Payment Gateway - SMS Service - Logistics│
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Monitoring & Management                     │
│  - Prometheus - Grafana - Rediq CLI Dashboard                   │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 Deployment Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Load Balancer                             │
│                 (Nginx/ALB)                                  │
└──────────────────┬──────────────────────────────────────────┘
                   │
     ┌─────────────┼─────────────┐
     ▼             ▼             ▼
┌─────────┐  ┌─────────┐  ┌─────────┐
│ API 1   │  │ API 2   │  │ API 3   │  ← Order service cluster
└─────────┘  └─────────┘  └─────────┘
     │             │             │
     └─────────────┼─────────────┘
                   ▼
          ┌─────────────────┐
          │  Redis Cluster  │  ← Shared storage
          │   (3 master + 3 replica) │
          └─────────────────┘
                   │
     ┌─────────────┼─────────────┐
     ▼             ▼             ▼
┌─────────┐  ┌─────────┐  ┌─────────┐
│Worker 1 │  │Worker 2 │  │Worker 3 │  ← Worker cluster
│(high)   │  │(medium) │  │(low)    │
└─────────┘  └─────────┘  └─────────┘
```

---

## 4. Environment Setup

### 4.1 System Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| Rust | 1.70+ | 1.75+ |
| Redis | 6.0+ | 7.0+ Cluster |
| CPU | 2 cores | 4+ cores |
| Memory | 4GB | 8GB+ |
| Disk | 20GB SSD | 50GB+ SSD |

### 4.2 Redis Deployment

**Standalone Mode (Development):**

```bash
# Using Docker
docker run -d -p 6379:6379 redis:7-alpine

# With password
docker run -d -p 6379:6379 redis:7-alpine \
  redis-server --requirepass your_password
```

**Cluster Mode (Production):**

```bash
# Create Redis Cluster (3 master + 3 replica)
docker network create redis-cluster

for port in 7000 7001 7002 7003 7004 7005; do
  docker run -d --net redis-cluster -p ${port}:${port} \
    --name redis-${port} redis:7-alpine \
    redis-server --port ${port} --cluster-enabled yes \
    --cluster-config-file nodes-${port}.conf \
    --appendonly yes
done

# Initialize cluster
docker exec -it redis-7000 redis-cli --cluster create \
  172.18.0.2:7000 172.18.0.3:7001 172.18.0.4:7002 \
  172.18.0.5:7003 172.18.0.6:7004 172.18.0.7:7005 \
  --cluster-replicas 1
```

### 4.3 Environment Variables

Create `.env` file:

```bash
# Redis configuration
REDIS_URL=redis://:password@localhost:6379/0

# Service configuration
SERVICE_PORT=8080
WORKER_CONCURRENCY=10
WORKER_QUEUES=inventory,payment,shipping,notifications,cancellations,reports

# Monitoring configuration
METRICS_PORT=9090
METRICS_ENABLED=true

# Business configuration
ORDER_TIMEOUT_HOURS=24
PAYMENT_RETRY_SECONDS=300
SMS_RATE_LIMIT=100
```

---

## 5. Project Initialization

### 5.1 Create Project

```bash
# Create new project
cargo new order-system && cd order-system

# Add dependencies
cargo add rediq --features metrics-http
cargo add tokio --features full
cargo add serde --features derive
cargo add serde_json
cargo add anyhow
cargo add tracing
cargo add tracing-subscriber
cargo add uuid --features serde,v4
cargo add chrono --features serde
cargo add async-trait
cargo add clap --features derive
cargo add dotenv
```

### 5.2 Project Structure

```
order-system/
├── Cargo.toml
├── .env
├── src/
│   ├── main.rs              # Program entry
│   ├── config.rs            # Configuration management
│   ├── handlers/            # Task handlers
│   │   ├── mod.rs
│   │   ├── inventory.rs     # Inventory handler
│   │   ├── payment.rs       # Payment handler
│   │   ├── notification.rs  # Notification handler
│   │   └── shipping.rs      # Shipping handler
│   ├── tasks/               # Task definitions
│   │   ├── mod.rs
│   │   └── orders.rs        # Order tasks
│   ├── api/                 # API service
│   │   ├── mod.rs
│   │   └── orders.rs        # Order API
│   └── middleware.rs        # Custom middleware
├── tests/
│   └── integration_test.rs  # Integration tests
└── docker-compose.yml       # Deployment configuration
```

### 5.3 Configuration File

`src/config.rs`:

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub redis_url: String,
    pub redis_mode: RedisMode,
    pub service_port: u16,
    pub worker_concurrency: usize,
    pub worker_queues: Vec<String>,
    pub metrics_port: u16,
    pub metrics_enabled: bool,
    pub order_timeout_hours: u64,
    pub payment_retry_seconds: u64,
    pub sms_rate_limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RedisMode {
    Standalone,
    Cluster,
    Sentinel,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenv::dotenv().ok();

        Ok(Config {
            redis_url: env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://localhost:6379".to_string()),
            redis_mode: RedisMode::Standalone,
            service_port: env::var("SERVICE_PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()?,
            worker_concurrency: env::var("WORKER_CONCURRENCY")
                .unwrap_or_else(|_| "10".to_string())
                .parse()?,
            worker_queues: env::var("WORKER_QUEUES")
                .unwrap_or_else(|_| "default".to_string())
                .split(',')
                .map(|s| s.to_string())
                .collect(),
            metrics_port: env::var("METRICS_PORT")
                .unwrap_or_else(|_| "9090".to_string())
                .parse()?,
            metrics_enabled: env::var("METRICS_ENABLED")
                .unwrap_or_else(|_| "true".to_string())
                .parse()?,
            order_timeout_hours: env::var("ORDER_TIMEOUT_HOURS")
                .unwrap_or_else(|_| "24".to_string())
                .parse()?,
            payment_retry_seconds: env::var("PAYMENT_RETRY_SECONDS")
                .unwrap_or_else(|_| "300".to_string())
                .parse()?,
            sms_rate_limit: env::var("SMS_RATE_LIMIT")
                .unwrap_or_else(|_| "100".to_string())
                .parse()?,
        })
    }
}
```

---

## 6. Core Feature Implementation

### 6.1 Order Data Structures

`src/tasks/orders.rs`:

```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: Uuid,
    pub user_id: Uuid,
    pub items: Vec<OrderItem>,
    pub total_amount: u64, // in cents
    pub status: OrderStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderItem {
    pub product_id: Uuid,
    pub quantity: u32,
    pub price: u64, // in cents
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OrderStatus {
    Pending,      // Pending processing
    Reserved,     // Inventory reserved
    Paid,         // Paid
    Shipped,      // Shipped
    Cancelled,    // Cancelled
    Timeout,      // Timeout cancelled
}

// Task Payload structures

#[derive(Debug, Serialize, Deserialize)]
pub struct InventoryTaskPayload {
    pub order_id: Uuid,
    pub items: Vec<OrderItem>,
    pub operation: InventoryOperation,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum InventoryOperation {
    Reserve,   // Reserve inventory
    Commit,    // Commit (after payment success)
    Rollback,  // Rollback (payment failure/cancellation)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaymentTaskPayload {
    pub order_id: Uuid,
    pub user_id: Uuid,
    pub amount: u64,
    pub payment_method: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NotificationTaskPayload {
    pub user_id: Uuid,
    pub template: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ShippingTaskPayload {
    pub order_id: Uuid,
    pub address: ShippingAddress,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ShippingAddress {
    pub recipient: String,
    pub phone: String,
    pub province: String,
    pub city: String,
    pub district: String,
    pub detail: String,
    pub postal_code: String,
}
```

### 6.2 Inventory Handler

`src/handlers/inventory.rs`:

```rust
use anyhow::Result;
use async_trait::async_trait;
use rediq::{Task, Handler};
use tracing::{info, error, warn};

use crate::tasks::orders::{InventoryTaskPayload, InventoryOperation};

pub struct InventoryHandler {
    // Inject inventory service client in real projects
}

impl InventoryHandler {
    pub fn new() -> Self {
        Self {}
    }

    async fn reserve_inventory(&self, order_id: uuid::Uuid, items: Vec<crate::tasks::orders::OrderItem>) -> Result<()> {
        // Call inventory service to reserve inventory
        info!(order_id = %order_id, "Reserving inventory for {} items", items.len());

        // TODO: Call actual inventory service API
        // let response = inventory_client.reserve(order_id, items).await?;

        // Simulate processing
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        info!(order_id = %order_id, "Inventory reserved successfully");
        Ok(())
    }

    async fn commit_inventory(&self, order_id: uuid::Uuid) -> Result<()> {
        info!(order_id = %order_id, "Committing inventory");

        // TODO: Call actual inventory service API
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        info!(order_id = %order_id, "Inventory committed successfully");
        Ok(())
    }

    async fn rollback_inventory(&self, order_id: uuid::Uuid) -> Result<()> {
        warn!(order_id = %order_id, "Rolling back inventory");

        // TODO: Call actual inventory service API
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        info!(order_id = %order_id, "Inventory rolled back successfully");
        Ok(())
    }
}

#[async_trait]
impl Handler for InventoryHandler {
    async fn handle(&self, task: &Task) -> Result<()> {
        let payload: InventoryTaskPayload = serde_json::from_str(&task.payload)?;

        match payload.operation {
            InventoryOperation::Reserve => {
                self.reserve_inventory(payload.order_id, payload.items).await?;
            }
            InventoryOperation::Commit => {
                self.commit_inventory(payload.order_id).await?;
            }
            InventoryOperation::Rollback => {
                self.rollback_inventory(payload.order_id).await?;
            }
        }

        Ok(())
    }
}
```

### 6.3 Payment Handler

`src/handlers/payment.rs`:

```rust
use anyhow::Result;
use async_trait::async_trait;
use rediq::{Task, Handler, Client};
use tracing::{info, error, warn};

use crate::tasks::orders::{PaymentTaskPayload, NotificationTaskPayload};
use uuid::Uuid;

pub struct PaymentHandler {
    client: Client,
}

impl PaymentHandler {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    async fn process_payment(&self, order_id: Uuid, user_id: Uuid, amount: u64, method: &str) -> Result<bool> {
        info!(
            order_id = %order_id,
            user_id = %user_id,
            amount = amount,
            method = method,
            "Processing payment"
        );

        // TODO: Call actual payment gateway
        // Simulate processing
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        // Simulate 95% success rate
        let success = rand::random::<f32>() > 0.05;

        if success {
            info!(order_id = %order_id, "Payment successful");
        } else {
            error!(order_id = %order_id, "Payment failed");
        }

        Ok(success)
    }

    async fn send_payment_success_notification(&self, user_id: Uuid, order_id: Uuid) -> Result<()> {
        let task = Task::builder("order:sms")
            .queue("notifications")
            .payload(&serde_json::to_value(NotificationTaskPayload {
                user_id,
                template: "payment_success".to_string(),
                params: serde_json::json!({"order_id": order_id}),
            })?)?
            .priority(50)
            .build()?;

        self.client.enqueue(task).await?;
        Ok(())
    }
}

#[async_trait]
impl Handler for PaymentHandler {
    async fn handle(&self, task: &Task) -> Result<()> {
        let payload: PaymentTaskPayload = serde_json::from_str(&task.payload)?;

        let success = self.process_payment(
            payload.order_id,
            payload.user_id,
            payload.amount,
            &payload.payment_method,
        ).await?;

        if success {
            // Payment successful, send notification
            self.send_payment_success_notification(payload.user_id, payload.order_id).await?;
        } else {
            // Payment failed, needs retry
            return Err(anyhow::anyhow!("Payment processing failed"));
        }

        Ok(())
    }
}
```

### 6.4 Notification Handler

`src/handlers/notification.rs`:

```rust
use anyhow::Result;
use async_trait::async_trait;
use rediq::Task;
use tracing::info;

use crate::tasks::orders::NotificationTaskPayload;

pub struct NotificationHandler;

impl NotificationHandler {
    pub fn new() -> Self {
        Self
    }

    async fn send_sms(&self, user_id: uuid::Uuid, template: &str, params: serde_json::Value) -> Result<()> {
        info!(
            user_id = %user_id,
            template = template,
            params = %params,
            "Sending SMS notification"
        );

        // TODO: Call actual SMS service API
        // sms_client.send(user_id, template, params).await?;

        // Simulate sending
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        info!("SMS sent successfully");
        Ok(())
    }
}

#[async_trait]
impl Handler for NotificationHandler {
    async fn handle(&self, task: &Task) -> Result<()> {
        let payload: NotificationTaskPayload = serde_json::from_str(&task.payload)?;

        self.send_sms(payload.user_id, &payload.template, payload.params).await?;

        Ok(())
    }
}
```

### 6.5 Order API

`src/api/orders.rs`:

```rust
use anyhow::Result;
use rediq::{Client, Task};
use serde_json::json;
use uuid::Uuid;

use crate::tasks::orders::{
    Order, OrderItem, OrderStatus,
    InventoryTaskPayload, InventoryOperation,
    PaymentTaskPayload, ShippingTaskPayload,
};

pub struct OrderApi {
    client: Client,
}

impl OrderApi {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Create order
    pub async fn create_order(&self, user_id: Uuid, items: Vec<OrderItem>) -> Result<Order> {
        let order_id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let total_amount: u64 = items.iter().map(|i| i.price * i.quantity as u64).sum();

        // 1. Create order record (save to database in real project)
        let order = Order {
            id: order_id,
            user_id,
            items: items.clone(),
            total_amount,
            status: OrderStatus::Pending,
            created_at: now,
            updated_at: now,
        };

        // TODO: Save order to database
        // db.save_order(&order).await?;

        // 2. Create inventory reserve task (high priority)
        let inventory_task = Task::builder("order:inventory")
            .queue("inventory")
            .payload(&serde_json::to_value(InventoryTaskPayload {
                order_id,
                items,
                operation: InventoryOperation::Reserve,
            })?)?
            .priority(0)  // Highest priority
            .max_retry(3)
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        self.client.enqueue(inventory_task).await?;

        // 3. Create timeout check task (24 hours later)
        let timeout_task = Task::builder("order:timeout")
            .queue("cancellations")
            .payload(&json!({
                "order_id": order_id,
            }))?
            .delay(std::time::Duration::from_secs(24 * 3600))
            .unique_key(format!("timeout:{}", order_id))  // Prevent duplicates
            .build()?;

        self.client.enqueue(timeout_task).await?;

        tracing::info!(order_id = %order_id, "Order created successfully");

        Ok(order)
    }

    /// Pay for order
    pub async fn pay_order(&self, order_id: Uuid, user_id: Uuid, amount: u64, method: String) -> Result<()> {
        // Create payment task
        let payment_task = Task::builder("order:payment")
            .queue("payment")
            .payload(&serde_json::to_value(PaymentTaskPayload {
                order_id,
                user_id,
                amount,
                payment_method: method,
            })?)?
            .priority(0)  // Highest priority
            .max_retry(5)  // More retries for payment
            .timeout(std::time::Duration::from_secs(60))
            .build()?;

        self.client.enqueue(payment_task).await?;

        tracing::info!(order_id = %order_id, "Payment task enqueued");

        Ok(())
    }

    /// Cancel order
    pub async fn cancel_order(&self, order_id: Uuid) -> Result<()> {
        // Create inventory rollback task
        let rollback_task = Task::builder("order:inventory")
            .queue("inventory")
            .payload(&json!({
                "order_id": order_id,
                "items": [],
                "operation": "rollback",
            }))?
            .priority(0)  // High priority, process ASAP
            .max_retry(1)  // Only retry once
            .build()?;

        self.client.enqueue(rollback_task).await?;

        tracing::info!(order_id = %order_id, "Order cancellation task enqueued");

        Ok(())
    }
}
```

---

## 7. Advanced Feature Applications

### 7.1 Task Dependencies: Ensure Shipping After Payment

```rust
use rediq::{Client, Task};

pub async fn create_shipping_after_payment(
    client: &Client,
    order_id: Uuid,
    address: ShippingAddress,
    payment_task_id: String,
) -> Result<()> {
    let shipping_task = Task::builder("order:shipping")
        .queue("shipping")
        .payload(&serde_json::to_value(ShippingTaskPayload {
            order_id,
            address,
        })?)?
        .priority(30)
        .depends_on(&[payment_task_id])  // Depends on payment task
        .build()?;

    client.enqueue(shipping_task).await?;

    Ok(())
}
```

### 7.2 Progress Tracking: Batch Import Orders

```rust
use rediq::{Task, TaskProgressExt};
use async_trait::async_trait;

pub struct BatchImportHandler {
    client: Client,
}

#[async_trait]
impl Handler for BatchImportHandler {
    async fn handle(&self, task: &Task) -> Result<()> {
        if let Some(progress) = task.progress() {
            let total: usize = task.extra.get("total").unwrap();

            // Report initial progress
            progress.report_with_message(0, Some("Starting import...")).await?;

            for (i, order_data) in orders_data.iter().enumerate() {
                // Process each order
                self.import_single_order(order_data).await?;

                // Report progress
                let percent = ((i + 1) * 100 / total) as u8;
                progress.report_with_message(
                    percent,
                    Some(&format!("Imported {}/{}", i + 1, total))
                ).await?;
            }

            progress.report_with_message(100, Some("Import completed!")).await?;
        }

        Ok(())
    }
}
```

### 7.3 Middleware: Enhanced Logging

```rust
use rediq::middleware::{Middleware, MiddlewareContext};
use async_trait::async_trait;

pub struct OrderMiddleware;

#[async_trait]
impl Middleware for OrderMiddleware {
    async fn before(&self, ctx: &MiddlewareContext) -> rediq::Result<()> {
        tracing::info!(
            task_id = %ctx.task.id,
            task_type = %ctx.task.task_type,
            queue = %ctx.task.queue,
            "Starting task processing"
        );
        Ok(())
    }

    async fn after(&self, ctx: &MiddlewareContext, result: &rediq::Result<()>) {
        match result {
            Ok(()) => {
                tracing::info!(
                    task_id = %ctx.task.id,
                    "Task completed successfully"
                );
            }
            Err(e) => {
                tracing::error!(
                    task_id = %ctx.task.id,
                    error = %e,
                    "Task failed"
                );
            }
        }
    }
}
```

### 7.4 Cron Tasks: Scheduled Timeout Check

```rust
// Check for timeout unpaid orders every hour
let cron_task = Task::builder("order:timeout_check")
    .queue("cancellations")
    .cron("0 * * * *")  // Run every hour
    .build()?;

client.enqueue(cron_task).await?;
```

---

## 8. Monitoring & Operations

### 8.1 Enable Metrics

```rust
use rediq::server::Server;

let mut server = Server::from(state);

// Enable HTTP metrics endpoint
if config.metrics_enabled {
    let addr = format!("0.0.0.0:{}", config.metrics_port).parse()?;
    server.enable_metrics_on(addr);
    tracing::info!(port = config.metrics_port, "Metrics server started");
}
```

### 8.2 Prometheus Metrics Example

```yaml
# prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'rediq'
    static_configs:
      - targets: ['worker1:9090', 'worker2:9090', 'worker3:9090']
```

### 8.3 Grafana Dashboard

Recommended monitoring metrics:

| Metric | Description | Alert Threshold |
|--------|-------------|-----------------|
| `rediq_queue_pending` | Pending task count | > 1000 |
| `rediq_queue_retry` | Retry task count | > 100 |
| `rediq_queue_dead` | Dead task count | > 10 |
| `rediq_task_processing_duration` | Task processing time | P99 > 5s |
| `rediq_task_error_total` | Total errors | Growth rate > 10/min |

### 8.4 CLI Dashboard Usage

```bash
# Start real-time monitoring
rediq dash

# Custom refresh interval
rediq dash --interval 2000

# View specific queue
rediq queue inspect payment

# View worker status
rediq worker list

# View task details
rediq task inspect <task-id>

# Pause queue (for maintenance)
rediq queue pause payment

# Resume queue
rediq queue resume payment

# Flush dead letter queue (use with caution)
rediq queue flush dead:payment
```

---

## 9. Troubleshooting

### 9.1 Tasks Stuck Not Processing

**Symptoms**: Queue has tasks but Worker doesn't consume

**Debug Steps**:

1. Check if queue is paused:
```bash
rediq queue list
```

2. Check Worker status:
```bash
rediq worker list
```

3. Check task details:
```bash
rediq task inspect <task-id>
```

**Solution**:
```bash
# Resume paused queue
rediq queue resume <queue-name>

# Restart Worker
systemctl restart order-worker
```

### 9.2 Tasks Keep Retrying

**Symptoms**: Task enters retry queue loop

**Possible Causes**:
- Handler code bug
- Dependent service unavailable
- Task payload error

**Debug**:
```bash
# View retry queue
rediq queue inspect retry:payment

# View error logs
journalctl -u order-worker -f
```

**Solution**:
```rust
// Add max retry count
.max_retry(3)

// Or use retry_for strategy
.timeout(Duration::from_secs(30))
```

### 9.3 Redis Connection Issues

**Symptoms**: Worker frequently disconnects

**Debug**:
```bash
# Check Redis status
redis-cli ping

# Check connection count
redis-cli info clients

# Check slow queries
redis-cli slowlog get 10
```

**Solution**:
```rust
// Increase connection pool config
let client = Client::builder()
    .redis_url(&config.redis_url)
    .connection_pool_size(20)
    .build()
    .await?;
```

### 9.4 Memory Leaks

**Symptoms**: Worker memory continuously grows

**Debug**:
```bash
# Check with valgrind
valgrind --leak-check=full ./order-worker

# Profile with heaptrack
heaptrack ./order-worker
```

**Common Causes**:
- Task payload too large
- Middleware data accumulation
- Unreleased connections

---

## 10. Best Practices

### 10.1 Task Design Principles

1. **Idempotency**: Repeated task execution should produce the same result
```rust
pub async fn handle(&self, task: &Task) -> Result<()> {
    // Check if already processed first
    if self.is_already_processed(task.id).await? {
        return Ok(());
    }
    // Processing logic...
}
```

2. **Atomicity**: Use Redis transactions to ensure operation atomicity
3. **Short Lifecycle**: Single task processing time < 5 minutes
4. **Small Payload**: Task data < 512KB

### 10.2 Queue Design

| Queue Type | Priority | Worker Count | Description |
|------------|----------|--------------|-------------|
| `inventory` | 0 | High | Core business, fast response |
| `payment` | 0 | High | Involves money, highest priority |
| `shipping` | 30 | Medium | Daily processing |
| `notifications` | 50 | Low | Can be delayed |
| `reports` | 100 | Low | Scheduled tasks |

### 10.3 Error Handling

```rust
use rediq::error::Error;

pub async fn handle(&self, task: &Task) -> Result<()> {
    match self.process(task).await {
        Ok(_) => Ok(()),
        Err(e) => {
            // Decide whether to retry based on error type
            if is_transient_error(&e) {
                Err(Error::Retryable(e))
            } else {
                Err(Error::Permanent(e))
            }
        }
    }
}
```

### 10.4 Deployment Recommendations

1. **Separate Worker Deployment**: Deploy workers separately from API services
2. **Multiple Instances**: At least 2 worker instances
3. **Resource Limits**: Set CPU/memory limits
```yaml
# Kubernetes resources
resources:
  requests:
    cpu: 500m
    memory: 512Mi
  limits:
    cpu: 2000m
    memory: 2Gi
```

4. **Health Checks**:
```rust
// /health endpoint
async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "redis_connected": redis.ping().await.is_ok(),
        "queues": worker.active_queues()
    }))
}
```

### 10.5 Security Recommendations

1. **Redis Password**: Must set password in production
2. **TLS Encryption**: Use rediss:// connection
3. **Network Isolation**: Redis not exposed externally
4. **Sensitive Data**: Task payload should not contain plaintext passwords

---

## Appendix

### A. Complete Examples

See `examples/` directory for complete example code.

### B. API Reference

Complete API documentation: [docs/api-reference.md](api-reference.md)

### C. Error Codes

| Error Code | Description | Solution |
|-----------|-------------|----------|
| E001 | Redis connection failed | Check Redis service |
| E002 | Task serialization failed | Check Payload format |
| E003 | Queue doesn't exist | Auto-create or manual init |
| E004 | Worker timeout | Increase timeout or optimize logic |

### D. Common Commands Quick Reference

```bash
# View all queues
rediq queue list

# View queue details
rediq queue inspect <name>

# Pause/resume queue
rediq queue pause <name>
rediq queue resume <name>

# View workers
rediq worker list

# View dashboard
rediq dash

# View task
rediq task inspect <id>

# Clean up dead letter queue
rediq queue flush dead:<queue-name>
```

---

**Document Version**: 1.0.0
**Last Updated**: 2026-02-07
**Maintainer**: Rediq Team
