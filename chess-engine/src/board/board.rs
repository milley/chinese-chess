use std::collections::HashMap;

use crate::pieces::{Color, Piece, PieceType, Move};
use super::Position;

/// 中国象棋初始局面 FEN
const INITIAL_FEN: &str = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";

/// 棋盘表示
#[derive(Clone, Debug)]
pub struct Board {
    /// 棋子位置映射 (Position -> Piece)
    pieces: HashMap<Position, Piece>,
    /// 当前走子方
    side_to_move: Color,
    /// 红方物质分 (增量维护)
    red_material_score: i32,
    /// 黑方物质分 (增量维护)
    black_material_score: i32,
}

impl Board {
    /// 创建空棋盘
    pub fn new() -> Self {
        Self {
            pieces: HashMap::new(),
            side_to_move: Color::Red,
            red_material_score: 0,
            black_material_score: 0,
        }
    }

    /// 创建初始局面
    pub fn initial() -> Self {
        Self::from_fen(INITIAL_FEN).expect("Initial FEN should be valid")
    }

    /// 从 FEN 字符串解析棋盘
    pub fn from_fen(fen: &str) -> Result<Self, String> {
        let mut board = Self::new();
        let parts: Vec<&str> = fen.split_whitespace().collect();
        if parts.is_empty() {
            return Err("Empty FEN string".to_string());
        }

        let rows: Vec<&str> = parts[0].split('/').collect();
        if rows.len() != 10 {
            return Err(format!("FEN must have 10 rows, got {}", rows.len()));
        }

        for (row_idx, &row_str) in rows.iter().enumerate() {
            let row = row_idx as u8;
            let mut col: u8 = 0;
            for ch in row_str.chars() {
                if ch.is_ascii_digit() {
                    // 数字表示连续空格
                    col += ch.to_digit(10).unwrap() as u8;
                } else if let Some(piece) = Piece::from_fen_char(ch) {
                    if col > 8 {
                        return Err(format!("Column out of bounds at row {}: col {}", row, col));
                    }
                    let pos = Position::new(col, row);
                    board.add_piece(pos, piece);
                    col += 1;
                } else {
                    return Err(format!("Invalid FEN character: '{}'", ch));
                }
            }
            if col != 9 {
                return Err(format!("Row {} has {} columns, expected 9", row, col));
            }
        }

        // 解析走子方
        if parts.len() > 1 {
            match parts[1] {
                "w" => board.side_to_move = Color::Red,
                "b" => board.side_to_move = Color::Black,
                _ => return Err(format!("Invalid side to move: '{}'", parts[1])),
            }
        }

        Ok(board)
    }

    /// 导出为 FEN 字符串
    pub fn to_fen(&self) -> String {
        let mut fen = String::new();

        for row in 0..10u8 {
            let mut empty_count = 0;
            for col in 0..9u8 {
                let pos = Position::new(col, row);
                if let Some(piece) = self.pieces.get(&pos) {
                    if empty_count > 0 {
                        fen.push_str(&empty_count.to_string());
                        empty_count = 0;
                    }
                    fen.push(piece.to_fen_char());
                } else {
                    empty_count += 1;
                }
            }
            if empty_count > 0 {
                fen.push_str(&empty_count.to_string());
            }
            if row < 9 {
                fen.push('/');
            }
        }

        // 走子方
        fen.push(' ');
        match self.side_to_move {
            Color::Red => fen.push('w'),
            Color::Black => fen.push('b'),
        }

        // 补全 FEN 的其余部分 (简化版，不追踪半步和全步)
        fen.push_str(" - - 0 1");

        fen
    }

    /// 添加棋子 (内部方法，用于 FEN 解析)
    fn add_piece(&mut self, pos: Position, piece: Piece) {
        let value = piece.base_value();
        match piece.color {
            Color::Red => self.red_material_score += value,
            Color::Black => self.black_material_score += value,
        }
        self.pieces.insert(pos, piece);
    }

    /// 获取某位置棋子
    pub fn piece_at(&self, pos: Position) -> Option<Piece> {
        self.pieces.get(&pos).copied()
    }

