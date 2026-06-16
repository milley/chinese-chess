use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use uuid::Uuid;

use crate::db::repositories::game_repo::GameRepository;
use crate::websocket::message::ServerMessage;
use crate::websocket::room::{GameRoom, MoveResult};

/// 房间管理器
pub struct RoomManager {
    /// 活跃房间映射
    rooms: Arc<RwLock<HashMap<Uuid, Arc<GameRoom>>>>,
    /// 数据库仓库
    game_repo: GameRepository,
}

impl RoomManager {
    pub fn with_game_repo(game_repo: GameRepository) -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
            game_repo,
        }
    }

    /// 获取或创建房间 (懒加载)
    pub async fn get_or_create_room(&self, game_id: Uuid) -> Result<Arc<GameRoom>, String> {
        let rooms = self.rooms.read().await;
        if let Some(room) = rooms.get(&game_id) {
            return Ok(room.clone());
        }
        drop(rooms);

        // 从数据库加载
        let game = self.game_repo.find_by_id(game_id).await
            .map_err(|e| format!("DB error: {}", e))?
            .ok_or("Game not found".to_string())?;

        let room = Arc::new(GameRoom::new(game_id, &game.fen, self.game_repo.clone()));

        let mut rooms = self.rooms.write().await;
        rooms.insert(game_id, room.clone());

        Ok(room)
    }

    /// 执行走法 (REST 和 WS 统一入口)
    pub async fn make_move(
        &self,
        game_id: Uuid,
        user_id: Uuid,
        player_color: chess_engine::Color,
        from: &str,
        to: &str,
    ) -> Result<MoveResult, String> {
        let room = self.get_or_create_room(game_id).await?;
        room.make_move(user_id, player_color, from, to).await
    }

    /// 提议和棋
    pub async fn offer_draw(&self, game_id: Uuid, user_id: Uuid) -> Result<(), String> {
        let room = self.get_or_create_room(game_id).await?;
        room.offer_draw(user_id).await
    }

    /// 响应和棋提议
    pub async fn respond_draw(&self, game_id: Uuid, user_id: Uuid, accept: bool) -> Result<(), String> {
        let room = self.get_or_create_room(game_id).await?;
        room.respond_draw(user_id, accept).await
    }

    /// 离开对局
    pub async fn leave(&self, game_id: Uuid, user_id: Uuid) -> Result<(), String> {
        let room = self.get_or_create_room(game_id).await?;
        room.leave(user_id).await
    }

    /// 客户端断连
    pub async fn handle_disconnect(&self, game_id: Uuid, user_id: Uuid) {
        if let Ok(room) = self.get_or_create_room(game_id).await {
            room.handle_disconnect(user_id).await;
        }
    }

    /// 启动超时检查器
    pub fn start_timeout_checker(&self) {
        let rooms = self.rooms.clone();
        let game_repo = self.game_repo.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                let rooms_guard = rooms.read().await;
                for (_, room) in rooms_guard.iter() {
                    // Check timeout: get game state info from the room
                    let fen = room.fen().await;
                    let game_id = room.game_id();
                    // Load game from DB for time control info
                    if let Ok(Some(game)) = game_repo.find_by_id(game_id).await {
                        if game.status != "playing" { continue; }

                        let move_start = room.move_start_time().await;
                        let elapsed = move_start.elapsed().as_secs() as i32;

                        // Check move time limit
                        if let Some(time_limit) = game.move_time_limit {
                            if elapsed >= time_limit {
                                // Timeout: determine who lost
                                let color = room.current_side().await;
                                let (result_str, reason_str) = match color {
                                    chess_engine::Color::Red => ("black_win", "timeout"),
                                    chess_engine::Color::Black => ("red_win", "timeout"),
                                };

                                // End the game in the room
                                room.timeout(color).await;

                                // Update DB
                                let _ = game_repo.finish_game(game_id, result_str, reason_str, &fen, "[]").await;

                                // Broadcast
                                let msg = ServerMessage::GameOver {
                                    game_id: game_id.to_string(),
                                    result: result_str.to_string(),
                                    reason: reason_str.to_string(),
                                };
                                room.broadcast(&msg).await;
                            }
                        }
                    }
                }
            }
        });
    }
}

impl Clone for RoomManager {
    fn clone(&self) -> Self {
        Self {
            rooms: self.rooms.clone(),
            game_repo: self.game_repo.clone(),
        }
    }
}
