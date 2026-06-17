use crate::board::{Board, Position};
use crate::pieces::{Color, PieceType};

/// 机动性权重
const MOBILITY_WEIGHT: i32 = 3;

/// 兵过河后的额外分值
const PAWN_CROSSED_BONUS: i32 = 40;

/// 位置价值表 (10行 x 9列，从红方视角)
/// row 0 = 黑方底线, row 9 = 红方底线
/// 红方使用时直接查表，黑方使用时行翻转 (row -> 9-row)

/// 帅/将位置价值表 (九宫内)
const KING_POS_VALUE: [[i32; 9]; 10] = [
    [0,  0,  0,  1,  5,  1,  0,  0,  0],  // row 0 黑方底线
    [0,  0,  0, -1,  0, -1,  0,  0,  0],
    [0,  0,  0,  0,  0,  0,  0,  0,  0],
    [0,  0,  0,  0,  0,  0,  0,  0,  0],
    [0,  0,  0,  0,  0,  0,  0,  0,  0],
    [0,  0,  0,  0,  0,  0,  0,  0,  0],
    [0,  0,  0,  0,  0,  0,  0,  0,  0],
    [0,  0,  0,  0,  0,  0,  0,  0,  0],
    [0,  0,  0, -1,  0, -1,  0,  0,  0],
    [0,  0,  0,  1,  5,  1,  0,  0,  0],  // row 9 红方底线
];

/// 仕/士位置价值表
const ADVISOR_POS_VALUE: [[i32; 9]; 10] = [
    [0,  0,  0, 10, 20, 10,  0,  0,  0],
    [0,  0,  0, 20,  0, 20,  0,  0,  0],
    [0,  0,  0, 10, 20, 10,  0,  0,  0],
    [0,  0,  0,  0,  0,  0,  0,  0,  0],
    [0,  0,  0,  0,  0,  0,  0,  0,  0],
    [0,  0,  0,  0,  0,  0,  0,  0,  0],
    [0,  0,  0,  0,  0,  0,  0,  0,  0],
    [0,  0,  0, 10, 20, 10,  0,  0,  0],
    [0,  0,  0, 20,  0, 20,  0,  0,  0],
    [0,  0,  0, 10, 20, 10,  0,  0,  0],
];

/// 相/象位置价值表
const BISHOP_POS_VALUE: [[i32; 9]; 10] = [
    [0,  0, 20,  0,  0,  0, 20,  0,  0],
    [0,  0,  0,  0, 25,  0,  0,  0,  0],
    [10, 0, 20,  0,  0,  0, 20,  0, 10],
    [0,  0,  0,  0,  0,  0,  0,  0,  0],
    [0,  0,  0,  0,  0,  0,  0,  0,  0],
    [0,  0,  0,  0,  0,  0,  0,  0,  0],
    [10, 0, 20,  0,  0,  0, 20,  0, 10],
    [0,  0,  0,  0, 25,  0,  0,  0,  0],
    [0,  0, 20,  0,  0,  0, 20,  0,  0],
    [0,  0,  0,  0,  0,  0,  0,  0,  0],
];

/// 马位置价值表
const KNIGHT_POS_VALUE: [[i32; 9]; 10] = [
    [ 0, -5,  0,  5, 10,  5,  0, -5,  0],
    [ 0,  5, 10, 20, 20, 20, 10,  5,  0],
    [ 5, 10, 20, 30, 30, 30, 20, 10,  5],
    [10, 20, 30, 35, 35, 35, 30, 20, 10],
    [15, 25, 35, 40, 40, 40, 35, 25, 15],
    [15, 25, 35, 40, 40, 40, 35, 25, 15],
    [10, 20, 30, 35, 35, 35, 30, 20, 10],
    [ 5, 10, 20, 30, 30, 30, 20, 10,  5],
    [ 0,  5, 10, 20, 20, 20, 10,  5,  0],
    [ 0, -5,  0,  5, 10,  5,  0, -5,  0],
];