    /// 获取所有棋子及其位置
    pub fn all_pieces(&self) -> impl Iterator<Item = (Position, Piece)> + '_ {
        self.pieces.iter().map(|(&pos, &piece)| (pos, piece))
    }

    /// 获取某颜色的所有棋子
    pub fn pieces_of_color(&self, color: Color) -> impl Iterator<Item = (Position, Piece)> + '_ {
        self.pieces.iter().filter_map(move |(&pos, &piece)| {
            if piece.color == color {
                Some((pos, piece))
            } else {
                None
            }
        })
    }

    /// 获取当前走子方
    pub fn side_to_move(&self) -> Color {
        self.side_to_move
    }

    /// 设置走子方
    pub fn set_side_to_move(&mut self, color: Color) {
        self.side_to_move = color;
    }

    /// 获取红方物质分
    pub fn red_material_score(&self) -> i32 {
        self.red_material_score
    }

    /// 获取黑方物质分
    pub fn black_material_score(&self) -> i32 {
        self.black_material_score
    }

    /// 执行走法，返回被吃棋子
    pub fn make_move(&mut self, m: Move) -> Option<Piece> {
        let piece = self.pieces.remove(&m.from).expect("Piece must exist at from position");
        let captured = self.pieces.insert(m.to, piece);

        // 更新物质分
        if let Some(cap) = captured {
            match cap.color {
                Color::Red => self.red_material_score -= cap.base_value(),
                Color::Black => self.black_material_score -= cap.base_value(),
            }
        }

        // 切换走子方
        self.side_to_move = self.side_to_move.opposite();

        captured
    }

    /// 撤销走法
    pub fn undo_move(&mut self, m: Move, captured: Option<Piece>) {
        // 恢复走子方
        self.side_to_move = self.side_to_move.opposite();

        let piece = self.pieces.remove(&m.to).expect("Piece must exist at to position");
        self.pieces.insert(m.from, piece);

        // 恢复被吃棋子
        if let Some(cap) = captured {
            match cap.color {
                Color::Red => self.red_material_score += cap.base_value(),
                Color::Black => self.black_material_score += cap.base_value(),
            }
            self.pieces.insert(m.to, cap);
        }
    }

    /// 坐标是否在棋盘内
    pub fn is_in_bounds(pos: Position) -> bool {
        pos.is_valid()
    }

    /// 棋子数量
    pub fn piece_count(&self) -> usize {
        self.pieces.len()
    }

    /// 找到某方将/帅的位置
    pub fn find_king(&self, color: Color) -> Option<Position> {
        for (&pos, &piece) in &self.pieces {
            if piece.color == color && piece.piece_type == PieceType::King {
                return Some(pos);
            }
        }
        None
    }
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_position() {
        let board = Board::initial();
        assert_eq!(board.piece_count(), 32);
        assert_eq!(board.side_to_move(), Color::Red);
    }

    #[test]
    fn test_initial_fen_roundtrip() {
        let board = Board::initial();
        let fen = board.to_fen();
        // 重新解析应得到相同局面
        let board2 = Board::from_fen(&fen).unwrap();
        assert_eq!(board2.to_fen(), fen);
    }

    #[test]
    fn test_fen_parsing() {
        let fen = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        // 验证红方帅的位置
        let king_pos = board.find_king(Color::Red).unwrap();
        assert_eq!(king_pos, Position::new(4, 9));
        // 验证黑方将的位置
        let king_pos = board.find_king(Color::Black).unwrap();
        assert_eq!(king_pos, Position::new(4, 0));
    }

    #[test]
    fn test_material_scores() {
        let board = Board::initial();
        // 初始局面双方物质分应相等
        assert_eq!(board.red_material_score(), board.black_material_score());
    }

    #[test]
    fn test_piece_at() {
        let board = Board::initial();
        // 红方左车
        let piece = board.piece_at(Position::new(0, 9));
        assert!(piece.is_some());
        let p = piece.unwrap();
        assert_eq!(p.color, Color::Red);
        assert_eq!(p.piece_type, PieceType::Rook);

        // 空位
        let empty = board.piece_at(Position::new(4, 4));
        assert!(empty.is_none());
    }

    #[test]
    fn test_make_undo_move() {
        let board = Board::initial();
        let fen_before = board.to_fen();
        let score_before_r = board.red_material_score();
        let score_before_b = board.black_material_score();

        let mut board = board;
        // 红方炮二平五: 炮从 (1,7) 到 (4,7)
        let m = Move::new(Position::new(1, 7), Position::new(4, 7));
        let captured = board.make_move(m);
        assert!(captured.is_none()); // 初始局面此走法不吃子
        assert_eq!(board.side_to_move(), Color::Black);

        // 撤销
        board.undo_move(m, captured);
        assert_eq!(board.to_fen(), fen_before);
        assert_eq!(board.side_to_move(), Color::Red);
        assert_eq!(board.red_material_score(), score_before_r);
        assert_eq!(board.black_material_score(), score_before_b);
    }

    #[test]
    fn test_position_uci_roundtrip() {
        for col in 0..=8u8 {
            for row in 0..=9u8 {
                let pos = Position::new(col, row);
                let uci = pos.to_uci();
                let parsed = Position::from_uci(&uci).unwrap();
                assert_eq!(pos, parsed);
            }
        }
    }

    #[test]
    fn test_move_uci_roundtrip() {
        let m = Move::new(Position::new(0, 9), Position::new(0, 8));
        let uci = m.to_uci();
        assert_eq!(uci, "a9a8");
        let parsed = Move::from_uci(&uci).unwrap();
        assert_eq!(m, parsed);
    }

    #[test]
    fn test_empty_board() {
        let board = Board::new();
        assert_eq!(board.piece_count(), 0);
        assert_eq!(board.side_to_move(), Color::Red);
        assert_eq!(board.red_material_score(), 0);
        assert_eq!(board.black_material_score(), 0);
        assert!(board.find_king(Color::Red).is_none());
    }

    #[test]
    fn test_fen_invalid_rows() {
        // Too few rows
        assert!(Board::from_fen("rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9 w").is_err());
        // Too many rows
        assert!(Board::from_fen("rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR/9 w").is_err());
    }

    #[test]
    fn test_fen_invalid_columns() {
        // Row with only 8 columns
        assert!(Board::from_fen("rnbakabn/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w").is_err());
    }

    #[test]
    fn test_fen_invalid_characters() {
        assert!(Board::from_fen("rnbakabxr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w").is_err());
    }

    #[test]
    fn test_fen_empty_string() {
        assert!(Board::from_fen("").is_err());
    }

    #[test]
    fn test_fen_side_to_move() {
        let fen_red = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";
        let board_red = Board::from_fen(fen_red).unwrap();
        assert_eq!(board_red.side_to_move(), Color::Red);

        let fen_black = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR b - - 0 1";
        let board_black = Board::from_fen(fen_black).unwrap();
        assert_eq!(board_black.side_to_move(), Color::Black);
    }

    #[test]
    fn test_fen_invalid_side() {
        let fen = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR x - - 0 1";
        assert!(Board::from_fen(fen).is_err());
    }

    #[test]
    fn test_initial_position_piece_placements() {
        let board = Board::initial();
        // Black back rank
        assert_eq!(board.piece_at(Position::new(0, 0)).unwrap().piece_type, PieceType::Rook);
        assert_eq!(board.piece_at(Position::new(1, 0)).unwrap().piece_type, PieceType::Knight);
        assert_eq!(board.piece_at(Position::new(2, 0)).unwrap().piece_type, PieceType::Bishop);
        assert_eq!(board.piece_at(Position::new(3, 0)).unwrap().piece_type, PieceType::Advisor);
        assert_eq!(board.piece_at(Position::new(4, 0)).unwrap().piece_type, PieceType::King);
        assert_eq!(board.piece_at(Position::new(5, 0)).unwrap().piece_type, PieceType::Advisor);
        assert_eq!(board.piece_at(Position::new(6, 0)).unwrap().piece_type, PieceType::Bishop);
        assert_eq!(board.piece_at(Position::new(7, 0)).unwrap().piece_type, PieceType::Knight);
        assert_eq!(board.piece_at(Position::new(8, 0)).unwrap().piece_type, PieceType::Rook);

        // Red back rank
        assert_eq!(board.piece_at(Position::new(0, 9)).unwrap().piece_type, PieceType::Rook);
        assert_eq!(board.piece_at(Position::new(4, 9)).unwrap().piece_type, PieceType::King);

        // Black cannons
        assert_eq!(board.piece_at(Position::new(1, 2)).unwrap().piece_type, PieceType::Cannon);
        assert_eq!(board.piece_at(Position::new(7, 2)).unwrap().piece_type, PieceType::Cannon);

        // Red cannons
        assert_eq!(board.piece_at(Position::new(1, 7)).unwrap().piece_type, PieceType::Cannon);
        assert_eq!(board.piece_at(Position::new(7, 7)).unwrap().piece_type, PieceType::Cannon);

        // Pawns
        for col in (0..=8).step_by(2) {
            assert_eq!(board.piece_at(Position::new(col, 3)).unwrap().piece_type, PieceType::Pawn);
            assert_eq!(board.piece_at(Position::new(col, 3)).unwrap().color, Color::Black);
            assert_eq!(board.piece_at(Position::new(col, 6)).unwrap().piece_type, PieceType::Pawn);
            assert_eq!(board.piece_at(Position::new(col, 6)).unwrap().color, Color::Red);
        }
    }

    #[test]
    fn test_initial_position_color_counts() {
        let board = Board::initial();
        let red_count = board.pieces_of_color(Color::Red).count();
        let black_count = board.pieces_of_color(Color::Black).count();
        assert_eq!(red_count, 16);
        assert_eq!(black_count, 16);
    }

    #[test]
    fn test_make_move_captures() {
        // Set up a position where a capture happens
        // Black king at b0 (1,0), Red rook at h0 (7,0) on same row
        let fen = "1k5R1/9/9/9/9/9/9/9/9/4K4 w - - 0 1";
        let mut board = Board::from_fen(fen).unwrap();
        let red_score_before = board.red_material_score();
        let black_score_before = board.black_material_score();

        // Red rook captures black king at (1,0)
        let m = Move::new(Position::new(7, 0), Position::new(1, 0));
        let captured = board.make_move(m);
        assert!(captured.is_some());
        assert_eq!(captured.unwrap().piece_type, PieceType::King);
        assert_eq!(captured.unwrap().color, Color::Black);
        assert_eq!(board.black_material_score(), black_score_before - 10000);

        // Undo
        board.undo_move(m, captured);
        assert_eq!(board.red_material_score(), red_score_before);
        assert_eq!(board.black_material_score(), black_score_before);
    }

    #[test]
    fn test_multiple_make_undo_moves() {
        let board = Board::initial();
        let fen_before = board.to_fen();

        let mut board = board;
        let mut history: Vec<(Move, Option<Piece>)> = Vec::new();

        // Make several moves
        let moves = [
            Move::new(Position::new(1, 7), Position::new(4, 7)), // 炮二平五
            Move::new(Position::new(1, 0), Position::new(2, 2)), // 马8进7
            Move::new(Position::new(7, 7), Position::new(4, 7)), // This is invalid (already occupied) - let's pick a different move
        ];

        // First two moves
        for &m in &moves[0..2] {
            let captured = board.make_move(m);
            history.push((m, captured));
        }

        // Undo all
        for (m, captured) in history.into_iter().rev() {
            board.undo_move(m, captured);
        }

        assert_eq!(board.to_fen(), fen_before);
    }

    #[test]
    fn test_fen_custom_position() {
        // Only two kings on the board
        let fen = "4k4/9/9/9/9/9/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        assert_eq!(board.piece_count(), 2);
        assert_eq!(board.find_king(Color::Red), Some(Position::new(4, 9)));
        assert_eq!(board.find_king(Color::Black), Some(Position::new(4, 0)));
    }

    #[test]
    fn test_set_side_to_move() {
        let mut board = Board::new();
        board.set_side_to_move(Color::Black);
        assert_eq!(board.side_to_move(), Color::Black);
        board.set_side_to_move(Color::Red);
        assert_eq!(board.side_to_move(), Color::Red);
    }

    #[test]
    fn test_is_in_bounds() {
        assert!(Board::is_in_bounds(Position::new(0, 0)));
        assert!(Board::is_in_bounds(Position::new(8, 9)));
        assert!(Board::is_in_bounds(Position::new(4, 5)));
        // Position with col > 8 or row > 9 cannot be constructed via Position::new
        // since u8 can hold any value, but is_valid() checks
    }
}
