//! Redis client factory for CLI commands
//!
//! Provides centralized client creation and initialization.

use color_eyre::Result;
use rediq::client::Client;

/// Create a Rediq client from a Redis URL
///
/// # Arguments
/// * `redis_url` - Redis connection URL (e.g., "redis://localhost:6379")
///
/// # Returns
/// A configured Client instance
///
/// # Example
/// ```no_run
/// use rediq_cli::client::create_client;
///
/// # async fn example() -> color_eyre::Result<()> {
/// let client = create_client("redis://localhost:6379").await?;
/// # Ok(())
/// # }
/// ```
pub async fn create_client(redis_url: &str) -> Result<Client> {
    Client::builder()
        .redis_url(redis_url)
        .build()
        .await
        .map_err(|e| color_eyre::eyre::eyre!(e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires Redis server"]
    async fn test_create_client() {
        let redis_url = std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://localhost:6379".to_string());

        let client = create_client(&redis_url).await;
        assert!(client.is_ok());
    }
}
