use crate::pieces::Color;
use crate::board::{Board, Position};

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

/// 检查两点之间的直线路径是否畅通 (不含起点和终点)
/// 两点必须在同一行或同一列
pub fn is_line_clear(board: &Board, from: Position, to: Position) -> bool {
    if from.col == to.col {
        let min_row = from.row.min(to.row);
        let max_row = from.row.max(to.row);
        for row in (min_row + 1)..max_row {
            if board.piece_at(Position::new(from.col, row)).is_some() {
                return false;
            }
        }
    } else if from.row == to.row {
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

/// 计算两点之间直线路径上的棋子数 (不含起点和终点)
/// 两点必须在同一行或同一列
pub fn count_pieces_on_line(board: &Board, from: Position, to: Position) -> usize {
    let mut count = 0;
    if from.col == to.col {
        let min_row = from.row.min(to.row);
        let max_row = from.row.max(to.row);
        for row in (min_row + 1)..max_row {
            if board.piece_at(Position::new(from.col, row)).is_some() {
                count += 1;
            }
        }
    } else if from.row == to.row {
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

/// 检查马是否能攻击到目标位置 (包含蹩腿检测)
/// 复用 knight_leg_positions 进行统一的蹩腿检测
pub fn can_knight_reach(board: &Board, from: Position, to: Position) -> bool {
    let dc = (to.col as i8 - from.col as i8).abs();
    let dr = (to.row as i8 - from.row as i8).abs();

    if !((dc == 1 && dr == 2) || (dc == 2 && dr == 1)) {
        return false;
    }

    // 使用 knight_leg_positions 查找匹配的 (腿, 目标) 对
    let legs = knight_leg_positions(from);
    for (leg, target) in legs {
        if target == to && leg.is_valid() {
            return board.piece_at(leg).is_none();
        }
    }
    false
}

/// 兵/卒是否能攻击到目标位置
pub fn can_pawn_attack(from: Position, target: Position, color: Color) -> bool {
    let dc = (target.col as i8 - from.col as i8).abs();
    let dr = (target.row as i8 - from.row as i8).abs();

    if dc + dr != 1 {
        return false; // 只能走一步
    }

    let crossed = has_crossed_river(from.row, color);
    let forward = pawn_forward_offset(color);

    if dc == 1 {
        // 横向移动，必须已过河
        crossed && from.row == target.row
    } else {
        // 纵向移动，必须前进
        (target.row as i8 - from.row as i8) == forward
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Position;

    // === is_in_palace tests ===

    #[test]
    fn test_red_palace_interior() {
        // Red palace: col 3-5, row 7-9
        for col in 3..=5u8 {
            for row in 7..=9u8 {
                assert!(is_in_palace(Position::new(col, row), Color::Red),
                    "Red palace should contain ({},{})", col, row);
            }
        }
    }

    #[test]
    fn test_red_palace_exterior() {
        // Outside red palace
        assert!(!is_in_palace(Position::new(2, 8), Color::Red), "col 2 is outside palace");
        assert!(!is_in_palace(Position::new(6, 8), Color::Red), "col 6 is outside palace");
        assert!(!is_in_palace(Position::new(4, 6), Color::Red), "row 6 is outside palace");
        assert!(!is_in_palace(Position::new(4, 0), Color::Red), "row 0 is outside palace");
    }

    #[test]
    fn test_black_palace_interior() {
        // Black palace: col 3-5, row 0-2
        for col in 3..=5u8 {
            for row in 0..=2u8 {
                assert!(is_in_palace(Position::new(col, row), Color::Black),
                    "Black palace should contain ({},{})", col, row);
            }
        }
    }

    #[test]
    fn test_black_palace_exterior() {
        assert!(!is_in_palace(Position::new(2, 1), Color::Black), "col 2 is outside palace");
        assert!(!is_in_palace(Position::new(6, 1), Color::Black), "col 6 is outside palace");
        assert!(!is_in_palace(Position::new(4, 3), Color::Black), "row 3 is outside palace");
        assert!(!is_in_palace(Position::new(4, 9), Color::Black), "row 9 is outside palace");
    }

    // === is_on_own_side tests ===

    #[test]
    fn test_red_own_side() {
        // Red side: row 5-9
        assert!(is_on_own_side(5, Color::Red), "row 5 is red side");
        assert!(is_on_own_side(9, Color::Red), "row 9 is red side");
        assert!(!is_on_own_side(4, Color::Red), "row 4 is not red side");
        assert!(!is_on_own_side(0, Color::Red), "row 0 is not red side");
    }

    #[test]
    fn test_black_own_side() {
        // Black side: row 0-4
        assert!(is_on_own_side(0, Color::Black), "row 0 is black side");
        assert!(is_on_own_side(4, Color::Black), "row 4 is black side");
        assert!(!is_on_own_side(5, Color::Black), "row 5 is not black side");
        assert!(!is_on_own_side(9, Color::Black), "row 9 is not black side");
    }

    // === has_crossed_river tests ===

    #[test]
    fn test_red_crossed_river() {
        // Red crossed: row <= 4
        assert!(has_crossed_river(0, Color::Red), "row 0 is crossed for red");
        assert!(has_crossed_river(4, Color::Red), "row 4 is crossed for red");
        assert!(!has_crossed_river(5, Color::Red), "row 5 is not crossed for red");
        assert!(!has_crossed_river(9, Color::Red), "row 9 is not crossed for red");
    }

    #[test]
    fn test_black_crossed_river() {
        // Black crossed: row >= 5
        assert!(has_crossed_river(5, Color::Black), "row 5 is crossed for black");
        assert!(has_crossed_river(9, Color::Black), "row 9 is crossed for black");
        assert!(!has_crossed_river(4, Color::Black), "row 4 is not crossed for black");
        assert!(!has_crossed_river(0, Color::Black), "row 0 is not crossed for black");
    }

    // === knight_leg_positions tests ===

    #[test]
    fn test_knight_leg_positions_center() {
        // Knight at e5 (4,5) - center of board
        let pos = Position::new(4, 5);
        let legs = knight_leg_positions(pos);

        // Verify all 8 directions produce valid leg/target pairs
        // (leg, target) where leg is the blocking square and target is the destination
        let expected: [(Position, Position); 8] = [
            // (leg, target)
            (Position::new(4, 4), Position::new(3, 3)), // up-left
            (Position::new(4, 4), Position::new(5, 3)), // up-right
            (Position::new(4, 6), Position::new(3, 7)), // down-left
            (Position::new(4, 6), Position::new(5, 7)), // down-right
            (Position::new(3, 5), Position::new(2, 4)), // left-up
            (Position::new(3, 5), Position::new(2, 6)), // left-down
            (Position::new(5, 5), Position::new(6, 4)), // right-up
            (Position::new(5, 5), Position::new(6, 6)), // right-down
        ];

        for i in 0..8 {
            assert!(legs.contains(&expected[i]),
                "Knight at (4,5) should have leg/target pair {:?}, got {:?}", expected[i], legs[i]);
        }
    }

    #[test]
    fn test_knight_leg_positions_corner() {
        // Knight at a0 (0,0) - corner, some targets will be off-board
        let pos = Position::new(0, 0);
        let legs = knight_leg_positions(pos);
        // Should still return 8 pairs (some with invalid positions)
        assert_eq!(legs.len(), 8);
        // Verify at least the valid ones: leg (1,0) -> target (2,1), leg (0,1) -> target (1,2)
        let valid_targets: Vec<Position> = legs.iter()
            .filter(|(_, t)| t.is_valid())
            .map(|(_, t)| *t)
            .collect();
        assert!(valid_targets.contains(&Position::new(2, 1)), "Knight at a0 can reach c1");
        assert!(valid_targets.contains(&Position::new(1, 2)), "Knight at a0 can reach b2");
    }

    // === bishop_eye_and_target tests ===

    #[test]
    fn test_bishop_eye_and_target_center() {
        // Bishop at e5 (4,5) - center of own side for red
        let pos = Position::new(4, 5);
        let eyes = bishop_eye_and_target(pos);

        let expected: [(Position, Position); 4] = [
            (Position::new(3, 4), Position::new(2, 3)), // left-up
            (Position::new(5, 4), Position::new(6, 3)), // right-up
            (Position::new(3, 6), Position::new(2, 7)), // left-down
            (Position::new(5, 6), Position::new(6, 7)), // right-down
        ];

        for i in 0..4 {
            assert!(eyes.contains(&expected[i]),
                "Bishop at (4,5) should have eye/target pair {:?}", expected[i]);
        }
    }

    #[test]
    fn test_bishop_eye_and_target_all_four_directions() {
        // Bishop at c7 (2,7)
        let pos = Position::new(2, 7);
        let eyes = bishop_eye_and_target(pos);
        assert_eq!(eyes.len(), 4);

        // Eye positions should be diagonally adjacent
        for (eye, target) in &eyes {
            let dc = (eye.col as i8 - pos.col as i8).abs();
            let dr = (eye.row as i8 - pos.row as i8).abs();
            assert_eq!(dc, 1, "Eye should be 1 col away");
            assert_eq!(dr, 1, "Eye should be 1 row away");

            let tdc = (target.col as i8 - pos.col as i8).abs();
            let tdr = (target.row as i8 - pos.row as i8).abs();
            assert_eq!(tdc, 2, "Target should be 2 cols away");
            assert_eq!(tdr, 2, "Target should be 2 rows away");
        }
    }

    // === pawn_forward_offset tests ===

    // === is_line_clear tests ===

    #[test]
    fn test_is_line_clear_vertical_no_pieces() {
        let fen = "4k4/9/9/9/9/9/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        assert!(is_line_clear(&board, Position::new(1, 2), Position::new(1, 7)));
    }

    #[test]
    fn test_is_line_clear_vertical_blocked() {
        let fen = "4k4/9/9/9/9/4P4/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        // e-file: pawn at (4,5) blocks between (4,2) and (4,7)
        assert!(!is_line_clear(&board, Position::new(4, 2), Position::new(4, 7)));
    }

    #[test]
    fn test_is_line_clear_horizontal_no_pieces() {
        let fen = "4k4/9/9/9/9/9/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        assert!(is_line_clear(&board, Position::new(0, 5), Position::new(8, 5)));
    }

    #[test]
    fn test_is_line_clear_horizontal_blocked() {
        let fen = "4k4/9/9/9/9/4P4/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        // Row 5: pawn at (4,5) blocks between (0,5) and (8,5)
        assert!(!is_line_clear(&board, Position::new(0, 5), Position::new(8, 5)));
    }

    #[test]
    fn test_is_line_clear_adjacent() {
        // Adjacent positions have no pieces between them, always clear
        let fen = "4k4/9/9/9/9/9/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        assert!(is_line_clear(&board, Position::new(4, 5), Position::new(4, 6)));
    }

    #[test]
    fn test_is_line_clear_diagonal() {
        let fen = "4k4/9/9/9/9/9/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        // Non-linear path: different row and column — always returns true (no pieces to check)
        assert!(is_line_clear(&board, Position::new(0, 0), Position::new(1, 1)));
    }

    // === count_pieces_on_line tests ===

    #[test]
    fn test_count_pieces_on_line_vertical_zero() {
        let fen = "4k4/9/9/9/9/9/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        assert_eq!(count_pieces_on_line(&board, Position::new(1, 2), Position::new(1, 7)), 0);
    }

    #[test]
    fn test_count_pieces_on_line_vertical_one() {
        let fen = "4k4/9/9/9/9/4P4/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        assert_eq!(count_pieces_on_line(&board, Position::new(4, 2), Position::new(4, 7)), 1);
    }

    #[test]
    fn test_count_pieces_on_line_vertical_two() {
        let fen = "4k4/9/9/4P4/9/4P4/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        assert_eq!(count_pieces_on_line(&board, Position::new(4, 2), Position::new(4, 7)), 2);
    }

    #[test]
    fn test_count_pieces_on_line_horizontal_zero() {
        let fen = "4k4/9/9/9/9/9/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        assert_eq!(count_pieces_on_line(&board, Position::new(0, 5), Position::new(8, 5)), 0);
    }

    #[test]
    fn test_count_pieces_on_line_horizontal_one() {
        let fen = "4k4/9/9/9/9/4P4/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        assert_eq!(count_pieces_on_line(&board, Position::new(0, 5), Position::new(8, 5)), 1);
    }

    #[test]
    fn test_count_pieces_on_line_diagonal() {
        let fen = "4k4/9/9/9/9/9/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        // Diagonal: not on same row or column, returns 0
        assert_eq!(count_pieces_on_line(&board, Position::new(0, 0), Position::new(8, 9)), 0);
    }

    // === can_knight_reach tests ===

    #[test]
    fn test_can_knight_reach_valid() {
        let fen = "4k4/9/9/9/9/4N4/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        // Knight at (4,5) can reach (3,3) — valid "日" move, no leg block
        assert!(can_knight_reach(&board, Position::new(4, 5), Position::new(3, 3)));
    }

    #[test]
    fn test_can_knight_reach_blocked_leg() {
        // Knight at e8 (4,7) with pawn at e9 (4,8) blocking the upward leg
        let fen = "4k4/9/9/9/9/9/9/4N4/4P4/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        // Knight at (4,7) trying to reach (3,9) — leg at (4,8) is blocked
        assert!(!can_knight_reach(&board, Position::new(4, 7), Position::new(3, 9)));
    }

    #[test]
    fn test_can_knight_reach_invalid_pattern() {
        let fen = "4k4/9/9/9/9/4N4/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        // Not a valid knight move pattern: same column, one row
        assert!(!can_knight_reach(&board, Position::new(4, 5), Position::new(4, 4)));
        // Diagonal: not a knight pattern
        assert!(!can_knight_reach(&board, Position::new(4, 5), Position::new(5, 6)));
        // Two steps same direction: not a knight pattern
        assert!(!can_knight_reach(&board, Position::new(4, 5), Position::new(4, 3)));
    }

    // === can_pawn_attack tests ===

    #[test]
    fn test_can_pawn_attack_red_forward() {
        // Red pawn at (4,5) attacking (4,4) — forward
        assert!(can_pawn_attack(Position::new(4, 5), Position::new(4, 4), Color::Red));
    }

    #[test]
    fn test_can_pawn_attack_red_backward_rejected() {
        // Red pawn cannot attack backward
        assert!(!can_pawn_attack(Position::new(4, 5), Position::new(4, 6), Color::Red));
    }

    #[test]
    fn test_can_pawn_attack_red_sideways_crossed() {
        // Red pawn after river (row 4) can attack sideways
        assert!(can_pawn_attack(Position::new(4, 4), Position::new(3, 4), Color::Red));
        assert!(can_pawn_attack(Position::new(4, 4), Position::new(5, 4), Color::Red));
    }

    #[test]
    fn test_can_pawn_attack_red_sideways_before_river_rejected() {
        // Red pawn before river (row 6) cannot attack sideways
        assert!(!can_pawn_attack(Position::new(4, 6), Position::new(3, 6), Color::Red));
    }

    #[test]
    fn test_can_pawn_attack_black_forward() {
        // Black pawn at (4,4) attacking (4,5) — forward for black
        assert!(can_pawn_attack(Position::new(4, 4), Position::new(4, 5), Color::Black));
    }

    #[test]
    fn test_can_pawn_attack_black_backward_rejected() {
        // Black pawn cannot attack backward
        assert!(!can_pawn_attack(Position::new(4, 4), Position::new(4, 3), Color::Black));
    }

    #[test]
    fn test_can_pawn_attack_black_sideways_crossed() {
        // Black pawn after river (row 5) can attack sideways
        assert!(can_pawn_attack(Position::new(4, 5), Position::new(3, 5), Color::Black));
    }

    #[test]
    fn test_can_pawn_attack_black_sideways_before_river_rejected() {
        // Black pawn before river (row 3) cannot attack sideways
        assert!(!can_pawn_attack(Position::new(4, 3), Position::new(3, 3), Color::Black));
    }

    #[test]
    fn test_can_pawn_attack_diagonal_rejected() {
        // Pawn cannot attack diagonally
        assert!(!can_pawn_attack(Position::new(4, 4), Position::new(3, 3), Color::Red));
    }

    #[test]
    fn test_pawn_forward_offset_red() {
        assert_eq!(pawn_forward_offset(Color::Red), -1, "Red pawns move upward (row decreases)");
    }

    #[test]
    fn test_pawn_forward_offset_black() {
        assert_eq!(pawn_forward_offset(Color::Black), 1, "Black pawns move downward (row increases)");
    }
}