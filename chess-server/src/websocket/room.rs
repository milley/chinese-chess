use std::sync::Arc;
use std::time::Instant;

use tokio::sync::RwLock;
use uuid::Uuid;

use crate::db::repositories::game_repo::GameRepository;
use crate::websocket::client::Client;
use crate::websocket::message::ServerMessage;

/// 走棋结果
pub struct MoveResult {
    pub fen: String,
    pub is_check: bool,
    pub is_game_over: bool,
    pub result: Option<String>,
    pub end_reason: Option<String>,
    pub move_history_uci: Vec<String>,
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
    /// 当前走子方开始走棋时间
    move_start_time: Arc<RwLock<Instant>>,
    /// 和棋请求状态
    draw_offer: Arc<RwLock<Option<chess_engine::Color>>>,
    /// 数据库仓库
    game_repo: GameRepository,
}

impl GameRoom {
    pub fn new(game_id: Uuid, fen: &str, game_repo: GameRepository) -> Self {
        let game_state = chess_engine::GameState::from_fen(fen)
            .unwrap_or_else(|_| chess_engine::GameState::new());

        Self {
            game_state: Arc::new(RwLock::new(game_state)),
            game_id,
            red_player: Arc::new(RwLock::new(None)),
            black_player: Arc::new(RwLock::new(None)),
            spectators: Arc::new(RwLock::new(Vec::new())),
            move_start_time: Arc::new(RwLock::new(Instant::now())),
            draw_offer: Arc::new(RwLock::new(None)),
            game_repo,
        }
    }

    /// 玩家加入
    pub async fn join(&self, client: Client, color: chess_engine::Color) -> Result<(), String> {
        match color {
            chess_engine::Color::Red => {
                let mut player = self.red_player.write().await;
                *player = Some(client);
            }
            chess_engine::Color::Black => {
                let mut player = self.black_player.write().await;
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

        drop(state);

        // 更新走棋开始时间
        *self.move_start_time.write().await = Instant::now();

        // 广播走法
        let msg = ServerMessage::MoveMade {
            game_id: self.game_id.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            fen: fen.clone(),
            is_check,
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

        // Collect move history as UCI strings
        let move_history_uci = {
            let state = self.game_state.read().await;
            state.history().iter().map(|(m, _)| m.to_uci()).collect()
        };

        Ok(MoveResult {
            fen,
            is_check,
            is_game_over,
            result,
            end_reason,
            move_history_uci,
        })
    }

    /// 认输
    pub async fn resign(&self, user_id: Uuid) -> Result<(), String> {
        let mut state = self.game_state.write().await;

        let color = self.player_color(user_id).await?;

        state.resign(color);
        drop(state);

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

        Ok(())
    }

    /// 提议和棋
    pub async fn offer_draw(&self, user_id: Uuid) -> Result<(), String> {
        let color = self.player_color(user_id).await?;

        // 检查游戏是否已结束
        if self.game_state.read().await.is_game_over() {
            return Err("Game is already over".into());
        }

        // 设置和棋提议
        *self.draw_offer.write().await = Some(color);

        // 通知对方有和棋提议
        let msg = ServerMessage::DrawOffered {
            game_id: self.game_id.to_string(),
        };
        self.broadcast_to_opponent(color, &msg).await;

        Ok(())
    }

    /// 响应和棋提议
    pub async fn respond_draw(&self, user_id: Uuid, accept: bool) -> Result<(), String> {
        let color = self.player_color(user_id).await?;

        // 检查是否有待处理的和棋提议
        let offer = *self.draw_offer.read().await;
        if offer.is_none() {
            return Err("No draw offer to respond to".into());
        }
        if offer == Some(color) {
            return Err("You cannot respond to your own draw offer".into());
        }

        if accept {
            // 清除和棋提议
            *self.draw_offer.write().await = None;

            // 执行和棋
            self.game_state.write().await.draw();

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
        } else {
            // 拒绝和棋，清除提议
            *self.draw_offer.write().await = None;

            let msg = ServerMessage::DrawResponse {
                game_id: self.game_id.to_string(),
                accepted: false,
            };
            self.broadcast(&msg).await;
        }

        Ok(())
    }

    /// 玩家离开对局 (对局中离开判负)
    pub async fn leave(&self, user_id: Uuid) -> Result<(), String> {
        let state = self.game_state.read().await;
        if state.is_game_over() {
            // 对局已结束，只需移除玩家
            self.remove_player(user_id).await;
            return Ok(());
        }
        drop(state);

        // 对局中离开 = 认输
        let color = self.player_color(user_id).await?;
        self.remove_player(user_id).await;
        self.game_state.write().await.resign(color);

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

        Ok(())
    }

    /// 客户端断连处理
    pub async fn handle_disconnect(&self, user_id: Uuid) {
        self.remove_player(user_id).await;

        let msg = ServerMessage::OpponentDisconnected {
            game_id: self.game_id.to_string(),
        };
        self.broadcast(&msg).await;
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

    /// 从房间移除玩家
    async fn remove_player(&self, user_id: Uuid) {
        {
            let red = self.red_player.read().await;
            if red.as_ref().map(|c| c.user_id) == Some(user_id) {
                drop(red);
                *self.red_player.write().await = None;
                return;
            }
        }
        {
            let black = self.black_player.read().await;
            if black.as_ref().map(|c| c.user_id) == Some(user_id) {
                drop(black);
                *self.black_player.write().await = None;
            }
        }
    }

    /// 发送消息给对方玩家
    async fn broadcast_to_opponent(&self, color: chess_engine::Color, message: &ServerMessage) {
        let json = serde_json::to_string(message).unwrap_or_default();
        let player = match color {
            chess_engine::Color::Red => self.black_player.read().await,
            chess_engine::Color::Black => self.red_player.read().await,
        };
        if let Some(client) = player.as_ref() {
            client.send(&json);
        }
    }

    /// 广播消息到房间内所有客户端
    pub async fn broadcast(&self, message: &ServerMessage) {
        let json = serde_json::to_string(message).unwrap_or_default();

        let red = self.red_player.read().await;
        if let Some(client) = red.as_ref() {
            client.send(&json);
        }
        drop(red);

        let black = self.black_player.read().await;
        if let Some(client) = black.as_ref() {
            client.send(&json);
        }
        drop(black);

        let spectators = self.spectators.read().await;
        for spectator in spectators.iter() {
            spectator.send(&json);
        }
    }

    /// 获取当前 FEN
    pub async fn fen(&self) -> String {
        self.game_state.read().await.to_fen()
    }

    /// 获取对局 ID
    pub fn game_id(&self) -> Uuid {
        self.game_id
    }

    /// 获取走子开始时间的副本
    pub async fn move_start_time(&self) -> Instant {
        *self.move_start_time.read().await
    }

    /// 获取当前走子方
    pub async fn current_side(&self) -> chess_engine::Color {
        self.game_state.read().await.side_to_move()
    }

    /// 超时判负
    pub async fn timeout(&self, color: chess_engine::Color) {
        self.game_state.write().await.timeout(color);
    }
}
