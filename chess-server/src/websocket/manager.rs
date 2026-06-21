use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::db::repositories::game_repo::GameRepository;
use crate::db::repositories::user_repo::UserRepository;
use crate::websocket::message::ServerMessage;
use crate::websocket::room::{GameRepo, GameRoom, MoveResult};

/// Deduct elapsed downtime from player times after server restart.
/// `last_tick_at` is when the server last ticked the time control.
/// `fen` is used to determine which side was to move.
/// Returns (adjusted_red_time, adjusted_black_time).
fn deduct_downtime(
    red_time: Option<i32>,
    black_time: Option<i32>,
    fen: &str,
    last_tick_at: chrono::DateTime<Utc>,
) -> (Option<i32>, Option<i32>) {
    let now = Utc::now();
    if now <= last_tick_at {
        return (red_time, black_time);
    }
    let elapsed_secs = (now - last_tick_at).num_seconds() as i32;
    if elapsed_secs <= 0 {
        return (red_time, black_time);
    }
    let side_to_move = fen.split(' ').nth(1).unwrap_or("w");
    match side_to_move {
        "w" => {
            // Red was to move — deduct from red's time
            (red_time.map(|rt| (rt - elapsed_secs).max(0)), black_time)
        }
        _ => {
            // Black was to move — deduct from black's time
            (red_time, black_time.map(|bt| (bt - elapsed_secs).max(0)))
        }
    }
}

/// 房间管理器
pub struct RoomManager {
    /// 活跃房间映射
    rooms: Arc<RwLock<HashMap<Uuid, Arc<GameRoom>>>>,
    /// 数据库仓库 (kept for find_by_id and other direct queries)
    game_repo: GameRepository,
    /// 用户仓库 (for Elo updates on timeout)
    user_repo: UserRepository,
}

impl RoomManager {
    pub fn with_repos(game_repo: GameRepository, user_repo: UserRepository) -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
            game_repo,
            user_repo,
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

        // If game is playing and last_tick_at is set, deduct elapsed time
        // since the last tick. This handles server restart/crash recovery:
        // the time between last_tick_at and now was "lost" and should be
        // deducted from the active player's remaining time.
        let (red_time, black_time) = if game.status == "playing"
            && let Some(last_tick) = game.last_tick_at {
                deduct_downtime(game.red_time, game.black_time, &game.fen, last_tick)
            } else {
                (game.red_time, game.black_time)
            };