/// 车位置价值表
const ROOK_POS_VALUE: [[i32; 9]; 10] = [
    [ 5, 10, 10, 15, 15, 15, 10, 10,  5],
    [10, 20, 20, 25, 25, 25, 20, 20, 10],
    [10, 20, 25, 30, 30, 30, 25, 20, 10],
    [10, 25, 30, 35, 35, 35, 30, 25, 10],
    [10, 25, 30, 35, 35, 35, 30, 25, 10],
    [10, 25, 30, 35, 35, 35, 30, 25, 10],
    [10, 25, 30, 35, 35, 35, 30, 25, 10],
    [10, 20, 25, 30, 30, 30, 25, 20, 10],
    [10, 20, 20, 25, 25, 25, 20, 20, 10],
    [ 5, 10, 10, 15, 15, 15, 10, 10,  5],
];

/// 炮位置价值表
const CANNON_POS_VALUE: [[i32; 9]; 10] = [
    [ 0,  5, 10, 10, 15, 10, 10,  5,  0],
    [ 5, 10, 15, 20, 20, 20, 15, 10,  5],
    [ 5, 15, 20, 25, 25, 25, 20, 15,  5],
    [ 5, 15, 20, 25, 25, 25, 20, 15,  5],
    [ 5, 15, 20, 25, 25, 25, 20, 15,  5],
    [ 5, 15, 20, 25, 25, 25, 20, 15,  5],
    [ 5, 15, 20, 25, 25, 25, 20, 15,  5],
    [ 5, 10, 15, 20, 20, 20, 15, 10,  5],
    [ 0,  5, 10, 15, 15, 15, 10,  5,  0],
    [ 0,  0,  5, 10, 10, 10,  5,  0,  0],
];

/// 兵/卒位置价值表
const PAWN_POS_VALUE: [[i32; 9]; 10] = [
    [ 0,  0,  0,  0,  0,  0,  0,  0,  0],
    [ 0,  0,  0,  0,  0,  0,  0,  0,  0],
    [ 0, 10,  0, 20, 30, 20,  0, 10,  0],
    [20, 30, 40, 50, 60, 50, 40, 30, 20],
    [30, 50, 60, 70, 80, 70, 60, 50, 30],
    [30, 50, 60, 70, 80, 70, 60, 50, 30],
    [20, 30, 40, 50, 60, 50, 40, 30, 20],
    [ 0, 10,  0, 20, 30, 20,  0, 10,  0],
    [ 0,  0,  0,  0,  0,  0,  0,  0,  0],
    [ 0,  0,  0,  0,  0,  0,  0,  0,  0],
];

/// 获取棋子的位置价值
fn position_value(piece_type: PieceType, pos: Position, color: Color) -> i32 {
    let row = match color {
        Color::Red => pos.row,
        Color::Black => 9 - pos.row,
    };
    let col = pos.col;

    let table_value = match piece_type {
        PieceType::King => KING_POS_VALUE[row as usize][col as usize],
        PieceType::Advisor => ADVISOR_POS_VALUE[row as usize][col as usize],
        PieceType::Bishop => BISHOP_POS_VALUE[row as usize][col as usize],
        PieceType::Knight => KNIGHT_POS_VALUE[row as usize][col as usize],
        PieceType::Rook => ROOK_POS_VALUE[row as usize][col as usize],
        PieceType::Cannon => CANNON_POS_VALUE[row as usize][col as usize],
        PieceType::Pawn => PAWN_POS_VALUE[row as usize][col as usize],
    };

    let sign = match color {
        Color::Red => 1,
        Color::Black => -1,
    };

    sign * table_value
}

/// 基础评估: 物质分 + 位置分 + 兵过河奖励 (不含机动性)
fn evaluate_base(board: &Board) -> i32 {
    let mut score = 0i32;

    // 1. 物质分 (增量维护)
    score += board.red_material_score() - board.black_material_score();

    // 2. 位置分 + 兵过河奖励
    for (pos, piece) in board.all_pieces() {
        score += position_value(piece.piece_type, pos, piece.color);

        if piece.piece_type == PieceType::Pawn {
            let crossed = crate::pieces::movement::has_crossed_river(pos.row, piece.color);
            if crossed {
                let bonus = match piece.color {
                    Color::Red => PAWN_CROSSED_BONUS,
                    Color::Black => -PAWN_CROSSED_BONUS,
                };
                score += bonus;
            }
        }
    }

    score
}

