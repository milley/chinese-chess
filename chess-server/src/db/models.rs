use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// === 数据库行模型 ===

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    #[serde(skip)]
    pub password_hash: String,
    pub display_name: Option<String>,
    pub rating: i32,
    pub wins: i32,
    pub losses: i32,
    pub draws: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Game {
    pub id: Uuid,
    pub red_player_id: Option<Uuid>,
    pub black_player_id: Option<Uuid>,
    pub status: String,
    pub result: Option<String>,
    pub end_reason: Option<String>,
    pub fen: String,
    pub move_history: Option<String>,
    pub initial_fen: Option<String>,
    pub time_control: Option<i32>,
    pub move_time_limit: Option<i32>,
    pub byoyomi: Option<i32>,
    pub red_time: Option<i32>,
    pub black_time: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

// === 请求 DTO ===

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct CreateUser {
    pub username: String,
    pub password: String,
    pub display_name: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateUserRequest {
    pub display_name: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateGameRequest {
    pub player_color: Option<String>,  // "red" or "black", default "red"
    pub time_control: Option<i32>,     // 局时(秒)
    pub move_time_limit: Option<i32>,  // 步时限(秒)
    pub byoyomi: Option<i32>,          // 读秒(秒)
}

#[derive(Deserialize)]
pub struct MakeMoveRequest {
    pub from: String,
    pub to: String,
}

#[derive(Deserialize)]
pub struct ValidMovesRequest {
    pub fen: String,
    pub from: String,
}

#[derive(Deserialize)]
pub struct AiMoveRequest {
    pub fen: String,
    pub depth: Option<u8>,
}

// === 响应 DTO ===

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserInfo,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct UserInfo {
    pub id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub rating: i32,
    pub wins: i32,
    pub losses: i32,
    pub draws: i32,
}

impl From<User> for UserInfo {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            username: u.username,
            display_name: u.display_name,
            rating: u.rating,
            wins: u.wins,
            losses: u.losses,
            draws: u.draws,
        }
    }
}

#[derive(Serialize, Clone)]
pub struct GameInfo {
    pub id: Uuid,
    pub red_player: Option<UserInfo>,
    pub black_player: Option<UserInfo>,
    pub status: String,
    pub result: Option<String>,
    pub end_reason: Option<String>,
    pub fen: String,
    pub time_control: Option<i32>,
    pub move_time_limit: Option<i32>,
    pub byoyomi: Option<i32>,
    pub red_time: Option<i32>,
    pub black_time: Option<i32>,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct CreateGameResponse {
    pub game_id: Uuid,
    pub color: String,
}

#[derive(Serialize)]
pub struct MakeMoveResponse {
    pub fen: String,
    pub is_check: bool,
    pub is_game_over: bool,
    pub result: Option<String>,
    pub end_reason: Option<String>,
}

#[derive(Serialize)]
pub struct ValidMovesResponse {
    pub moves: Vec<String>,
}

#[derive(Serialize)]
pub struct AiMoveResponse {
    pub best_move: Option<String>,
    pub depth: u8,
}
