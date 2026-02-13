# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.1] - 2026-02-14

### Added
- Added README.md documentation for `rediq-macros` crate

### Changed
- Updated documentation version references to 0.2
- Updated README files with v0.2.0 new features documentation

[0.2.1]: https://github.com/imjdl/rediq/releases/tag/v0.2.1

## [0.2.0] - 2026-02-11

### Added
- **Attribute Macros Support** - Simplified Handler registration with `#[task_handler]` macro
  - New `rediq-macros` crate with proc macros
  - `#[task_handler]` attribute macro for automatic Handler trait implementation
  - `register_handlers!` macro for batch handler registration
  - `handler_fn!` and `def_handler!` convenience macros
  - Enable with `features = ["macros"]`
- **Task Aggregation** - Support for batch processing scenarios
  - `Aggregator` trait for custom aggregation logic
  - `GroupAggregatorFunc` for function-based aggregation
  - `AggregatorManager` for managing aggregators
  - `TaskBuilder::group()` method for setting task group
  - New Redis keys: `group:{name}` and `meta:group:{name}`
  - `AggregatorConfig` for controlling aggregation behavior
- **Janitor Cleanup Mechanism** - Automatic cleanup of expired task details
  - `Janitor` and `JanitorConfig` for managing cleanup
  - Configurable cleanup interval, batch size, and TTL threshold
  - Prevents Redis memory leaks from stale task data
- **Connection Pool Configuration** - Fixed pool_size not being applied
  - New `PoolConfig` struct with comprehensive pool settings
  - `pool_size`, `min_idle`, `connection_timeout`, `idle_timeout`, `max_lifetime`
  - Works for both Client and Server
- **Payload Convenience Methods** - Easy payload deserialization
  - `Task::payload_json()` for JSON deserialization
  - `Task::payload_msgpack()` for MessagePack deserialization
- **New Example** - `macro_example.rs` demonstrating attribute macro usage

### Changed
- `ClientBuilder::pool_size()` now correctly applies the pool size setting
- `ServerConfig` now includes `pool_config`, `aggregator_config`, and `janitor_config` fields
- `TaskOptions` now includes optional `group` field

### Fixed
- Fixed `ClientBuilder::build()` ignoring `pool_size` configuration

[0.2.0]: https://github.com/imjdl/rediq/releases/tag/v0.2.0

## [0.1.3] - 2026-02-08

### Fixed
- Added missing README.md to rediq-cli package

[0.1.3]: https://github.com/imjdl/rediq/releases/tag/v0.1.3

## [0.1.2] - 2026-02-08

### Fixed
- **Security**: Fixed priority queue atomicity issue with improved retry logic
- **Security**: Fixed global configuration deadlock risk in `update_config()`
- **Security**: Enhanced task cancellation to check all queue states (pending, active, delayed, retry, dead, priority)
- **Security**: Added `ProgressGuard` to prevent memory leaks in progress tracking
- **Security**: Dynamic heartbeat TTL calculation to prevent false worker death detection

### Changed
- `cancel_task()` now returns `Option<String>` indicating where task was found
- `update_config()` signature changed to use `FnOnce(&mut RediqConfig)` instead of consuming/returning config
- Improved logging for concurrent dequeue operations

[0.1.2]: https://github.com/imjdl/rediq/releases/tag/v0.1.2

## [0.1.1] - 2026-02-07

### Added
- **Task execution progress tracking**: Report real-time progress (0-100%) during task execution
  - `TaskProgressExt` trait for accessing progress reporter in handlers
  - `ProgressContext` for reporting progress with optional messages
  - `Inspector::get_task_progress()` for querying task progress
  - Progress bar display in CLI Dashboard
  - Throttled updates (500ms default) to avoid excessive Redis writes
  - Independent Redis Hash storage (`rediq:progress:{task_id}`)
  - Auto-cleanup with TTL (1 hour default)
- Example: `progress_example.rs` demonstrating video processing with progress updates

### Changed
- Updated documentation (README, API reference, CLI docs) with progress tracking feature

[0.1.1]: https://github.com/imjdl/rediq/releases/tag/v0.1.1

## [0.1.0] - 2025-02-07

### Added
- Distributed task queue framework for Rust based on Redis
- Multiple queue support with concurrent task processing
- Priority queue support (0-100, lower value = higher priority)
- Delayed task scheduling
- Cron-style periodic task support
- Automatic retry mechanism with exponential backoff
- Dead letter queue
- Task dependencies
- Task deduplication (unique_key)
- Middleware system (before/after hooks)
- Prometheus metrics collection
- HTTP metrics endpoint (optional)
- Redis Cluster support
- Redis Sentinel high availability support
- CLI management tool (rediq-cli)
- Real-time monitoring dashboard
- Queue control (pause/resume/flush)
- Task cancellation
- Batch operations support
- Centralized configuration system
- Zero unwrap() panic risk
- Redis Pipeline optimizations for batch operations

[0.1.0]: https://github.com/imjdl/rediq/releases/tag/v0.1.0
