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
    Query(q): Query<ListUsersQuery>,
    State(state): State<AppState>,
) -> Result<Json<Vec<UserInfo>>, AppError> {
    let page = q.page.unwrap_or(1);
    let page_size = q.page_size.unwrap_or(20);
    let users = state.user_repo.list(page, page_size).await?;
    Ok(Json(users.into_iter().map(UserInfo::from).collect()))
}
