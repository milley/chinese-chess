use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::db::repositories::game_repo::GameRepository;
use crate::websocket::client::Client;
use crate::websocket::message::ServerMessage;

/// A structured entry for each move in the game's move history.
/// Stored as a JSON array in the `move_history` DB column, enabling
/// full game replay and bug reproduction with per-move context.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MoveEntry {
    /// UCI move string e.g. "b0-c2"
    #[serde(rename = "move")]
    pub mv: String,
    /// Which color made the move: "red" or "black"
    pub color: String,
    /// Board state (FEN) after the move
    pub fen: String,
    /// Whether this move delivers check
    pub is_check: bool,
    /// Seconds the player spent on this move (None if no time control)
    pub time_spent: Option<i32>,
    /// Red's remaining time after the move (seconds)
    pub red_time: Option<i32>,
    /// Black's remaining time after the move (seconds)
    pub black_time: Option<i32>,
    /// ISO 8601 UTC timestamp of when the move was made
    pub timestamp: String,
}

/// 走棋结果
pub struct MoveResult {
    pub fen: String,
    pub is_check: bool,
    pub is_game_over: bool,
    pub result: Option<String>,
    pub end_reason: Option<String>,
    pub move_history: Vec<MoveEntry>,
}

/// 游戏房间
pub struct GameRoom {
    /// 游戏状态
    game_state: Arc<RwLock<chess_engine::GameState>>,
    /// 对局 ID
    game_id: Uuid,
    /// 红方玩家
    red_player: Arc<RwLock<Option<Client>>>,
    /// 黑方玩家
    black_player: Arc<RwLock<Option<Client>>>,
    /// 观战者
    spectators: Arc<RwLock<Vec<Client>>>,
    /// 时间控制
    time_control: Arc<RwLock<Option<chess_engine::TimeControl>>>,
    /// 和棋请求状态
    draw_offer: Arc<RwLock<Option<chess_engine::Color>>>,
    /// 结构化走法记录 (每步含用时、剩余时间、时间戳)
    move_log: Arc<RwLock<Vec<MoveEntry>>>,
    /// 数据库仓库
    game_repo: GameRepository,
}

impl GameRoom {
    pub fn new(
        game_id: Uuid,
        fen: &str,
        game_repo: GameRepository,
        time_control: Option<i32>,
        move_time_limit: Option<i32>,
        byoyomi: Option<i32>,
    ) -> Self {
        let game_state = chess_engine::GameState::from_fen(fen)
            .unwrap_or_else(|_| chess_engine::GameState::new());

        let tc = if time_control.is_some() || move_time_limit.is_some() || byoyomi.is_some() {
            Some(chess_engine::TimeControl::new(time_control, move_time_limit, byoyomi))
        } else {
            None
        };

        Self {
            game_state: Arc::new(RwLock::new(game_state)),
            game_id,
            red_player: Arc::new(RwLock::new(None)),
            black_player: Arc::new(RwLock::new(None)),
            spectators: Arc::new(RwLock::new(Vec::new())),
            time_control: Arc::new(RwLock::new(tc)),
            draw_offer: Arc::new(RwLock::new(None)),
            move_log: Arc::new(RwLock::new(Vec::new())),
            game_repo,
        }
    }

    /// Create a room restoring from persisted DB state (for server restart recovery).
    pub fn new_with_state(
        game_id: Uuid,
        fen: &str,
        game_repo: GameRepository,
        time_control: Option<i32>,
        move_time_limit: Option<i32>,
        byoyomi: Option<i32>,
        red_time: Option<i32>,
        black_time: Option<i32>,
    ) -> Self {
        let game_state = chess_engine::GameState::from_fen(fen)
            .unwrap_or_else(|_| chess_engine::GameState::new());

        let tc = if time_control.is_some() || move_time_limit.is_some() || byoyomi.is_some() {
            let red_remaining = red_time.unwrap_or(0);
            let black_remaining = black_time.unwrap_or(0);
            Some(chess_engine::TimeControl::new_with_state(
                time_control,
                move_time_limit,
                byoyomi,
                red_remaining,
                black_remaining,
            ))
        } else {
            None
        };

        Self {
            game_state: Arc::new(RwLock::new(game_state)),
            game_id,
            red_player: Arc::new(RwLock::new(None)),
            black_player: Arc::new(RwLock::new(None)),
            spectators: Arc::new(RwLock::new(Vec::new())),
            time_control: Arc::new(RwLock::new(tc)),
            draw_offer: Arc::new(RwLock::new(None)),
            move_log: Arc::new(RwLock::new(Vec::new())),
            game_repo,
        }
    }

