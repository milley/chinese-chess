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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::auth::generate_token;
    use axum::http::{Method, Request};

    #[tokio::test]
    async fn test_auth_user_from_valid_token() {
        let secret = "test-secret";
        let user_id = Uuid::new_v4();
        let token = generate_token(&user_id, "testuser", secret).unwrap();

        let mut req = Request::builder()
            .method(Method::GET)
            .uri("/")
            .header("Authorization", format!("Bearer {}", token))
            .body(())
            .unwrap();
        req.extensions_mut().insert(secret.to_string());

        let (mut parts, _) = req.into_parts();
        let auth_user = AuthUser::from_request_parts(&mut parts, &()).await;
        assert!(auth_user.is_ok());
        let user = auth_user.unwrap();
        assert_eq!(user.user_id, user_id);
        assert_eq!(user.username, "testuser");
    }

    #[tokio::test]
    async fn test_auth_user_missing_header() {
        let mut req = Request::builder()
            .method(Method::GET)
            .uri("/")
            .body(())
            .unwrap();
        req.extensions_mut().insert("secret".to_string());

        let (mut parts, _) = req.into_parts();
        let result = AuthUser::from_request_parts(&mut parts, &()).await;
        assert_eq!(result.unwrap_err(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_user_invalid_token_format() {
        let mut req = Request::builder()
            .method(Method::GET)
            .uri("/")
            .header("Authorization", "Bearer invalidtoken")
            .body(())
            .unwrap();
        req.extensions_mut().insert("secret".to_string());

        let (mut parts, _) = req.into_parts();
        let result = AuthUser::from_request_parts(&mut parts, &()).await;
        assert_eq!(result.unwrap_err(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_user_wrong_secret() {
        let token = generate_token(&Uuid::new_v4(), "user", "correct-secret").unwrap();

        let mut req = Request::builder()
            .method(Method::GET)
            .uri("/")
            .header("Authorization", format!("Bearer {}", token))
            .body(())
            .unwrap();
        req.extensions_mut().insert("wrong-secret".to_string());

        let (mut parts, _) = req.into_parts();
        let result = AuthUser::from_request_parts(&mut parts, &()).await;
        assert_eq!(result.unwrap_err(), StatusCode::UNAUTHORIZED);
    }
}
