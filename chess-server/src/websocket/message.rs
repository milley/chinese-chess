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
        red_time: Option<i64>,
        black_time: Option<i64>,
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
    #[serde(rename = "opponent_reconnected")]
    OpponentReconnected {
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
        red_in_byoyomi: bool,
        black_in_byoyomi: bool,
    },
    #[serde(rename = "pong")]
    Pong,
    #[serde(rename = "error")]
    Error {
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_message_serde_roundtrip() {
        // Auth
        let msg = ClientMessage::Auth { token: "abc".into() };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: ClientMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(decoded, ClientMessage::Auth { .. }));

        // JoinGame
        let msg = ClientMessage::JoinGame { game_id: "123".into() };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"join_game\""));

        // MakeMove
        let msg = ClientMessage::MakeMove { game_id: "g1".into(), from: "a0".into(), to: "a1".into() };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"make_move\""));

        // Ping
        let msg = ClientMessage::Ping;
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"ping\""));

        // RespondDraw
        let msg = ClientMessage::RespondDraw { game_id: "g1".into(), accept: true };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"accept\":true"));
    }

    #[test]
    fn test_server_message_serde_roundtrip() {
        // Pong
        let msg = ServerMessage::Pong;
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(decoded, ServerMessage::Pong));

        // MoveMade
        let msg = ServerMessage::MoveMade {
            game_id: "g1".into(),
            from: "a0".into(),
            to: "a1".into(),
            fen: "test".into(),
            is_check: false,
            red_time: Some(600),
            black_time: Some(580),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"move_made\""));

        // GameOver
        let msg = ServerMessage::GameOver {
            game_id: "g1".into(),
            result: "red_win".into(),
            reason: "checkmate".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"game_over\""));

        // Error
        let msg = ServerMessage::Error { message: "test".into() };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"error\""));
    }

    #[test]
    fn test_time_update_serde_roundtrip() {
        let msg = ServerMessage::TimeUpdate {
            game_id: "g1".into(),
            red_time: 600,
            black_time: 580,
            active_color: "red".into(),
            red_in_byoyomi: false,
            black_in_byoyomi: true,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"time_update\""));
        assert!(json.contains("\"red_time\":600"));
        assert!(json.contains("\"black_time\":580"));
        assert!(json.contains("\"active_color\":\"red\""));
        assert!(json.contains("\"red_in_byoyomi\":false"));
        assert!(json.contains("\"black_in_byoyomi\":true"));

        let decoded: ServerMessage = serde_json::from_str(&json).unwrap();
        if let ServerMessage::TimeUpdate {
            game_id, red_time, black_time, active_color, red_in_byoyomi, black_in_byoyomi,
        } = decoded
        {
            assert_eq!(game_id, "g1");
            assert_eq!(red_time, 600);
            assert_eq!(black_time, 580);
            assert_eq!(active_color, "red");
            assert!(!red_in_byoyomi);
            assert!(black_in_byoyomi);
        } else {
            panic!("Expected TimeUpdate variant");
        }
    }

    #[test]
    fn test_move_made_with_time_serde() {
        let msg = ServerMessage::MoveMade {
            game_id: "g1".into(),
            from: "e0".into(),
            to: "e1".into(),
            fen: "test_fen".into(),
            is_check: false,
            red_time: Some(600),
            black_time: Some(580),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"move_made\""));
        assert!(json.contains("\"red_time\":600"));
        assert!(json.contains("\"black_time\":580"));

        let decoded: ServerMessage = serde_json::from_str(&json).unwrap();
        if let ServerMessage::MoveMade { red_time, black_time, .. } = decoded {
            assert_eq!(red_time, Some(600));
            assert_eq!(black_time, Some(580));
        } else {
            panic!("Expected MoveMade variant");
        }
    }

    #[test]
    fn test_server_message_deserialize_invalid_type() {
        let json = r#"{"type":"unknown"}"#;
        let result: Result<ServerMessage, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_client_message_deserialize_invalid_type() {
        let json = r#"{"type":"unknown"}"#;
        let result: Result<ClientMessage, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}
