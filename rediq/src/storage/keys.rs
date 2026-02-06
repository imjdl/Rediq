//! Redis Key manager
//!
//! Provides unified Redis key naming convention.

/// Redis Key prefix
const PREFIX: &str = "rediq";

/// Redis Key manager
#[derive(Debug, Clone)]
pub struct Keys;

impl Keys {
    /// Queue Key (List)
    /// Example: rediq:queue:default
    pub fn queue(queue_name: &str) -> String {
        format!("{}:queue:{}", PREFIX, queue_name)
    }

    /// Priority Queue Key (ZSet)
    /// Uses score where lower value = higher priority
    /// Example: rediq:pqueue:default
    pub fn priority_queue(queue_name: &str) -> String {
        format!("{}:pqueue:{}", PREFIX, queue_name)
    }

    /// Active task queue Key (List)
    /// Example: rediq:active:default
    pub fn active(queue_name: &str) -> String {
        format!("{}:active:{}", PREFIX, queue_name)
    }

    /// Delayed queue Key (ZSet)
    /// Example: rediq:delayed:default
    pub fn delayed(queue_name: &str) -> String {
        format!("{}:delayed:{}", PREFIX, queue_name)
    }

    /// Retry queue Key (ZSet)
    /// Example: rediq:retry:default
    pub fn retry(queue_name: &str) -> String {
        format!("{}:retry:{}", PREFIX, queue_name)
    }

    /// Dead letter queue Key (List)
    /// Example: rediq:dead:default
    pub fn dead(queue_name: &str) -> String {
        format!("{}:dead:{}", PREFIX, queue_name)
    }

    /// Deduplication set Key (Set)
    /// Example: rediq:dedup:default
    pub fn dedup(queue_name: &str) -> String {
        format!("{}:dedup:{}", PREFIX, queue_name)
    }

    /// Task detail Key (Hash)
    /// Example: rediq:task:a1b2c3d4-...
    pub fn task(task_id: &str) -> String {
        format!("{}:task:{}", PREFIX, task_id)
    }

    /// All queues set Key (Set)
    pub fn meta_queues() -> String {
        format!("{}:meta:queues", PREFIX)
    }

    /// All workers set Key (Set)
    pub fn meta_workers() -> String {
        format!("{}:meta:workers", PREFIX)
    }

    /// Worker detail Key (Hash)
    /// Example: rediq:meta:worker:worker-1
    pub fn meta_worker(worker_id: &str) -> String {
        format!("{}:meta:worker:{}", PREFIX, worker_id)
    }

    /// Worker heartbeat Key (String)
    /// Example: rediq:meta:heartbeat:worker-1
    pub fn meta_heartbeat(worker_id: &str) -> String {
        format!("{}:meta:heartbeat:{}", PREFIX, worker_id)
    }

    /// Queue pause marker Key (String)
    /// Example: rediq:pause:default
    pub fn pause(queue_name: &str) -> String {
        format!("{}:pause:{}", PREFIX, queue_name)
    }

    /// Queue statistics Key (Hash)
    /// Example: rediq:stats:default
    pub fn stats(queue_name: &str) -> String {
        format!("{}:stats:{}", PREFIX, queue_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_formats() {
        assert_eq!(Keys::queue("default"), "rediq:queue:default");
        assert_eq!(Keys::priority_queue("default"), "rediq:pqueue:default");
        assert_eq!(Keys::active("default"), "rediq:active:default");
        assert_eq!(Keys::delayed("default"), "rediq:delayed:default");
        assert_eq!(Keys::retry("default"), "rediq:retry:default");
        assert_eq!(Keys::dead("default"), "rediq:dead:default");
        assert_eq!(Keys::dedup("default"), "rediq:dedup:default");
        assert_eq!(Keys::task("abc123"), "rediq:task:abc123");
        assert_eq!(Keys::meta_queues(), "rediq:meta:queues");
        assert_eq!(Keys::meta_workers(), "rediq:meta:workers");
        assert_eq!(Keys::meta_worker("worker-1"), "rediq:meta:worker:worker-1");
        assert_eq!(Keys::meta_heartbeat("worker-1"), "rediq:meta:heartbeat:worker-1");
        assert_eq!(Keys::pause("default"), "rediq:pause:default");
        assert_eq!(Keys::stats("default"), "rediq:stats:default");
    }
}
