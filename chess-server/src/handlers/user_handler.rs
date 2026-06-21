use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::db::models::*;
use crate::error::AppError;
use crate::middleware::auth::AuthUser;
use crate::utils::auth::generate_token;
use crate::utils::password::{hash_password, verify_password};
use crate::AppState;

const RESERVED_NAMES: &[&str] = &["root", "admin", "system", "guest", "administrator", "moderator"];

fn validate_username(username: &str) -> Result<(), AppError> {
    if username.len() < 3 || username.len() > 20 {
        return Err(AppError::BadRequest("Username must be 3-20 characters".into()));
    }
    if !username.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false) {
        return Err(AppError::BadRequest("Username must start with a letter".into()));
    }
    if !username.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(AppError::BadRequest("Username can only contain letters, digits, and underscores".into()));
    }
    if RESERVED_NAMES.contains(&username.to_lowercase().as_str()) {
        return Err(AppError::BadRequest("This username is reserved".into()));
    }
    Ok(())
}

fn validate_password(password: &str) -> Result<(), AppError> {
    if password.len() < 6 || password.len() > 100 {
        return Err(AppError::BadRequest("Password must be 6-100 characters".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // === validate_username tests ===

    #[test]
    fn test_validate_username_valid() {
        assert!(validate_username("alice").is_ok());
        assert!(validate_username("Bob_123").is_ok());
        assert!(validate_username("a").is_err()); // too short
    }

    #[test]
    fn test_validate_username_too_short() {
        assert!(validate_username("ab").is_err());
        assert!(validate_username("").is_err());
    }

    #[test]
    fn test_validate_username_too_long() {
        let long_name = "a".repeat(21);
        assert!(validate_username(&long_name).is_err());
        let max_name = "a".repeat(20);
        assert!(validate_username(&max_name).is_ok());
    }

    #[test]
    fn test_validate_username_must_start_with_letter() {
        assert!(validate_username("1abc").is_err());
        assert!(validate_username("_abc").is_err());
        assert!(validate_username("abc").is_ok());
    }

    #[test]
    fn test_validate_username_special_chars() {
        assert!(validate_username("abc@def").is_err());
        assert!(validate_username("abc def").is_err());
        assert!(validate_username("abc-def").is_err());
        assert!(validate_username("abc_def").is_ok()); // underscore is allowed
    }

    #[test]
    fn test_validate_username_reserved() {
        assert!(validate_username("root").is_err());
        assert!(validate_username("admin").is_err());
        assert!(validate_username("system").is_err());
        assert!(validate_username("guest").is_err());
        assert!(validate_username("administrator").is_err());
        assert!(validate_username("moderator").is_err());
        // Case-insensitive
        assert!(validate_username("Admin").is_err());
        assert!(validate_username("ROOT").is_err());
    }

    // === validate_password tests ===

    #[test]
    fn test_validate_password_valid() {
        assert!(validate_password("123456").is_ok());
        assert!(validate_password("securepassword").is_ok());
    }

    #[test]
    fn test_validate_password_too_short() {
        assert!(validate_password("12345").is_err());
        assert!(validate_password("").is_err());
    }

    #[test]
    fn test_validate_password_too_long() {
        let long_pass = "a".repeat(101);
        assert!(validate_password(&long_pass).is_err());
        let max_pass = "a".repeat(100);
        assert!(validate_password(&max_pass).is_ok());
    }
}

/// POST /api/users — 注册
pub async fn register(
    State(state): State<AppState>,
    Json(data): Json<CreateUser>,
) -> Result<Json<LoginResponse>, AppError> {
    validate_username(&data.username)?;
    validate_password(&data.password)?;

    if state.user_repo.find_by_username(&data.username).await?.is_some() {
        return Err(AppError::BadRequest("Username already exists".into()));
    }

    let hash = hash_password(&data.password)?;
    let user = state.user_repo.create(&data.username, &hash, data.display_name.as_deref()).await?;
    let token = generate_token(&user.id, &user.username, &state.jwt_secret)?;

    Ok(Json(LoginResponse { token, user: user.into() }))
}

/// POST /api/users/login — 登录
pub async fn login(
    State(state): State<AppState>,
    Json(data): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let user = state.user_repo.find_by_username(&data.username).await?
        .ok_or(AppError::Unauthorized("Invalid credentials".into()))?;

    if !verify_password(&data.password, &user.password_hash)? {
        return Err(AppError::Unauthorized("Invalid credentials".into()));
    }

    let token = generate_token(&user.id, &user.username, &state.jwt_secret)?;
    Ok(Json(LoginResponse { token, user: user.into() }))
}

/// GET /api/users/me
pub async fn get_current_user(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<UserInfo>, AppError> {
    let user = state.user_repo.find_by_id(auth.user_id).await?
        .ok_or(AppError::NotFound("User not found".into()))?;
    Ok(Json(user.into()))
}

/// PUT /api/users/me
pub async fn update_user(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(data): Json<UpdateUserRequest>,
) -> Result<Json<UserInfo>, AppError> {
    let user = state.user_repo.update(auth.user_id, data.display_name.as_deref()).await?;
    Ok(Json(user.into()))
}

/// DELETE /api/users/me
pub async fn delete_user(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    state.user_repo.delete(auth.user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/users/{id}
pub async fn get_user(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<UserInfo>, AppError> {
    let user = state.user_repo.find_by_id(id).await?
        .ok_or(AppError::NotFound("User not found".into()))?;
    Ok(Json(user.into()))
}

#[derive(Deserialize)]
pub struct ListUsersQuery {
    page: Option<i64>,
    page_size: Option<i64>,
}

/// GET /api/users
pub async fn list_users(
    _auth: AuthUser,
    Query(q): Query<ListUsersQuery>,
    State(state): State<AppState>,
) -> Result<Json<Vec<UserInfo>>, AppError> {
    let page = q.page.unwrap_or(1).max(1);
    let page_size = q.page_size.unwrap_or(20).clamp(1, 100);
    let users = state.user_repo.list(page, page_size).await?;
    Ok(Json(users.into_iter().map(UserInfo::from).collect()))
}
