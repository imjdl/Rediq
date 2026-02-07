# Rediq

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

**Rediq** 是一个基于 Rust 的分布式任务队列框架，灵感来自 [Asynq](https://github.com/hibiken/asynq)。它提供了一个强大的、可用于生产环境的后台任务处理解决方案，使用 Redis 作为后端存储。

## 为什么选择 Rediq？

Rediq 简化了 Rust 应用中的后台任务处理。无论是发送邮件、处理支付、生成报表，还是处理任何异步工作，Rediq 都能提供可靠、可扩展的解决方案。

### 核心优势

- **简单易用的 API** - 直观的构建器模式，轻松创建任务
- **生产就绪** - 经过实战验证，具备重试机制、死信队列和监控能力
- **高性能** - 基于 Tokio 构建，采用高效的 Redis Pipeline 操作
- **可观测性** - 内置 Prometheus 指标和 HTTP 端点
- **灵活强大** - 支持优先级、延迟、Cron 和任务依赖

### 仪表板

![Rediq 仪表板](img/dash.png)

## 功能特性

| 特性 | 描述 |
|---------|-------------|
| **多队列支持** | 同时从多个队列并发处理任务，支持不同优先级 |
| **优先级队列** | 基于优先级执行任务（0-100，数值越小优先级越高） |
| **延迟任务** | 安排任务在指定延迟后执行 |
| **周期性任务** | Cron 风格的周期性任务调度（`0 2 * * *`） |
| **自动重试** | 可配置的重试机制，支持指数退避（2s、4s、8s...） |
| **死信队列** | 失败任务自动移动到死信队列 |
| **任务依赖** | 定义任务执行依赖关系（B 等待 A 完成后执行） |
| **中间件系统** | 内置日志、指标中间件和自定义钩子支持 |
| **Prometheus 指标** | 内置可观测性，可选 HTTP 端点（`/metrics`） |
| **Redis 高可用** | 支持 Redis Cluster 和 Sentinel 实现高可用 |

## 快速开始

### 安装

```toml
[dependencies]
rediq = "0.1"
tokio = { version = "1", features = ["full"] }
```

### 基本示例

**生产者（入队任务）：**

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
        "subject": "欢迎！"
    }))?
    .build()?;

client.enqueue(task).await?;
```

**消费者（处理任务）：**

```rust
use rediq::server::{Server, ServerBuilder};
use rediq::processor::{Handler, Mux};
use async_trait::async_trait;

struct EmailHandler;

