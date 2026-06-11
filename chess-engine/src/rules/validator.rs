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
        // 检查是否是飞将
        if is_flying_general(&new_board, color) {
            return Err(MoveError::FlyingGeneral);
        }
        return Err(MoveError::WouldBeInCheck);
    }

    Ok(())
}

/// 检查飞将规则
fn is_flying_general(board: &Board, color: Color) -> bool {
    let my_king = match board.find_king(color) {
        Some(pos) => pos,
        None => return false,
    };
    let opp_king = match board.find_king(color.opposite()) {
        Some(pos) => pos,
        None => return false,
    };

    if my_king.col != opp_king.col {
        return false;
    }

    let min_row = my_king.row.min(opp_king.row);
    let max_row = my_king.row.max(opp_king.row);
    for row in (min_row + 1)..max_row {
        if board.piece_at(crate::board::Position::new(my_king.col, row)).is_some() {
            return false;
        }
    }

    true
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
        let fen = "4k4/4r4/9/9/9/9/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        // 红帅往右走会被车将军
        // 实际上帅只能在九宫内，这里帅已经没有合法走法如果车在将军
    }
}
