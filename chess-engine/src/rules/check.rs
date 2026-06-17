use crate::board::{Board, Position};
use crate::pieces::{Color, PieceType};
use crate::pieces::movement::{is_line_clear, count_pieces_on_line, can_knight_reach, can_pawn_attack as pawn_can_attack};

/// 找到某方将/帅的位置
pub fn find_king(board: &Board, color: Color) -> Option<Position> {
    board.find_king(color)
}

/// 检测 color 方是否被将
/// 包含"飞将"规则：两将不能面对面
pub fn is_in_check(board: &Board, color: Color) -> bool {
    let king_pos = match find_king(board, color) {
        Some(pos) => pos,
        None => return false, // 没有将，不被将（不应发生）
    };

    let opponent = color.opposite();

    // 检查对方所有棋子的伪合法走法是否能攻击到将的位置
    for (pos, piece) in board.pieces_of_color(opponent) {
        match piece.piece_type {
            PieceType::Rook => {
                if can_rook_attack(board, pos, king_pos) {
                    return true;
                }
            }
            PieceType::Cannon => {
                if can_cannon_attack(board, pos, king_pos) {
                    return true;
                }
            }
            PieceType::Knight => {
                if can_knight_reach(board, pos, king_pos) {
                    return true;
                }
            }
            PieceType::Pawn => {
                if pawn_can_attack(pos, king_pos, opponent) {
                    return true;
                }
            }
            PieceType::King => {
                // 飞将规则：两将面对面
                if pos.col == king_pos.col && is_line_clear(board, pos, king_pos) {
                    return true;
                }
            }
            // 仕/士和相/象不能攻击将（它们不能离开九宫/己方半场）
            PieceType::Advisor | PieceType::Bishop => {}
        }
    }

    false
}

/// 车是否能攻击到目标位置
fn can_rook_attack(board: &Board, from: Position, target: Position) -> bool {
    if from.col != target.col && from.row != target.row {
        return false; // 不在同一行或列
    }
    is_line_clear(board, from, target)
}

/// 炮是否能攻击到目标位置
fn can_cannon_attack(board: &Board, from: Position, target: Position) -> bool {
    if from.col != target.col && from.row != target.row {
        return false; // 不在同一行或列
    }

    // 炮吃子需要恰好一个炮架
    count_pieces_on_line(board, from, target) == 1
}

/// 检测是否将杀 (checkmate)
pub fn is_checkmate(board: &Board, color: Color) -> bool {
    is_in_check(board, color) && board.generate_legal_moves(color).is_empty()
}

