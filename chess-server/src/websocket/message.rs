use serde::{Deserialize, Serialize};

use crate::db::models::UserInfo;

// === 客户端 → 服务端 ===

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "type")]
pub enum ClientMessage {
    #[serde(rename = "auth")]
    Auth { token: String },
    #[serde(rename = "join_game")]
    JoinGame { game_id: String },
    #[serde(rename = "leave_game")]
    LeaveGame { game_id: String },
    #[serde(rename = "make_move")]
    MakeMove { game_id: String, from: String, to: String },
    #[serde(rename = "resign")]
    Resign { game_id: String },
    #[serde(rename = "offer_draw")]
    OfferDraw { game_id: String },
    #[serde(rename = "respond_draw")]
    RespondDraw { game_id: String, accept: bool },
    #[serde(rename = "ping")]
    Ping,
}

// === 服务端 → 客户端 ===

#[derive(Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum ServerMessage {
    #[serde(rename = "joined_game")]
    JoinedGame {
        game_id: String,
        color: String,
        fen: String,
    },
    #[serde(rename = "opponent_joined")]
    OpponentJoined {
        game_id: String,
        opponent: UserInfo,
        fen: String,
    },
    #[serde(rename = "move_made")]
    MoveMade {
        game_id: String,
        from: String,
        to: String,
        fen: String,
        is_check: bool,
    },
    #[serde(rename = "illegal_move")]
    IllegalMove {
        game_id: String,
        reason: String,
    },
    #[serde(rename = "game_over")]
    GameOver {
        game_id: String,
        result: String,
        reason: String,
    },
    #[serde(rename = "opponent_disconnected")]
    OpponentDisconnected {
        game_id: String,
    },
    #[serde(rename = "draw_offered")]
    DrawOffered {
        game_id: String,
    },
    #[serde(rename = "draw_response")]
    DrawResponse {
        game_id: String,
        accepted: bool,
    },
    #[serde(rename = "time_update")]
    TimeUpdate {
        game_id: String,
        red_time: i64,
        black_time: i64,
        active_color: String,
    },
    #[serde(rename = "pong")]
    Pong,
    #[serde(rename = "error")]
    Error {
        message: String,
    },
}
