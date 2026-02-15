//! Example of using the `task_handler` attribute macro
//!
//! Run with:
//! ```sh
//! cargo run --example macro_example --features macros
//! ```

use rediq::client::Client;
use rediq::{Result, server::{Server, ServerBuilder}, Task};
use rediq_macros::{register_handlers, task_handler};

// Define handlers using the #[task_handler] attribute macro
#[task_handler]
async fn send_email(task: &Task) -> Result<()> {
    println!("Processing email task: {}", task.id);
    // Deserialize payload
    #[derive(serde::Deserialize)]
    struct EmailData {
        to: String,
        subject: String,
    }
    let data: EmailData = task.payload_json()?;
    println!("  To: {}", data.to);
    println!("  Subject: {}", data.subject);
    Ok(())
}

#[task_handler]
async fn send_sms(task: &Task) -> Result<()> {
    println!("Processing SMS task: {}", task.id);
    #[derive(serde::Deserialize)]
    struct SmsData {
        phone: String,
        message: String,
    }
    let data: SmsData = task.payload_json()?;
    println!("  Phone: {}", data.phone);
    println!("  Message: {}", data.message);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Get Redis URL from environment
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());

    // Create client and enqueue tasks
    let client = Client::builder()
        .redis_url(&redis_url)
        .build()
        .await?;

    // Enqueue email task
    let email_task = Task::builder("email:send")
        .queue("default")
        .payload(&serde_json::json!({
            "to": "user@example.com",
            "subject": "Hello from Rediq!"
        }))?
        .build()?;

    client.enqueue(email_task).await?;
    println!("Enqueued email task");

    // Enqueue SMS task
    let sms_task = Task::builder("sms:send")
        .queue("default")
        .payload(&serde_json::json!({
            "phone": "+1234567890",
            "message": "Your code is 123456"
        }))?
        .build()?;

    client.enqueue(sms_task).await?;
    println!("Enqueued SMS task");

    // Build server state
    let state = ServerBuilder::new()
        .redis_url(&redis_url)
        .queues(&["default"])
        .concurrency(2)
        .build()
        .await?;

    // Create server with handlers registered using the macro
    let server = Server::from(state);

    // Use register_handlers! macro to register all handlers at once
    let mux = register_handlers!(
        "email:send" => send_email,
        "sms:send" => send_sms,
    );

    println!("Starting server with {} handlers", mux.handler_count());

    // Run server (this blocks until Ctrl+C)
    server.run(mux).await?;

    Ok(())
}
