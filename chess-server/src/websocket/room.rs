use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::RwLock;
use uuid::Uuid;

use crate::db::models::UserInfo;
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
        user_id: Uuid,
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

        Ok(MoveResult {
            fen,
            is_check,
            is_game_over,
            result,
            end_reason,
        })
    }

    /// 认输
    pub async fn resign(&self, user_id: Uuid) -> Result<(), String> {
        let mut state = self.game_state.write().await;

        let color = {
            let red = self.red_player.read().await;
            let black = self.black_player.read().await;
            if red.as_ref().map(|c| c.user_id) == Some(user_id) {
                chess_engine::Color::Red
            } else if black.as_ref().map(|c| c.user_id) == Some(user_id) {
                chess_engine::Color::Black
            } else {
                return Err("You are not a player in this game".into());
            }
        };

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
}
