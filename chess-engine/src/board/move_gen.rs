use crate::pieces::{Color, PieceType, Move};
use crate::pieces::movement::*;
use super::{Board, Position};

impl Board {
    /// 生成所有伪合法走法 (不检查是否导致自己被将)
    pub fn generate_pseudo_legal_moves(&self, color: Color) -> Vec<Move> {
        let mut moves = Vec::new();
        for (pos, piece) in self.pieces_of_color(color) {
            match piece.piece_type {
                PieceType::King => self.gen_king_moves(pos, color, &mut moves),
                PieceType::Advisor => self.gen_advisor_moves(pos, color, &mut moves),
                PieceType::Bishop => self.gen_bishop_moves(pos, color, &mut moves),
                PieceType::Knight => self.gen_knight_moves(pos, color, &mut moves),
                PieceType::Rook => self.gen_rook_moves(pos, color, &mut moves),
                PieceType::Cannon => self.gen_cannon_moves(pos, color, &mut moves),
                PieceType::Pawn => self.gen_pawn_moves(pos, color, &mut moves),
            }
        }
        moves
    }

    /// 生成所有合法走法 (过滤掉导致自己被将的走法)
    pub fn generate_legal_moves(&self, color: Color) -> Vec<Move> {
        self.generate_pseudo_legal_moves(color)
            .into_iter()
            .filter(|&m| {
                // 不能吃自己的子
                if let Some(p) = self.piece_at(m.to) {
                    if p.color == color {
                        return false;
                    }
                }
                // 执行走法后检查是否被将
                let mut board = self.clone();
                board.make_move(m);
                !crate::rules::is_in_check(&board, color)
            })
            .collect()
    }

    /// 帅/将走法: 九宫内一步直走
    fn gen_king_moves(&self, pos: Position, color: Color, moves: &mut Vec<Move>) {
        let directions: [(i8, i8); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];
        for (dc, dr) in directions {
            let nc = pos.col as i8 + dc;
            let nr = pos.row as i8 + dr;
            if nc < 0 || nc > 8 || nr < 0 || nr > 9 {
                continue;
            }
            let target = Position::new(nc as u8, nr as u8);
            if !is_in_palace(target, color) {
                continue;
            }
            if let Some(p) = self.piece_at(target) {
                if p.color == color {
                    continue;
                }
            }
            moves.push(Move::new(pos, target));
        }
    }

    /// 仕/士走法: 九宫内一步斜走
    fn gen_advisor_moves(&self, pos: Position, color: Color, moves: &mut Vec<Move>) {
        let directions: [(i8, i8); 4] = [(-1, -1), (-1, 1), (1, -1), (1, 1)];
        for (dc, dr) in directions {
            let nc = pos.col as i8 + dc;
            let nr = pos.row as i8 + dr;
            if nc < 0 || nc > 8 || nr < 0 || nr > 9 {
                continue;
            }
            let target = Position::new(nc as u8, nr as u8);
            if !is_in_palace(target, color) {
                continue;
            }
            if let Some(p) = self.piece_at(target) {
                if p.color == color {
                    continue;
                }
            }
            moves.push(Move::new(pos, target));
        }
    }

    /// 相/象走法: 走"田"字，不能过河，塞象眼不能走
    fn gen_bishop_moves(&self, pos: Position, color: Color, moves: &mut Vec<Move>) {
        for (eye, target) in bishop_eye_and_target(pos) {
            // 检查目标位置是否在棋盘内
            if !target.is_valid() || !eye.is_valid() {
                continue;
            }
            // 不能过河
            if !is_on_own_side(target.row, color) {
                continue;
            }
            // 塞象眼检测
            if self.piece_at(eye).is_some() {
                continue;
            }
            // 不能吃自己的子
            if let Some(p) = self.piece_at(target) {
                if p.color == color {
                    continue;
                }
            }
            moves.push(Move::new(pos, target));
        }
    }

