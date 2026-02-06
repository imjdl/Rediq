//! Prometheus metrics collector for Rediq
//!
//! Provides task processing metrics for monitoring and observability.

use prometheus::{
    Counter, CounterVec, Histogram, HistogramVec, IntCounter, IntCounterVec, IntGauge, IntGaugeVec,
    Registry, TextEncoder, Opts, HistogramOpts,
};
use std::sync::Arc;
use std::time::Instant;

/// Rediq metrics collector
///
/// Collects metrics for task processing, queue status, and worker health.
#[derive(Clone)]
pub struct RediqMetrics {
    registry: Arc<Registry>,

    // Task counters
    tasks_enqueued_total: IntCounterVec,
    tasks_processed_total: IntCounterVec,
    tasks_failed_total: IntCounterVec,
    tasks_retried_total: IntCounterVec,

    // Task timing
    task_duration_seconds: HistogramVec,

    // Queue gauges
    queue_pending_tasks: IntGaugeVec,
    queue_active_tasks: IntGaugeVec,
    queue_delayed_tasks: IntGaugeVec,
    queue_retry_tasks: IntGaugeVec,
    queue_dead_tasks: IntGaugeVec,

    // Worker gauges
    worker_active_tasks: IntGaugeVec,
    worker_heartbeat: IntGaugeVec,

    // Processing metrics
    processing_duration_seconds: Histogram,
}

impl RediqMetrics {
    /// Create a new metrics collector
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();

        // Task counters
        let tasks_enqueued_total = IntCounterVec::new(
            Opts::new("rediq_tasks_enqueued_total", "Total number of tasks enqueued"),
            &["queue", "task_type"]
        )?;

        let tasks_processed_total = IntCounterVec::new(
            Opts::new("rediq_tasks_processed_total", "Total number of tasks processed successfully"),
            &["queue", "task_type"]
        )?;

        let tasks_failed_total = IntCounterVec::new(
            Opts::new("rediq_tasks_failed_total", "Total number of tasks that failed"),
            &["queue", "task_type", "error_type"]
        )?;

        let tasks_retried_total = IntCounterVec::new(
            Opts::new("rediq_tasks_retried_total", "Total number of task retries"),
            &["queue", "task_type"]
        )?;

        // Task timing
        let task_duration_seconds = HistogramVec::new(
            HistogramOpts::new("rediq_task_duration_seconds", "Task processing duration in seconds"),
            &["queue", "task_type"]
        )?;

        // Queue gauges
        let queue_pending_tasks = IntGaugeVec::new(
            Opts::new("rediq_queue_pending_tasks", "Number of pending tasks in queue"),
            &["queue"]
        )?;

        let queue_active_tasks = IntGaugeVec::new(
            Opts::new("rediq_queue_active_tasks", "Number of active tasks in queue"),
            &["queue"]
        )?;

        let queue_delayed_tasks = IntGaugeVec::new(
            Opts::new("rediq_queue_delayed_tasks", "Number of delayed tasks in queue"),
            &["queue"]
        )?;

        let queue_retry_tasks = IntGaugeVec::new(
            Opts::new("rediq_queue_retry_tasks", "Number of retry tasks in queue"),
            &["queue"]
        )?;

        let queue_dead_tasks = IntGaugeVec::new(
            Opts::new("rediq_queue_dead_tasks", "Number of dead tasks in queue"),
            &["queue"]
        )?;

        // Worker gauges
        let worker_active_tasks = IntGaugeVec::new(
            Opts::new("rediq_worker_active_tasks", "Number of active tasks per worker"),
            &["worker_id", "queue"]
        )?;

        let worker_heartbeat = IntGaugeVec::new(
            Opts::new("rediq_worker_heartbeat_timestamp", "Last heartbeat timestamp of worker"),
            &["worker_id"]
        )?;

