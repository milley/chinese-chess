use crate::board::{Board, Position};
use crate::pieces::{Color, PieceType};

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
                if can_knight_attack(board, pos, king_pos) {
                    return true;
                }
            }
            PieceType::Pawn => {
                if can_pawn_attack(pos, king_pos, opponent) {
                    return true;
                }
            }
            PieceType::King => {
                // 飞将规则：两将面对面
                if pos.col == king_pos.col {
                    let min_row = pos.row.min(king_pos.row);
                    let max_row = pos.row.max(king_pos.row);
                    let mut blocked = false;
                    for row in (min_row + 1)..max_row {
                        if board.piece_at(Position::new(pos.col, row)).is_some() {
                            blocked = true;
                            break;
                        }
                    }
                    if !blocked {
                        return true;
                    }
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

    // 检查路径上是否有阻挡
    if from.col == target.col {
        let min_row = from.row.min(target.row);
        let max_row = from.row.max(target.row);
        for row in (min_row + 1)..max_row {
            if board.piece_at(Position::new(from.col, row)).is_some() {
                return false;
            }
        }
    } else {
        let min_col = from.col.min(target.col);
        let max_col = from.col.max(target.col);
        for col in (min_col + 1)..max_col {
            if board.piece_at(Position::new(col, from.row)).is_some() {
                return false;
            }
        }
    }

    true
}

/// 炮是否能攻击到目标位置
fn can_cannon_attack(board: &Board, from: Position, target: Position) -> bool {
    if from.col != target.col && from.row != target.row {
        return false; // 不在同一行或列
    }

    // 计算路径上的棋子数
    let mut count = 0;
    if from.col == target.col {
        let min_row = from.row.min(target.row);
        let max_row = from.row.max(target.row);
        for row in (min_row + 1)..max_row {
            if board.piece_at(Position::new(from.col, row)).is_some() {
                count += 1;
            }
        }
    } else {
        let min_col = from.col.min(target.col);
        let max_col = from.col.max(target.col);
        for col in (min_col + 1)..max_col {
            if board.piece_at(Position::new(col, from.row)).is_some() {
                count += 1;
            }
        }
    }

    // 炮吃子需要恰好一个炮架
    count == 1
}

/// 马是否能攻击到目标位置
fn can_knight_attack(board: &Board, from: Position, target: Position) -> bool {
    let dc = (target.col as i8 - from.col as i8).abs();
    let dr = (target.row as i8 - from.row as i8).abs();

    // 马走"日"字
    if !((dc == 1 && dr == 2) || (dc == 2 && dr == 1)) {
        return false;
    }

    // 蹩马腿检测
    let leg_col: i8;
    let leg_row: i8;
    if dc == 2 {
        // 横向走2格，马腿在横向第1格
        leg_col = from.col as i8 + if target.col > from.col { 1 } else { -1 };
        leg_row = from.row as i8;
    } else {
        // 纵向走2格，马腿在纵向第1格
        leg_col = from.col as i8;
        leg_row = from.row as i8 + if target.row > from.row { 1 } else { -1 };
    }

    if leg_col < 0 || leg_col > 8 || leg_row < 0 || leg_row > 9 {
        return false;
    }

    board.piece_at(Position::new(leg_col as u8, leg_row as u8)).is_none()
}

/// 兵/卒是否能攻击到目标位置
fn can_pawn_attack(from: Position, target: Position, color: Color) -> bool {
    let dc = (target.col as i8 - from.col as i8).abs();
    let dr = (target.row as i8 - from.row as i8).abs();

    if dc + dr != 1 {
        return false; // 只能走一步
    }

    let crossed = crate::pieces::movement::has_crossed_river(from.row, color);
    let forward = crate::pieces::movement::pawn_forward_offset(color);

    if dc == 1 {
        // 横向移动，必须已过河
        crossed && from.row == target.row
    } else {
        // 纵向移动，必须前进
        (target.row as i8 - from.row as i8) == forward
    }
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
}
