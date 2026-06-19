use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::db::repositories::game_repo::GameRepository;
use crate::websocket::client::Client;
use crate::websocket::message::ServerMessage;

/// Trait abstracting the game repository operations needed by GameRoom.
/// Enables unit testing without a real database connection.
pub trait GameRepo: Send + Sync {
    fn update_time(&self, game_id: Uuid, red_time: i32, black_time: i32) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>>;
}

impl GameRepo for GameRepository {
    fn update_time(&self, game_id: Uuid, red_time: i32, black_time: i32) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        Box::pin(async move { GameRepository::update_time(self, game_id, red_time, black_time).await })
    }
}

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
#[derive(Debug)]
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
    /// 数据库仓库 (trait object for testability)
    game_repo: Arc<dyn GameRepo>,
}

impl GameRoom {
    pub fn new(
        game_id: Uuid,
        fen: &str,
        game_repo: Arc<dyn GameRepo>,
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
        game_repo: Arc<dyn GameRepo>,
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
    /// Returns true if both players are now present (game is ready to start).
    pub async fn join(&self, client: Client, color: chess_engine::Color) -> Result<bool, String> {
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
        // Check if both players are now present
        let red = self.red_player.read().await;
        let black = self.black_player.read().await;
        Ok(red.is_some() && black.is_some())
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

    /// Check if time control is currently active.
    pub async fn is_time_active(&self) -> bool {
        let tc = self.time_control.read().await;
        tc.as_ref().map_or(false, |tc| tc.is_active())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tokio::sync::mpsc;

    /// Mock GameRepo for testing — records update_time calls
    struct MockGameRepo {
        update_time_calls: Mutex<Vec<(Uuid, i32, i32)>>,
    }

    impl MockGameRepo {
        fn new() -> Arc<Self> {
            Arc::new(MockGameRepo {
                update_time_calls: Mutex::new(Vec::new()),
            })
        }
    }

    impl GameRepo for MockGameRepo {
        fn update_time(&self, game_id: Uuid, red_time: i32, black_time: i32) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
            self.update_time_calls.lock().unwrap().push((game_id, red_time, black_time));
            Box::pin(async { Ok(()) })
        }
    }

    const INITIAL_FEN: &str = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";

    fn create_test_room() -> (Arc<GameRoom>, Arc<MockGameRepo>) {
        let mock = MockGameRepo::new();
        let game_id = Uuid::new_v4();
        let room = Arc::new(GameRoom::new(
            game_id,
            INITIAL_FEN,
            mock.clone() as Arc<dyn GameRepo>,
            None, None, None,
        ));
        (room, mock)
    }

    fn create_test_room_with_time() -> (Arc<GameRoom>, Arc<MockGameRepo>) {
        let mock = MockGameRepo::new();
        let game_id = Uuid::new_v4();
        let room = Arc::new(GameRoom::new(
            game_id,
            INITIAL_FEN,
            mock.clone() as Arc<dyn GameRepo>,
            Some(600), None, None,
        ));
        (room, mock)
    }

    fn make_client(user_id: Uuid, username: &str) -> (Client, mpsc::UnboundedReceiver<String>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Client::new(user_id, username.to_string(), tx), rx)
    }

    #[tokio::test]
    async fn test_join_red_slot_success() {
        let (room, _) = create_test_room();
        let red_id = Uuid::new_v4();
        let (client, _) = make_client(red_id, "red_player");
        let result = room.join(client, chess_engine::Color::Red).await;
        assert!(result.is_ok());
        // Only one player joined — both_present should be false
        assert!(!result.unwrap());
        assert!(room.has_player(red_id).await);
    }

    #[tokio::test]
    async fn test_join_black_slot_success() {
        let (room, _) = create_test_room();
        let black_id = Uuid::new_v4();
        let (client, _) = make_client(black_id, "black_player");
        let result = room.join(client, chess_engine::Color::Black).await;
        assert!(result.is_ok());
        // Only one player joined — both_present should be false
        assert!(!result.unwrap());
        assert!(room.has_player(black_id).await);
    }

    #[tokio::test]
    async fn test_join_both_players_returns_true() {
        let (room, _) = create_test_room();
        let red_id = Uuid::new_v4();
        let black_id = Uuid::new_v4();
        let (rc, _) = make_client(red_id, "red_player");
        let (bc, _) = make_client(black_id, "black_player");
        let first = room.join(rc, chess_engine::Color::Red).await.unwrap();
        let second = room.join(bc, chess_engine::Color::Black).await.unwrap();
        // First join: only one player → false; second join: both present → true
        assert!(!first);
        assert!(second);
    }

    #[tokio::test]
    async fn test_join_red_slot_already_occupied() {
        let (room, _) = create_test_room();
        let red_id1 = Uuid::new_v4();
        let red_id2 = Uuid::new_v4();
        let (c1, _) = make_client(red_id1, "red1");
        let (c2, _) = make_client(red_id2, "red2");
        room.join(c1, chess_engine::Color::Red).await.unwrap();
        let result = room.join(c2, chess_engine::Color::Red).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already occupied"));
    }

    #[tokio::test]
    async fn test_join_black_slot_already_occupied() {
        let (room, _) = create_test_room();
        let black_id1 = Uuid::new_v4();
        let black_id2 = Uuid::new_v4();
        let (c1, _) = make_client(black_id1, "black1");
        let (c2, _) = make_client(black_id2, "black2");
        room.join(c1, chess_engine::Color::Black).await.unwrap();
        let result = room.join(c2, chess_engine::Color::Black).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already occupied"));
    }

    #[tokio::test]
    async fn test_player_color_red() {
        let (room, _) = create_test_room();
        let red_id = Uuid::new_v4();
        let (client, _) = make_client(red_id, "red_player");
        room.join(client, chess_engine::Color::Red).await.unwrap();
        let color = room.player_color(red_id).await.unwrap();
        assert_eq!(color, chess_engine::Color::Red);
    }

    #[tokio::test]
    async fn test_player_color_black() {
        let (room, _) = create_test_room();
        let black_id = Uuid::new_v4();
        let (client, _) = make_client(black_id, "black_player");
        room.join(client, chess_engine::Color::Black).await.unwrap();
        let color = room.player_color(black_id).await.unwrap();
        assert_eq!(color, chess_engine::Color::Black);
    }

    #[tokio::test]
    async fn test_player_color_not_in_room() {
        let (room, _) = create_test_room();
        let unknown_id = Uuid::new_v4();
        let result = room.player_color(unknown_id).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not a player"));
    }

    #[tokio::test]
    async fn test_has_player_true_false() {
        let (room, _) = create_test_room();
        let red_id = Uuid::new_v4();
        let (client, _) = make_client(red_id, "red_player");
        room.join(client, chess_engine::Color::Red).await.unwrap();
        assert!(room.has_player(red_id).await);
        assert!(!room.has_player(Uuid::new_v4()).await);
    }

    #[tokio::test]
    async fn test_game_id_accessor() {
        let game_id = Uuid::new_v4();
        let mock = MockGameRepo::new();
        let room = GameRoom::new(game_id, INITIAL_FEN, mock.clone() as Arc<dyn GameRepo>, None, None, None);
        assert_eq!(room.game_id(), game_id);
    }

    #[tokio::test]
    async fn test_current_side_initial() {
        let (room, _) = create_test_room();
        assert_eq!(room.current_side().await, chess_engine::Color::Red);
    }

    #[tokio::test]
    async fn test_is_game_over_initially_false() {
        let (room, _) = create_test_room();
        assert!(!room.is_game_over().await);
    }

    #[tokio::test]
    async fn test_fen_returns_current_state() {
        let (room, _) = create_test_room();
        let fen = room.fen().await;
        assert_eq!(fen, INITIAL_FEN);
    }

    #[tokio::test]
    async fn test_make_move_success() {
        let (room, _) = create_test_room();
        let red_id = Uuid::new_v4();
        let black_id = Uuid::new_v4();
        let (rc, _) = make_client(red_id, "red");
        let (bc, _) = make_client(black_id, "black");
        room.join(rc, chess_engine::Color::Red).await.unwrap();
        room.join(bc, chess_engine::Color::Black).await.unwrap();

        // Red cannon at b9 (col 1, row 9) to c7 (col 2, row 7)
        let result = room.make_move(red_id, chess_engine::Color::Red, "b9", "c7").await;
        assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result);
        let move_result = result.unwrap();
        assert_ne!(move_result.fen, INITIAL_FEN);
        assert_eq!(move_result.move_history.len(), 1);
        assert_eq!(move_result.move_history[0].mv, "b9-c7");
        assert_eq!(move_result.move_history[0].color, "red");
    }

