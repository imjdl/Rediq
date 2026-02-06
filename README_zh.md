# Rediq

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

**Rediq** 是一个基于 Rust 的分布式任务队列框架，灵感来自 [Asynq](https://github.com/hibiken/asynq)。它提供了一个强大的、可用于生产环境的后台任务处理解决方案，使用 Redis 作为后端存储。

## 功能特性

- **多队列支持** - 同时从多个队列处理任务
- **优先级队列** - 支持按优先级执行任务（0-100，数字越小优先级越高）
- **延迟任务** - 安排任务在指定延迟后执行
- **周期性任务** - 支持 Cron 风格的周期性任务调度
- **自动重试** - 可配置的重试机制，支持指数退避
- **死信队列** - 失败的任务会被移动到死信队列
- **任务依赖** - 定义任务执行之间的依赖关系
- **中间件系统** - 内置中间件，支持日志、指标和自定义钩子
- **Prometheus 指标** - 内置可观测性，可选 HTTP 端点
- **Redis 高可用** - 支持 Redis Cluster 和 Sentinel

## 快速开始

### 安装

在 `Cargo.toml` 中添加：

```toml
[dependencies]
rediq = "0.1"
tokio = { version = "1", features = ["full"] }
```

### 基本用法

#### 生产者（入队任务）

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
    // 创建客户端
    let client = Client::builder()
        .redis_url("redis://localhost:6379")
        .build()
        .await?;

    // 创建并入队任务
    let task = Task::builder("email:send")
        .queue("default")
        .payload(&EmailPayload {
            to: "user@example.com".to_string(),
            subject: "欢迎".to_string(),
            body: "你好！".to_string(),
        })?
        .max_retry(5)
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    client.enqueue(task).await?;

    Ok(())
}
```

#### 消费者（处理任务）

```rust
use rediq::server::{Server, ServerBuilder};
use rediq::processor::{Handler, Mux};
use async_trait::async_trait;
use rediq::Task;

struct EmailHandler;

