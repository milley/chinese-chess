use axum::extract::ws::{WebSocket, Message};
use axum::extract::State;
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use uuid::Uuid;

use crate::utils::auth::verify_token;
use crate::utils::validation::{validate_game_id_string, validate_position_string, validate_token_string};
use crate::websocket::message::{ClientMessage, ServerMessage};
use crate::AppState;

/// GET /ws — WebSocket 升级
pub async fn ws_handler(
    ws: axum::extract::WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = crate::websocket::client::Client::create_channel();

    // 启动发送任务
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // 客户端信息 (认证后填充: user_id, username, JWT exp timestamp)
    let mut authenticated_user: Option<(Uuid, String, usize)> = None;
    // 当前加入的对局 ID (用于断连清理)
    let mut current_game_ids: Vec<Uuid> = Vec::new();
    // Track consecutive auth failures to prevent brute-force attacks
    let mut auth_failures: u32 = 0;
    const MAX_AUTH_FAILURES: u32 = 5;

    // 接收消息循环
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                let client_msg: ClientMessage = match serde_json::from_str(&text) {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                match client_msg {
                    ClientMessage::Auth { token } => {
                        if let Err(e) = validate_token_string(&token) {
                            tx.try_send(serde_json::to_string(&ServerMessage::Error { message: e.to_string() }).unwrap_or_default()).ok();
                            continue;
                        }
                        if let Ok(claims) = verify_token(&token, &state.jwt_secret) {
                            if let Ok(user_id) = Uuid::parse_str(&claims.sub) {
                                authenticated_user = Some((user_id, claims.username, claims.exp));
                                auth_failures = 0;
                                tx.try_send(serde_json::to_string(&ServerMessage::Pong).unwrap_or_default()).ok();
                            }
                        } else {
                            auth_failures += 1;
                            tx.try_send(serde_json::to_string(&ServerMessage::Error { message: "Authentication failed".into() }).unwrap_or_default()).ok();
                            if auth_failures >= MAX_AUTH_FAILURES {
                                // Too many failed auth attempts — close connection to prevent brute-force
                                break;
                            }
                        }
                    }
                    ClientMessage::Ping => {
                        // Re-verify JWT expiry on each ping (every ~30s from client)
                        if let Some((_, _, exp)) = &authenticated_user {
                            let now = chrono::Utc::now().timestamp() as usize;
                            if *exp < now {
                                tx.try_send(serde_json::to_string(&ServerMessage::Error {
                                    message: "Token expired. Please reconnect.".into(),
                                }).unwrap_or_default()).ok();
                                break;
                            }
                        }
                        tx.try_send(serde_json::to_string(&ServerMessage::Pong).unwrap_or_default()).ok();
                    }
                    _ if authenticated_user.is_none() => {
                        // Reject all non-Auth/Ping messages when not authenticated
                        tx.try_send(serde_json::to_string(&ServerMessage::Error {
                            message: "Authentication required".into(),
                        }).unwrap_or_default()).ok();
                    }
                    // All game-action messages below are rate-limited per user
                    msg => {
                        if let Some((user_id, _, _)) = &authenticated_user {
                            if !state.ws_rate_limit.check(&user_id.to_string()).await {
                                tx.try_send(serde_json::to_string(&ServerMessage::Error {
                                    message: "Rate limited. Slow down.".into(),
                                }).unwrap_or_default()).ok();
                                continue;
                            }
                        }
                        match msg {
                            ClientMessage::MakeMove { game_id, from, to } => {
                                if validate_game_id_string(&game_id).is_err()
                                    || validate_position_string(&from).is_err()
                                    || validate_position_string(&to).is_err() {
                                        tx.try_send(serde_json::to_string(&ServerMessage::Error {
                                            message: "Invalid move format".into(),
                                        }).unwrap_or_default()).ok();
                                        continue;
                                    }
                                if let Some((user_id, _username, _)) = &authenticated_user
                                    && let Ok(gid) = Uuid::parse_str(&game_id) {
                                        let room = state.room_manager.get_or_create_room(gid).await;
                                        if let Ok(room) = room {
                                            // Check game status from room (in-memory, no DB query)
                                            if room.is_game_over().await {
                                                let msg = ServerMessage::IllegalMove {
                                                    game_id: game_id.clone(),
                                                    reason: "Game is not in progress".into(),
                                                };
                                                tx.try_send(serde_json::to_string(&msg).unwrap_or_default()).ok();
                                                continue;
                                            }
                                            // Determine player color from room (in-memory, no DB query)
                                            let player_color = match room.player_color(*user_id).await {
                                                Ok(c) => c,
                                                Err(_) => continue, // Not a player in this room
                                            };
                                            let result = state.room_manager.make_move(gid, *user_id, player_color, &from, &to).await;
                                            match result {
                                                Ok(move_result) => {
                                                    if move_result.is_game_over {
                                                        let (result_str, reason_str) = match move_result.result.as_deref() {
                                                            Some("red_win") => ("red_win", move_result.end_reason.as_deref().unwrap_or("checkmate")),
                                                            Some("black_win") => ("black_win", move_result.end_reason.as_deref().unwrap_or("checkmate")),
                                                            Some("draw") => ("draw", move_result.end_reason.as_deref().unwrap_or("draw")),
                                                            _ => ("draw", "unknown"),
                                                        };
                                                        let moves_json = serde_json::to_string(&move_result.move_history).unwrap_or("[]".into());
                                                        state.persist_game_end(gid, result_str, reason_str, &move_result.fen, &moves_json).await;
                                                    } else {
                                                        let moves_json = serde_json::to_string(&move_result.move_history).unwrap_or("[]".into());
                                                        if let Err(e) = state.game_repo.update_fen(gid, &move_result.fen, &moves_json).await {
                                                            tracing::warn!("Failed to update FEN for game {}: {}", gid, e);
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    let msg = ServerMessage::IllegalMove {
                                                        game_id: game_id.clone(),
                                                        reason: e,
                                                    };
                                                    tx.try_send(serde_json::to_string(&msg).unwrap_or_default()).ok();
                                                }
                                            }
                                        }
                                    }
                            }
                            ClientMessage::JoinGame { game_id } => {
                                if validate_game_id_string(&game_id).is_err() {
                                    tx.try_send(serde_json::to_string(&ServerMessage::Error {
                                        message: "Invalid game ID format".into(),
                                    }).unwrap_or_default()).ok();
                                    continue;
                                }
                                if let Some((user_id, username, _)) = &authenticated_user
                                    && let Ok(gid) = Uuid::parse_str(&game_id) {
                                        if !current_game_ids.contains(&gid) {
                                            current_game_ids.push(gid);
                                        }
                                        let room = state.room_manager.get_or_create_room(gid).await;
                                        if let Ok(room) = room {
                                            // Determine color from room's stored player IDs (no extra DB query)
                                            let color = match room.player_color_from_db(*user_id) {
                                                Ok(c) => c,
                                                Err(_) => continue, // Not a player in this game
                                            };
                                            let client = crate::websocket::client::Client::new(*user_id, username.clone(), tx.clone());
                                            let both_present = room.join(client, color).await.ok().unwrap_or(false);

                                            // Activate time control when both players are present in the
                                            // WS room AND time control is not yet active.
                                            if both_present && !room.is_time_active().await {
                                                room.activate_time().await;
                                            }

                                            // Send JoinedGame back to the joining player
                                            let fen = room.fen().await;
                                            let color_str = match color {
                                                chess_engine::Color::Red => "red",
                                                chess_engine::Color::Black => "black",
                                            };
                                            let joined_msg = ServerMessage::JoinedGame {
                                                game_id: game_id.clone(),
                                                color: color_str.to_string(),
                                                fen: fen.clone(),
                                            };
                                            tx.try_send(serde_json::to_string(&joined_msg).unwrap_or_default()).ok();

                                            // Notify opponent
                                            let opponent_user_id = room.opponent_player_id(color).await;
                                            if let Some(opp_id) = opponent_user_id {
                                                // Check if opponent is actually connected in the room
                                                let opponent_present = room.has_player(opp_id).await;
                                                if opponent_present {
                                                    if both_present {
                                                        // Second player just joined — notify opponent of new player
                                                        if let Ok(Some(opp_user)) = state.user_repo.find_by_id(opp_id).await {
                                                            let opp_info = crate::db::models::UserInfo::from(opp_user);
                                                            let opp_joined_msg = ServerMessage::OpponentJoined {
                                                                game_id: game_id.clone(),
                                                                opponent: opp_info,
                                                                fen: fen.clone(),
                                                            };
                                                            room.broadcast_to_opponent(color, &opp_joined_msg).await;
                                                        }
                                                    } else {
                                                        // Game is already playing — this is a reconnection.
                                                        let reconnected_msg = ServerMessage::OpponentReconnected {
                                                            game_id: game_id.clone(),
                                                        };
                                                        room.broadcast_to_opponent(color, &reconnected_msg).await;

                                                        // Log reconnect event (fire-and-forget)
                                                        let game_repo = state.game_repo.clone();
                                                        let uid = *user_id;
                                                        let ev_color = match color {
                                                            chess_engine::Color::Red => "red",
                                                            chess_engine::Color::Black => "black",
                                                        };
                                                        tokio::spawn(async move {
                                                            if let Err(e) = game_repo.append_event(gid, "reconnect", Some(uid), serde_json::json!({ "color": ev_color })).await {
                                                                tracing::info!("Failed to append reconnect event for game {}: {}", gid, e);
                                                            }
                                                        });
                                                    }
                                                }
                                            }
                                        }
                                    }
                            }
                            ClientMessage::LeaveGame { game_id } => {
                                if validate_game_id_string(&game_id).is_err() { continue; }
                                if let Some((user_id, _, _)) = &authenticated_user
                                    && let Ok(gid) = Uuid::parse_str(&game_id) {
                                        let room = state.room_manager.get_or_create_room(gid).await;
                                        if let Ok(room) = room
                                            && let Ok(Some((_, result_str, reason_str))) = room.leave(*user_id).await {
                                                let fen = room.fen().await;
                                                let moves_json = room.move_history_json().await;
                                                state.persist_game_end(gid, &result_str, &reason_str, &fen, &moves_json).await;
                                            }
                                        current_game_ids.retain(|id| id != &gid);
                                    }
                            }
                            ClientMessage::Resign { game_id } => {
                                if validate_game_id_string(&game_id).is_err() { continue; }
                                if let Some((user_id, _, _)) = &authenticated_user
                                    && let Ok(gid) = Uuid::parse_str(&game_id) {
                                        let room = state.room_manager.get_or_create_room(gid).await;
                                        if let Ok(room) = room
                                            && let Ok((_, result_str, reason_str)) = room.resign(*user_id).await {
                                                let fen = room.fen().await;
                                                let moves_json = room.move_history_json().await;
                                                state.persist_game_end(gid, &result_str, &reason_str, &fen, &moves_json).await;
                                            }
                                    }
                            }
                            ClientMessage::OfferDraw { game_id } => {
                                if validate_game_id_string(&game_id).is_err() { continue; }
                                if let Some((user_id, _, _)) = &authenticated_user
                                    && let Ok(gid) = Uuid::parse_str(&game_id) {
                                        let room = state.room_manager.get_or_create_room(gid).await;
                                        if let Ok(room) = room
                                            && let Err(e) = room.offer_draw(*user_id).await {
                                                let msg = ServerMessage::Error { message: e };
                                                tx.try_send(serde_json::to_string(&msg).unwrap_or_default()).ok();
                                            }
                                    }
                            }
                            ClientMessage::RespondDraw { game_id, accept } => {
                                if validate_game_id_string(&game_id).is_err() { continue; }
                                if let Some((user_id, _, _)) = &authenticated_user
                                    && let Ok(gid) = Uuid::parse_str(&game_id) {
                                        let room = state.room_manager.get_or_create_room(gid).await;
                                        if let Ok(room) = room {
                                            match room.respond_draw(*user_id, accept).await {
                                                Ok(Some((_, result_str, reason_str))) => {
                                                    let fen = room.fen().await;
                                                    let moves_json = room.move_history_json().await;
                                                    state.persist_game_end(gid, &result_str, &reason_str, &fen, &moves_json).await;
                                                }
                                                Ok(None) => {
                                                    // Draw rejected — nothing to persist
                                                }
                                                Err(e) => {
                                                    let msg = ServerMessage::Error { message: e };
                                                    tx.try_send(serde_json::to_string(&msg).unwrap_or_default()).ok();
                                                }
                                            }
                                        }
                                    }
                            }
                            // Auth and Ping are handled above, not reachable here
                            _ => {}
                        }
                    }
                }
            }
            Ok(Message::Close(_)) => break,
            Err(_) => break,
            _ => {}
        }
    }

    // 断连清理: 从所有已加入的房间标记玩家断连并启动宽限期
    // Grace period: player has 30 seconds to reconnect before game ends.
    // The timeout checker (in RoomManager) will end the game if the grace period expires.
    if let Some((user_id, _, _)) = &authenticated_user {
        for gid in &current_game_ids {
            let room = state.room_manager.get_or_create_room(*gid).await;
            if let Ok(room) = room {
                // mark_disconnected starts the grace period — game does NOT end immediately
                let result = room.mark_disconnected(*user_id).await;
                // If result is Some, game ended immediately (spectator or already-over game)
                if let Ok(Some((_, result_str, reason_str))) = result {
                    let fen = room.fen().await;
                    let moves_json = room.move_history_json().await;
                    state.persist_game_end(*gid, &result_str, &reason_str, &fen, &moves_json).await;
                }
            }
        }
    }

    send_task.abort();
}
