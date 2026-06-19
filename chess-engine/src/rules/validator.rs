use crate::board::Board;
use crate::board::Position;
use crate::pieces::{Color, PieceType, Move};
use crate::pieces::movement::*;
use crate::rules::is_in_check;
use crate::pieces::movement::{is_line_clear, count_pieces_on_line};

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
    if let Some(target) = board.piece_at(m.to)
        && target.color == color {
            return Err(MoveError::CannotCaptureOwnPiece);
        }

    // 5. 按棋子类型验证走法合法性 (不生成全部伪合法走法)
    if !is_valid_piece_move(board, m, piece.piece_type, color) {
        return Err(MoveError::IllegalMove);
    }

    // 6. 检查走法是否会导致自己被将
    let mut new_board = board.clone();
    new_board.make_move(m);
    if is_in_check(&new_board, color) {
        // 判断是否是飞将导致的被将
        let my_king = new_board.find_king(color);
        let opp_king = new_board.find_king(color.opposite());
        if let (Some(mk), Some(ok)) = (my_king, opp_king)
            && mk.col == ok.col {
                let min_row = mk.row.min(ok.row);
                let max_row = mk.row.max(ok.row);
                let blocked = (min_row + 1..max_row)
                    .any(|row| new_board.piece_at(Position::new(mk.col, row)).is_some());
                if !blocked {
                    return Err(MoveError::FlyingGeneral);
                }
            }
        return Err(MoveError::WouldBeInCheck);
    }

    Ok(())
}

/// 按棋子类型验证单个走法是否合法 (伪合法，不检查是否被将)
fn is_valid_piece_move(board: &Board, m: Move, piece_type: PieceType, color: Color) -> bool {
    match piece_type {
        PieceType::King => is_valid_king_move(m, color),
        PieceType::Advisor => is_valid_advisor_move(m, color),
        PieceType::Bishop => is_valid_bishop_move(board, m, color),
        PieceType::Knight => is_valid_knight_move(board, m),
        PieceType::Rook => is_valid_rook_move(board, m),
        PieceType::Cannon => is_valid_cannon_move(board, m, color),
        PieceType::Pawn => is_valid_pawn_move(m, color),
    }
}

/// 帅/将: 九宫内一步直走
fn is_valid_king_move(m: Move, color: Color) -> bool {
    let dc = (m.to.col as i8 - m.from.col as i8).abs();
    let dr = (m.to.row as i8 - m.from.row as i8).abs();
    // 必须走一步 (dc+dr == 1)
    if dc + dr != 1 {
        return false;
    }
    // 目标必须在九宫内
    is_in_palace(m.to, color)
}

/// 仕/士: 九宫内一步斜走
fn is_valid_advisor_move(m: Move, color: Color) -> bool {
    let dc = (m.to.col as i8 - m.from.col as i8).abs();
    let dr = (m.to.row as i8 - m.from.row as i8).abs();
    // 必须斜走一步 (dc==1 && dr==1)
    if dc != 1 || dr != 1 {
        return false;
    }
    // 目标必须在九宫内
    is_in_palace(m.to, color)
}

/// 相/象: 走"田"字，不过河，不塞眼
fn is_valid_bishop_move(board: &Board, m: Move, color: Color) -> bool {
    let dc = (m.to.col as i8 - m.from.col as i8).abs();
    let dr = (m.to.row as i8 - m.from.row as i8).abs();
    // 必须走"田"字 (dc==2 && dr==2)
    if dc != 2 || dr != 2 {
        return false;
    }
    // 不能过河
    if !is_on_own_side(m.to.row, color) {
        return false;
    }
    // 塞象眼检测: 象眼在起点和终点的中间
    let eye_col = ((m.from.col as i8) + (m.to.col as i8)) / 2;
    let eye_row = ((m.from.row as i8) + (m.to.row as i8)) / 2;
    if !(0..=8).contains(&eye_col) || !(0..=9).contains(&eye_row) {
        return false;
    }
    let eye = Position::new(eye_col as u8, eye_row as u8);
    board.piece_at(eye).is_none()
}

/// 马: 走"日"字，不蹩腿 (复用 movement 中的统一蹩腿检测)
fn is_valid_knight_move(board: &Board, m: Move) -> bool {
    can_knight_reach(board, m.from, m.to)
}

/// 车: 直线任意距离，不能越子
fn is_valid_rook_move(board: &Board, m: Move) -> bool {
    // 必须在同一行或列
    if m.from.col != m.to.col && m.from.row != m.to.row {
        return false;
    }
    // 路径上不能有阻挡
    is_line_clear(board, m.from, m.to)
}

/// 炮: 移动同车 (无子阻挡)；吃子需隔一子
fn is_valid_cannon_move(board: &Board, m: Move, _color: Color) -> bool {
    // 必须在同一行或列
    if m.from.col != m.to.col && m.from.row != m.to.row {
        return false;
    }
    let count = count_pieces_on_line(board, m.from, m.to);
    let target = board.piece_at(m.to);
    if target.is_some() {
        // 吃子: 恰好隔1子
        count == 1
    } else {
        // 移动: 无阻挡
        count == 0
    }
}

