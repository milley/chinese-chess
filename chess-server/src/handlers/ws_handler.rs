use axum::extract::ws::{WebSocket, Message};
use axum::extract::State;
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::utils::auth::verify_token;
use crate::websocket::client::Client;
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
                    ClientMessage::MakeMove { game_id, from, to } => {
                        if let Some((user_id, _username)) = &authenticated_user {
                            if let Ok(gid) = Uuid::parse_str(&game_id) {
                                // Try to make move via room manager
                                // For now, we need the player's color from the game data
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
                        // handled in REST for now
                    }
                    ClientMessage::LeaveGame { game_id } => {
                        // cleanup
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
                        // TODO: implement draw offer
                    }
                    ClientMessage::RespondDraw { game_id, accept } => {
                        // TODO: implement draw response
                    }
                }
            }
            Ok(Message::Close(_)) => break,
            Err(_) => break,
            _ => {}
        }
    }

    // 清理
    send_task.abort();
}