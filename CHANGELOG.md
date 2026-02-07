# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
