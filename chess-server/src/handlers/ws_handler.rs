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
    let mut current_game_id: Option<Uuid> = None;

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
                                tx.send(serde_json::to_string(&ServerMessage::Pong).unwrap_or_default()).ok();
                            }
                        } else {
                            tx.send(serde_json::to_string(&ServerMessage::Error { message: "Authentication failed".into() }).unwrap_or_default()).ok();
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
                                    let player_color = if game.red_player_id == Some(*user_id) {
                                        chess_engine::Color::Red
                                    } else if game.black_player_id == Some(*user_id) {
                                        chess_engine::Color::Black
                                    } else {
                                        continue; // Not a player
                                    };
                                    let result = state.room_manager.make_move(gid, *user_id, player_color, &from, &to).await;
                                    if let Err(e) = result {
                                        let msg = ServerMessage::IllegalMove { game_id: game_id.clone(), reason: e };
                                        tx.send(serde_json::to_string(&msg).unwrap_or_default()).ok();
                                    }
                                }
                            }
                        }
                    }
                    ClientMessage::JoinGame { game_id } => {
                        if let Some((user_id, username)) = &authenticated_user {
                            if let Ok(gid) = Uuid::parse_str(&game_id) {
                                current_game_id = Some(gid);
                                let room = state.room_manager.get_or_create_room(gid).await;
                                if let Ok(room) = room {
                                    // Determine color from DB
                                    let game = state.game_repo.find_by_id(gid).await.ok().flatten();
                                    if let Some(game) = game {
                                        let color = if game.red_player_id == Some(*user_id) {
                                            chess_engine::Color::Red
                                        } else if game.black_player_id == Some(*user_id) {
                                            chess_engine::Color::Black
                                        } else {
                                            continue;
                                        };
                                        let client = crate::websocket::client::Client::new(*user_id, username.clone(), tx.clone());
                                        room.join(client, color).await.ok();
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
                                    room.leave(*user_id).await.ok();
                                }
                                current_game_id = None;
                            }
                        }
                    }
                    ClientMessage::Resign { game_id } => {
                        if let Some((user_id, _)) = &authenticated_user {
                            if let Ok(gid) = Uuid::parse_str(&game_id) {
                                let room = state.room_manager.get_or_create_room(gid).await;
                                if let Ok(room) = room {
                                    room.resign(*user_id).await.ok();
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
                                    if let Err(e) = room.respond_draw(*user_id, accept).await {
                                        let msg = ServerMessage::Error { message: e };
                                        tx.send(serde_json::to_string(&msg).unwrap_or_default()).ok();
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

    // 断连清理: 从房间移除玩家并通知对手
    if let Some((user_id, _)) = &authenticated_user {
        if let Some(gid) = current_game_id {
            let room = state.room_manager.get_or_create_room(gid).await;
            if let Ok(room) = room {
                room.handle_disconnect(*user_id).await;
            }
        }
    }

    send_task.abort();
}
