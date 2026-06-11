use crate::board::Board;
use crate::pieces::{Color, Move, Piece};
use crate::rules::{is_checkmate, is_stalemate};
use crate::rules::validator::MoveError;

/// 游戏结果
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GameResult {
    RedWin,
    BlackWin,
    Draw,
}

/// 游戏结束原因
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GameEndReason {
    Checkmate,          // 将杀
    Stalemate,          // 困毙 (中国象棋中困毙=输)
    Resign(Color),      // 认输
    DrawAgreement,      // 协议和棋
    Timeout(Color),     // 超时
}

/// 游戏状态
#[derive(Clone, Debug)]
pub struct GameState {
    board: Board,
    history: Vec<(Move, Option<Piece>)>,  // (走法, 被吃棋子)
    result: Option<(GameResult, GameEndReason)>,
}

impl GameState {
    /// 创建新的游戏状态 (初始局面)
    pub fn new() -> Self {
        Self {
            board: Board::initial(),
            history: Vec::new(),
            result: None,
        }
    }

    /// 从 FEN 字符串创建
    pub fn from_fen(fen: &str) -> Result<Self, String> {
        let board = Board::from_fen(fen)?;
        Ok(Self {
            board,
            history: Vec::new(),
            result: None,
        })
    }

    /// 获取棋盘引用
    pub fn board(&self) -> &Board {
        &self.board
    }

    /// 获取棋盘可变引用
    pub fn board_mut(&mut self) -> &mut Board {
        &mut self.board
    }

    /// 获取当前走子方
    pub fn side_to_move(&self) -> Color {
        self.board.side_to_move()
    }

    /// 获取走法历史
    pub fn history(&self) -> &[(Move, Option<Piece>)] {
        &self.history
    }

    /// 获取游戏结果
    pub fn result(&self) -> Option<&(GameResult, GameEndReason)> {
        self.result.as_ref()
    }

    /// 游戏是否已结束
    pub fn is_game_over(&self) -> bool {
        self.result.is_some()
    }

    /// 获取当前 FEN
    pub fn to_fen(&self) -> String {
        self.board.to_fen()
    }

    /// 执行走法
    pub fn make_move(&mut self, m: Move) -> Result<(), MoveError> {
        if self.is_game_over() {
            return Err(MoveError::IllegalMove);
        }

        let color = self.side_to_move();

        // 验证走法
        crate::rules::validator::validate_move(&self.board, m, color)?;

        // 执行走法
        let captured = self.board.make_move(m);

        // 记录历史
        self.history.push((m, captured));

        // 检查游戏是否结束
        self.check_game_end();

        Ok(())
    }

    /// 撤销走法
    pub fn undo_move(&mut self) -> Option<Move> {
        if let Some((m, captured)) = self.history.pop() {
            self.board.undo_move(m, captured);
            self.result = None; // 撤销后游戏恢复
            Some(m)
        } else {
            None
        }
    }

    /// 认输
    pub fn resign(&mut self, color: Color) {
        if self.is_game_over() {
            return;
        }
        let result = match color {
            Color::Red => GameResult::BlackWin,
            Color::Black => GameResult::RedWin,
        };
        self.result = Some((result, GameEndReason::Resign(color)));
    }

    /// 和棋
    pub fn draw(&mut self) {
        if self.is_game_over() {
            return;
        }
        self.result = Some((GameResult::Draw, GameEndReason::DrawAgreement));
    }

    /// 超时
    pub fn timeout(&mut self, color: Color) {
        if self.is_game_over() {
            return;
        }
        let result = match color {
            Color::Red => GameResult::BlackWin,
            Color::Black => GameResult::RedWin,
        };
        self.result = Some((result, GameEndReason::Timeout(color)));
    }

    /// 检查游戏是否结束
    fn check_game_end(&mut self) {
        let opponent = self.side_to_move(); // 走完后轮到对手

        if is_checkmate(&self.board, opponent) {
            let result = match opponent {
                Color::Red => GameResult::BlackWin,
                Color::Black => GameResult::RedWin,
            };
            self.result = Some((result, GameEndReason::Checkmate));
        } else if is_stalemate(&self.board, opponent) {
            // 中国象棋中困毙 = 输
            let result = match opponent {
                Color::Red => GameResult::BlackWin,
                Color::Black => GameResult::RedWin,
            };
            self.result = Some((result, GameEndReason::Stalemate));
        }
    }

