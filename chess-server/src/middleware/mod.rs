pub mod auth;
pub mod rate_limit;

use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, Response};
use axum::middleware::Next;

/// Middleware to inject JWT secret into request extensions
pub async fn add_jwt_secret_to_extensions(
    State(jwt_secret): State<String>,
    mut req: Request<Body>,
    next: Next,
) -> Response<Body> {
    req.extensions_mut().insert(jwt_secret);
    next.run(req).await
}
