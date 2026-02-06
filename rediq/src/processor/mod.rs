//! Processor module
//!
//! Provides Handler trait and multiplexer

use async_trait::async_trait;
use crate::{Result};
use crate::task::Task;

/// Handler trait - Task processor
#[async_trait]
pub trait Handler: Send + Sync {
    /// Handle task
    async fn handle(&self, task: &Task) -> Result<()>;
}

/// Multiplexer - Used to register multiple handlers
pub struct Mux {}

impl Mux {
    /// Create a new multiplexer
    pub fn new() -> Self {
        Self {}
    }

    /// Register handler
    pub fn handle<H: Handler>(&mut self, task_type: &str, handler: H) {
        let _ = task_type;
        let _ = handler;
        // TODO: Implement handler registration
    }
}

impl Default for Mux {
    fn default() -> Self {
        Self::new()
    }
}
