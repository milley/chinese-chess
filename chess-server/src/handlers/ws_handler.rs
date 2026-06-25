use axum::extract::ws::{WebSocket, Message};
use axum::extract::State;
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use uuid::Uuid;

use crate::utils::auth::verify_token;
use crate::utils::validation::{validate_game_id_string, validate_position_string, validate_token_string};
use crate::websocket::message::{ClientMessage, LobbyGameInfo, ServerMessage};
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
                            if !state.ws_rate_limit.check(&user_id.to_string()) {
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
                                                        let result_str = move_result.result.as_deref().unwrap_or("draw");
                                                        let reason_str = move_result.end_reason.as_deref().unwrap_or("unknown");
                                                        state.persist_game_end(gid, result_str, reason_str, &move_result.fen).await;
                                                    } else {
                                                        if let Err(e) = state.game_repo.update_fen(gid, &move_result.fen).await {
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
                                        tracing::info!("[WS JoinGame] user={} ({}) joining game={}", username, user_id, gid);
                                        if !current_game_ids.contains(&gid) {
                                            current_game_ids.push(gid);
                                        }
                                        let room = state.room_manager.get_or_create_room(gid).await;
                                        if let Ok(room) = room {
                                            // Determine color from room's stored player IDs.
                                            // If the room was created before the opponent joined
                                            // via REST, the IDs may be stale — refresh from DB.
                                            let color = match room.player_color_from_db(*user_id).await {
                                                Ok(c) => {
                                                    tracing::info!("[WS JoinGame] user={} color={:?} (from cached IDs)", username, c);
                                                    c
                                                }
                                                Err(_) => {
                                                    // Room was cached before this player joined via REST.
                                                    // Refresh player IDs from DB and retry.
                                                    tracing::info!("[WS JoinGame] user={} color lookup failed, refreshing from DB", username);
                                                    if let Ok(Some(game)) = state.game_repo.find_by_id(gid).await {
                                                        room.refresh_player_ids(game.red_player_id, game.black_player_id).await;
                                                        tracing::info!("[WS JoinGame] refreshed IDs: red={:?} black={:?}", game.red_player_id, game.black_player_id);
                                                    }
                                                    match room.player_color_from_db(*user_id).await {
                                                        Ok(c) => {
                                                            tracing::info!("[WS JoinGame] user={} color={:?} (after refresh)", username, c);
                                                            c
                                                        }
                                                        Err(e) => {
                                                            tracing::warn!("[WS JoinGame] user={} still can't determine color after refresh: {}", username, e);
                                                            continue; // Not a player in this game
                                                        }
                                                    }
                                                }
                                            };
                                            let client = crate::websocket::client::Client::new(*user_id, username.clone(), tx.clone());
                                            // Check if THIS specific player was disconnected (reconnect).
                                            // Must check the specific user, not just "is anyone disconnected",
                                            // to avoid misidentifying a first-time join as a reconnect.
                                            let was_disconnected = room.is_user_disconnected(*user_id).await;
                                            let both_present = room.join(client, color).await.ok().unwrap_or(false);
                                            tracing::info!("[WS JoinGame] user={} joined: was_disconnected={}, both_present={}", username, was_disconnected, both_present);

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

                                            // Notify players about each other
                                            let opponent_user_id = room.opponent_player_id(color).await;
                                            tracing::info!("[WS JoinGame] user={} opponent_player_id={:?}", username, opponent_user_id);
                                            if let Some(opp_id) = opponent_user_id {
                                                // Check if opponent is actually connected in the room
                                                let opponent_present = room.has_player(opp_id).await;
                                                tracing::info!("[WS JoinGame] user={} opponent_present={} (opp_id={})", username, opponent_present, opp_id);
                                                if opponent_present {
                                                    if was_disconnected {
                                                        // Player was disconnected and just reconnected
                                                        tracing::info!("[WS JoinGame] user={} → sending OpponentReconnected to opponent", username);
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
                                                    } else if both_present {
                                                        // Both players are now present — notify BOTH players.
                                                        // This handles the race where Player B's JoinGame is
                                                        // processed before Player A's: when A later joins,
                                                        // A also needs to know that B is already here.
                                                        tracing::info!("[WS JoinGame] user={} → both present, notifying both players", username);

                                                        // 1. Notify the OPPONENT about the joining player
                                                        if let Ok(Some(joining_user)) = state.user_repo.find_by_id(*user_id).await {
                                                            let joining_info = crate::db::models::UserInfo::from(joining_user);
                                                            let opp_joined_msg = ServerMessage::OpponentJoined {
                                                                game_id: game_id.clone(),
                                                                opponent: joining_info,
                                                                fen: fen.clone(),
                                                            };
                                                            room.broadcast_to_opponent(color, &opp_joined_msg).await;
                                                        }

                                                        // 2. Notify the JOINING player about the opponent
                                                        //    (opponent was already in the room waiting)
                                                        if let Ok(Some(opp_user)) = state.user_repo.find_by_id(opp_id).await {
                                                            let opp_info = crate::db::models::UserInfo::from(opp_user);
                                                            let opp_waiting_msg = ServerMessage::OpponentJoined {
                                                                game_id: game_id.clone(),
                                                                opponent: opp_info,
                                                                fen: fen.clone(),
                                                            };
                                                            // Send directly to the joining player (not via broadcast_to_opponent
                                                            // which would send to the opponent instead)
                                                            let json = serde_json::to_string(&opp_waiting_msg).unwrap_or_default();
                                                            tx.try_send(json).ok();
                                                        }
                                                    } else {
                                                        // Opponent is present but both_present is false — this means
                                                        // one player slot is empty (e.g., a third party or the opponent
                                                        // filled the slot while the other player's slot was vacated).
                                                        // Treat as reconnection since the game was already in progress.
                                                        tracing::info!("[WS JoinGame] user={} → fallback: sending OpponentReconnected (opponent present, both_present=false)", username);
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
                                                } else {
                                                    tracing::info!("[WS JoinGame] user={} opponent NOT present (opp_id={}), no notification sent", username, opp_id);
                                                }
                                            } else {
                                                tracing::info!("[WS JoinGame] user={} opponent_player_id is None, no notification sent", username);
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
                                                state.persist_game_end(gid, &result_str, &reason_str, &fen).await;
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
                                                state.persist_game_end(gid, &result_str, &reason_str, &fen).await;
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
                                                    state.persist_game_end(gid, &result_str, &reason_str, &fen).await;
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
                            ClientMessage::SubscribeLobby => {
                                if let Some((user_id, username, _)) = &authenticated_user {
                                    let client = crate::websocket::client::Client::new(*user_id, username.clone(), tx.clone());
                                    state.room_manager.subscribe_lobby(client).await;

                                    // Send initial lobby state immediately
                                    if let Ok(rows) = state.game_repo.list_with_players(None, 1, 100).await {
                                        let games: Vec<LobbyGameInfo> = rows.into_iter().map(|(game, red_player, black_player)| {
                                            LobbyGameInfo {
                                                id: game.id.to_string(),
                                                red_player,
                                                black_player,
                                                status: game.status,
                                                time_control: game.time_control,
                                                move_time_limit: game.move_time_limit,
                                                byoyomi: game.byoyomi,
                                                created_at: game.created_at.to_rfc3339(),
                                            }
                                        }).collect();
                                        let msg = ServerMessage::LobbyUpdate { games };
                                        tx.try_send(serde_json::to_string(&msg).unwrap_or_default()).ok();
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
        // Unsubscribe from lobby updates
        state.room_manager.unsubscribe_lobby(*user_id).await;

        for gid in &current_game_ids {
            let room = state.room_manager.get_or_create_room(*gid).await;
            if let Ok(room) = room {
                // mark_disconnected starts the grace period — game does NOT end immediately
                let result = room.mark_disconnected(*user_id).await;
                // If result is Some, game ended immediately (spectator or already-over game)
                if let Ok(Some((_, result_str, reason_str))) = result {
                    let fen = room.fen().await;
                    state.persist_game_end(*gid, &result_str, &reason_str, &fen).await;
                }
            }
        }
    }

    send_task.abort();
}
