use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use axum::body::Body;
use axum::extract::ConnectInfo;
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
    /// Optional trusted proxy header name (e.g., "x-real-ip" set by nginx).
    /// When set, only this header is trusted for IP extraction.
    /// When unset, uses the direct socket address via ConnectInfo.
    trusted_header: Option<String>,
}

impl RateLimitState {
    pub fn new(max_requests: u64, window_secs: u64) -> Self {
        Self {
            config: RateLimitConfig {
                max_requests,
                window_secs,
            },
            buckets: Arc::new(Mutex::new(HashMap::new())),
            trusted_header: None,
        }
    }

    /// Create with a trusted proxy header for IP extraction.
    pub fn with_trusted_header(max_requests: u64, window_secs: u64, trusted_header: String) -> Self {
        Self {
            config: RateLimitConfig {
                max_requests,
                window_secs,
            },
            buckets: Arc::new(Mutex::new(HashMap::new())),
            trusted_header: Some(trusted_header),
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

/// Extract client IP from request.
///
/// IP extraction priority:
/// 1. If `trusted_header` is configured, use that header (set by a known reverse proxy).
/// 2. Fall back to `ConnectInfo<SocketAddr>` (direct connection IP from the TCP socket).
/// 3. Last resort: "unknown" (shared bucket — safe but coarse).
///
/// **Security note:** We do NOT trust `X-Forwarded-For` by default because it is
/// client-controlled and trivially spoofable. Only a specifically configured header
/// (typically set by a trusted reverse proxy like nginx) is used.
fn extract_client_ip(req: &Request<Body>, trusted_header: Option<&str>) -> String {
    // Only trust a specific header if configured (set by known reverse proxy)
    if let Some(header_name) = trusted_header {
        if let Some(val) = req.headers().get(header_name) {
            if let Ok(ip) = val.to_str() {
                let ip = ip.trim();
                if !ip.is_empty() {
                    return ip.to_string();
                }
            }
        }
    }

    // Direct connection: use socket address from ConnectInfo
    if let Some(addr) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
        return addr.0.ip().to_string();
    }

    // Last resort: cannot determine IP — use a shared bucket to be safe
    "unknown".to_string()
}

/// Rate limiting middleware using a sliding window algorithm.
/// Returns 429 Too Many Requests if the client exceeds the configured limit.
pub async fn rate_limit_middleware(
    State(state): State<RateLimitState>,
    req: Request<Body>,
    next: Next,
) -> Response<Body> {
    let client_ip = extract_client_ip(&req, state.trusted_header.as_deref());

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

    #[test]
    fn test_extract_client_ip_uses_connect_info() {
        let req = Request::builder()
            .extension(ConnectInfo(SocketAddr::from(([192, 168, 1, 100], 12345))))
            .body(Body::empty())
            .unwrap();
        let ip = extract_client_ip(&req, None);
        assert_eq!(ip, "192.168.1.100");
    }

    #[test]
    fn test_extract_client_ip_ignores_xff_without_trusted_header() {
        let req = Request::builder()
            .header("x-forwarded-for", "1.2.3.4")
            .header("x-real-ip", "5.6.7.8")
            .extension(ConnectInfo(SocketAddr::from(([192, 168, 1, 100], 12345))))
            .body(Body::empty())
            .unwrap();
        // Without trusted_header configured, XFF and X-Real-IP are ignored
        let ip = extract_client_ip(&req, None);
        assert_eq!(ip, "192.168.1.100");
    }

    #[test]
    fn test_extract_client_ip_uses_trusted_header() {
        let req = Request::builder()
            .header("x-real-ip", "5.6.7.8")
            .header("x-forwarded-for", "1.2.3.4")
            .extension(ConnectInfo(SocketAddr::from(([192, 168, 1, 100], 12345))))
            .body(Body::empty())
            .unwrap();
        // With trusted_header="x-real-ip", use that header
        let ip = extract_client_ip(&req, Some("x-real-ip"));
        assert_eq!(ip, "5.6.7.8");
    }

    #[test]
    fn test_extract_client_ip_falls_back_to_unknown() {
        let req = Request::builder()
            .body(Body::empty())
            .unwrap();
        // No ConnectInfo, no trusted header → "unknown"
        let ip = extract_client_ip(&req, None);
        assert_eq!(ip, "unknown");
    }

    #[test]
    fn test_extract_client_ip_trusted_header_empty_value() {
        let req = Request::builder()
            .header("x-real-ip", "  ")
            .extension(ConnectInfo(SocketAddr::from(([192, 168, 1, 100], 12345))))
            .body(Body::empty())
            .unwrap();
        // Empty trusted header value → fall back to ConnectInfo
        let ip = extract_client_ip(&req, Some("x-real-ip"));
        assert_eq!(ip, "192.168.1.100");
    }

    #[test]
    fn test_rate_limit_with_trusted_header() {
        let state = RateLimitState::with_trusted_header(2, 60, "x-real-ip".to_string());
        assert_eq!(state.trusted_header, Some("x-real-ip".to_string()));
    }
}
