//! # Rediq
//!
//! Rediq is a distributed task queue framework based on Rust and Redis.
//!
//! ## Features
//!
//! - Task creation, enqueue, delayed execution
//! - Multi-queue support, priority queues
//! - Automatic retry mechanism, dead letter queue
//! - Middleware system
//! - Prometheus monitoring
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use rediq::client::Client;
//! use rediq::processor::{Handler, Mux};
//! use rediq::server::{Server, ServerBuilder};
//! use rediq::task::TaskBuilder;
//! use async_trait::async_trait;
//! use std::time::Duration;
//!
//! # struct EmailHandler;
//! # #[async_trait]
//! # impl Handler for EmailHandler {
//! #     async fn handle(&self, task: &rediq::Task) -> rediq::Result<()> {
//! #         Ok(())
//! #     }
//! # }
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Client side: create and enqueue tasks
//! let client = Client::builder()
//!     .redis_url("redis://localhost:6379")
//!     .build()
//!     .await?;
//!
//! # let email_data = serde_json::json!({"to": "user@example.com"});
//! let task = TaskBuilder::new("email:send")
//!     .queue("default")
//!     .payload(&email_data)?
//!     .max_retry(5)
//!     .build()?;
//!
//! client.enqueue(task).await?;
//!
//! // Server side: process tasks
//! let state = ServerBuilder::new()
//!     .redis_url("redis://localhost:6379")
//!     .queues(&["default"])
//!     .build()
//!     .await?;
//!
//! let server = Server::from(state);
//! let mut mux = Mux::new();
//! mux.handle("email:send", EmailHandler);
//! server.run(mux).await?;
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

// Public module exports
pub mod config;
pub mod error;
pub mod task;

// Client SDK
pub mod client;

// Server/Worker
pub mod server;

// Processor
pub mod processor;

// Middleware
pub mod middleware;

// Storage layer
pub mod storage;

// Observability
pub mod observability;

// Progress tracking
pub mod progress;

// Re-export common types
pub use error::{Error, Result};
pub use task::Task;
pub use progress::{TaskProgress, ProgressContext, ProgressConfig};
