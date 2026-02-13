//! Task aggregation module
//!
//! Provides functionality for grouping and aggregating tasks for batch processing.

use crate::{Result, Task};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Configuration for task aggregation
#[derive(Debug, Clone)]
pub struct AggregatorConfig {
    /// Maximum number of tasks in a group before aggregation is triggered
    pub max_size: usize,
    /// Maximum time to wait before aggregating a partial group (grace period)
    pub grace_period: Duration,
    /// Maximum time from first task to aggregation (absolute max delay)
    pub max_delay: Duration,
}

impl Default for AggregatorConfig {
    fn default() -> Self {
        Self {
            max_size: 10,
            grace_period: Duration::from_secs(30),
            max_delay: Duration::from_secs(300), // 5 minutes
        }
    }
}

impl AggregatorConfig {
    /// Create a new aggregator configuration
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum group size
    #[must_use]
    pub fn max_size(mut self, size: usize) -> Self {
        self.max_size = size;
        self
    }

    /// Set the grace period for partial groups
    #[must_use]
    pub fn grace_period(mut self, duration: Duration) -> Self {
        self.grace_period = duration;
        self
    }

    /// Set the maximum delay from first task
    #[must_use]
    pub fn max_delay(mut self, duration: Duration) -> Self {
        self.max_delay = duration;
        self
    }
}

/// Trait for task aggregators
///
/// Implement this trait to define custom aggregation logic.
pub trait Aggregator: Send + Sync {
    /// Aggregate a group of tasks into a single batch task
    ///
    /// # Arguments
    /// * `group` - The group name
    /// * `tasks` - The tasks to aggregate
    ///
    /// # Returns
    /// * `Ok(Some(task))` - A new batch task that replaces the grouped tasks
    /// * `Ok(None)` - No batch task created (tasks will be discarded)
    /// * `Err(_)` - Aggregation failed
    fn aggregate(&self, group: &str, tasks: Vec<Task>) -> Result<Option<Task>>;
}

/// Function-based aggregator
///
/// A simple aggregator that uses a closure to define aggregation logic.
pub struct GroupAggregatorFunc {
    config: AggregatorConfig,
    f: Arc<dyn Fn(&str, Vec<Task>) -> Result<Option<Task>> + Send + Sync>,
}

impl GroupAggregatorFunc {
    /// Create a new function-based aggregator
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&str, Vec<Task>) -> Result<Option<Task>> + Send + Sync + 'static,
    {
        Self {
            config: AggregatorConfig::default(),
            f: Arc::new(f),
        }
    }

    /// Create a new function-based aggregator with custom configuration
    pub fn with_config<F>(f: F, config: AggregatorConfig) -> Self
    where
        F: Fn(&str, Vec<Task>) -> Result<Option<Task>> + Send + Sync + 'static,
    {
        Self {
            config,
            f: Arc::new(f),
        }
    }

    /// Get the aggregator configuration
    pub fn config(&self) -> &AggregatorConfig {
        &self.config
    }
}

impl Aggregator for GroupAggregatorFunc {
    fn aggregate(&self, group: &str, tasks: Vec<Task>) -> Result<Option<Task>> {
        (self.f)(group, tasks)
    }
}

/// Default aggregator that simply returns the first task
///
/// This is useful for testing or when you want to process only one task from a group.
pub struct FirstTaskAggregator;

impl Aggregator for FirstTaskAggregator {
    fn aggregate(&self, _group: &str, mut tasks: Vec<Task>) -> Result<Option<Task>> {
        Ok(tasks.pop())
    }
}

/// Manager for handling group aggregation
pub struct AggregatorManager {
    /// Registered aggregators by group name pattern
    aggregators: HashMap<String, Arc<dyn Aggregator>>,
    /// Default configuration
    default_config: AggregatorConfig,
}

impl AggregatorManager {
    /// Create a new aggregator manager
    pub fn new() -> Self {
        Self {
            aggregators: HashMap::new(),
            default_config: AggregatorConfig::default(),
        }
    }

    /// Register an aggregator for a specific group
    pub fn register(&mut self, group: &str, aggregator: Arc<dyn Aggregator>) {
        self.aggregators.insert(group.to_string(), aggregator);
    }

    /// Register an aggregator with a function
    pub fn register_fn<F>(&mut self, group: &str, f: F)
    where
        F: Fn(&str, Vec<Task>) -> Result<Option<Task>> + Send + Sync + 'static,
    {
        let aggregator = GroupAggregatorFunc::new(f);
        self.aggregators.insert(group.to_string(), Arc::new(aggregator));
    }

    /// Get the aggregator for a group
    pub fn get(&self, group: &str) -> Option<&Arc<dyn Aggregator>> {
        self.aggregators.get(group)
    }

    /// Check if a group has an aggregator
    pub fn has(&self, group: &str) -> bool {
        self.aggregators.contains_key(group)
    }

    /// Get the default configuration
    pub fn default_config(&self) -> &AggregatorConfig {
        &self.default_config
    }

    /// Set the default configuration
    pub fn set_default_config(&mut self, config: AggregatorConfig) {
        self.default_config = config;
    }
}

impl Default for AggregatorManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregator_config() {
        let config = AggregatorConfig::new()
            .max_size(20)
            .grace_period(Duration::from_secs(60))
            .max_delay(Duration::from_secs(600));

        assert_eq!(config.max_size, 20);
        assert_eq!(config.grace_period, Duration::from_secs(60));
        assert_eq!(config.max_delay, Duration::from_secs(600));
    }

    #[test]
    fn test_aggregator_manager() {
        let mut manager = AggregatorManager::new();

        manager.register_fn("test_group", |_, tasks| {
            Ok(tasks.into_iter().next())
        });

        assert!(manager.has("test_group"));
        assert!(!manager.has("unknown_group"));
    }

    #[test]
    fn test_first_task_aggregator() {
        let aggregator = FirstTaskAggregator;

        let task1 = Task {
            id: "task1".to_string(),
            task_type: "test".to_string(),
            queue: "default".to_string(),
            payload: vec![1, 2, 3],
            options: Default::default(),
            status: Default::default(),
            created_at: 0,
            enqueued_at: None,
            processed_at: None,
            retry_cnt: 0,
            last_error: None,
        };

        let task2 = Task {
            id: "task2".to_string(),
            ..task1.clone()
        };

        let result = aggregator.aggregate("test", vec![task1, task2]).unwrap();
        assert!(result.is_some());
        // FirstTaskAggregator returns the last task (pop)
        assert_eq!(result.unwrap().id, "task2");
    }

    #[test]
    fn test_group_aggregator_func() {
        let aggregator = GroupAggregatorFunc::new(|_group, tasks| {
            Ok(Some(Task::builder("batch").queue("default").payload(&tasks.len())?.build()?))
        });

        let task = Task {
            id: "task1".to_string(),
            task_type: "test".to_string(),
            queue: "default".to_string(),
            payload: vec![1, 2, 3],
            options: Default::default(),
            status: Default::default(),
            created_at: 0,
            enqueued_at: None,
            processed_at: None,
            retry_cnt: 0,
            last_error: None,
        };

        let result = aggregator.aggregate("test", vec![task.clone(), task]).unwrap();
        assert!(result.is_some());
    }
}
