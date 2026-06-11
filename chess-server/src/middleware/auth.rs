use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use uuid::Uuid;

use crate::utils::auth::verify_token;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub username: String,
}

impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Get JWT secret from extensions
        let jwt_secret = parts
            .extensions
            .get::<String>()
            .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(StatusCode::UNAUTHORIZED)?;

        let claims = verify_token(token, jwt_secret)
            .map_err(|_| StatusCode::UNAUTHORIZED)?;

        let user_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| StatusCode::UNAUTHORIZED)?;

        Ok(AuthUser {
            user_id,
            username: claims.username,
        })
    }
}
