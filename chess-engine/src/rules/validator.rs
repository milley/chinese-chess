use crate::board::Board;
use crate::board::Position;
use crate::pieces::{Color, PieceType, Move};
use crate::pieces::movement::*;
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
        if let (Some(mk), Some(ok)) = (my_king, opp_king) {
            if mk.col == ok.col {
                let min_row = mk.row.min(ok.row);
                let max_row = mk.row.max(ok.row);
                let blocked = (min_row + 1..max_row)
                    .any(|row| new_board.piece_at(Position::new(mk.col, row)).is_some());
                if !blocked {
                    return Err(MoveError::FlyingGeneral);
                }
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
    if eye_col < 0 || eye_col > 8 || eye_row < 0 || eye_row > 9 {
        return false;
    }
    let eye = Position::new(eye_col as u8, eye_row as u8);
    board.piece_at(eye).is_none()
}

/// 马: 走"日"字，不蹩腿
fn is_valid_knight_move(board: &Board, m: Move) -> bool {
    let dc = (m.to.col as i8 - m.from.col as i8).abs();
    let dr = (m.to.row as i8 - m.from.row as i8).abs();
    // 必须走"日"字
    if !((dc == 1 && dr == 2) || (dc == 2 && dr == 1)) {
        return false;
    }
    // 蹩马腿检测
    let leg_col: i8;
    let leg_row: i8;
    if dc == 2 {
        // 横向走2格，马腿在横向第1格
        leg_col = m.from.col as i8 + if m.to.col > m.from.col { 1 } else { -1 };
        leg_row = m.from.row as i8;
    } else {
        // 纵向走2格，马腿在纵向第1格
        leg_col = m.from.col as i8;
        leg_row = m.from.row as i8 + if m.to.row > m.from.row { 1 } else { -1 };
    }
    if leg_col < 0 || leg_col > 8 || leg_row < 0 || leg_row > 9 {
        return false;
    }
    board.piece_at(Position::new(leg_col as u8, leg_row as u8)).is_none()
}

/// 车: 直线任意距离，不能越子
fn is_valid_rook_move(board: &Board, m: Move) -> bool {
    // 必须在同一行或列
    if m.from.col != m.to.col && m.from.row != m.to.row {
        return false;
    }
    // 路径上不能有阻挡
    is_path_clear(board, m.from, m.to)
}

/// 炮: 移动同车 (无子阻挡)；吃子需隔一子
fn is_valid_cannon_move(board: &Board, m: Move, _color: Color) -> bool {
    // 必须在同一行或列
    if m.from.col != m.to.col && m.from.row != m.to.row {
        return false;
    }
    let count = count_pieces_between(board, m.from, m.to);
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

    // 只能走一步
    if dc + dr.abs() != 1 && (dc != 1 || dr != 0) && (dc != 0 || dr.abs() != 1) {
        return false;
    }

    if dc == 1 {
        // 横向移动: 必须已过河，且在同一行
        crossed && m.from.row == m.to.row
    } else {
        // 纵向移动: 必须前进
        let forward = pawn_forward_offset(color);
        dr == forward
    }
}

/// 检查两点之间的直线路径是否畅通 (不包含起点和终点)
fn is_path_clear(board: &Board, from: Position, to: Position) -> bool {
    if from.col == to.col {
        let min_row = from.row.min(to.row);
        let max_row = from.row.max(to.row);
        for row in (min_row + 1)..max_row {
            if board.piece_at(Position::new(from.col, row)).is_some() {
                return false;
            }
        }
    } else {
        let min_col = from.col.min(to.col);
        let max_col = from.col.max(to.col);
        for col in (min_col + 1)..max_col {
            if board.piece_at(Position::new(col, from.row)).is_some() {
                return false;
            }
        }
    }
    true
}

/// 计算两点之间直线路径上的棋子数 (不包含起点和终点)
fn count_pieces_between(board: &Board, from: Position, to: Position) -> usize {
    let mut count = 0;
    if from.col == to.col {
        let min_row = from.row.min(to.row);
        let max_row = from.row.max(to.row);
        for row in (min_row + 1)..max_row {
            if board.piece_at(Position::new(from.col, row)).is_some() {
                count += 1;
            }
        }
    } else {
        let min_col = from.col.min(to.col);
        let max_col = from.col.max(to.col);
        for col in (min_col + 1)..max_col {
            if board.piece_at(Position::new(col, from.row)).is_some() {
                count += 1;
            }
        }
    }
    count
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
