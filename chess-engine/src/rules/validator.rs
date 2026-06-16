use crate::board::Board;
use crate::pieces::{Color, Move};
use crate::rules::is_in_check;

/// 走法错误类型
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MoveError {
    OutOfBounds,
    NoPieceAtFrom,
    WrongColor,
    CannotCaptureOwnPiece,
    IllegalMove,
    WouldBeInCheck,
    FlyingGeneral,
}

impl std::fmt::Display for MoveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MoveError::OutOfBounds => write!(f, "Position out of bounds"),
            MoveError::NoPieceAtFrom => write!(f, "No piece at source position"),
            MoveError::WrongColor => write!(f, "Not your piece"),
            MoveError::CannotCaptureOwnPiece => write!(f, "Cannot capture own piece"),
            MoveError::IllegalMove => write!(f, "Illegal move for this piece type"),
            MoveError::WouldBeInCheck => write!(f, "Move would leave king in check"),
            MoveError::FlyingGeneral => write!(f, "Flying general violation"),
        }
    }
}

impl std::error::Error for MoveError {}

/// 验证走法合法性
pub fn validate_move(board: &Board, m: Move, color: Color) -> Result<(), MoveError> {
    // 1. 检查坐标是否在棋盘内
    if !Board::is_in_bounds(m.from) || !Board::is_in_bounds(m.to) {
        return Err(MoveError::OutOfBounds);
    }

    // 2. 检查起点是否有棋子
    let piece = board.piece_at(m.from).ok_or(MoveError::NoPieceAtFrom)?;

    // 3. 检查是否是自己的棋子
    if piece.color != color {
        return Err(MoveError::WrongColor);
    }

    // 4. 检查是否吃自己的子
    if let Some(target) = board.piece_at(m.to) {
        if target.color == color {
            return Err(MoveError::CannotCaptureOwnPiece);
        }
    }

    // 5. 检查走法是否在伪合法走法列表中
    let pseudo_legal = board.generate_pseudo_legal_moves(color);
    if !pseudo_legal.contains(&m) {
        return Err(MoveError::IllegalMove);
    }

    // 6. 检查走法是否会导致自己被将
    let mut new_board = board.clone();
    new_board.make_move(m);
    if is_in_check(&new_board, color) {
        // 判断是否是飞将导致的被将 (两将同列面对面无遮挡)
        // 飞将检测已包含在 is_in_check 中，这里只做区分：如果唯一攻击方是对方的将，则是飞将
        let my_king = new_board.find_king(color);
        let opp_king = new_board.find_king(color.opposite());
        if let (Some(mk), Some(ok)) = (my_king, opp_king) {
            if mk.col == ok.col {
                // 检查两将之间是否无遮挡
                let min_row = mk.row.min(ok.row);
                let max_row = mk.row.max(ok.row);
                let blocked = (min_row + 1..max_row)
                    .any(|row| new_board.piece_at(crate::board::Position::new(mk.col, row)).is_some());
                if !blocked {
                    return Err(MoveError::FlyingGeneral);
                }
            }
        }
        return Err(MoveError::WouldBeInCheck);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;
    use crate::board::Position;

    #[test]
    fn test_validate_legal_move() {
        let board = Board::initial();
        // 红方炮二平五
        let m = Move::new(Position::new(1, 7), Position::new(4, 7));
        assert!(validate_move(&board, m, Color::Red).is_ok());
    }

    #[test]
    fn test_validate_wrong_color() {
        let board = Board::initial();
        // 红方尝试走黑方棋子
        let m = Move::new(Position::new(0, 0), Position::new(0, 1));
        assert_eq!(validate_move(&board, m, Color::Red), Err(MoveError::WrongColor));
    }

    #[test]
    fn test_validate_no_piece() {
        let board = Board::initial();
        // 尝试移动空位
        let m = Move::new(Position::new(4, 4), Position::new(4, 5));
        assert_eq!(validate_move(&board, m, Color::Red), Err(MoveError::NoPieceAtFrom));
    }

    #[test]
    fn test_validate_would_be_in_check() {
        // 构造一个走法会导致自己被将的局面
        // Red king at d9 (3,9), black rook at e8 (4,8)
        // King moving to e9 (4,9) would be on same file as rook at (4,8) - in check
        let fen = "4k4/9/9/9/9/9/9/9/4r4/3K5 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        let m = Move::new(Position::new(3, 9), Position::new(4, 9));
        assert_eq!(validate_move(&board, m, Color::Red), Err(MoveError::WouldBeInCheck));
    }
}
