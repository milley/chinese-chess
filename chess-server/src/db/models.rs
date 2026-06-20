use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// === 数据库行模型 ===

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct GameEvent {
    pub id: i64,
    pub game_id: Uuid,
    pub seq_num: i32,
    pub event_type: String,
    pub actor_id: Option<Uuid>,
    pub data: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

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
    pub last_tick_at: Option<DateTime<Utc>>,
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
    pub initial_fen: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn test_user_info_from_user_excludes_password() {
        let user = User {
            id: Uuid::new_v4(),
            username: "testuser".to_string(),
            password_hash: "secret_hash".to_string(),
            display_name: Some("Test".to_string()),
            rating: 1500,
            wins: 5,
            losses: 3,
            draws: 2,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let info = UserInfo::from(user);
        assert_eq!(info.username, "testuser");
        assert_eq!(info.display_name, Some("Test".to_string()));
        assert_eq!(info.rating, 1500);
        assert_eq!(info.wins, 5);
        assert_eq!(info.losses, 3);
        assert_eq!(info.draws, 2);
        // UserInfo does not have a password_hash field — compile-time guarantee
    }

    #[test]
    fn test_create_game_request_deserialize_full() {
        let json = r#"{
            "player_color": "black",
            "time_control": 600,
            "move_time_limit": 30,
            "byoyomi": 10
        }"#;
        let req: CreateGameRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.player_color, Some("black".to_string()));
        assert_eq!(req.time_control, Some(600));
        assert_eq!(req.move_time_limit, Some(30));
        assert_eq!(req.byoyomi, Some(10));
    }

    #[test]
    fn test_create_game_request_deserialize_defaults() {
        let json = r#"{
            "player_color": null,
            "time_control": null,
            "move_time_limit": null,
            "byoyomi": null
        }"#;
        let req: CreateGameRequest = serde_json::from_str(json).unwrap();
        assert!(req.player_color.is_none());
        assert!(req.time_control.is_none());
        assert!(req.move_time_limit.is_none());
        assert!(req.byoyomi.is_none());
    }

    #[test]
    fn test_make_move_request_deserialize() {
        let json = r#"{"from": "b0", "to": "c2"}"#;
        let req: MakeMoveRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.from, "b0");
        assert_eq!(req.to, "c2");
    }

    #[test]
    fn test_valid_moves_request_deserialize() {
        let json = r#"{"fen": "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w", "from": "b0"}"#;
        let req: ValidMovesRequest = serde_json::from_str(json).unwrap();
        assert!(!req.fen.is_empty());
        assert_eq!(req.from, "b0");
    }

    #[test]
    fn test_ai_move_request_deserialize_defaults() {
        let json = r#"{"fen": "some_fen", "depth": null}"#;
        let req: AiMoveRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.fen, "some_fen");
        assert!(req.depth.is_none());
    }
}
