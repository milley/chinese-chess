use axum::extract::ws::{WebSocket, Message};
use axum::extract::State;
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::utils::auth::verify_token;
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
    let (tx, mut rx): (mpsc::UnboundedSender<String>, mpsc::UnboundedReceiver<String>) = mpsc::unbounded_channel();

    // 启动发送任务
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // 客户端信息 (认证后填充)
    let mut authenticated_user: Option<(Uuid, String)> = None;
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
                        if let Ok(claims) = verify_token(&token, &state.jwt_secret) {
                            if let Ok(user_id) = Uuid::parse_str(&claims.sub) {
                                authenticated_user = Some((user_id, claims.username));
                                auth_failures = 0;
                                tx.send(serde_json::to_string(&ServerMessage::Pong).unwrap_or_default()).ok();
                            }
                        } else {
                            auth_failures += 1;
                            tx.send(serde_json::to_string(&ServerMessage::Error { message: "Authentication failed".into() }).unwrap_or_default()).ok();
                            if auth_failures >= MAX_AUTH_FAILURES {
                                // Too many failed auth attempts — close connection to prevent brute-force
                                break;
                            }
                        }
                    }
                    ClientMessage::Ping => {
                        tx.send(serde_json::to_string(&ServerMessage::Pong).unwrap_or_default()).ok();
                    }
                    _ if authenticated_user.is_none() => {
                        // Reject all non-Auth/Ping messages when not authenticated
                        tx.send(serde_json::to_string(&ServerMessage::Error {
                            message: "Authentication required".into(),
                        }).unwrap_or_default()).ok();
                    }
                    ClientMessage::MakeMove { game_id, from, to } => {
                        if let Some((user_id, _username)) = &authenticated_user {
                            if let Ok(gid) = Uuid::parse_str(&game_id) {
                                let game = state.game_repo.find_by_id(gid).await.ok().flatten();
                                if let Some(game) = game {
                                    if game.status != "playing" {
                                        let msg = ServerMessage::IllegalMove {
                                            game_id: game_id.clone(),
                                            reason: "Game is not in progress".into(),
                                        };
                                        tx.send(serde_json::to_string(&msg).unwrap_or_default()).ok();
                                        continue;
                                    }
                                    let player_color = if game.red_player_id == Some(*user_id) {
                                        chess_engine::Color::Red
                                    } else if game.black_player_id == Some(*user_id) {
                                        chess_engine::Color::Black
                                    } else {
                                        continue; // Not a player
                                    };
                                    let result = state.room_manager.make_move(gid, *user_id, player_color, &from, &to).await;
                                    match result {
                                        Ok(move_result) => {
                                            // Persist to database (same logic as REST handler)
                                            if move_result.is_game_over {
                                                let (result_str, reason_str) = match move_result.result.as_deref() {
                                                    Some("red_win") => ("red_win", move_result.end_reason.as_deref().unwrap_or("checkmate")),
                                                    Some("black_win") => ("black_win", move_result.end_reason.as_deref().unwrap_or("checkmate")),
                                                    Some("draw") => ("draw", move_result.end_reason.as_deref().unwrap_or("draw")),
                                                    _ => ("draw", "unknown"),
                                                };
                                                let moves_json = serde_json::to_string(&move_result.move_history).unwrap_or("[]".into());
                                                let _ = crate::services::elo_service::finish_game_with_elo(
                                                    &state.game_repo,
                                                    &state.user_repo,
                                                    gid,
                                                    &game,
                                                    result_str,
                                                    reason_str,
                                                    &move_result.fen,
                                                    &moves_json,
                                                ).await;
                                            } else {
                                                let moves_json = serde_json::to_string(&move_result.move_history).unwrap_or("[]".into());
                                                let _ = state.game_repo.update_fen(gid, &move_result.fen, &moves_json).await;
                                            }
                                        }
                                        Err(e) => {
                                            let msg = ServerMessage::IllegalMove {
                                                game_id: game_id.clone(),
                                                reason: e,
                                            };
                                            tx.send(serde_json::to_string(&msg).unwrap_or_default()).ok();
                                        }
                                    }
                                }
                            }
                        }
                    }
                    ClientMessage::JoinGame { game_id } => {
                        if let Some((user_id, username)) = &authenticated_user {
                            if let Ok(gid) = Uuid::parse_str(&game_id) {
                                if !current_game_ids.contains(&gid) {
                                    current_game_ids.push(gid);
                                }
                                let room = state.room_manager.get_or_create_room(gid).await;
                                if let Ok(room) = room {
                                    // Determine color from DB
                                    let game = state.game_repo.find_by_id(gid).await.ok().flatten();
                                    if let Some(game) = game {
                                        let was_waiting = game.status == "waiting";
                                        let color = if game.red_player_id == Some(*user_id) {
                                            chess_engine::Color::Red
                                        } else if game.black_player_id == Some(*user_id) {
                                            chess_engine::Color::Black
                                        } else {
                                            continue;
                                        };
                                        let client = crate::websocket::client::Client::new(*user_id, username.clone(), tx.clone());
                                        room.join(client, color).await.ok();

                                        // Activate time control when game transitions to playing
                                        if was_waiting {
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
                                        tx.send(serde_json::to_string(&joined_msg).unwrap_or_default()).ok();

                                        // Notify opponent
                                        let opponent_user_id = match color {
                                            chess_engine::Color::Red => game.black_player_id,
                                            chess_engine::Color::Black => game.red_player_id,
                                        };
                                        if let Some(opp_id) = opponent_user_id {
                                            // Check if opponent is actually connected in the room
                                            let opponent_present = room.has_player(opp_id).await;
                                            if opponent_present {
                                                if was_waiting {
                                                    // First time opponent sees this player — send OpponentJoined
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
                                                    // Notify opponent that this player has reconnected.
                                                    let reconnected_msg = ServerMessage::OpponentReconnected {
                                                        game_id: game_id.clone(),
                                                    };
                                                    room.broadcast_to_opponent(color, &reconnected_msg).await;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    ClientMessage::LeaveGame { game_id } => {
                        if let Some((user_id, _)) = &authenticated_user {
                            if let Ok(gid) = Uuid::parse_str(&game_id) {
                                let room = state.room_manager.get_or_create_room(gid).await;
                                if let Ok(room) = room {
                                    if let Ok(Some((_, result_str, reason_str))) = room.leave(*user_id).await {
                                        // Game ended by resignation — persist to DB with Elo
                                        let game = state.game_repo.find_by_id(gid).await.ok().flatten();
                                        if let Some(game) = game {
                                            let fen = room.fen().await;
                                            let moves_json = game.move_history.as_deref().unwrap_or("[]");
                                            let _ = crate::services::elo_service::finish_game_with_elo(
                                                &state.game_repo,
                                                &state.user_repo,
                                                gid,
                                                &game,
                                                &result_str,
                                                &reason_str,
                                                &fen,
                                                moves_json,
                                            ).await;
                                        }
                                    }
                                }
                                current_game_ids.retain(|id| id != &gid);
                            }
                        }
                    }
                    ClientMessage::Resign { game_id } => {
                        if let Some((user_id, _)) = &authenticated_user {
                            if let Ok(gid) = Uuid::parse_str(&game_id) {
                                let room = state.room_manager.get_or_create_room(gid).await;
                                if let Ok(room) = room {
                                    if let Ok((_, result_str, reason_str)) = room.resign(*user_id).await {
                                        // Game ended by resignation — persist to DB with Elo
                                        let game = state.game_repo.find_by_id(gid).await.ok().flatten();
                                        if let Some(game) = game {
                                            let fen = room.fen().await;
                                            let moves_json = game.move_history.as_deref().unwrap_or("[]");
                                            let _ = crate::services::elo_service::finish_game_with_elo(
                                                &state.game_repo,
                                                &state.user_repo,
                                                gid,
                                                &game,
                                                &result_str,
                                                &reason_str,
                                                &fen,
                                                moves_json,
                                            ).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    ClientMessage::OfferDraw { game_id } => {
                        if let Some((user_id, _)) = &authenticated_user {
                            if let Ok(gid) = Uuid::parse_str(&game_id) {
                                let room = state.room_manager.get_or_create_room(gid).await;
                                if let Ok(room) = room {
                                    if let Err(e) = room.offer_draw(*user_id).await {
                                        let msg = ServerMessage::Error { message: e };
                                        tx.send(serde_json::to_string(&msg).unwrap_or_default()).ok();
                                    }
                                }
                            }
                        }
                    }
                    ClientMessage::RespondDraw { game_id, accept } => {
                        if let Some((user_id, _)) = &authenticated_user {
                            if let Ok(gid) = Uuid::parse_str(&game_id) {
                                let room = state.room_manager.get_or_create_room(gid).await;
                                if let Ok(room) = room {
                                    match room.respond_draw(*user_id, accept).await {
                                        Ok(Some((_, result_str, reason_str))) => {
                                            // Draw accepted — game ended, persist to DB with Elo
                                            let game = state.game_repo.find_by_id(gid).await.ok().flatten();
                                            if let Some(game) = game {
                                                let fen = room.fen().await;
                                                let moves_json = game.move_history.as_deref().unwrap_or("[]");
                                                let _ = crate::services::elo_service::finish_game_with_elo(
                                                    &state.game_repo,
                                                    &state.user_repo,
                                                    gid,
                                                    &game,
                                                    &result_str,
                                                    &reason_str,
                                                    &fen,
                                                    moves_json,
                                                ).await;
                                            }
                                        }
                                        Ok(None) => {
                                            // Draw rejected — nothing to persist
                                        }
                                        Err(e) => {
                                            let msg = ServerMessage::Error { message: e };
                                            tx.send(serde_json::to_string(&msg).unwrap_or_default()).ok();
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Ok(Message::Close(_)) => break,
            Err(_) => break,
            _ => {}
        }
    }

    // 断连清理: 从所有已加入的房间移除玩家并通知对手
    // If a player disconnects mid-game, it counts as a loss (resignation by disconnect).
    if let Some((user_id, _)) = &authenticated_user {
        for gid in &current_game_ids {
            let room = state.room_manager.get_or_create_room(*gid).await;
            if let Ok(room) = room {
                if let Ok(Some((_, result_str, reason_str))) = room.handle_disconnect(*user_id).await {
                    // Game ended by disconnect — persist to DB with Elo
                    let game = state.game_repo.find_by_id(*gid).await.ok().flatten();
                    if let Some(game) = game {
                        let fen = room.fen().await;
                        let moves_json = game.move_history.as_deref().unwrap_or("[]");
                        let _ = crate::services::elo_service::finish_game_with_elo(
                            &state.game_repo,
                            &state.user_repo,
                            *gid,
                            &game,
                            &result_str,
                            &reason_str,
                            &fen,
                            moves_json,
                        ).await;
                    }
                }
            }
        }
    }

    send_task.abort();
}
