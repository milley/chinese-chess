use crate::pieces::Color;
use crate::board::Position;

/// 九宫范围
/// 红方九宫: col 3-5, row 7-9
/// 黑方九宫: col 3-5, row 0-2
pub fn is_in_palace(pos: Position, color: Color) -> bool {
    if pos.col < 3 || pos.col > 5 {
        return false;
    }
    match color {
        Color::Red => pos.row >= 7 && pos.row <= 9,
        Color::Black => pos.row <= 2,
    }
}

/// 河界检测
/// 红方侧: row 5-9
/// 黑方侧: row 0-4
pub fn is_on_own_side(row: u8, color: Color) -> bool {
    match color {
        Color::Red => row >= 5,
        Color::Black => row <= 4,
    }
}

/// 是否已过河
pub fn has_crossed_river(row: u8, color: Color) -> bool {
    match color {
        Color::Red => row <= 4,
        Color::Black => row >= 5,
    }
}

/// 马的蹩腿检测
/// 马走"日"字：先走一步正交方向（马腿），再走一步斜方向
/// 如果马腿位置有棋子，则不能走
pub fn knight_leg_positions(pos: Position) -> [(Position, Position); 8] {
    // (马腿位置, 目标位置) 的 8 个方向
    let col = pos.col as i8;
    let row = pos.row as i8;

    let offsets: [(i8, i8, i8, i8); 8] = [
        // (腿col偏移, 腿row偏移, 目标col偏移, 目标row偏移)
        (0, -1, -1, -2), // 上 -> 左上
        (0, -1, 1, -2),  // 上 -> 右上
        (0, 1, -1, 2),   // 下 -> 左下
        (0, 1, 1, 2),    // 下 -> 右下
        (-1, 0, -2, -1), // 左 -> 左上
        (-1, 0, -2, 1),  // 左 -> 左下
        (1, 0, 2, -1),   // 右 -> 右上
        (1, 0, 2, 1),    // 右 -> 右下
    ];

    let mut result = [(Position { col: 0, row: 0 }, Position { col: 0, row: 0 }); 8];
    for (i, &(lc, lr, tc, tr)) in offsets.iter().enumerate() {
        let leg_col = col + lc;
        let leg_row = row + lr;
        let target_col = col + tc;
        let target_row = row + tr;
        result[i] = (
            Position { col: leg_col as u8, row: leg_row as u8 },
            Position { col: target_col as u8, row: target_row as u8 },
        );
    }
    result
}

/// 象的塞眼检测
/// 象走"田"字：目标位置的中心即为象眼
pub fn bishop_eye_and_target(pos: Position) -> [(Position, Position); 4] {
    let col = pos.col as i8;
    let row = pos.row as i8;

    let offsets: [(i8, i8, i8, i8); 4] = [
        // (眼col偏移, 眼row偏移, 目标col偏移, 目标row偏移)
        (-1, -1, -2, -2), // 左上
        (1, -1, 2, -2),   // 右上
        (-1, 1, -2, 2),   // 左下
        (1, 1, 2, 2),     // 右下
    ];

    let mut result = [(Position { col: 0, row: 0 }, Position { col: 0, row: 0 }); 4];
    for (i, &(ec, er, tc, tr)) in offsets.iter().enumerate() {
        let eye_col = col + ec;
        let eye_row = row + er;
        let target_col = col + tc;
        let target_row = row + tr;
        result[i] = (
            Position { col: eye_col as u8, row: eye_row as u8 },
            Position { col: target_col as u8, row: target_row as u8 },
        );
    }
    result
}

/// 兵的前进方向偏移
pub fn pawn_forward_offset(color: Color) -> i8 {
    match color {
        Color::Red => -1, // 红方向上 (row 递减)
        Color::Black => 1, // 黑方向下 (row 递增)
    }
}