    /// 马走法: 走"日"字，蹩马腿不能走
    fn gen_knight_moves(&self, pos: Position, color: Color, moves: &mut Vec<Move>) {
        for (leg, target) in knight_leg_positions(pos) {
            // 检查目标位置是否在棋盘内
            if !target.is_valid() || !leg.is_valid() {
                continue;
            }
            // 蹩马腿检测
            if self.piece_at(leg).is_some() {
                continue;
            }
            // 不能吃自己的子
            if let Some(p) = self.piece_at(target) {
                if p.color == color {
                    continue;
                }
            }
            moves.push(Move::new(pos, target));
        }
    }

    /// 车走法: 直线任意距离，不能越子
    fn gen_rook_moves(&self, pos: Position, color: Color, moves: &mut Vec<Move>) {
        let directions: [(i8, i8); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];
        for (dc, dr) in directions {
            let mut nc = pos.col as i8 + dc;
            let mut nr = pos.row as i8 + dr;
            while nc >= 0 && nc <= 8 && nr >= 0 && nr <= 9 {
                let target = Position::new(nc as u8, nr as u8);
                if let Some(p) = self.piece_at(target) {
                    if p.color != color {
                        moves.push(Move::new(pos, target)); // 吃子
                    }
                    break; // 遇到棋子停止
                }
                moves.push(Move::new(pos, target));
                nc += dc;
                nr += dr;
            }
        }
    }

    /// 炮走法: 移动同车；吃子需隔一子（炮架）
    fn gen_cannon_moves(&self, pos: Position, color: Color, moves: &mut Vec<Move>) {
        let directions: [(i8, i8); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];
        for (dc, dr) in directions {
            let mut nc = pos.col as i8 + dc;
            let mut nr = pos.row as i8 + dr;
            let mut jumped = false; // 是否已翻越炮架

            while nc >= 0 && nc <= 8 && nr >= 0 && nr <= 9 {
                let target = Position::new(nc as u8, nr as u8);
                if !jumped {
                    // 未翻越炮架：非吃子走法（同车）
                    if let Some(_p) = self.piece_at(target) {
                        jumped = true; // 遇到第一个子，成为炮架
                    } else {
                        moves.push(Move::new(pos, target)); // 空位可走
                    }
                } else {
                    // 已翻越炮架：只能吃子
                    if let Some(p) = self.piece_at(target) {
                        if p.color != color {
                            moves.push(Move::new(pos, target)); // 隔一子吃
                        }
                        break; // 无论吃不吃，遇到第二个子就停
                    }
                }
                nc += dc;
                nr += dr;
            }
        }
    }