        let room = Arc::new(GameRoom::new_with_state(
            game_id,
            &game.fen,
            Arc::new(self.game_repo.clone()) as Arc<dyn GameRepo>,
            game.time_control,
            game.move_time_limit,
            game.byoyomi,
            red_time,
            black_time,
            game.red_player_id,
            game.black_player_id,
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
        let user_repo = self.user_repo.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            let mut cleanup_counter: u32 = 0;
            loop {
                interval.tick().await;

                // Snapshot room IDs under a short read lock, then release the lock
                // before processing. This prevents blocking room creation/join
                // during the potentially slow per-room tick + DB + broadcast cycle.
                let room_ids: Vec<(Uuid, Arc<GameRoom>)> = {
                    let rooms_guard = rooms.read().await;
                    rooms_guard.iter().map(|(id, room)| (*id, room.clone())).collect()
                };

                // Collect finished game IDs during iteration for periodic cleanup
                let mut finished_ids: Vec<Uuid> = Vec::new();

                for (game_id, room) in room_ids {
                    // Check disconnect grace period timeout first
                    if let Some((gid, result_str, reason_str)) = room.check_disconnect_timeout().await {
                        // Grace period expired — game ended by disconnect
                        let fen = room.fen().await;
                        let moves_json = room.move_history_json().await;
                        let game = game_repo.find_by_id(gid).await.ok().flatten();
                        if let Some(ref game) = game {
                            let _ = crate::services::elo_service::finish_game_with_elo(
                                &game_repo,
                                &user_repo,
                                gid,
                                game,
                                &result_str,
                                &reason_str,
                                &fen,
                                &moves_json,
                            ).await;
                        } else {
                            let _ = game_repo.finish_game(gid, &result_str, &reason_str, &fen, &moves_json).await;
                        }
                        room.persist_time().await;
                        finished_ids.push(game_id);
                        continue;
                    }

                    // Skip if game is already over (also collect for cleanup)
                    if room.is_game_over().await {
                        finished_ids.push(game_id);
                        continue;
                    }

                    // Tick the time control
                    let tick_result = room.tick_time().await;
                    match tick_result {
                        Some(chess_engine::TickResult::Timeout(color)) => {
                            // Double-check: another handler (resign/move/draw) may have ended the game
                            // between the tick and this point
                            if room.is_game_over().await {
                                finished_ids.push(game_id);
                                continue;
                            }

                            // End the game
                            room.timeout(color).await;
                            let fen = room.fen().await;

                            let (result_str, reason_str) = match color {
                                chess_engine::Color::Red => ("black_win", "timeout"),
                                chess_engine::Color::Black => ("red_win", "timeout"),
                            };

                            // Use finish_game_with_elo for proper Elo rating updates
                            // (fixes bug where timeouts previously skipped Elo)
                            let moves_json = room.move_history_json().await;
                            let game = game_repo.find_by_id(game_id).await.ok().flatten();
                            let was_first = if let Some(ref game) = game {
                                crate::services::elo_service::finish_game_with_elo(
                                    &game_repo,
                                    &user_repo,
                                    game_id,
                                    game,
                                    result_str,
                                    reason_str,
                                    &fen,
                                    &moves_json,
                                ).await.ok().unwrap_or(false)
                            } else {
                                // Fallback: no game record found, just finish without Elo
                                game_repo.finish_game(
                                    game_id, result_str, reason_str, &fen, &moves_json
                                ).await.ok().flatten().is_some()
                            };

                            // Only broadcast GameOver if we were the first to finish the game
                            if was_first {
                                // Persist final time state
                                room.persist_time().await;

                                // Log timeout event (fire-and-forget)
                                let ev_repo: Arc<dyn GameRepo> = Arc::new(game_repo.clone());
                                let ev_game_id = game_id;
                                let ev_color = match color {
                                    chess_engine::Color::Red => "red",
                                    chess_engine::Color::Black => "black",
                                };
                                tokio::spawn(async move {
                                    if let Err(e) = ev_repo.append_event(ev_game_id, "timeout".to_string(), None, serde_json::json!({
                                        "color": ev_color, "result": result_str, "reason": reason_str,
                                    })).await {
                                        tracing::info!("Failed to append timeout event for game {}: {}", ev_game_id, e);
                                    }
                                });

                                // Broadcast GameOver
                                let (rt, bt) = room.remaining_time().await;
                                let msg = ServerMessage::GameOver {
                                    game_id: game_id.to_string(),
                                    result: result_str.to_string(),
                                    reason: reason_str.to_string(),
                                    red_time: rt,
                                    black_time: bt,
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
                            // Also update last_tick_at for crash recovery time drift correction.
                            {
                                let should_persist = {
                                    let tc = room.time_control_read().await;
                                    tc.as_ref().is_some_and(|tc| tc.tick_count() % 5 == 0)
                                };
                                // Lock is released before persist_time, which acquires its own lock
                                if should_persist {
                                    room.persist_time().await;
                                    // Record last_tick_at for crash recovery
                                    if let Err(e) = game_repo.update_last_tick(game_id).await {
                                        tracing::warn!("Failed to update last_tick_at for game {}: {}", game_id, e);
                                    }
                                }
                            }
                        }
                        None => {
                            // No time control configured for this room
                        }
                    }
                }

                // Periodic cleanup: every 60 seconds, remove finished game rooms
                // to prevent unbounded memory growth. Game results are already
                // persisted to the DB, so the room data is just a runtime cache.
                cleanup_counter += 1;
                if cleanup_counter >= 60 {
                    cleanup_counter = 0;
                    if !finished_ids.is_empty() {
                        let mut rooms_guard = rooms.write().await;
                        for id in &finished_ids {
                            rooms_guard.remove(id);
                        }
                        tracing::info!("Cleaned up {} finished game rooms", finished_ids.len());
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
            user_repo: self.user_repo.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_deduct_downtime_red_to_move() {
        // Red was to move (FEN has "w"), 30 seconds of downtime
        let last_tick = Utc::now() - Duration::seconds(30);
        let (red, black) = deduct_downtime(
            Some(300), Some(300),
            "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1",
            last_tick,
        );
        assert_eq!(red, Some(270)); // 300 - 30
        assert_eq!(black, Some(300)); // unchanged
    }

    #[test]
    fn test_deduct_downtime_black_to_move() {
        // Black was to move (FEN has "b"), 30 seconds of downtime
        let last_tick = Utc::now() - Duration::seconds(30);
        let (red, black) = deduct_downtime(
            Some(300), Some(300),
            "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR b - - 0 1",
            last_tick,
        );
        assert_eq!(red, Some(300)); // unchanged
        assert_eq!(black, Some(270)); // 300 - 30
    }

    #[test]
    fn test_deduct_downtime_floor_at_zero() {
        // Red had only 10s left, 30 seconds of downtime → clamped to 0
        let last_tick = Utc::now() - Duration::seconds(30);
        let (red, _) = deduct_downtime(
            Some(10), Some(300),
            "some_fen w - - 0 1",
            last_tick,
        );
        assert_eq!(red, Some(0));
    }

    #[test]
    fn test_deduct_downtime_no_downtime() {
        // last_tick is in the future (clock skew) → no deduction
        let last_tick = Utc::now() + Duration::seconds(10);
        let (red, black) = deduct_downtime(
            Some(300), Some(300),
            "some_fen w - - 0 1",
            last_tick,
        );
        assert_eq!(red, Some(300));
        assert_eq!(black, Some(300));
    }

    #[test]
    fn test_deduct_downtime_none_times() {
        // No time control configured → no deduction
        let last_tick = Utc::now() - Duration::seconds(30);
        let (red, black) = deduct_downtime(
            None, None,
            "some_fen w - - 0 1",
            last_tick,
        );
        assert_eq!(red, None);
        assert_eq!(black, None);
    }
}
