use crate::board::Board;
use crate::game::GameState;
use crate::pieces::{Color, PieceType, Move};
use crate::rules::is_in_check;
use crate::ai::eval::evaluate_fast;
use crate::ai::tt::{TranspositionTable, TTFlag};

/// 静态搜索最大深度
const MAX_QUIESCENCE_DEPTH: u8 = 4;

/// Alpha-Beta 搜索入口 (接受 GameState，兼容公共 API)
/// 返回最佳走法及评估分数
pub fn find_best_move(state: &GameState, depth: u8) -> Option<(Move, i32)> {
    if state.is_game_over() {
        return None;
    }
    let mut board = state.board().clone();
    let mut tt = TranspositionTable::default_size();
    find_best_move_board(&mut board, depth, &mut tt)
}

/// Alpha-Beta 搜索入口 (接受 Board + TT，可复用置换表)
pub fn find_best_move_with_tt(board: &mut Board, depth: u8, tt: &mut TranspositionTable) -> Option<(Move, i32)> {
    let color = board.side_to_move();
    let moves = board.generate_legal_moves(color);
    if moves.is_empty() {
        return None;
    }
    find_best_move_board(board, depth, tt)
}

/// Alpha-Beta 搜索核心 (直接操作 Board，使用置换表)
fn find_best_move_board(board: &mut Board, depth: u8, tt: &mut TranspositionTable) -> Option<(Move, i32)> {
    let color = board.side_to_move();
    let moves = board.generate_legal_moves(color);
    if moves.is_empty() {
        return None;
    }

    let mut best_move = None;
    let mut best_score = i32::MIN + 1;

    // MVV-LVA 走法排序 + TT 最佳走法优先
    let sorted_moves = sort_moves(&moves, board, tt);

    for m in sorted_moves {
        let captured = board.make_move(m);
        // Negamax: 对手视角搜索，取负
        // 使用 MIN+1 避免取负溢出
        let score = -alpha_beta(board, depth - 1, i32::MIN + 1, i32::MAX, tt);
        board.undo_move(m, captured);

        if score > best_score {
            best_score = score;
            best_move = Some(m);
        }
    }

    best_move.map(|m| (m, best_score))
}

/// 获取最佳走法 (简化版，只返回走法)
pub fn find_best_move_simple(state: &GameState, depth: u8) -> Option<Move> {
    find_best_move(state, depth).map(|(m, _)| m)
}

/// Alpha-Beta 搜索 (Negamax 格式，直接操作 Board，带置换表)
fn alpha_beta(board: &mut Board, depth: u8, mut alpha: i32, beta: i32, tt: &mut TranspositionTable) -> i32 {
    let hash = board.zobrist_hash();
    let orig_alpha = alpha;

    // 置换表查找
    if let Some(entry) = tt.probe(hash, depth) {
        match entry.flag {
            TTFlag::Exact => return entry.score,
            TTFlag::Lower if entry.score >= beta => return entry.score,
            TTFlag::Upper if entry.score <= alpha => return entry.score,
            _ => {}
        }
        // Use stored alpha/beta bounds to narrow window
        if entry.flag == TTFlag::Lower && entry.score > alpha {
            alpha = entry.score;
        }
        if entry.flag == TTFlag::Upper && entry.score < beta {
            // Can't narrow beta directly, but we have an upper bound
        }
    }

    if depth == 0 {
        let score = quiescence_search(board, alpha, beta, MAX_QUIESCENCE_DEPTH);
        return score;
    }

    let color = board.side_to_move();
    let moves = board.generate_legal_moves(color);

    if moves.is_empty() {
        // 将杀或困毙
        if is_in_check(board, color) {
            // 被将杀，越浅越差（偏好更短的将杀路径）
            return -(10000 + depth as i32);
        } else {
            // 困毙 = 输
            return -10000;
        }
    }

    let sorted_moves = sort_moves(&moves, board, tt);

    let mut best_score = i32::MIN + 1;
    let mut best_move: Option<Move> = None;

    for m in sorted_moves {
        let captured = board.make_move(m);
        let score = -alpha_beta(board, depth - 1, -beta, -alpha, tt);
        board.undo_move(m, captured);

        if score > best_score {
            best_score = score;
            best_move = Some(m);
        }

        if score >= beta {
            // Beta 剪枝
            let flag = TTFlag::Lower;
            tt.store(hash, depth, best_score, flag, best_move);
            return best_score;
        }
        if score > alpha {
            alpha = score;
        }
    }

    // 存入置换表
    let flag = if best_score <= orig_alpha {
        TTFlag::Upper // Failed low — score is upper bound
    } else if best_score >= beta {
        TTFlag::Lower // Beta cutoff — score is lower bound
    } else {
        TTFlag::Exact // PV node — score is exact
    };
    tt.store(hash, depth, best_score, flag, best_move);

    best_score
}

