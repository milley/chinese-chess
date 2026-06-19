use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use uuid::Uuid;

use crate::db::repositories::game_repo::GameRepository;
use crate::websocket::message::ServerMessage;
use crate::websocket::room::{GameRepo, GameRoom, MoveResult};

/// 房间管理器
pub struct RoomManager {
    /// 活跃房间映射
    rooms: Arc<RwLock<HashMap<Uuid, Arc<GameRoom>>>>,
    /// 数据库仓库 (kept for find_by_id and other direct queries)
    game_repo: GameRepository,
}

impl RoomManager {
    pub fn with_game_repo(game_repo: GameRepository) -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
            game_repo,
        }
    }

    /// 获取或创建房间 (懒加载，从数据库恢复状态)
    /// Uses write lock from the start to prevent TOCTOU duplicate room creation.
    pub async fn get_or_create_room(&self, game_id: Uuid) -> Result<Arc<GameRoom>, String> {
        // Fast path: check with read lock first
        {
            let rooms = self.rooms.read().await;
            if let Some(room) = rooms.get(&game_id) {
                return Ok(room.clone());
            }
        }

        // Slow path: acquire write lock and double-check (like a mutex guard pattern)
        let mut rooms = self.rooms.write().await;
        if let Some(room) = rooms.get(&game_id) {
            return Ok(room.clone());
        }

        // 从数据库加载
        let game = self.game_repo.find_by_id(game_id).await
            .map_err(|e| format!("DB error: {}", e))?
            .ok_or("Game not found".to_string())?;

        let room = Arc::new(GameRoom::new_with_state(
            game_id,
            &game.fen,
            Arc::new(self.game_repo.clone()) as Arc<dyn GameRepo>,
            game.time_control,
            game.move_time_limit,
            game.byoyomi,
            game.red_time,
            game.black_time,
        ));

        // If game is already playing, activate time control
        if game.status == "playing" {
            room.activate_time().await;
        }

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
    pub async fn respond_draw(&self, game_id: Uuid, user_id: Uuid, accept: bool) -> Result<Option<(String, String, String)>, String> {
        let room = self.get_or_create_room(game_id).await?;
        room.respond_draw(user_id, accept).await
    }

    /// 离开对局
    pub async fn leave(&self, game_id: Uuid, user_id: Uuid) -> Result<Option<(String, String, String)>, String> {
        let room = self.get_or_create_room(game_id).await?;
        room.leave(user_id).await
    }

    /// 客户端断连. Returns the disconnect result if the room exists.
    /// Ok(Some((game_id, result_str, reason_str))) = game ended by disconnect
    /// Ok(None) = game already over or player was spectator
    /// Err = room not found or player not in room
    pub async fn handle_disconnect(&self, game_id: Uuid, user_id: Uuid) -> Result<Option<(String, String, String)>, String> {
        let room = self.get_or_create_room(game_id).await?;
        room.handle_disconnect(user_id).await
    }

    /// Remove a room from the in-memory map (used when a game is deleted via REST).
    /// This prevents zombie rooms that no longer correspond to any DB record.
    pub async fn remove_room(&self, game_id: Uuid) {
        let mut rooms = self.rooms.write().await;
        rooms.remove(&game_id);
    }

    /// 启动超时检查器 (每秒 tick 一次)
    pub fn start_timeout_checker(&self) {
        let rooms = self.rooms.clone();
        let game_repo = self.game_repo.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;

                // Snapshot room IDs under a short read lock, then release the lock
                // before processing. This prevents blocking room creation/join
                // during the potentially slow per-room tick + DB + broadcast cycle.
                let room_ids: Vec<(Uuid, Arc<GameRoom>)> = {
                    let rooms_guard = rooms.read().await;
                    rooms_guard.iter().map(|(id, room)| (*id, room.clone())).collect()
                };

                for (game_id, room) in room_ids {
                    // Skip if game is already over
                    if room.is_game_over().await {
                        continue;
                    }

                    // Tick the time control
                    let tick_result = room.tick_time().await;
                    match tick_result {
                        Some(chess_engine::TickResult::Timeout(color)) => {
                            // Double-check: another handler (resign/move/draw) may have ended the game
                            // between the tick and this point
                            if room.is_game_over().await {
                                continue;
                            }

                            // End the game
                            room.timeout(color).await;
                            let fen = room.fen().await;

                            let (result_str, reason_str) = match color {
                                chess_engine::Color::Red => ("black_win", "timeout"),
                                chess_engine::Color::Black => ("red_win", "timeout"),
                            };

                            // Update DB (idempotent: WHERE status != 'finished')
                            // Preserve existing move history from the in-memory log
                            let moves_json = room.move_history_json().await;
                            let result = game_repo.finish_game(
                                game_id, result_str, reason_str, &fen, &moves_json
                            ).await;

                            // Only broadcast GameOver if we were the first to finish the game
                            if let Ok(Some(_)) = result {
                                // Persist final time state
                                room.persist_time().await;

                                // Broadcast GameOver
                                let msg = ServerMessage::GameOver {
                                    game_id: game_id.to_string(),
                                    result: result_str.to_string(),
                                    reason: reason_str.to_string(),
                                };
                                room.broadcast(&msg).await;
                            }
                        }
                        Some(chess_engine::TickResult::Ok { .. }) => {
                            // Broadcast TimeUpdate to both players
                            let (red_time, black_time, active_color, red_in_byoyomi, black_in_byoyomi) =
                                room.time_state().await;

                            let msg = ServerMessage::TimeUpdate {
                                game_id: game_id.to_string(),
                                red_time: red_time as i64,
                                black_time: black_time as i64,
                                active_color,
                                red_in_byoyomi,
                                black_in_byoyomi,
                            };
                            room.broadcast(&msg).await;

                            // Persist time to DB periodically (every 5 seconds to reduce write load)
                            // The server is authoritative — TimeUpdate broadcasts every second
                            // ensure clients stay in sync even if DB writes are less frequent.
                            {
                                let should_persist = {
                                    let tc = room.time_control_read().await;
                                    tc.as_ref().is_some_and(|tc| tc.tick_count() % 5 == 0)
                                };
                                // Lock is released before persist_time, which acquires its own lock
                                if should_persist {
                                    room.persist_time().await;
                                }
                            }
                        }
                        None => {
                            // No time control configured for this room
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