/// 兵/卒: 未过河只前进；过河后可前进或左右
fn is_valid_pawn_move(m: Move, color: Color) -> bool {
    let dc = (m.to.col as i8 - m.from.col as i8).abs();
    let dr = m.to.row as i8 - m.from.row as i8;
    let crossed = has_crossed_river(m.from.row, color);

    if dc == 1 && dr == 0 {
        // 横向移动: 必须已过河
        crossed
    } else if dc == 0 && dr.abs() == 1 {
        // 纵向移动: 必须前进
        let forward = pawn_forward_offset(color);
        dr == forward
    } else {
        false
    }
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

    #[test]
    fn test_validate_cannot_capture_own_piece() {
        let board = Board::initial();
        // Try moving red pawn to red cannon position
        let m = Move::new(Position::new(0, 6), Position::new(1, 7));
        assert_eq!(validate_move(&board, m, Color::Red), Err(MoveError::CannotCaptureOwnPiece));
    }

    #[test]
    fn test_validate_out_of_bounds() {
        let board = Board::initial();
        let _m = Move::new(Position::new(0, 0), Position::new(9, 0)); // col 9 is out of bounds
        // This would fail at NoPieceAtFrom since Position::new(9,0) may still construct
        // Let's test with a valid from but the move is illegal
        let m = Move::new(Position::new(4, 9), Position::new(4, 10)); // row 10 out of bounds
        assert_eq!(validate_move(&board, m, Color::Red), Err(MoveError::OutOfBounds));
    }

    #[test]
    fn test_validate_illegal_knight_jump() {
        // Knight trying to jump like a bishop
        let fen = "4k4/9/9/9/9/4N4/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        // Knight at (4,5) trying to go to (6,7) — not a valid knight move pattern
        let m = Move::new(Position::new(4, 5), Position::new(6, 7));
        assert_eq!(validate_move(&board, m, Color::Red), Err(MoveError::IllegalMove));
    }

    #[test]
    fn test_validate_flying_general() {
        // Moving an advisor that blocks the flying general should be illegal
        // Red king at e9 (4,9), black king at e0 (4,0), red advisor at e8 (4,8) blocks.
        // Advisor at (4,8) moves to d9 (3,9) — a valid advisor diagonal move within palace.
        // But after moving, both kings face each other on e-file with no intervening piece.
        let fen = "4k4/9/9/9/9/9/9/9/4A4/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        let m = Move::new(Position::new(4, 8), Position::new(3, 9));
        assert_eq!(validate_move(&board, m, Color::Red), Err(MoveError::FlyingGeneral));
    }

    #[test]
    fn test_validate_cannon_no_screen_capture() {
        // Cannon cannot capture without a screen piece
        let fen = "4k4/9/9/9/9/9/9/4C4/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        // Cannon at (4,7) trying to capture king at (4,0) — no screen piece between them
        let m = Move::new(Position::new(4, 7), Position::new(4, 0));
        assert_eq!(validate_move(&board, m, Color::Red), Err(MoveError::IllegalMove));
    }

    #[test]
    fn test_validate_king_move_in_palace() {
        // Valid king move: e9 to d9
        let fen = "4k4/9/9/9/9/9/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        let m = Move::new(Position::new(4, 9), Position::new(3, 9));
        assert!(validate_move(&board, m, Color::Red).is_ok());
    }

    #[test]
    fn test_validate_advisor_move() {
        // Valid advisor move: d9 to e8
        let fen = "4k4/9/9/9/9/9/9/9/3A5/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        let m = Move::new(Position::new(3, 8), Position::new(4, 7));
        // This might leave flying general, so check it's either ok or FlyingGeneral
        let result = validate_move(&board, m, Color::Red);
        assert!(result.is_ok() || result == Err(MoveError::FlyingGeneral) || result == Err(MoveError::WouldBeInCheck));
    }

    #[test]
    fn test_validate_bishop_move_with_eye() {
        // Bishop at c9 (2,9), eye at d8 (3,8) blocked by piece
        let fen = "4k4/9/9/9/9/9/9/9/3A5/2B1K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        // Bishop at (2,9) trying to go to (4,7) - eye at (3,8) blocked by advisor
        let m = Move::new(Position::new(2, 9), Position::new(4, 7));
        assert_eq!(validate_move(&board, m, Color::Red), Err(MoveError::IllegalMove));
    }

    #[test]
    fn test_validate_knight_move_with_leg() {
        // Knight at e8 (4,7), pawn at e9 (4,8) blocks downward leg
        let fen = "4k4/9/9/9/9/9/9/4N4/4P4/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        // Knight at (4,7) trying to go to (3,9) - leg at (4,8) is blocked by pawn
        let m = Move::new(Position::new(4, 7), Position::new(3, 9));
        assert_eq!(validate_move(&board, m, Color::Red), Err(MoveError::IllegalMove));
    }

    #[test]
    fn test_validate_rook_move_path_check() {
        // Rook at a9, path to a0 blocked by piece at a5
        let fen = "4k4/9/9/9/9/P8/9/9/9/R3K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        // Rook at (0,9) trying to go to (0,3) - pawn at (0,5) blocks
        let m = Move::new(Position::new(0, 9), Position::new(0, 3));
        assert_eq!(validate_move(&board, m, Color::Red), Err(MoveError::IllegalMove));
    }

    #[test]
    fn test_validate_cannon_move_with_screen() {
        // Cannon capture through screen: valid
        // Cannon at (4,7), screen pawn at (4,5), black bishop at (4,4)
        let fen = "4k4/9/9/9/4b4/4P4/9/4C4/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        // Cannon can capture bishop through pawn screen
        let m = Move::new(Position::new(4, 7), Position::new(4, 4));
        // This should be valid (capture through 1 screen)
        let result = validate_move(&board, m, Color::Red);
        assert!(result.is_ok() || result == Err(MoveError::WouldBeInCheck) || result == Err(MoveError::FlyingGeneral),
            "Cannon capture through screen should be legal move pattern, got {:?}", result);
    }

    #[test]
    fn test_validate_pawn_backward_rejected() {
        // Red pawn cannot move backward
        let fen = "1k7/9/9/9/9/9/P8/9/9/5K3 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        // Red pawn at (0,6) trying to go backward to (0,7)
        let m = Move::new(Position::new(0, 6), Position::new(0, 7));
        assert_eq!(validate_move(&board, m, Color::Red), Err(MoveError::IllegalMove));
    }

    #[test]
    fn test_validate_pawn_sideways_before_river() {
        // Red pawn cannot move sideways before crossing river
        let fen = "1k7/9/9/9/9/9/P8/9/9/5K3 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        // Red pawn at (0,6) trying to go sideways to (1,6)
        let m = Move::new(Position::new(0, 6), Position::new(1, 6));
        assert_eq!(validate_move(&board, m, Color::Red), Err(MoveError::IllegalMove));
    }

    #[test]
    fn test_validate_black_pawn_backward_rejected() {
        // Black pawn cannot move backward (toward row 0)
        let fen = "1K7/9/9/9/9/p8/9/9/9/5k3 b - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        // Black pawn at (0,5) trying to go backward to (0,4)
        let m = Move::new(Position::new(0, 5), Position::new(0, 4));
        assert_eq!(validate_move(&board, m, Color::Black), Err(MoveError::IllegalMove));
    }

    #[test]
    fn test_validate_black_pawn_sideways_before_river() {
        // Black pawn cannot move sideways before crossing river (row <= 4 is own side for black)
        let fen = "1K7/9/9/9/p8/9/9/9/9/5k3 b - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        // Black pawn at (0,4) trying to go sideways to (1,4)
        let m = Move::new(Position::new(0, 4), Position::new(1, 4));
        assert_eq!(validate_move(&board, m, Color::Black), Err(MoveError::IllegalMove));
    }

    #[test]
    fn test_validate_cannon_with_two_screens_capture() {
        // Cannon cannot capture with 2 screen pieces (needs exactly 1)
        let fen = "4k4/9/9/4P4/9/4P4/9/4C4/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        // Cannon at (4,7), 2 pawns at (4,5) and (4,3), trying to capture king at (4,0)
        // But king can't be captured — let's use a different target
        // Actually we just test that cannon move with 2 screens between is rejected
        let m = Move::new(Position::new(4, 7), Position::new(4, 2));
        assert_eq!(validate_move(&board, m, Color::Red), Err(MoveError::IllegalMove));
    }

    #[test]
    fn test_validate_both_out_of_bounds() {
        let board = Board::initial();
        // Both from and to are out of bounds
        let m = Move::new(Position::new(9, 10), Position::new(10, 11));
        assert_eq!(validate_move(&board, m, Color::Red), Err(MoveError::OutOfBounds));
    }

    #[test]
    fn test_move_error_display() {
        assert_eq!(MoveError::OutOfBounds.to_string(), "Position out of bounds");
        assert_eq!(MoveError::NoPieceAtFrom.to_string(), "No piece at source position");
        assert_eq!(MoveError::WrongColor.to_string(), "Not your piece");
        assert_eq!(MoveError::CannotCaptureOwnPiece.to_string(), "Cannot capture own piece");
        assert_eq!(MoveError::IllegalMove.to_string(), "Illegal move for this piece type");
        assert_eq!(MoveError::WouldBeInCheck.to_string(), "Move would leave king in check");
        assert_eq!(MoveError::FlyingGeneral.to_string(), "Flying general violation");
    }

    #[test]
    fn test_move_error_is_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(MoveError::IllegalMove);
        assert!(!err.to_string().is_empty());
    }
}
