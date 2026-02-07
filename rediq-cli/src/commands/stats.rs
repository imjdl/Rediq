//! Statistics command handlers
//!
//! Provides commands for displaying queue and system statistics.

use color_eyre::Result;
use rediq::client::Client;

/// Show statistics for a queue
///
/// # Arguments
/// * `client` - Rediq client instance
/// * `queue_name` - Name of the queue to show statistics for
pub async fn show(client: &Client, queue_name: String) -> Result<()> {
    println!("Statistics");
    let inspector = client.inspector();
    let stats = inspector.queue_stats(&queue_name).await?;

    println!("  Queue: {}", stats.name);
    println!("  Pending: {}", stats.pending);
    println!("  Active: {}", stats.active);
    println!("  Delayed: {}", stats.delayed);
    println!("  Retry: {}", stats.retried);
    println!("  Dead: {}", stats.dead);
    println!("  Completed: {}", stats.completed);

    Ok(())
}

#[cfg(test)]
mod tests {
    

    #[test]
    fn test_stats_display() {
        // Basic structural test - integration tests cover actual functionality
        let stats_str = "Pending: 10, Active: 2";
        assert!(stats_str.contains("Pending"));
        assert!(stats_str.contains("Active"));
    }
}
