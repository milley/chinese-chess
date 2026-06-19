use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),

    #[error("Game error: {0}")]
    GameError(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            AppError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, self.to_string()),
            AppError::Forbidden(_) => (StatusCode::FORBIDDEN, self.to_string()),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal error".into()),
            AppError::GameError(_) => (StatusCode::BAD_REQUEST, self.to_string()),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_found_into_response() {
        let response = AppError::NotFound("test".into()).into_response();
        let (parts, _body) = response.into_parts();
        assert_eq!(parts.status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_unauthorized_into_response() {
        let response = AppError::Unauthorized("test".into()).into_response();
        let (parts, _body) = response.into_parts();
        assert_eq!(parts.status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_forbidden_into_response() {
        let response = AppError::Forbidden("test".into()).into_response();
        let (parts, _body) = response.into_parts();
        assert_eq!(parts.status, StatusCode::FORBIDDEN);
    }

    #[test]
    fn test_bad_request_into_response() {
        let response = AppError::BadRequest("test".into()).into_response();
        let (parts, _body) = response.into_parts();
        assert_eq!(parts.status, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_internal_into_response() {
        // Internal errors should return 500 and hide internal details from the response body
        let response = AppError::Internal(anyhow::anyhow!("some detail")).into_response();
        let (parts, _body) = response.into_parts();
        assert_eq!(parts.status, StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_game_error_into_response() {
        let response = AppError::GameError("test".into()).into_response();
        let (parts, _body) = response.into_parts();
        assert_eq!(parts.status, StatusCode::BAD_REQUEST);
    }
}