    /// 玩家加入
    pub async fn join(&self, client: Client, color: chess_engine::Color) -> Result<(), String> {
        match color {
            chess_engine::Color::Red => {
                let mut player = self.red_player.write().await;
                if player.is_some() {
                    return Err("Red player slot is already occupied".into());
                }
                *player = Some(client);
            }
            chess_engine::Color::Black => {
                let mut player = self.black_player.write().await;
                if player.is_some() {
                    return Err("Black player slot is already occupied".into());
                }
                *player = Some(client);
            }
        }
        Ok(())
    }

    /// 执行走法
    pub async fn make_move(
        &self,
        _user_id: Uuid,
        player_color: chess_engine::Color,
        from: &str,
        to: &str,
    ) -> Result<MoveResult, String> {
        let from_pos = chess_engine::Position::from_uci(from)
            .ok_or("Invalid from position")?;
        let to_pos = chess_engine::Position::from_uci(to)
            .ok_or("Invalid to position")?;
        let m = chess_engine::Move::new(from_pos, to_pos);

        let mut state = self.game_state.write().await;

        // 检查是否轮到该玩家
        if state.side_to_move() != player_color {
            return Err("Not your turn".into());
        }

        // 执行走法
        state.make_move(m).map_err(|e| format!("{:?}", e))?;

        let fen = state.to_fen();
        let is_check = chess_engine::is_in_check(state.board(), state.side_to_move());
        let is_game_over = state.is_game_over();

        let (result, end_reason) = if is_game_over {
            match state.result() {
                Some((r, reason)) => {
                    let r_str = match r {
                        chess_engine::GameResult::RedWin => "red_win",
                        chess_engine::GameResult::BlackWin => "black_win",
                        chess_engine::GameResult::Draw => "draw",
                    };
                    let reason_str = match reason {
                        chess_engine::GameEndReason::Checkmate => "checkmate",
                        chess_engine::GameEndReason::Stalemate => "stalemate",
                        chess_engine::GameEndReason::Resign(_) => "resign",
                        chess_engine::GameEndReason::DrawAgreement => "draw_agreement",
                        chess_engine::GameEndReason::Timeout(_) => "timeout",
                    };
                    (Some(r_str.to_string()), Some(reason_str.to_string()))
                }
                None => (None, None),
            }
        } else {
            (None, None)
        };

        // Update time control: reset move_elapsed for the player who just moved.
        // Capture the elapsed time before it's reset.
        // This is done while still holding the game_state write lock to prevent
        // inconsistency between game state and time state (e.g., tick_time running
        // between the two locks and seeing stale move_elapsed).
        let time_spent = {
            let mut tc = self.time_control.write().await;
            if let Some(ref mut tc) = *tc {
                Some(tc.on_move_made(player_color))
            } else {
                None
            }
        };

        drop(state);

        // Get current time state for MoveMade message and MoveEntry
        let (red_time, black_time) = {
            let tc = self.time_control.read().await;
            match tc.as_ref() {
                Some(tc) => (Some(tc.remaining(chess_engine::Color::Red) as i64), Some(tc.remaining(chess_engine::Color::Black) as i64)),
                None => (None, None),
            }
        };

        // 广播走法
        let msg = ServerMessage::MoveMade {
            game_id: self.game_id.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            fen: fen.clone(),
            is_check,
            red_time,
            black_time,
        };
        self.broadcast(&msg).await;

        // 如果游戏结束，广播结果
        if is_game_over {
            if let (Some(res), Some(reason)) = (&result, &end_reason) {
                let over_msg = ServerMessage::GameOver {
                    game_id: self.game_id.to_string(),
                    result: res.clone(),
                    reason: reason.clone(),
                };
                self.broadcast(&over_msg).await;
            }
        }

        // Persist time to DB after each move
        self.persist_time().await;

        // Build structured move entry and append to the in-memory log
        let color_str = match player_color {
            chess_engine::Color::Red => "red",
            chess_engine::Color::Black => "black",
        };
        let entry = MoveEntry {
            mv: format!("{}-{}", from, to),
            color: color_str.to_string(),
            fen: fen.clone(),
            is_check,
            time_spent,
            red_time: red_time.map(|t| t as i32),
            black_time: black_time.map(|t| t as i32),
            timestamp: Utc::now().to_rfc3339(),
        };
        {
            let mut log = self.move_log.write().await;
            log.push(entry);
        }

        // Return the full move history as structured entries
        let move_history = {
            let log = self.move_log.read().await;
            log.clone()
        };

        Ok(MoveResult {
            fen,
            is_check,
            is_game_over,
            result,
            end_reason,
            move_history,
        })
    }

