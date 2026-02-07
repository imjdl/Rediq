//! HTTP metrics server
//!
//! Provides an HTTP endpoint for Prometheus metrics scraping.

#[cfg(feature = "metrics-http")]
use super::RediqMetrics;
use std::net::SocketAddr;
#[cfg(feature = "metrics-http")]
use std::sync::Arc;
#[cfg(feature = "metrics-http")]
use warp::Filter;

/// HTTP metrics server
///
/// Serves Prometheus metrics on `/metrics` endpoint.
#[cfg(feature = "metrics-http")]
pub struct MetricsServer {
    metrics: Arc<RediqMetrics>,
    bind_address: SocketAddr,
}

#[cfg(feature = "metrics-http")]
impl MetricsServer {
    /// Create a new metrics server
    pub fn new(metrics: Arc<RediqMetrics>, bind_address: SocketAddr) -> Self {
        Self {
            metrics,
            bind_address,
        }
    }

    /// Create a new metrics server binding to 0.0.0.0:9090
    pub fn new_default(metrics: Arc<RediqMetrics>) -> Self {
        Self::new(
            metrics,
            SocketAddr::from(([0, 0, 0, 0], 9090))
        )
    }

    /// Get the metrics as a string
    pub fn get_metrics(&self) -> String {
        let encoder = prometheus::TextEncoder::new();
        let metric_families = self.metrics.registry().gather();
        let result = encoder.encode_to_string(&metric_families);
        match result {
            Ok(s) => s,
            Err(_) => String::from("# Error encoding metrics\n"),
        }
    }

    /// Start the HTTP server
    pub async fn run(self) -> Result<(), crate::Error> {
        let metrics = self.metrics.clone();

        // Build a simple warp filter that returns metrics as plain text
        let route = warp::path!("metrics")
            .and(warp::get())
            .map(move || {
                let encoder = prometheus::TextEncoder::new();
                let metric_families = metrics.registry().gather();
                let result = encoder.encode_to_string(&metric_families);
                let output = match result {
                    Ok(s) => s,
                    Err(_) => String::from("# Error encoding metrics\n"),
                };
                warp::reply::with_header(output, "content-type", "text/plain")
            });

        let addr = self.bind_address;
        tracing::info!("Metrics server listening on http://{}", addr);
        tracing::info!("Metrics available at http://{}/metrics", addr);

        warp::serve(route).run(addr).await;
        Ok(())
    }

    /// Start the HTTP server in the background
    pub fn spawn(self) -> tokio::task::JoinHandle<Result<(), crate::Error>> {
        tokio::spawn(async move {
            self.run().await
        })
    }
}

#[cfg(not(feature = "metrics-http"))]
/// Metrics server is only available with the `metrics-http` feature
///
/// This is a placeholder type when the `metrics-http` feature is disabled.
pub struct MetricsServer {}

#[cfg(not(feature = "metrics-http"))]
impl MetricsServer {
    /// Create a new metrics server (only available with metrics-http feature)
    ///
    /// This function will panic if the `metrics-http` feature is not enabled.
    pub fn new(_metrics: std::sync::Arc<super::RediqMetrics>, _bind_address: SocketAddr) -> Self {
        panic!("MetricsServer requires the `metrics-http` feature to be enabled");
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "metrics-http")]
    #[test]
    fn test_bind_address() {
        use super::*;
        use super::super::RediqMetrics;
        use std::sync::Arc;

        let metrics = Arc::new(RediqMetrics::new_or_default());
        let server = MetricsServer::new_default(metrics);
        assert_eq!(server.bind_address.port(), 9090);
    }
}