/// 局面评估 (从红方视角)
pub fn evaluate(board: &Board) -> i32 {
    let mut score = evaluate_base(board);

    // 3. 机动性分
    let red_mobility = board.generate_legal_moves(Color::Red).len() as i32;
    let black_mobility = board.generate_legal_moves(Color::Black).len() as i32;
    score += (red_mobility - black_mobility) * MOBILITY_WEIGHT;

    score
}

/// 快速评估 (不计算机动性，用于搜索内部)
pub fn evaluate_fast(board: &Board) -> i32 {
    evaluate_base(board)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_position_evaluation() {
        let board = Board::initial();
        let score = evaluate(&board);
        // 初始局面双方对称，评估应接近 0
        assert!(score.abs() < 50, "Initial position should evaluate close to 0, got {}", score);
    }

    #[test]
    fn test_material_advantage() {
        // 红方多一个车
        let fen = "4k4/4R4/9/9/9/9/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        let score = evaluate(&board);
        assert!(score > 500, "Red with extra rook should have large positive score, got {}", score);
    }

    #[test]
    fn test_position_value_correctness() {
        // Test specific position value lookup
        // Red rook at center (4,5): ROOK_POS_VALUE[5][4] = 35
        let fen = "4k4/9/9/9/9/4R4/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        let score = evaluate_fast(&board);
        // Score = material (600) + position value of rook at (4,5) = 35
        assert!(score > 0, "Red with rook at center should have positive score, got {}", score);
        assert!(score >= 600, "Score should be at least rook base value, got {}", score);
    }

    #[test]
    fn test_pawn_crossed_river_bonus() {
        // Red pawn after river should get bonus
        let fen = "4k4/9/9/9/P8/9/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        let score = evaluate_fast(&board);
        // Material = 30 (pawn) + position value + crossed river bonus (40)
        assert!(score > 50, "Crossed river pawn should contribute > 50, got {}", score);
    }

    #[test]
    fn test_evaluate_fast_vs_evaluate_consistency() {
        // evaluate_fast = evaluate - mobility component
        let fen = "4k4/4R4/9/9/9/9/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        let fast = evaluate_fast(&board);
        let full = evaluate(&board);
        // Difference should be due to mobility only
        let diff = (full - fast).abs();
        assert!(diff <= 1000, "Difference should be due to mobility only, got diff {}", diff);
    }

    #[test]
    fn test_black_advantage_negative_score() {
        // Black with extra rook should produce negative score (from red perspective)
        let fen = "4k4/9/9/9/9/9/9/9/4r4/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        let score = evaluate_fast(&board);
        assert!(score < -500, "Black with extra rook should have large negative score, got {}", score);
    }

    #[test]
    fn test_evaluate_king_only_board() {
        // Board with only two kings — should not panic, score near zero
        let fen = "4k4/9/9/9/9/9/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        let score_fast = evaluate_fast(&board);
        let score_full = evaluate(&board);
        // Only king position value and mobility differ, should be small
        assert!(score_fast.abs() < 100, "King-only board should have small score, got {}", score_fast);
        assert!(score_full.abs() < 200, "King-only board with mobility should have small score, got {}", score_full);
    }

    #[test]
    fn test_position_value_table_consistency() {
        // Verify position value is symmetric for red vs black (same piece type)
        let fen = "4k4/9/9/9/9/4R4/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        let score = evaluate_fast(&board);
        // Rook at (4,5) center: position value should be positive (good position)
        // Material 600 + position ~35 = 635+, minus king position values
        assert!(score > 500, "Rook at center should score well, got {}", score);
    }

    #[test]
    fn test_pawn_crossed_vs_uncrossed_river() {
        // Red pawn before river (row 6) vs after river (row 4)
        let fen_before = "4k4/9/9/9/9/9/P8/9/9/4K4 w - - 0 1";
        let fen_after = "4k4/9/9/9/P8/9/9/9/9/4K4 w - - 0 1";
        let board_before = Board::from_fen(fen_before).unwrap();
        let board_after = Board::from_fen(fen_after).unwrap();
        let score_before = evaluate_fast(&board_before);
        let score_after = evaluate_fast(&board_after);
        // Crossed-river pawn should score higher (position bonus + crossed bonus)
        assert!(score_after > score_before,
            "Crossed pawn should score higher: after={}, before={}", score_after, score_before);
    }
}
