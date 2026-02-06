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
//! use rediq::client::{Client, TaskBuilder};
//! use rediq::server::{Server, Mux};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Client side: create and enqueue tasks
//!     let client = Client::builder()
//!         .redis_url("redis://localhost:6379")
//!         .build()
//!         .await?;
//!
//!     let task = Task::builder("email:send")
//!         .queue("default")
//!         .payload(&email_data)?
//!         .max_retry(5)
//!         .build()?;
//!
//!     client.enqueue(task).await?;
//!
//!     // Server side: process tasks
//!     let mut server = Server::builder()
//!         .redis_url("redis://localhost:6379")
//!         .queues(&["default"])
//!         .build()
//!         .await?;
//!
//!     let mut mux = Mux::new();
//!     mux.handle("email:send", EmailHandler);
//!     server.run(mux).await?;
//!
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

// Public module exports
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

// Scheduler
pub mod scheduler;

// Storage layer
pub mod storage;

// Observability
pub mod observability;

// Re-export common types
pub use error::{Error, Result};
pub use task::Task;