    /// 认输
    /// Returns (game_id, result_str, reason_str) so the caller can persist to DB.
    pub async fn resign(&self, user_id: Uuid) -> Result<(String, String, String), String> {
        // Check if game is already over before resigning
        {
            let state = self.game_state.read().await;
            if state.is_game_over() {
                return Err("Game is already over".into());
            }
        }

        let color = self.player_color(user_id).await?;

        let (result_str, reason_str) = match color {
            chess_engine::Color::Red => ("black_win", "resign"),
            chess_engine::Color::Black => ("red_win", "resign"),
        };

        {
            let mut state = self.game_state.write().await;
            state.resign(color);
        }

        let msg = ServerMessage::GameOver {
            game_id: self.game_id.to_string(),
            result: result_str.to_string(),
            reason: reason_str.to_string(),
        };
        self.broadcast(&msg).await;

        Ok((self.game_id.to_string(), result_str.to_string(), reason_str.to_string()))
    }

    /// 提议和棋
    /// Atomically checks game-over and sets draw offer under a single game_state read lock
    /// to prevent TOCTOU between the check and the offer.
    pub async fn offer_draw(&self, user_id: Uuid) -> Result<(), String> {
        let color = self.player_color(user_id).await?;

        // Atomically check game-over
        {
            let state = self.game_state.read().await;
            if state.is_game_over() {
                return Err("Game is already over".into());
            }
        }

        // Set draw offer
        *self.draw_offer.write().await = Some(color);

        // Notify opponent
        let msg = ServerMessage::DrawOffered {
            game_id: self.game_id.to_string(),
        };
        self.broadcast_to_opponent(color, &msg).await;

        Ok(())
    }

    /// 响应和棋提议
    /// Returns Some((game_id, result_str, reason_str)) if draw accepted (game ended),
    /// None if draw rejected or no game end.
    ///
    /// Uses a single game_state write lock to atomically check is_game_over and execute
    /// the draw, preventing TOCTOU between the check and the draw() call.
    pub async fn respond_draw(&self, user_id: Uuid, accept: bool) -> Result<Option<(String, String, String)>, String> {
        let color = self.player_color(user_id).await?;

        // Check for pending draw offer
        let offer = *self.draw_offer.read().await;
        if offer.is_none() {
            return Err("No draw offer to respond to".into());
        }
        if offer == Some(color) {
            return Err("You cannot respond to your own draw offer".into());
        }

        if accept {
            // Atomically check game-over + execute draw under one write lock
            let already_over = {
                let mut state = self.game_state.write().await;
                if state.is_game_over() {
                    true
                } else {
                    state.draw();
                    false
                }
            };

            if already_over {
                // Clear the offer since game is already over
                *self.draw_offer.write().await = None;
                return Err("Game is already over".into());
            }

            // Clear draw offer
            *self.draw_offer.write().await = None;

            let msg = ServerMessage::DrawResponse {
                game_id: self.game_id.to_string(),
                accepted: true,
            };
            self.broadcast(&msg).await;

            let over_msg = ServerMessage::GameOver {
                game_id: self.game_id.to_string(),
                result: "draw".to_string(),
                reason: "draw_agreement".to_string(),
            };
            self.broadcast(&over_msg).await;

            Ok(Some((self.game_id.to_string(), "draw".to_string(), "draw_agreement".to_string())))
        } else {
            // Reject draw, clear offer
            *self.draw_offer.write().await = None;

            let msg = ServerMessage::DrawResponse {
                game_id: self.game_id.to_string(),
                accepted: false,
            };
            self.broadcast(&msg).await;

            Ok(None)
        }
    }