    /// 兵/卒走法: 未过河只能前进一步；过河后可前进或左右一步
    fn gen_pawn_moves(&self, pos: Position, color: Color, moves: &mut Vec<Move>) {
        let forward = pawn_forward_offset(color);
        let crossed = has_crossed_river(pos.row, color);

        // 前进
        let nr = pos.row as i8 + forward;
        if nr >= 0 && nr <= 9 {
            let target = Position::new(pos.col, nr as u8);
            if let Some(p) = self.piece_at(target) {
                if p.color != color {
                    moves.push(Move::new(pos, target));
                }
            } else {
                moves.push(Move::new(pos, target));
            }
        }

        // 过河后可左右
        if crossed {
            for dc in [-1i8, 1i8] {
                let nc = pos.col as i8 + dc;
                if nc >= 0 && nc <= 8 {
                    let target = Position::new(nc as u8, pos.row);
                    if let Some(p) = self.piece_at(target) {
                        if p.color != color {
                            moves.push(Move::new(pos, target));
                        }
                    } else {
                        moves.push(Move::new(pos, target));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_position_red_moves() {
        let board = Board::initial();
        let red_moves = board.generate_legal_moves(Color::Red);
        assert_eq!(red_moves.len(), 44, "Initial position should have 44 legal moves for Red");
    }

    #[test]
    fn test_initial_position_black_moves() {
        let board = Board::initial();
        let black_moves = board.generate_legal_moves(Color::Black);
        assert_eq!(black_moves.len(), 44, "Initial position should have 44 legal moves for Black");
    }

    #[test]
    fn test_pawn_before_river() {
        // 红方兵未过河只能前进
        // 将帅不在同一列，避免飞将干扰
        let fen = "1k7/9/9/9/9/9/P8/9/9/5K3 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        // 红方兵在 a6 (col=0, row=6)，未过河
        let pawn_pos = Position::new(0, 6);
        let pawn = board.piece_at(pawn_pos);
        assert!(pawn.is_some());
        assert_eq!(pawn.unwrap().piece_type, PieceType::Pawn);
        let moves: Vec<Move> = board.generate_legal_moves(Color::Red)
            .into_iter()
            .filter(|m| m.from == pawn_pos)
            .collect();
        // 未过河兵只有1个走法：前进到 a5
        assert_eq!(moves.len(), 1, "Pawn before river should have 1 move, got {}", moves.len());
        assert_eq!(moves[0].to, Position::new(0, 5));
    }

    #[test]
    fn test_pawn_after_river() {
        // 红方兵已过河可前进或左右
        let fen = "1k7/9/9/9/P8/9/9/9/9/5K3 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        let pawn_pos = Position::new(0, 4);
        let pawn = board.piece_at(pawn_pos);
        assert!(pawn.is_some(), "Should have pawn at a4");
        let moves: Vec<Move> = board.generate_legal_moves(Color::Red)
            .into_iter()
            .filter(|m| m.from == pawn_pos)
            .collect();
        // 过河兵在a列有2个走法：前进、右
        assert!(moves.len() >= 2, "Crossed pawn should have at least 2 moves, got {}", moves.len());
    }

    #[test]
    fn test_knight_leg_blocking() {
        // 马被蹩腿
        let fen = "4k4/9/9/9/9/9/9/4N4/4P4/3K5 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        let knight_pos = Position::new(4, 7);
        let moves: Vec<Move> = board.generate_legal_moves(Color::Red)
            .into_iter()
            .filter(|m| m.from == knight_pos)
            .collect();
        // 马在 (4,7)，兵在 (4,8) 蹩住向下方向的马腿
        // 向下方向的两个目标 (3,9) 和 (5,9) 应被阻挡
        let blocked_targets: Vec<Position> = moves.iter().map(|m| m.to).collect();
        assert!(!blocked_targets.contains(&Position::new(3, 9)), "Knight should be blocked by leg at (3,9)");
        assert!(!blocked_targets.contains(&Position::new(5, 9)), "Knight should be blocked by leg at (5,9)");
    }

    #[test]
    fn test_cannon_capture() {
        // 炮吃子需隔一子
        let fen = "4k4/9/9/9/9/9/9/4C4/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        let cannon_pos = Position::new(4, 7);
        let moves: Vec<Move> = board.generate_pseudo_legal_moves(Color::Red)
            .into_iter()
            .filter(|m| m.from == cannon_pos)
            .collect();
        // 炮在 (4,7)，可以上下左右移动，但没有炮架所以不能吃子
        // 所有走法都应该是非吃子走法
        for m in &moves {
            assert!(board.piece_at(m.to).is_none());
        }
    }

    #[test]
    fn test_king_confined_to_palace() {
        let fen = "4k4/9/9/9/9/9/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        let king_pos = board.find_king(Color::Red).unwrap();
        let moves: Vec<Move> = board.generate_legal_moves(Color::Red)
            .into_iter()
            .filter(|m| m.from == king_pos)
            .collect();
        // 帅只能在九宫内移动
        for m in &moves {
            assert!(is_in_palace(m.to, Color::Red), "King move target must be in palace");
        }
    }

    #[test]
    fn test_bishop_cannot_cross_river() {
        let fen = "4k4/9/9/9/9/9/9/4B4/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        let bishop_pos = Position::new(4, 7);
        let moves: Vec<Move> = board.generate_legal_moves(Color::Red)
            .into_iter()
            .filter(|m| m.from == bishop_pos)
            .collect();
        // 相不能过河
        for m in &moves {
            assert!(is_on_own_side(m.to.row, Color::Red), "Bishop must stay on own side");
        }
    }
}
