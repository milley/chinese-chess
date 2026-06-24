use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;

use crate::AppState;

/// GET /health — Health check with database connectivity verification.
///
/// Returns 200 "OK" if the database is reachable, or 503 "Database unavailable"
/// if the `SELECT 1` probe fails. This prevents load balancers from routing
/// traffic to instances that have lost their DB connection.
pub async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query("SELECT 1").execute(&state.pool).await {
        Ok(_) => (StatusCode::OK, "OK").into_response(),
        Err(e) => {
            tracing::error!("Health check DB probe failed: {}", e);
            (StatusCode::SERVICE_UNAVAILABLE, "Database unavailable").into_response()
        }
    }
}