        // Processing metrics
        let processing_duration_seconds = Histogram::with_opts(
            HistogramOpts::new("rediq_processing_duration_seconds", "Total processing duration")
                .buckets(vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0])
        )?;

        // Register all metrics
        registry.register(Box::new(tasks_enqueued_total.clone()))?;
        registry.register(Box::new(tasks_processed_total.clone()))?;
        registry.register(Box::new(tasks_failed_total.clone()))?;
        registry.register(Box::new(tasks_retried_total.clone()))?;
        registry.register(Box::new(task_duration_seconds.clone()))?;
        registry.register(Box::new(queue_pending_tasks.clone()))?;
        registry.register(Box::new(queue_active_tasks.clone()))?;
        registry.register(Box::new(queue_delayed_tasks.clone()))?;
        registry.register(Box::new(queue_retry_tasks.clone()))?;
        registry.register(Box::new(queue_dead_tasks.clone()))?;
        registry.register(Box::new(worker_active_tasks.clone()))?;
        registry.register(Box::new(worker_heartbeat.clone()))?;
        registry.register(Box::new(processing_duration_seconds.clone()))?;

        Ok(Self {
            registry: Arc::new(registry),
            tasks_enqueued_total,
            tasks_processed_total,
            tasks_failed_total,
            tasks_retried_total,
            task_duration_seconds,
            queue_pending_tasks,
            queue_active_tasks,
            queue_delayed_tasks,
            queue_retry_tasks,
            queue_dead_tasks,
            worker_active_tasks,
            worker_heartbeat,
            processing_duration_seconds,
        })
    }

    /// Record task enqueued
    pub fn record_task_enqueued(&self, queue: &str, task_type: &str) {
        self.tasks_enqueued_total
            .with_label_values(&[queue, task_type])
            .inc();
    }

    /// Record task processed successfully
    pub fn record_task_processed(&self, queue: &str, task_type: &str, duration_secs: f64) {
        self.tasks_processed_total
            .with_label_values(&[queue, task_type])
            .inc();
        self.task_duration_seconds
            .with_label_values(&[queue, task_type])
            .observe(duration_secs);
    }

    /// Record task failed
    pub fn record_task_failed(&self, queue: &str, task_type: &str, error_type: &str) {
        self.tasks_failed_total
            .with_label_values(&[queue, task_type, error_type])
            .inc();
    }

    /// Record task retried
    pub fn record_task_retried(&self, queue: &str, task_type: &str) {
        self.tasks_retried_total
            .with_label_values(&[queue, task_type])
            .inc();
    }

    /// Update queue metrics
    pub fn update_queue_metrics(
        &self,
        queue: &str,
        pending: u64,
        active: u64,
        delayed: u64,
        retry: u64,
        dead: u64,
    ) {
        self.queue_pending_tasks
            .with_label_values(&[queue])
            .set(pending as i64);
        self.queue_active_tasks
            .with_label_values(&[queue])
            .set(active as i64);
        self.queue_delayed_tasks
            .with_label_values(&[queue])
            .set(delayed as i64);
        self.queue_retry_tasks
            .with_label_values(&[queue])
            .set(retry as i64);
        self.queue_dead_tasks
            .with_label_values(&[queue])
            .set(dead as i64);
    }

    /// Update worker heartbeat
    pub fn update_worker_heartbeat(&self, worker_id: &str, timestamp: i64) {
        self.worker_heartbeat
            .with_label_values(&[worker_id])
            .set(timestamp);
    }

    /// Get the registry for custom metrics
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Gather metrics in Prometheus text format
    pub fn gather(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        encoder.encode_to_string(&metric_families).unwrap_or_default()
    }
}

impl Default for RediqMetrics {
    fn default() -> Self {
        Self::new().expect("failed to create metrics")
    }
}

/// Timer for measuring task duration
pub struct Timer {
    start: Instant,
    metrics: RediqMetrics,
    queue: String,
    task_type: String,
}

impl Timer {
    /// Start a new timer
    pub fn start(metrics: RediqMetrics, queue: &str, task_type: &str) -> Self {
        Self {
            start: Instant::now(),
            metrics,
            queue: queue.to_string(),
            task_type: task_type.to_string(),
        }
    }

    /// Stop the timer and record the duration
    pub fn stop(self) {
        let duration = self.start.elapsed().as_secs_f64();
        self.metrics
            .record_task_processed(&self.queue, &self.task_type, duration);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = RediqMetrics::new();
        assert!(metrics.is_ok());
    }

    #[test]
    fn test_record_task_enqueued() {
        let metrics = RediqMetrics::new().unwrap();
        metrics.record_task_enqueued("default", "test:task");

        let output = metrics.gather();
        assert!(output.contains("rediq_tasks_enqueued_total"));
        assert!(output.contains("queue=\"default\""));
        assert!(output.contains("task_type=\"test:task\""));
    }

    #[test]
    fn test_record_task_processed() {
        let metrics = RediqMetrics::new().unwrap();
        metrics.record_task_processed("default", "test:task", 0.5);

        let output = metrics.gather();
        assert!(output.contains("rediq_tasks_processed_total"));
        assert!(output.contains("rediq_task_duration_seconds"));
    }
}
