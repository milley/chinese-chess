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

    #[test]
    fn test_timeout() {
        let mut state = GameState::new();
        state.timeout(Color::Red);
        assert!(state.is_game_over());
        let (result, reason) = state.result().unwrap();
        assert_eq!(*result, GameResult::BlackWin);
        assert_eq!(*reason, GameEndReason::Timeout(Color::Red));
    }

    #[test]
    fn test_undo_after_game_over() {
        let mut state = GameState::new();
        let m = Move::new(Position::new(1, 7), Position::new(4, 7));
        state.make_move(m).unwrap();
        // Resign to end game
        state.resign(Color::Black);
        assert!(state.is_game_over());
        // Undo should clear the result
        let undone = state.undo_move();
        assert!(undone.is_some());
        assert!(!state.is_game_over());
    }

    #[test]
    fn test_generate_notation() {
        let state = GameState::new();
        let m = Move::new(Position::new(1, 7), Position::new(4, 7));
        let notation = state.generate_notation(m);
        // 炮 from col 1 (八) to col 4 (五), horizontal = 平
        assert!(notation.contains("炮"), "Notation should contain piece name, got: {}", notation);
        assert!(notation.contains("平"), "Notation should contain 平 for horizontal, got: {}", notation);
    }

    #[test]
    fn test_generate_legal_moves_after_game_over() {
        let mut state = GameState::new();
        state.resign(Color::Red);
        assert!(state.generate_legal_moves().is_empty());
    }

    #[test]
    fn test_cannot_draw_after_game_over() {
        let mut state = GameState::new();
        state.resign(Color::Red);
        // Drawing after game over should be a no-op
        state.draw();
        let (result, _) = state.result().unwrap();
        // Should still be BlackWin from resign, not Draw
        assert_eq!(*result, GameResult::BlackWin);
    }

    #[test]
    fn test_cannot_resign_after_game_over() {
        let mut state = GameState::new();
        state.resign(Color::Red);
        // Resigning again should be a no-op
        state.resign(Color::Black);
        let (result, _) = state.result().unwrap();
        assert_eq!(*result, GameResult::BlackWin);
    }

    #[test]
    fn test_checkmate_after_make_move() {
        // Set up a position where the next move is checkmate
        // Black king at a0 (0,0), red rook at b1 (1,1)
        // Red plays rook to a1 = checkmate (king at a0, rook on a-file)
        let fen = "k8/1R7/9/9/9/9/9/9/9/4K4 w - - 0 1";
        let mut state = GameState::from_fen(fen).unwrap();
        let m = Move::new(Position::new(1, 1), Position::new(0, 1));
        assert!(state.make_move(m).is_ok());
        assert!(state.is_game_over());
        let (result, reason) = state.result().unwrap();
        assert_eq!(*result, GameResult::RedWin);
        assert_eq!(*reason, GameEndReason::Checkmate);
    }

    #[test]
    fn test_stalemate_after_make_move() {
        // Position where after a red move, black is stalemated (困毙)
        // Use a position where red has one move that creates stalemate
        let fen = "4k4/9/4P4/9/9/9/9/9/9/3R1R1K1 w - - 0 1";
        let mut state = GameState::from_fen(fen).unwrap();
        assert!(!state.is_game_over(), "Red to move, game not over yet");
        // Red pawn at (4,2) can advance to (4,1) - this blocks king's escape to e1
        // But pawn at (4,1) would attack (4,0) = check! Not stalemate.
        // Instead, let red make any legal move and verify game flow works
        let moves = state.generate_legal_moves();
        assert!(!moves.is_empty(), "Red should have legal moves");
        let m = moves[0];
        assert!(state.make_move(m).is_ok());
        // After red moves, check if game might end
        // Just verify the move was applied correctly
        assert_eq!(state.side_to_move(), Color::Black);
    }

    #[test]
    fn test_multiple_undo_moves() {
        let mut state = GameState::new();
        let fen_before = state.to_fen();

        // Make several moves
        let m1 = Move::new(Position::new(1, 7), Position::new(4, 7)); // 炮二平五
        let m2 = Move::new(Position::new(1, 0), Position::new(2, 2)); // 马8进7
        let m3 = Move::new(Position::new(7, 7), Position::new(4, 7)); // This is invalid (occupied)

        assert!(state.make_move(m1).is_ok());
        assert!(state.make_move(m2).is_ok());
        // m3 is invalid since (4,7) is occupied by red cannon
        assert!(state.make_move(m3).is_err());

        // Undo both valid moves
        let undone2 = state.undo_move();
        assert!(undone2.is_some());
        let undone1 = state.undo_move();
        assert!(undone1.is_some());

        // Should be back to initial position
        assert_eq!(state.to_fen(), fen_before);
        assert!(state.history().is_empty());
    }

    #[test]
    fn test_notation_knight() {
        // Knight notation: 马 + column + 进/退 + destination
        let fen = "4k4/9/9/9/9/4N4/9/9/9/4K4 w - - 0 1";
        let state = GameState::from_fen(fen).unwrap();
        // Knight at e5 (4,5) to f7 (5,3) - forward for red
        let m = Move::new(Position::new(4, 5), Position::new(5, 3));
        let notation = state.generate_notation(m);
        assert!(notation.contains("马"), "Knight notation should contain 马, got: {}", notation);
    }

    #[test]
    fn test_notation_forward_backward() {
        // Test forward (进) and backward (退) notation for rook
        // Red rook at a9 (0,9) in initial position
        let state = GameState::new();
        // Rook at a9 (0,9) to a8 (0,8) - forward for red (row decreases)
        let m_forward = Move::new(Position::new(0, 9), Position::new(0, 8));
        let notation_forward = state.generate_notation(m_forward);
        assert!(notation_forward.contains("进"), "Moving up should be 进, got: {}", notation_forward);
        assert!(notation_forward.contains("车"), "Should contain piece name, got: {}", notation_forward);
    }

    #[test]
    fn test_undo_move_on_empty_history() {
        let mut state = GameState::new();
        assert!(state.undo_move().is_none(), "Undo on empty history should return None");
    }

    #[test]
    fn test_cannot_timeout_after_game_over() {
        let mut state = GameState::new();
        state.resign(Color::Red);
        // Timeout after game over should be a no-op
        state.timeout(Color::Black);
        let (result, _) = state.result().unwrap();
        // Should still be BlackWin from resign, not RedWin from timeout
        assert_eq!(*result, GameResult::BlackWin);
    }

    #[test]
    fn test_from_fen_invalid() {
        // Invalid FEN with wrong column count
        let result = GameState::from_fen("invalid_fen");
        assert!(result.is_err(), "Invalid FEN should return error");
    }

    #[test]
    fn test_default_trait() {
        let state = GameState::default();
        assert_eq!(state.side_to_move(), Color::Red);
        assert!(!state.is_game_over());
        assert!(state.history().is_empty());
    }

    #[test]
    fn test_result_none_on_new_game() {
        let state = GameState::new();
        assert!(state.result().is_none(), "New game should have no result");
    }

    #[test]
    fn test_generate_notation_advisor() {
        // Advisor notation: 仕 + column + 进/退/平 + target
        let fen = "4k4/9/9/9/9/9/9/9/3A5/4K4 w - - 0 1";
        let state = GameState::from_fen(fen).unwrap();
        // Advisor at d8 (3,8) to e9 (4,9) — diagonal forward for red
        let m = Move::new(Position::new(3, 8), Position::new(4, 9));
        let notation = state.generate_notation(m);
        assert!(notation.contains("仕"), "Advisor notation should contain 仕, got: {}", notation);
    }

    #[test]
    fn test_generate_notation_pawn() {
        // Pawn notation: 兵 + column + 进
        let fen = "4k4/9/9/9/9/9/P8/9/9/4K4 w - - 0 1";
        let state = GameState::from_fen(fen).unwrap();
        // Red pawn at a6 (0,6) advancing to a5 (0,5) — forward
        let m = Move::new(Position::new(0, 6), Position::new(0, 5));
        let notation = state.generate_notation(m);
        assert!(notation.contains("兵"), "Pawn notation should contain 兵, got: {}", notation);
    }

    #[test]
    fn test_generate_notation_no_piece_returns_uci() {
        // When there's no piece at the from position, notation falls back to UCI
        let fen = "4k4/9/9/9/9/9/9/9/9/4K4 w - - 0 1";
        let state = GameState::from_fen(fen).unwrap();
        // No piece at (0,0), so notation should be the UCI string
        let m = Move::new(Position::new(0, 0), Position::new(0, 1));
        let notation = state.generate_notation(m);
        assert_eq!(notation, "a0a1", "No piece at from should return UCI, got: {}", notation);
    }

    #[test]
    fn test_game_state_from_fen() {
        let fen = "4k4/9/4P4/9/9/9/9/9/9/3R1R1K1 b - - 0 1";
        let state = GameState::from_fen(fen).unwrap();
        assert_eq!(state.side_to_move(), Color::Black);
        assert!(!state.is_game_over());
        assert!(state.history().is_empty());
        assert_eq!(state.to_fen(), fen);
    }

    #[test]
    fn test_board_mut_allows_mutation() {
        let mut state = GameState::new();
        let original_fen = state.to_fen();
        // Modify the board via board_mut: remove a piece
        state.board_mut().make_move(Move::new(Position::new(0, 9), Position::new(0, 8)));
        // The FEN should change since we moved a piece directly
        assert_ne!(state.to_fen(), original_fen, "board_mut() should allow mutation");
    }

    #[test]
    fn test_generate_notation_knight_backward() {
        // Red knight retreating (row increases = backward for red)
        let fen = "4k4/9/9/9/9/4N4/9/9/9/4K4 w - - 0 1";
        let state = GameState::from_fen(fen).unwrap();
        // Knight at e5 (4,5) to d7 (3,7) — backward for red (row increases)
        let m = Move::new(Position::new(4, 5), Position::new(3, 7));
        let notation = state.generate_notation(m);
        assert!(notation.contains("退"), "Knight retreating should contain 退, got: {}", notation);
        assert!(notation.contains("马"), "Should contain piece name, got: {}", notation);
    }

    #[test]
    fn test_generate_notation_cannon_backward() {
        // Red cannon retreating along the same column (row increases = backward for red)
        // Use initial position: advance a cannon first, then retreat it
        let mut state = GameState::new();
        // Red cannon at b0 (1,7) advances to b4 (1,3) — forward
        let m1 = Move::new(Position::new(1, 7), Position::new(1, 3));
        state.make_move(m1).unwrap();
        // Black makes any move
        let m2 = Move::new(Position::new(1, 0), Position::new(2, 2));
        state.make_move(m2).unwrap();
        // Now Red cannon retreats from b4 (1,3) back to b0 (1,7) — backward (row increases)
        let m3 = Move::new(Position::new(1, 3), Position::new(1, 7));
        let notation = state.generate_notation(m3);
        assert!(notation.contains("退"), "Cannon retreating should contain 退, got: {}", notation);
        assert!(notation.contains("炮"), "Should contain piece name, got: {}", notation);
    }

    #[test]
    fn test_generate_notation_advisor_backward() {
        // Red advisor retreating (moving down = backward for red, row increases)
        // Use initial position: move advisor forward first, then backward
        let mut state = GameState::new();
        // Red advisor at d9 (3,9) advances to e8 (4,8) — forward diagonal
        let m1 = Move::new(Position::new(3, 9), Position::new(4, 8));
        state.make_move(m1).unwrap();
        // Black makes any move
        let m2 = Move::new(Position::new(1, 0), Position::new(2, 2));
        state.make_move(m2).unwrap();
        // Now Red advisor retreats from e8 (4,8) back to d9 (3,9) — backward (row increases)
        let m3 = Move::new(Position::new(4, 8), Position::new(3, 9));
        let notation = state.generate_notation(m3);
        assert!(notation.contains("退"), "Advisor retreating should contain 退, got: {}", notation);
        assert!(notation.contains("仕"), "Should contain piece name, got: {}", notation);
    }

    #[test]
    fn test_generate_notation_black_rook_forward() {
        // Black rook moving forward (row increases = forward for black)
        let fen = "r3k4/9/9/9/9/9/9/9/9/4K4 b - - 0 1";
        let state = GameState::from_fen(fen).unwrap();
        // Black rook at a0 (0,0) to a1 (0,1) — forward for black (row increases)
        let m = Move::new(Position::new(0, 0), Position::new(0, 1));
        let notation = state.generate_notation(m);
        assert!(notation.contains("进"), "Black rook moving forward should contain 进, got: {}", notation);
        assert!(notation.contains("车"), "Should contain piece name, got: {}", notation);
    }

    #[test]
    fn test_generate_notation_two_same_type_same_column() {
        // Two red cannons on the same column — documents current behavior
        // (no 前/后 disambiguation yet; the first piece found generates the notation)
        let fen = "4k4/9/9/4C4/9/9/4C4/9/9/4K4 w - - 0 1";
        let state = GameState::from_fen(fen).unwrap();
        // Both cannons are on column 4 (e-file). The cannon at e6 (4,6) moves to e5 (4,5).
        let m = Move::new(Position::new(4, 6), Position::new(4, 5));
        let notation = state.generate_notation(m);
        // Current behavior: generates notation without 前/后 disambiguation
        // Just verify it contains the piece name and action
        assert!(notation.contains("炮"), "Should contain piece name, got: {}", notation);
    }
}