#[async_trait]
impl Handler for EmailHandler {
    async fn handle(&self, task: &Task) -> rediq::Result<()> {
        println!("处理任务: {}", task.id);
        // 您的任务处理逻辑
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

## 核心概念

### 任务生命周期

```
┌──────────┐      ┌───────────┐      ┌──────────┐
│  待处理   │ ───▶ │   处理中  │ ───▶ │  已完成  │
│          │      │           │      │          │
└──────────┘      └───────────┘      └──────────┘
     │                   │
     ▼                   ▼
┌──────────┐      ┌───────────┐
│  延迟中   │      │   重试中  │
│          │      │           │
└──────────┘      └───────────┘
     │                   │
     └───────────────────┘
                 │
                 ▼
         ┌───────────┐
         │   死信队列  │
         └───────────┘
```

### 任务选项

| 选项 | 类型 | 默认值 | 描述 |
|--------|------|---------|-------------|
| `queue` | `String` | `"default"` | 目标队列名称 |
| `max_retry` | `u32` | `3` | 最大重试次数 |
| `timeout` | `Duration` | `30s` | 任务超时时间 |
| `delay` | `Duration` | `None` | 执行延迟 |
| `priority` | `i32` | `50` | 优先级（0-100，越小越高） |
| `cron` | `String` | `None` | Cron 表达式，用于周期性任务 |
| `unique_key` | `String` | `None` | 用于去重的唯一键 |
| `depends_on` | `Vec<String>` | `None` | 任务依赖 ID |

## 高级用法

### 优先级队列

优先处理紧急任务：

```rust
let task = Task::builder("urgent:payment")
    .queue("payments")
    .priority(0)  // 最高优先级
    .build()?;
```

### 延迟任务

在指定延迟后执行：

```rust
use std::time::Duration;

let task = Task::builder("scheduled:report")
    .delay(Duration::from_secs(3600))  // 1 小时后执行
    .build()?;
```

### 周期性任务

按 Cron 调度执行：

```rust
let task = Task::builder("daily:cleanup")
    .cron("0 2 * * *")  // 每天凌晨 2 点
    .build()?;
```

### 任务依赖

按顺序执行任务：

```rust
let task_b = Task::builder("process:b")
    .depends_on(&["task-a-id"])
    .build()?;
// B 将等待 A 完成后执行
```

### 中间件

添加日志和指标：

```rust
use rediq::middleware::{MiddlewareChain, LoggingMiddleware, MetricsMiddleware};

let middleware = MiddlewareChain::new()
    .add(LoggingMiddleware::new())
    .add(MetricsMiddleware::new());

let state = ServerBuilder::new()
    .middleware(middleware)
    .build()
    .await?;
```

### HTTP 指标端点

启用 Prometheus 指标：

```rust
let mut server = Server::from(state);
server.enable_metrics("0.0.0.0:9090".parse()?);
// 访问地址: http://localhost:9090/metrics
```

### Redis 高可用

**Cluster 模式：**

```rust
let client = Client::builder()
    .redis_url("redis://cluster-node:6379")
    .cluster_mode()
    .build()
    .await?;
```

**Sentinel 模式：**

```rust
let client = Client::builder()
    .redis_url("redis://sentinel:26379")
    .sentinel_mode()
    .build()
    .await?;
```

## CLI 工具

Rediq 包含一个队列管理 CLI 工具：

```bash
# 实时仪表板（带历史趋势）
rediq dash

# 仪表板功能：
# - 实时队列统计（待处理、处理中、已完成、死信、重试）
# - 实时历史趋势图表（Sparkline 可视化）
# - 错误率追踪和可视化
# - Worker 状态和健康监控
# - 自动缩放图表及时间范围指示器

# 队列操作
rediq queue list
rediq queue inspect <name>
rediq queue pause <name>
rediq queue resume <name>
rediq queue flush <name>

# 任务操作
rediq task list <queue>
rediq task inspect <id>
rediq task cancel --id <id> --queue <queue>

# Worker 管理
rediq worker list
rediq worker inspect <id>

# 统计信息
rediq stats --queue <name>
```

完整文档请参考 [CLI 指南](docs/cli.md)。

## 文档

| 文档 | 描述 |
|----------|-------------|
| [快速入门](docs/getting-started.md) | 安装和第一个任务 |
| [配置指南](docs/configuration.md) | 服务器、客户端和任务配置 |
| [架构设计](docs/architecture.md) | 系统设计和内部原理 |
| [API 参考](docs/api-reference.md) | 完整 API 文档 |
| [部署指南](docs/deployment.md) | 生产环境部署 |
| [故障排查](docs/troubleshooting.md) | 常见问题和解决方案 |
| [性能测试](docs/benchmark.md) | 性能基准测试 |

## 示例

```bash
# 基本用法
cargo run --example quickstart

# 优先级队列
cargo run --example priority_queue_example

# 周期性任务
cargo run --example cron_example

# 任务依赖
cargo run --example dependency_example

# 中间件
cargo run --example middleware_test

# HTTP 指标端点
cargo run --example metrics_http_example

# Redis 集群
cargo run --example cluster_example

# Redis 哨兵
cargo run --example sentinel_example
```

设置 `REDIS_URL` 环境变量以自定义 Redis 连接：

```bash
export REDIS_URL="redis://:password@host:6379/0"
cargo run --example quickstart
```

## 项目状态

Rediq 正在积极开发中，已可用于生产环境。当前状态：

- ✅ 核心任务队列功能
- ✅ 优先级队列
- ✅ 延迟和周期性任务
- ✅ 任务依赖
- ✅ 中间件系统
- ✅ Prometheus 指标
- ✅ Redis Cluster/Sentinel 支持
- ✅ CLI 工具和仪表板

查看 [CHANGELOG.md](CHANGELOG.md) 了解版本历史。

## 贡献

欢迎贡献！请随时：

1. 报告问题
2. 提出新功能建议
3. 提交 Pull Request

## 许可证

本项目采用以下任一许可证：

- MIT License ([LICENSE-MIT](LICENSE-MIT) 或 http://opensource.org/licenses/MIT)
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) 或 http://www.apache.org/licenses/LICENSE-2.0)

由您选择。

## 致谢

灵感来自 Go 语言的 [Asynq](https://github.com/hibiken/asynq)。