    /// 玩家离开对局 (对局中离开判负)
    /// Returns Some((game_id, result_str, reason_str)) if game ended by resignation,
    /// None if game was already over or player was just removed.
    ///
    /// Uses a single game_state write lock to avoid TOCTOU between is_game_over check
    /// and the resign operation.
    pub async fn leave(&self, user_id: Uuid) -> Result<Option<(String, String, String)>, String> {
        let color = self.player_color(user_id).await?;

        // Atomically check game-over + resign under one write lock
        let already_over = {
            let mut state = self.game_state.write().await;
            if state.is_game_over() {
                true
            } else {
                state.resign(color);
                false
            }
        };

        // Always remove the player from the room
        self.remove_player(user_id).await;

        if already_over {
            return Ok(None);
        }

        let (result_str, reason_str) = match color {
            chess_engine::Color::Red => ("black_win", "resign"),
            chess_engine::Color::Black => ("red_win", "resign"),
        };

        let msg = ServerMessage::GameOver {
            game_id: self.game_id.to_string(),
            result: result_str.to_string(),
            reason: reason_str.to_string(),
        };
        self.broadcast(&msg).await;

        Ok(Some((self.game_id.to_string(), result_str.to_string(), reason_str.to_string())))
    }

    /// 客户端断连处理
    /// If the game is in progress, the disconnecting player loses by resignation.
    /// If the game is already over, just remove the player.
    pub async fn handle_disconnect(&self, user_id: Uuid) -> Result<Option<(String, String, String)>, String> {
        // Check if this user is actually a player (not a spectator)
        let color_result = self.player_color(user_id).await;

        // Always remove the player from the room
        self.remove_player(user_id).await;

        // Notify opponent about disconnect
        let msg = ServerMessage::OpponentDisconnected {
            game_id: self.game_id.to_string(),
        };
        self.broadcast(&msg).await;

        // If not a player (spectator), nothing more to do
        let color = match color_result {
            Ok(c) => c,
            Err(_) => return Ok(None),
        };

        // If game is already over, nothing more to do
        if self.is_game_over().await {
            return Ok(None);
        }

        // Game is in progress and a player disconnected — treat as resignation
        {
            let mut state = self.game_state.write().await;
            if !state.is_game_over() {
                state.resign(color);
            }
        }

        let (result_str, reason_str) = match color {
            chess_engine::Color::Red => ("black_win", "disconnect"),
            chess_engine::Color::Black => ("red_win", "disconnect"),
        };

        let over_msg = ServerMessage::GameOver {
            game_id: self.game_id.to_string(),
            result: result_str.to_string(),
            reason: reason_str.to_string(),
        };
        self.broadcast(&over_msg).await;

        Ok(Some((self.game_id.to_string(), result_str.to_string(), reason_str.to_string())))
    }

    /// 获取玩家颜色
    pub async fn player_color(&self, user_id: Uuid) -> Result<chess_engine::Color, String> {
        let red = self.red_player.read().await;
        let black = self.black_player.read().await;
        if red.as_ref().map(|c| c.user_id) == Some(user_id) {
            Ok(chess_engine::Color::Red)
        } else if black.as_ref().map(|c| c.user_id) == Some(user_id) {
            Ok(chess_engine::Color::Black)
        } else {
            Err("You are not a player in this game".into())
        }
    }

    /// 从房间移除玩家 (single write lock to avoid TOCTOU)
    async fn remove_player(&self, user_id: Uuid) {
        // Use write locks from the start to avoid TOCTOU between read and write
        let mut red = self.red_player.write().await;
        if red.as_ref().map(|c| c.user_id) == Some(user_id) {
            *red = None;
            return;
        }
        drop(red);

        let mut black = self.black_player.write().await;
        if black.as_ref().map(|c| c.user_id) == Some(user_id) {
            *black = None;
        }
    }

    /// 发送消息给对方玩家
    pub async fn broadcast_to_opponent(&self, color: chess_engine::Color, message: &ServerMessage) {
        let json = serde_json::to_string(message).unwrap_or_default();
        let player = match color {
            chess_engine::Color::Red => self.black_player.read().await,
            chess_engine::Color::Black => self.red_player.read().await,
        };
        if let Some(client) = player.as_ref() {
            let _ = client.send(&json); // Ignore send failure — handled on disconnect
        }
    }

