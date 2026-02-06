//! Middleware module
//!
//! Provides middleware chain and built-in middleware

use crate::{Result};
use crate::task::Task;
use async_trait::async_trait;

/// Middleware trait
#[async_trait]
pub trait Middleware: Send + Sync {
    /// Called before task processing
    async fn before(&self, task: &Task) -> Result<()> {
        Ok(())
    }

    /// Called after task processing
    async fn after(&self, task: &Task, result: &Result<()>) -> Result<()> {
        Ok(())
    }
}

/// Middleware chain
pub struct MiddlewareChain {}

impl MiddlewareChain {
    /// Create a new middleware chain
    pub fn new() -> Self {
        Self {}
    }

    /// Add middleware
    pub fn add<M: Middleware>(self, middleware: M) -> Self {
        let _ = middleware;
        // TODO: Implement middleware addition
        self
    }
}

impl Default for MiddlewareChain {
    fn default() -> Self {
        Self::new()
    }
}