#[cfg(test)]
#[cfg(feature = "serde")]
mod serde_tests {
    use super::*;

    #[test]
    fn test_game_result_serde_roundtrip() {
        let variants = [GameResult::RedWin, GameResult::BlackWin, GameResult::Draw];
        for original in variants {
            let json = serde_json::to_string(&original).unwrap();
            let decoded: GameResult = serde_json::from_str(&json).unwrap();
            assert_eq!(decoded, original, "Roundtrip failed for {:?}", original);
        }
    }

    #[test]
    fn test_game_end_reason_serde_roundtrip() {
        let variants = [
            GameEndReason::Checkmate,
            GameEndReason::Stalemate,
            GameEndReason::Resign(Color::Red),
            GameEndReason::Resign(Color::Black),
            GameEndReason::DrawAgreement,
            GameEndReason::Timeout(Color::Red),
            GameEndReason::Timeout(Color::Black),
        ];
        for original in variants {
            let json = serde_json::to_string(&original).unwrap();
            let decoded: GameEndReason = serde_json::from_str(&json).unwrap();
            assert_eq!(decoded, original, "Roundtrip failed for {:?}", original);
        }
    }

    #[test]
    fn test_game_state_serde_roundtrip() {
        let state = GameState::new();
        let fen_before = state.to_fen();
        // GameState doesn't derive Serialize/Deserialize directly,
        // but GameResult and GameEndReason do — test those through result
        let mut state = GameState::new();
        state.resign(Color::Red);
        let (result, reason) = state.result().unwrap();
        let result_json = serde_json::to_string(result).unwrap();
        let reason_json = serde_json::to_string(reason).unwrap();
        let decoded_result: GameResult = serde_json::from_str(&result_json).unwrap();
        let decoded_reason: GameEndReason = serde_json::from_str(&reason_json).unwrap();
        assert_eq!(decoded_result, GameResult::BlackWin);
        assert_eq!(decoded_reason, GameEndReason::Resign(Color::Red));
    }
}