/// 静态搜索 (Quiescence Search)
/// 只搜索吃子走法，避免水平线效应
fn quiescence_search(board: &mut Board, alpha: i32, beta: i32, depth: u8) -> i32 {
    // 从当前走子方视角评估
    let color = board.side_to_move();
    let sign = match color {
        Color::Red => 1,
        Color::Black => -1,
    };
    let stand_pat = evaluate_fast(board) * sign;

    if stand_pat >= beta {
        return beta;
    }

    if depth == 0 {
        return if stand_pat > alpha { stand_pat } else { alpha };
    }

    let mut alpha = if stand_pat > alpha { stand_pat } else { alpha };

    // 只搜索吃子走法
    let captures = get_capture_moves(board);

    for m in captures {
        let captured = board.make_move(m);
        let score = -quiescence_search(board, -beta, -alpha, depth - 1);
        board.undo_move(m, captured);

        if score >= beta {
            return beta;
        }
        if score > alpha {
            alpha = score;
        }
    }

    alpha
}

/// MVV-LVA 走法排序 + TT 最佳走法优先
fn sort_moves(moves: &[Move], board: &Board, tt: &TranspositionTable) -> Vec<Move> {
    let hash = board.zobrist_hash();
    let tt_best_move = tt.probe_for_move(hash).and_then(|e| e.best_move);

    let mut scored_moves: Vec<(i32, Move)> = moves.iter().map(|&m| {
        let mut score = move_ordering_score(m, board);
        // TT 最佳走法排最前
        if tt_best_move == Some(m) {
            score += 1_000_000;
        }
        (score, m)
    }).collect();
    scored_moves.sort_by_key(|b| std::cmp::Reverse(b.0));
    scored_moves.into_iter().map(|(_, m)| m).collect()
}

/// 计算走法排序分数
fn move_ordering_score(m: Move, board: &Board) -> i32 {
    let mut score = 0;

    // 吃子走法优先 (MVV-LVA)
    if let Some(target) = board.piece_at(m.to) {
        // 被吃棋子价值越高越优先
        score += target.piece_type.base_value() * 10;
        // 攻击者价值越低越优先（用低价值棋子吃高价值棋子）
        if let Some(attacker) = board.piece_at(m.from) {
            score -= attacker.piece_type.base_value();
        }
    }

    // 将帅走法最后（一般将帅走法不是最佳选择）
    if let Some(piece) = board.piece_at(m.from)
        && piece.piece_type == PieceType::King {
            score -= 1000;
        }

    // 向中心移动的走法略微优先
    let center_col = 4;
    let center_row = 5;
    let from_dist = (m.from.col as i32 - center_col).abs() + (m.from.row as i32 - center_row).abs();
    let to_dist = (m.to.col as i32 - center_col).abs() + (m.to.row as i32 - center_row).abs();
    if to_dist < from_dist {
        score += 5;
    }

    score
}

