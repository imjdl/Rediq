//! Observability module
//!
//! Provides Prometheus metrics, distributed tracing and structured logging

pub mod metrics;
pub mod http_server;

pub use metrics::RediqMetrics;
#[cfg(feature = "metrics-http")]
pub use http_server::MetricsServer;
