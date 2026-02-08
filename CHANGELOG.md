# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