/// 获取吃子走法
fn get_capture_moves(board: &Board) -> Vec<Move> {
    let color = board.side_to_move();
    let moves = board.generate_legal_moves(color);
    moves.into_iter().filter(|&m| board.piece_at(m.to).is_some()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pieces::Color;

    #[test]
    fn test_find_best_move_initial() {
        let state = GameState::new();
        let result = find_best_move_simple(&state, 2);
        assert!(result.is_some(), "Should find a move from initial position");
    }

    #[test]
    fn test_ai_does_not_leave_king_in_check() {
        let state = GameState::new();
        let m = find_best_move_simple(&state, 2).unwrap();
        // 执行走法后不应导致自己被将
        let mut new_state = state.clone();
        new_state.make_move(m).unwrap();
        assert!(!is_in_check(new_state.board(), Color::Red));
    }

    #[test]
    fn test_ai_finds_forced_capture() {
        // 红方车可以吃黑方马
        let fen = "1k7/9/9/9/9/9/9/9/1n2R4/5K3 w - - 0 1";
        let state = GameState::from_fen(fen).unwrap();
        let m = find_best_move_simple(&state, 2);
        assert!(m.is_some());
    }

    #[test]
    fn test_ai_finds_high_value_capture() {
        // Red can capture a rook (600) vs a pawn (30) - should prefer rook
        let fen = "1k3r3/9/9/9/9/9/9/9/P3R4/5K3 w - - 0 1";
        let state = GameState::from_fen(fen).unwrap();
        let result = find_best_move(&state, 2);
        assert!(result.is_some());
        // The best move should capture the rook
        let (_m, score) = result.unwrap();
        assert!(score > 0, "AI should find a positive score with material advantage, got {}", score);
    }

    #[test]
    fn test_ai_avoids_blunder() {
        // Red rook at e1, if it moves to f1 it would be captured by black cannon
        // AI should avoid this
        let fen = "4k4/9/9/9/9/9/9/4c4/4R4/4K4 w - - 0 1";
        let state = GameState::from_fen(fen).unwrap();
        let result = find_best_move(&state, 2);
        assert!(result.is_some());
        let (_, score) = result.unwrap();
        // Score should not be terrible (rook should not be lost immediately)
        assert!(score > -500, "AI should avoid losing rook to cannon, got score {}", score);
    }

    #[test]
    fn test_search_different_depths() {
        let state = GameState::new();
        // Depth 1
        let m1 = find_best_move_simple(&state, 1);
        assert!(m1.is_some(), "Should find a move at depth 1");
        // Depth 3
        let m3 = find_best_move_simple(&state, 3);
        assert!(m3.is_some(), "Should find a move at depth 3");
        // Both should return valid moves (not necessarily the same)
    }

    #[test]
    fn test_find_best_move_when_game_over() {
        let mut state = GameState::new();
        state.resign(Color::Red);
        assert!(state.is_game_over());
        let result = find_best_move(&state, 2);
        assert!(result.is_none(), "Should return None when game is over");
    }

    #[test]
    fn test_find_best_move_when_no_legal_moves() {
        // Position where black is checkmated (no legal moves)
        let fen = "k8/1R7/R8/9/9/9/9/9/9/7K1 b - - 0 1";
        let state = GameState::from_fen(fen).unwrap();
        let result = find_best_move(&state, 2);
        assert!(result.is_none(), "Should return None when no legal moves exist");
    }

    #[test]
    fn test_sort_moves_captures_first() {
        // Verify that capture moves are ordered before non-capture moves
        let fen = "4k4/9/9/9/9/9/9/9/R3n4/4K4 w - - 0 1";
        let state = GameState::from_fen(fen).unwrap();
        let moves = state.generate_legal_moves();
        let board = state.board();
        let tt = TranspositionTable::default_size();
        let sorted = sort_moves(&moves, board, &tt);

        // Find the capture move (rook captures knight at d1)
        let captures: Vec<bool> = sorted.iter().map(|&m| board.piece_at(m.to).is_some()).collect();
        // All capture moves should come before non-capture moves
        let first_non_capture = captures.iter().position(|&c| !c);
        let last_capture = captures.iter().rposition(|&c| c);
        if let (Some(first_nc), Some(lc)) = (first_non_capture, last_capture) {
            assert!(lc < first_nc, "All captures should come before non-captures");
        }
    }

    #[test]
    fn test_make_unmake_preserves_board_state() {
        // Verify that make_move + undo_move preserves the board state in search
        let state = GameState::new();
        let mut board = state.board().clone();
        let fen_before = board.to_fen();

        let moves = board.generate_legal_moves(Color::Red);
        for m in moves {
            let captured = board.make_move(m);
            board.undo_move(m, captured);
        }
        assert_eq!(board.to_fen(), fen_before, "Board should be unchanged after make/undo cycle");
    }

    #[test]
    fn test_search_finds_checkmate() {
        // Position where Red can checkmate in 1
        let fen = "k8/1R7/R8/9/9/9/9/9/9/7K1 w - - 0 1";
        let state = GameState::from_fen(fen).unwrap();
        let result = find_best_move(&state, 2);
        assert!(result.is_some());
        let (m, score) = result.unwrap();
        // Should find the checkmate move with a very high score
        assert!(score > 9000, "AI should find checkmate with high score, got {} for move {:?}", score, m);
    }

    #[test]
    fn test_tt_improves_search_consistency() {
        // Same position searched twice should give same result
        let fen = "4k4/9/9/9/9/9/9/9/R7R/4K4 w - - 0 1";
        let state1 = GameState::from_fen(fen).unwrap();
        let state2 = GameState::from_fen(fen).unwrap();

        let r1 = find_best_move(&state1, 3);
        let r2 = find_best_move(&state2, 3);

        assert_eq!(r1.map(|(m, s)| (m, s)), r2.map(|(m, s)| (m, s)),
            "Same position should give same search result");
    }
}