#[async_trait]
impl Handler for EmailHandler {
    async fn handle(&self, task: &Task) -> rediq::Result<()> {
        // 处理任务
        println!("处理任务: {}", task.id);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 构建服务器状态
    let state = ServerBuilder::new()
        .redis_url("redis://localhost:6379")
        .queues(&["default", "critical"])
        .concurrency(10)
        .build()
        .await?;

    // 创建服务器
    let server = Server::from(state);

    // 注册处理器
    let mut mux = Mux::new();
    mux.handle("email:send", EmailHandler);

    // 运行服务器
    server.run(mux).await?;

    Ok(())
}
```

## 高级功能

### 优先级队列

```rust
let task = Task::builder("critical:task")
    .queue("default")
    .priority(0)  // 最高优先级
    .payload(&data)?
    .build()?;
```

### 延迟任务

```rust
use std::time::Duration;

let task = Task::builder("delayed:task")
    .queue("default")
    .delay(Duration::from_secs(300))  // 5分钟后执行
    .payload(&data)?
    .build()?;
```

### 周期性任务（Cron）

```rust
let task = Task::builder("scheduled:backup")
    .queue("default")
    .cron("0 2 * * *")  // 每天凌晨2点执行
    .payload(&data)?
    .build()?;
```

### 任务依赖

```rust
let task_b = Task::builder("process:b")
    .queue("default")
    .depends_on(&["task-a-id-123"])
    .payload(&data)?
    .build()?;
```

### 中间件

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

### Redis 集群

```rust
let client = Client::builder()
    .redis_url("redis://cluster-node:6379")
    .cluster_mode()
    .build()
    .await?;
```

### Redis 哨兵

```rust
let client = Client::builder()
    .redis_url("redis://sentinel:26379")
    .sentinel_mode()
    .build()
    .await?;
```

## 配置选项

### 服务器选项

| 选项 | 类型 | 默认值 | 描述 |
|------|------|--------|------|
| `redis_url` | `String` | `redis://localhost:6379` | Redis 连接 URL |
| `queues` | `Vec<String>` | `["default"]` | 要消费的队列列表 |
| `concurrency` | `usize` | `10` | 并发工作线程数 |
| `heartbeat_interval` | `u64` | `5` | 工作线程心跳间隔（秒） |
| `worker_timeout` | `u64` | `30` | 工作线程超时（秒） |
| `dequeue_timeout` | `u64` | `2` | BLPOP 超时（秒） |
| `poll_interval` | `u64` | `100` | 队列轮询间隔（毫秒） |

### 任务选项

| 选项 | 类型 | 默认值 | 描述 |
|------|------|--------|------|
| `queue` | `String` | `"default"` | 目标队列 |
| `max_retry` | `u32` | `3` | 最大重试次数 |
| `timeout` | `Duration` | `30s` | 任务超时时间 |
| `delay` | `Duration` | `None` | 执行延迟 |
| `priority` | `i32` | `50` | 优先级（0-100，越小越高） |
| `unique_key` | `String` | `None` | 去重唯一键 |
| `depends_on` | `Vec<String>` | `None` | 任务依赖 ID 列表 |

## CLI 工具

Rediq 包含一个 CLI 工具用于管理队列：

```bash
# 列出队列
rediq queue list

# 暂停队列
rediq queue pause default

# 恢复队列
rediq queue resume default

# 列出工作线程
rediq worker list

# 查看任务详情
rediq task show <task-id>
```

## 架构

```
┌─────────┐     ┌─────────┐     ┌──────────┐
│ Client  │────▶│  Redis  │────▶│ Worker   │
│ (生产者)     │  队列   │     │(消费者) │
└─────────┘     └─────────┘     └──────────┘
                                      │
                                      ▼
                                ┌──────────┐
                                │ Handler  │
                                └──────────┘
```

### Redis 键结构

| 键模式 | 类型 | 描述 |
|--------|------|------|
| `queue:{name}` | List | 待处理任务 |
| `pqueue:{name}` | ZSet | 优先级队列 |
| `active:{name}` | List | 正在处理 |
| `delayed:{name}` | ZSet | 延迟任务 |
| `retry:{name}` | ZSet | 等待重试 |
| `dead:{name}` | List | 死信队列 |
| `task:{id}` | Hash | 任务详情 |
| `pending_deps:{id}` | Set | 待满足的依赖 |
| `task_deps:{id}` | Set | 依赖此任务的任务 |

## 项目结构

```
Rediq/
├── rediq/           # 核心库
│   ├── client/      # 生产者 SDK
│   ├── server/      # 服务器和工作线程
│   ├── task/        # 任务模型
│   ├── processor/   # Handler trait 和路由器
│   ├── middleware/  # 中间件系统
│   ├── storage/     # Redis 存储层
│   └── observability/ # 指标和监控
├── rediq-cli/       # CLI 工具
├── examples/        # 使用示例
└── scripts/lua/     # Redis Lua 脚本
```

## 示例

查看 [examples](examples/) 目录了解更多示例：

- `quickstart.rs` - 基本用法
- `priority_queue_example.rs` - 优先级队列
- `cron_example.rs` - 周期性任务
- `dependency_example.rs` - 任务依赖
- `cluster_example.rs` - Redis 集群
- `sentinel_example.rs` - Redis 哨兵
- `middleware_test.rs` - 中间件使用
- `metrics_http_example.rs` - HTTP 指标端点

## 开发

```bash
# 构建
cargo build

# 运行测试
cargo test

# 运行示例
cargo run --example quickstart

# 格式化代码
cargo fmt

# 运行检查器
cargo clippy
```

## 贡献

欢迎贡献！请随时提交 Pull Request。

## 许可证

本项目采用以下任一许可证：

- MIT License ([LICENSE-MIT](LICENSE-MIT) 或 http://opensource.org/licenses/MIT)
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) 或 http://www.apache.org/licenses/LICENSE-2.0)

由您选择。

## 致谢

灵感来自 Go 语言的 [Asynq](https://github.com/hibiken/asynq)。