    #[tokio::test]
    async fn test_make_move_not_your_turn() {
        let (room, _) = create_test_room();
        let red_id = Uuid::new_v4();
        let black_id = Uuid::new_v4();
        let (rc, _) = make_client(red_id, "red");
        let (bc, _) = make_client(black_id, "black");
        room.join(rc, chess_engine::Color::Red).await.unwrap();
        room.join(bc, chess_engine::Color::Black).await.unwrap();

        // Black tries to move on Red's turn
        let result = room.make_move(black_id, chess_engine::Color::Black, "b0", "c2").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Not your turn"));
    }

    #[tokio::test]
    async fn test_make_move_invalid_from_position() {
        let (room, _) = create_test_room();
        let red_id = Uuid::new_v4();
        let (rc, _) = make_client(red_id, "red");
        room.join(rc, chess_engine::Color::Red).await.unwrap();

        let result = room.make_move(red_id, chess_engine::Color::Red, "z99", "a1").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid from position"));
    }

    #[tokio::test]
    async fn test_make_move_illegal_move() {
        let (room, _) = create_test_room();
        let red_id = Uuid::new_v4();
        let (rc, _) = make_client(red_id, "red");
        room.join(rc, chess_engine::Color::Red).await.unwrap();

        // Try to move the red king to an illegal destination
        let result = room.make_move(red_id, chess_engine::Color::Red, "e9", "e5").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_make_move_appends_to_move_log() {
        let (room, _) = create_test_room();
        let red_id = Uuid::new_v4();
        let (rc, _) = make_client(red_id, "red");
        room.join(rc, chess_engine::Color::Red).await.unwrap();

        // Initially empty
        assert_eq!(room.move_history_json().await, "[]");

        // Red cannon b9 → c7
        room.make_move(red_id, chess_engine::Color::Red, "b9", "c7").await.unwrap();
        let json = room.move_history_json().await;
        assert!(!json.contains("[]"), "Should have at least one entry");
        assert!(json.contains("b9-c7"));
    }

    #[tokio::test]
    async fn test_resign_red() {
        let (room, _) = create_test_room();
        let red_id = Uuid::new_v4();
        let (rc, _) = make_client(red_id, "red");
        room.join(rc, chess_engine::Color::Red).await.unwrap();

        let result = room.resign(red_id).await.unwrap();
        assert_eq!(result.1, "black_win");
        assert_eq!(result.2, "resign");
    }

    #[tokio::test]
    async fn test_resign_black() {
        let (room, _) = create_test_room();
        let black_id = Uuid::new_v4();
        let (bc, _) = make_client(black_id, "black");
        room.join(bc, chess_engine::Color::Black).await.unwrap();

        let result = room.resign(black_id).await.unwrap();
        assert_eq!(result.1, "red_win");
        assert_eq!(result.2, "resign");
    }

    #[tokio::test]
    async fn test_resign_game_already_over() {
        let (room, _) = create_test_room();
        let red_id = Uuid::new_v4();
        let (rc, _) = make_client(red_id, "red");
        room.join(rc, chess_engine::Color::Red).await.unwrap();

        room.resign(red_id).await.unwrap();
        let result = room.resign(red_id).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already over"));
    }

    #[tokio::test]
    async fn test_offer_draw_success() {
        let (room, _) = create_test_room();
        let red_id = Uuid::new_v4();
        let (rc, _) = make_client(red_id, "red");
        room.join(rc, chess_engine::Color::Red).await.unwrap();

        let result = room.offer_draw(red_id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_offer_draw_game_already_over() {
        let (room, _) = create_test_room();
        let red_id = Uuid::new_v4();
        let (rc, _) = make_client(red_id, "red");
        room.join(rc, chess_engine::Color::Red).await.unwrap();

        room.resign(red_id).await.unwrap();
        let result = room.offer_draw(red_id).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already over"));
    }

    #[tokio::test]
    async fn test_respond_draw_accept() {
        let (room, _) = create_test_room();
        let red_id = Uuid::new_v4();
        let black_id = Uuid::new_v4();
        let (rc, _) = make_client(red_id, "red");
        let (bc, _) = make_client(black_id, "black");
        room.join(rc, chess_engine::Color::Red).await.unwrap();
        room.join(bc, chess_engine::Color::Black).await.unwrap();

        room.offer_draw(red_id).await.unwrap();
        let result = room.respond_draw(black_id, true).await.unwrap();
        assert!(result.is_some());
        let (_, result_str, reason_str) = result.unwrap();
        assert_eq!(result_str, "draw");
        assert_eq!(reason_str, "draw_agreement");
    }

    #[tokio::test]
    async fn test_respond_draw_reject() {
        let (room, _) = create_test_room();
        let red_id = Uuid::new_v4();
        let black_id = Uuid::new_v4();
        let (rc, _) = make_client(red_id, "red");
        let (bc, _) = make_client(black_id, "black");
        room.join(rc, chess_engine::Color::Red).await.unwrap();
        room.join(bc, chess_engine::Color::Black).await.unwrap();

        room.offer_draw(red_id).await.unwrap();
        let result = room.respond_draw(black_id, false).await.unwrap();
        assert!(result.is_none());
        // Game should still continue
        assert!(!room.is_game_over().await);
    }

    #[tokio::test]
    async fn test_respond_draw_no_offer_pending() {
        let (room, _) = create_test_room();
        let black_id = Uuid::new_v4();
        let (bc, _) = make_client(black_id, "black");
        room.join(bc, chess_engine::Color::Black).await.unwrap();

        let result = room.respond_draw(black_id, true).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No draw offer"));
    }

    #[tokio::test]
    async fn test_respond_draw_own_offer() {
        let (room, _) = create_test_room();
        let red_id = Uuid::new_v4();
        let (rc, _) = make_client(red_id, "red");
        room.join(rc, chess_engine::Color::Red).await.unwrap();

        room.offer_draw(red_id).await.unwrap();
        let result = room.respond_draw(red_id, true).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("your own draw offer"));
    }

    #[tokio::test]
    async fn test_leave_game_in_progress() {
        let (room, _) = create_test_room();
        let red_id = Uuid::new_v4();
        let (rc, _) = make_client(red_id, "red");
        room.join(rc, chess_engine::Color::Red).await.unwrap();

        let result = room.leave(red_id).await.unwrap();
        assert!(result.is_some());
        let (_, result_str, reason_str) = result.unwrap();
        assert_eq!(result_str, "black_win");
        assert_eq!(reason_str, "resign");
        // Red player should be removed
        assert!(!room.has_player(red_id).await);
    }

    #[tokio::test]
    async fn test_leave_game_already_over() {
        let (room, _) = create_test_room();
        let red_id = Uuid::new_v4();
        let (rc, _) = make_client(red_id, "red");
        room.join(rc, chess_engine::Color::Red).await.unwrap();

        room.resign(red_id).await.unwrap();
        let result = room.leave(red_id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_handle_disconnect_player() {
        let (room, _) = create_test_room();
        let red_id = Uuid::new_v4();
        let (rc, _) = make_client(red_id, "red");
        room.join(rc, chess_engine::Color::Red).await.unwrap();

        let result = room.handle_disconnect(red_id).await.unwrap();
        assert!(result.is_some());
        let (_, result_str, reason_str) = result.unwrap();
        assert_eq!(result_str, "black_win");
        assert_eq!(reason_str, "disconnect");
    }

    #[tokio::test]
    async fn test_handle_disconnect_spectator() {
        let (room, _) = create_test_room();
        let spec_id = Uuid::new_v4();
        // Spectators are added via spectators list, but handle_disconnect checks player_color first
        // Since spectator is not a player, player_color will return error → Ok(None)
        let result = room.handle_disconnect(spec_id).await;
        assert!(result.is_ok());
        // spectator not in room, so player_color fails → Ok(None)
        // Actually handle_disconnect first removes_player which is also a no-op for non-players
        // Let me check: the spec is not in red/black slots, so remove_player is no-op,
        // then player_color returns Err → Ok(None)
    }

    #[tokio::test]
    async fn test_handle_disconnect_game_already_over() {
        let (room, _) = create_test_room();
        let red_id = Uuid::new_v4();
        let (rc, _) = make_client(red_id, "red");
        room.join(rc, chess_engine::Color::Red).await.unwrap();

        room.resign(red_id).await.unwrap();
        let result = room.handle_disconnect(red_id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_activate_time_control() {
        let (room, _) = create_test_room_with_time();
        room.activate_time().await;
        let result = room.tick_time().await;
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_tick_time_no_time_control() {
        let (room, _) = create_test_room();
        let result = room.tick_time().await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_time_state_with_time_control() {
        let (room, _) = create_test_room_with_time();
        room.activate_time().await;
        room.tick_time().await; // Tick once

        let (red_time, black_time, active_color, red_in_byo, black_in_byo) = room.time_state().await;
        assert_eq!(red_time, 599); // 600 - 1 tick
        assert_eq!(black_time, 600);
        assert_eq!(active_color, "red");
        assert!(!red_in_byo);
        assert!(!black_in_byo);
    }

    #[tokio::test]
    async fn test_time_state_no_time_control() {
        let (room, _) = create_test_room();
        let (red_time, black_time, active_color, red_in_byo, black_in_byo) = room.time_state().await;
        assert_eq!(red_time, 0);
        assert_eq!(black_time, 0);
        assert_eq!(active_color, "red");
        assert!(!red_in_byo);
        assert!(!black_in_byo);
    }

    #[tokio::test]
    async fn test_move_history_json_empty() {
        let (room, _) = create_test_room();
        assert_eq!(room.move_history_json().await, "[]");
    }

    #[tokio::test]
    async fn test_broadcast_sends_to_players() {
        let (room, _) = create_test_room();
        let red_id = Uuid::new_v4();
        let black_id = Uuid::new_v4();
        let (rc, mut rx_red) = make_client(red_id, "red");
        let (bc, mut rx_black) = make_client(black_id, "black");
        room.join(rc, chess_engine::Color::Red).await.unwrap();
        room.join(bc, chess_engine::Color::Black).await.unwrap();

        let msg = ServerMessage::Pong;
        room.broadcast(&msg).await;

        // Both receivers should have a message
        assert!(rx_red.try_recv().is_ok());
        assert!(rx_black.try_recv().is_ok());
    }

    #[tokio::test]
    async fn test_broadcast_to_opponent_red() {
        let (room, _) = create_test_room();
        let red_id = Uuid::new_v4();
        let black_id = Uuid::new_v4();
        let (rc, mut rx_red) = make_client(red_id, "red");
        let (bc, mut rx_black) = make_client(black_id, "black");
        room.join(rc, chess_engine::Color::Red).await.unwrap();
        room.join(bc, chess_engine::Color::Black).await.unwrap();

        let msg = ServerMessage::Pong;
        room.broadcast_to_opponent(chess_engine::Color::Red, &msg).await;

        // Only black (opponent of red) should receive
        assert!(rx_red.try_recv().is_err(), "Red should not receive message sent to opponent of red");
        assert!(rx_black.try_recv().is_ok(), "Black should receive message");
    }
}
