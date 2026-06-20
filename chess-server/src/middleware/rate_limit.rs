use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, Response, StatusCode};
use axum::middleware::Next;
use axum::response::IntoResponse;
use tokio::sync::Mutex;

/// Configuration for the rate limiter.
#[derive(Clone)]
pub struct RateLimitConfig {
    /// Maximum number of requests allowed in the window.
    pub max_requests: u64,
    /// Window duration in seconds.
    pub window_secs: u64,
}

/// Shared state for the rate limiter: maps client IP to request timestamps.
#[derive(Clone)]
pub struct RateLimitState {
    config: RateLimitConfig,
    buckets: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
}

impl RateLimitState {
    pub fn new(max_requests: u64, window_secs: u64) -> Self {
        Self {
            config: RateLimitConfig {
                max_requests,
                window_secs,
            },
            buckets: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check if a request from the given key is allowed.
    /// Returns true if the request is within limits, false if rate limited.
    pub async fn check(&self, key: &str) -> bool {
        let now = Instant::now();
        let window_duration = std::time::Duration::from_secs(self.config.window_secs);
        let cutoff = now - window_duration;

        let mut buckets = self.buckets.lock().await;

        let timestamps = buckets.entry(key.to_string()).or_insert_with(Vec::new);

        // Remove expired timestamps
        timestamps.retain(|t| *t > cutoff);

        // Check if we're within the limit
        if timestamps.len() < self.config.max_requests as usize {
            timestamps.push(now);
            true
        } else {
            false
        }
    }

    /// Clean up expired entries to prevent unbounded memory growth.
    /// Called periodically (every 60 seconds) from the cleanup task.
    pub async fn cleanup(&self) {
        let window_duration = std::time::Duration::from_secs(self.config.window_secs);
        let cutoff = Instant::now() - window_duration;

        let mut buckets = self.buckets.lock().await;
        for timestamps in buckets.values_mut() {
            timestamps.retain(|t| *t > cutoff);
        }
        // Remove entries with no remaining timestamps
        buckets.retain(|_, timestamps| !timestamps.is_empty());
    }
}

/// Extract client IP from request headers (X-Forwarded-For or connection info).
fn extract_client_ip(req: &Request<Body>) -> String {
    // Check X-Forwarded-For header first (for reverse proxy setups)
    if let Some(xff) = req.headers().get("x-forwarded-for") {
        if let Ok(val) = xff.to_str() {
            // X-Forwarded-For may contain multiple IPs; use the first (original client)
            if let Some(ip) = val.split(',').next() {
                return ip.trim().to_string();
            }
        }
    }

    // Fall back to a fixed key if no IP can be determined
    // (this is a simple approach; in production you'd use ConnectInfo from axum)
    "unknown".to_string()
}

/// Rate limiting middleware using a sliding window algorithm.
/// Returns 429 Too Many Requests if the client exceeds the configured limit.
pub async fn rate_limit_middleware(
    State(state): State<RateLimitState>,
    req: Request<Body>,
    next: Next,
) -> Response<Body> {
    let client_ip = extract_client_ip(&req);

    if state.check(&client_ip).await {
        next.run(req).await
    } else {
        (StatusCode::TOO_MANY_REQUESTS, "Rate limited. Please try again later.").into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limit_allows_within_limit() {
        let state = RateLimitState::new(3, 60);
        assert!(state.check("192.168.1.1").await);
        assert!(state.check("192.168.1.1").await);
        assert!(state.check("192.168.1.1").await);
    }

    #[tokio::test]
    async fn test_rate_limit_blocks_over_limit() {
        let state = RateLimitState::new(2, 60);
        assert!(state.check("10.0.0.1").await);
        assert!(state.check("10.0.0.1").await);
        assert!(!state.check("10.0.0.1").await); // 3rd request blocked
    }

    #[tokio::test]
    async fn test_rate_limit_independent_per_ip() {
        let state = RateLimitState::new(1, 60);
        assert!(state.check("1.1.1.1").await);
        assert!(state.check("2.2.2.2").await); // Different IP, separate bucket
        assert!(!state.check("1.1.1.1").await); // Same IP, now blocked
    }

    #[tokio::test]
    async fn test_rate_limit_cleanup_removes_expired() {
        let state = RateLimitState::new(1, 1); // 1-second window
        assert!(state.check("3.3.3.3").await);
        assert!(!state.check("3.3.3.3").await); // Blocked within window

        // Wait for window to expire
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        state.cleanup().await;

        // Should be allowed again after cleanup
        assert!(state.check("3.3.3.3").await);
    }
}