    /// 广播消息到房间内所有客户端
    /// Removes spectators whose send channel is closed (dead connections).
    pub async fn broadcast(&self, message: &ServerMessage) {
        let json = serde_json::to_string(message).unwrap_or_default();

        let red = self.red_player.read().await;
        if let Some(client) = red.as_ref() {
            if !client.send(&json) {
                // Red player's channel is dead — they'll be cleaned up on disconnect
            }
        }
        drop(red);

        let black = self.black_player.read().await;
        if let Some(client) = black.as_ref() {
            if !client.send(&json) {
                // Black player's channel is dead — they'll be cleaned up on disconnect
            }
        }
        drop(black);

        // Broadcast to spectators and prune dead ones
        let mut spectators = self.spectators.write().await;
        spectators.retain(|spectator| spectator.send(&json));
    }

    /// 获取当前 FEN
    pub async fn fen(&self) -> String {
        self.game_state.read().await.to_fen()
    }

    /// 检查指定玩家是否在房间内
    pub async fn has_player(&self, user_id: Uuid) -> bool {
        let red = self.red_player.read().await;
        if red.as_ref().map(|c| c.user_id) == Some(user_id) {
            return true;
        }
        drop(red);
        let black = self.black_player.read().await;
        black.as_ref().map(|c| c.user_id) == Some(user_id)
    }

    /// 获取对局 ID
    pub fn game_id(&self) -> Uuid {
        self.game_id
    }

    /// 获取当前走子方
    pub async fn current_side(&self) -> chess_engine::Color {
        self.game_state.read().await.side_to_move()
    }

    /// 检查游戏是否已结束
    pub async fn is_game_over(&self) -> bool {
        self.game_state.read().await.is_game_over()
    }

    /// 超时判负
    pub async fn timeout(&self, color: chess_engine::Color) {
        self.game_state.write().await.timeout(color);
    }

    /// 激活时间控制 (游戏开始时调用)
    pub async fn activate_time(&self) {
        let mut tc = self.time_control.write().await;
        if let Some(ref mut tc) = *tc {
            tc.activate();
        }
    }

    /// 执行一次时间 tick (由超时检查器每秒调用)
    /// 返回 None 表示没有时间控制
    pub async fn tick_time(&self) -> Option<chess_engine::TickResult> {
        let side = self.current_side().await;
        let mut tc = self.time_control.write().await;
        tc.as_mut().map(|tc| tc.tick(side))
    }

    /// 获取当前时间状态 (red_remaining, black_remaining, active_color, red_in_byoyomi, black_in_byoyomi)
    pub async fn time_state(&self) -> (i32, i32, String, bool, bool) {
        let tc = self.time_control.read().await;
        let side = self.current_side().await;
        match tc.as_ref() {
            Some(tc) => {
                let active_color = match side {
                    chess_engine::Color::Red => "red",
                    chess_engine::Color::Black => "black",
                };
                (
                    tc.remaining(chess_engine::Color::Red),
                    tc.remaining(chess_engine::Color::Black),
                    active_color.to_string(),
                    tc.phase(chess_engine::Color::Red) == chess_engine::TimePhase::Byoyomi,
                    tc.phase(chess_engine::Color::Black) == chess_engine::TimePhase::Byoyomi,
                )
            }
            None => (0, 0, "red".to_string(), false, false),
        }
    }

    /// 将当前时间持久化到数据库
    pub async fn persist_time(&self) {
        let tc = self.time_control.read().await;
        if let Some(ref tc) = *tc {
            let red_remaining = tc.remaining(chess_engine::Color::Red);
            let black_remaining = tc.remaining(chess_engine::Color::Black);
            let _ = self.game_repo.update_time(self.game_id, red_remaining, black_remaining).await;
        }
    }

    /// Get a read guard on the time control (for external inspection like tick_count).
    pub async fn time_control_read(&self) -> tokio::sync::RwLockReadGuard<'_, Option<chess_engine::TimeControl>> {
        self.time_control.read().await
    }

    /// Serialize the in-memory move log to a JSON string.
    /// Used by the timeout checker and other non-move game-end paths to preserve
    /// existing move history when writing to the DB.
    pub async fn move_history_json(&self) -> String {
        let log = self.move_log.read().await;
        serde_json::to_string(&*log).unwrap_or_else(|_| "[]".to_string())
    }
}