/// 检测是否困毙 (stalemate)
/// 注意：中国象棋中困毙 = 输，不是和棋！
pub fn is_stalemate(board: &Board, color: Color) -> bool {
    !is_in_check(board, color) && board.generate_legal_moves(color).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;
    use crate::pieces::Color;

    #[test]
    fn test_not_in_check_initial() {
        let board = Board::initial();
        assert!(!is_in_check(&board, Color::Red));
        assert!(!is_in_check(&board, Color::Black));
    }

    #[test]
    fn test_flying_general() {
        // 两将面对面，红方被将
        let fen = "4k4/9/9/9/9/9/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        // 两将面对面，红方走完后被将
        assert!(is_in_check(&board, Color::Red));
        assert!(is_in_check(&board, Color::Black));
    }

    #[test]
    fn test_simple_check() {
        // 红方车将军黑方
        let fen = "4k4/4R4/9/9/9/9/9/9/9/4K4 b - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        assert!(is_in_check(&board, Color::Black));
    }

    #[test]
    fn test_checkmate() {
        // 将杀局面: 黑将在角落，红车封锁
        // 黑将在 a0 (0,0)，红车在 a2 (0,2) 和 b1 (1,1)
        // 帅在 i9 (8,9) 远离
        let fen = "k8/1R7/R8/9/9/9/9/9/9/7K1 b - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        assert!(is_in_check(&board, Color::Black), "Black should be in check");
        assert!(is_checkmate(&board, Color::Black), "Black should be checkmated");
    }

    #[test]
    fn test_rook_check() {
        // 红车将军黑将 - rook at e1 (4,1) on same file as black king at e0 (4,0)
        let fen = "4k4/4R4/9/9/9/9/9/9/9/4K4 b - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        assert!(is_in_check(&board, Color::Black));
        assert!(!is_in_check(&board, Color::Red));
    }

    #[test]
    fn test_cannon_check() {
        // 红炮将军黑将 (通过炮架)
        // Black king at a0 (0,0), red pawn at c0 (2,0) as screen, red cannon at d0 (3,0)
        // Cannon at col 3, screen at col 2, king at col 0 - exactly 1 piece between = cannon check!
        let fen = "k1PC5/9/9/9/9/9/9/9/9/4K4 b - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        assert!(is_in_check(&board, Color::Black), "Cannon should check black king through screen");
    }

    #[test]
    fn test_knight_check() {
        // 红马将军黑将
        // Knight at c1 (2,1) can attack a0 (0,0) via the "日" pattern
        let fen = "k8/2N6/9/9/9/9/9/9/9/4K4 b - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        assert!(is_in_check(&board, Color::Black), "Knight should check black king");
    }

    #[test]
    fn test_pawn_check() {
        // 红兵将军黑将 (兵过河后)
        // Red pawn at a1 (0,1) can attack a0 (0,0) - forward
        let fen = "k8/P8/9/9/9/9/9/9/9/4K4 b - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        assert!(is_in_check(&board, Color::Black), "Red pawn should check black king");
    }

    #[test]
    fn test_not_checkmate_when_can_escape() {
        // 黑方被将但有逃跑路线
        // Black king at e0 (4,0), Red rook at e2 (4,2) on same file - checking
        // Black king can escape to d0, d1, f0, f1
        let fen = "4k4/9/4R4/9/9/9/9/9/9/4K4 b - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        assert!(is_in_check(&board, Color::Black), "Black should be in check");
        assert!(!is_checkmate(&board, Color::Black), "Black should not be checkmated - can escape");
    }

    #[test]
    fn test_stalemate() {
        // 困毙局面: 黑方不在将军中但没有合法走法
        // Black king at e0 (4,0). Only 5 possible king moves:
        // d0 (3,0) - controlled by red rook on d-file
        // d1 (3,1) - controlled by red rook on d-file
        // e1 (4,1) - controlled by red pawn at e2 (4,2) attacking forward
        // f0 (5,0) - controlled by red rook on f-file
        // f1 (5,1) - controlled by red rook on f-file
        // None of these attack e0 directly, so king is NOT in check.
        let fen = "4k4/9/4P4/9/9/9/9/9/9/3R1R1K1 b - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        assert!(!is_in_check(&board, Color::Black), "Black should NOT be in check");
        assert!(board.generate_legal_moves(Color::Black).is_empty(), "Black should have no legal moves");
        assert!(is_stalemate(&board, Color::Black), "Should be stalemate");
    }

    #[test]
    fn test_flying_general_with_intervening_piece() {
        // 两将同列但中间有子，不算飞将
        // Use a red pawn (not an attacking piece for black) between the kings
        let fen = "4k4/4P4/9/9/9/9/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        // Red pawn at (4,1) between the kings - blocks flying general
        assert!(!is_in_check(&board, Color::Red), "Flying general should be blocked by intervening piece");
    }

    #[test]
    fn test_double_check() {
        // 双将：被两个棋子同时将军
        // Red rook at a1 (0,1) and Red rook at e2 (4,2)
        // Black king at e0 (4,0) - checked by rook on e2 on same file
        // Also red rook at a1 on same row as potential escape
        // Let me use two rooks checking: one on file, one on row
        let fen = "4k4/R8/4R4/9/9/9/9/9/9/4K4 b - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        assert!(is_in_check(&board, Color::Black), "Black should be in check from rooks");
    }

    #[test]
    fn test_advisor_cannot_check() {
        // Advisor cannot check because it can't leave the palace
        // Place red king off the e-file so flying general doesn't apply
        let fen = "4k4/9/9/9/9/9/9/9/3A5/3K5 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        assert!(!is_in_check(&board, Color::Black), "Advisor cannot check");
    }

    #[test]
    fn test_bishop_cannot_check() {
        // Bishop cannot check because it can't cross the river
        let fen = "4k4/9/9/9/9/9/9/4B4/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        assert!(!is_in_check(&board, Color::Black), "Bishop cannot check");
    }

    #[test]
    fn test_no_king_not_in_check() {
        // If there's no king, is_in_check should return false
        let fen = "9/9/9/9/9/9/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        assert!(!is_in_check(&board, Color::Black), "No black king means not in check");
    }

    #[test]
    fn test_rook_blocked_by_own_piece_not_checking() {
        // Rook on same file as king but blocked by own piece
        // Red rook at e9 (4,9), red pawn at e8 (4,8) blocks the rook from seeing black king at e0 (4,0)
        let fen = "4k4/9/9/9/9/9/9/9/4P4/4RK3 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        assert!(!is_in_check(&board, Color::Black), "Rook blocked by own pawn should not check");
    }
}