    /// 生成中国象棋记谱法 (如 "炮二平五")
    /// 简化版实现
    pub fn generate_notation(&self, m: Move) -> String {
        let piece = match self.board.piece_at(m.from) {
            Some(p) => p,
            None => return m.to_uci(),
        };

        let name = piece.chinese_name();

        // 红方列号从右到左为一到九
        // 黑方列号从右到左为1到9
        let from_col_name = column_name(m.from.col, piece.color);
        let to_col_name = column_name(m.to.col, piece.color);

        let action = if m.from.row == m.to.row {
            "平"
        } else {
            let is_forward = match piece.color {
                Color::Red => m.to.row < m.from.row, // 红方向上为进
                Color::Black => m.to.row > m.from.row, // 黑方向下为进
            };
            if is_forward { "进" } else { "退" }
        };

        let target = if m.from.col == m.to.col {
            // 同列移动，目标用步数
            let steps = (m.to.row as i32 - m.from.row as i32).unsigned_abs();
            step_name(steps as u8, piece.color)
        } else {
            to_col_name.to_string()
        };

        format!("{}{}{}{}", name, from_col_name, action, target)
    }

    /// 生成合法走法
    pub fn generate_legal_moves(&self) -> Vec<Move> {
        if self.is_game_over() {
            return Vec::new();
        }
        self.board.generate_legal_moves(self.side_to_move())
    }
}

/// 列号名称
/// 红方: 从右到左为一到九 (col 8=一, col 0=九)
/// 黑方: 从右到左为1到9 (col 0=9, col 8=1)
fn column_name(col: u8, color: Color) -> &'static str {
    match color {
        Color::Red => {
            match 8 - col {
                0 => "九",
                1 => "八",
                2 => "七",
                3 => "六",
                4 => "五",
                5 => "四",
                6 => "三",
                7 => "二",
                8 => "一",
                _ => "?",
            }
        }
        Color::Black => {
            match col + 1 {
                1 => "1",
                2 => "2",
                3 => "3",
                4 => "4",
                5 => "5",
                6 => "6",
                7 => "7",
                8 => "8",
                9 => "9",
                _ => "?",
            }
        }
    }
}

/// 步数名称
fn step_name(steps: u8, color: Color) -> String {
    match color {
        Color::Red => {
            match steps {
                1 => "一".to_string(),
                2 => "二".to_string(),
                3 => "三".to_string(),
                4 => "四".to_string(),
                5 => "五".to_string(),
                6 => "六".to_string(),
                7 => "七".to_string(),
                8 => "八".to_string(),
                9 => "九".to_string(),
                _ => steps.to_string(),
            }
        }
        Color::Black => steps.to_string(),
    }
}

impl Default for GameState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Position;

    #[test]
    fn test_new_game() {
        let state = GameState::new();
        assert_eq!(state.side_to_move(), Color::Red);
        assert!(!state.is_game_over());
        assert!(state.history().is_empty());
    }

    #[test]
    fn test_make_move() {
        let mut state = GameState::new();
        let m = Move::new(Position::new(1, 7), Position::new(4, 7));
        assert!(state.make_move(m).is_ok());
        assert_eq!(state.side_to_move(), Color::Black);
        assert_eq!(state.history().len(), 1);
    }

    #[test]
    fn test_undo_move() {
        let mut state = GameState::new();
        let m = Move::new(Position::new(1, 7), Position::new(4, 7));
        state.make_move(m).unwrap();
        let undone = state.undo_move();
        assert_eq!(undone, Some(m));
        assert_eq!(state.side_to_move(), Color::Red);
        assert!(state.history().is_empty());
    }

    #[test]
    fn test_resign() {
        let mut state = GameState::new();
        state.resign(Color::Red);
        assert!(state.is_game_over());
        let (result, reason) = state.result().unwrap();
        assert_eq!(*result, GameResult::BlackWin);
        assert_eq!(*reason, GameEndReason::Resign(Color::Red));
    }

    #[test]
    fn test_draw() {
        let mut state = GameState::new();
        state.draw();
        assert!(state.is_game_over());
        let (result, reason) = state.result().unwrap();
        assert_eq!(*result, GameResult::Draw);
        assert_eq!(*reason, GameEndReason::DrawAgreement);
    }

    #[test]
    fn test_cannot_move_after_game_over() {
        let mut state = GameState::new();
        state.resign(Color::Red);
        let m = Move::new(Position::new(1, 7), Position::new(4, 7));
        assert!(state.make_move(m).is_err());
    }
}
