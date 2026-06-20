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
        red_time: Option<i64>,
        black_time: Option<i64>,
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
            red_time: Some(0),
            black_time: Some(300),
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

    #[test]
    fn test_client_message_resign_serde() {
        let msg = ClientMessage::Resign { game_id: "g1".into() };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"resign\""));
        assert!(json.contains("\"game_id\":\"g1\""));
        let decoded: ClientMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(decoded, ClientMessage::Resign { .. }));
    }

    #[test]
    fn test_client_message_offer_draw_serde() {
        let msg = ClientMessage::OfferDraw { game_id: "g2".into() };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"offer_draw\""));
        assert!(json.contains("\"game_id\":\"g2\""));
        let decoded: ClientMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(decoded, ClientMessage::OfferDraw { .. }));
    }

    #[test]
    fn test_client_message_leave_game_serde() {
        let msg = ClientMessage::LeaveGame { game_id: "g3".into() };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"leave_game\""));
        assert!(json.contains("\"game_id\":\"g3\""));
        let decoded: ClientMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(decoded, ClientMessage::LeaveGame { .. }));
    }

    #[test]
    fn test_server_message_joined_game_serde() {
        let msg = ServerMessage::JoinedGame {
            game_id: "g1".into(),
            color: "red".into(),
            fen: "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"joined_game\""));
        assert!(json.contains("\"game_id\":\"g1\""));
        assert!(json.contains("\"color\":\"red\""));
        let decoded: ServerMessage = serde_json::from_str(&json).unwrap();
        if let ServerMessage::JoinedGame { game_id, color, fen } = decoded {
            assert_eq!(game_id, "g1");
            assert_eq!(color, "red");
            assert!(!fen.is_empty());
        } else {
            panic!("Expected JoinedGame variant");
        }
    }

    #[test]
    fn test_server_message_opponent_disconnected_serde() {
        let msg = ServerMessage::OpponentDisconnected { game_id: "g1".into() };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"opponent_disconnected\""));
        assert!(json.contains("\"game_id\":\"g1\""));
        let decoded: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(decoded, ServerMessage::OpponentDisconnected { .. }));
    }

    #[test]
    fn test_server_message_opponent_reconnected_serde() {
        let msg = ServerMessage::OpponentReconnected { game_id: "g1".into() };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"opponent_reconnected\""));
        assert!(json.contains("\"game_id\":\"g1\""));
        let decoded: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(decoded, ServerMessage::OpponentReconnected { .. }));
    }

    #[test]
    fn test_server_message_draw_offered_serde() {
        let msg = ServerMessage::DrawOffered { game_id: "g1".into() };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"draw_offered\""));
        assert!(json.contains("\"game_id\":\"g1\""));
        let decoded: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(decoded, ServerMessage::DrawOffered { .. }));
    }

    #[test]
    fn test_server_message_draw_response_accepted_serde() {
        let msg = ServerMessage::DrawResponse { game_id: "g1".into(), accepted: true };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"draw_response\""));
        assert!(json.contains("\"accepted\":true"));
        let decoded: ServerMessage = serde_json::from_str(&json).unwrap();
        if let ServerMessage::DrawResponse { game_id, accepted } = decoded {
            assert_eq!(game_id, "g1");
            assert!(accepted);
        } else {
            panic!("Expected DrawResponse variant");
        }
    }

    #[test]
    fn test_server_message_draw_response_rejected_serde() {
        let msg = ServerMessage::DrawResponse { game_id: "g2".into(), accepted: false };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"draw_response\""));
        assert!(json.contains("\"accepted\":false"));
        let decoded: ServerMessage = serde_json::from_str(&json).unwrap();
        if let ServerMessage::DrawResponse { game_id, accepted } = decoded {
            assert_eq!(game_id, "g2");
            assert!(!accepted);
        } else {
            panic!("Expected DrawResponse variant");
        }
    }

    #[test]
    fn test_server_message_illegal_move_serde() {
        let msg = ServerMessage::IllegalMove {
            game_id: "g1".into(),
            reason: "invalid destination".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"illegal_move\""));
        assert!(json.contains("\"game_id\":\"g1\""));
        assert!(json.contains("\"reason\":\"invalid destination\""));
        let decoded: ServerMessage = serde_json::from_str(&json).unwrap();
        if let ServerMessage::IllegalMove { game_id, reason } = decoded {
            assert_eq!(game_id, "g1");
            assert_eq!(reason, "invalid destination");
        } else {
            panic!("Expected IllegalMove variant");
        }
    }

    #[test]
    fn test_move_entry_serde_roundtrip() {
        use crate::websocket::room::MoveEntry;

        let entry = MoveEntry {
            mv: "b0-c2".to_string(),
            color: "red".to_string(),
            fen: "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w".to_string(),
            is_check: true,
            time_spent: Some(5),
            red_time: Some(595),
            black_time: Some(600),
            timestamp: "2026-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        // The "mv" field serializes as "move" due to serde(rename)
        assert!(json.contains("\"move\":\"b0-c2\""));
        assert!(json.contains("\"color\":\"red\""));
        assert!(json.contains("\"is_check\":true"));
        assert!(json.contains("\"time_spent\":5"));
        assert!(json.contains("\"red_time\":595"));
        assert!(json.contains("\"black_time\":600"));

        let decoded: MoveEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.mv, "b0-c2");
        assert_eq!(decoded.color, "red");
        assert!(decoded.is_check);
        assert_eq!(decoded.time_spent, Some(5));
        assert_eq!(decoded.red_time, Some(595));
        assert_eq!(decoded.black_time, Some(600));
        assert_eq!(decoded.timestamp, "2026-01-01T00:00:00Z");
    }
}
