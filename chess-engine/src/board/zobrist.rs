//! Zobrist hashing for board position identification.
//!
//! Uses incremental XOR-based hashing: each piece on each square has a unique
//! random key, and the side-to-move has its own key. The hash is updated
//! incrementally in `Board::make_move` / `Board::undo_move`.

use crate::pieces::{Color, PieceType, Piece};

/// Number of distinct piece+color combinations: 2 colors × 7 piece types = 14
const NUM_PIECE_KINDS: usize = 14;

/// Number of squares on the board: 9 × 10 = 90
const NUM_SQUARES: usize = 90;

/// Zobrist key table: `PIECE_KEYS[piece_kind_index][square_index]`
///
/// `piece_kind_index` = `color_index * 7 + piece_type_index`
/// - Red pieces: indices 0..7 (King=0, Advisor=1, ..., Pawn=6)
/// - Black pieces: indices 7..14 (King=7, Advisor=8, ..., Pawn=13)
static PIECE_KEYS: [[u64; NUM_SQUARES]; NUM_PIECE_KINDS] = generate_piece_keys();

/// Side-to-move key: XOR'd when it's Black's turn
const SIDE_KEY: u64 = 0x9E3779B97F4A7C15;

/// Get the piece kind index for Zobrist lookup.
#[inline]
pub fn piece_kind_index(piece: Piece) -> usize {
    let color_offset = match piece.color {
        Color::Red => 0,
        Color::Black => 7,
    };
    let type_index = match piece.piece_type {
        PieceType::King => 0,
        PieceType::Advisor => 1,
        PieceType::Bishop => 2,
        PieceType::Knight => 3,
        PieceType::Rook => 4,
        PieceType::Cannon => 5,
        PieceType::Pawn => 6,
    };
    color_offset + type_index
}

/// Get the Zobrist key for a piece on a specific square.
#[inline]
pub fn piece_key(piece: Piece, square_index: usize) -> u64 {
    PIECE_KEYS[piece_kind_index(piece)][square_index]
}

/// Get the side-to-move key.
#[inline]
pub fn side_key() -> u64 {
    SIDE_KEY
}

/// Simple xorshift64 PRNG for generating Zobrist keys at compile time.
const fn xorshift64(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

/// Generate all Zobrist piece keys at compile time.
const fn generate_piece_keys() -> [[u64; NUM_SQUARES]; NUM_PIECE_KINDS] {
    let mut keys = [[0u64; NUM_SQUARES]; NUM_PIECE_KINDS];
    let mut state: u64 = 0x1234567890ABCDEF; // Fixed seed

    let mut kind = 0;
    while kind < NUM_PIECE_KINDS {
        let mut sq = 0;
        while sq < NUM_SQUARES {
            keys[kind][sq] = xorshift64(&mut state);
            sq += 1;
        }
        kind += 1;
    }

    keys
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;

    #[test]
    fn test_zobrist_keys_are_nonzero() {
        // All keys should be non-zero (zero would mean a piece has no effect on hash)
        for kind in 0..NUM_PIECE_KINDS {
            for sq in 0..NUM_SQUARES {
                assert_ne!(PIECE_KEYS[kind][sq], 0, "Key at kind={}, sq={} is zero", kind, sq);
            }
        }
    }

    #[test]
    fn test_zobrist_keys_are_unique() {
        // All keys should be distinct (collision would degrade TT performance)
        let mut seen = std::collections::HashSet::new();
        for kind in 0..NUM_PIECE_KINDS {
            for sq in 0..NUM_SQUARES {
                let key = PIECE_KEYS[kind][sq];
                assert!(seen.insert(key), "Duplicate key at kind={}, sq={}", kind, sq);
            }
        }
    }

    #[test]
    fn test_piece_kind_index_red() {
        let red_king = Piece::new(Color::Red, PieceType::King);
        assert_eq!(piece_kind_index(red_king), 0);
        let red_pawn = Piece::new(Color::Red, PieceType::Pawn);
        assert_eq!(piece_kind_index(red_pawn), 6);
    }

    #[test]
    fn test_piece_kind_index_black() {
        let black_king = Piece::new(Color::Black, PieceType::King);
        assert_eq!(piece_kind_index(black_king), 7);
        let black_pawn = Piece::new(Color::Black, PieceType::Pawn);
        assert_eq!(piece_kind_index(black_pawn), 13);
    }

    #[test]
    fn test_zobrist_hash_initial_position_consistent() {
        // Same position should always produce the same hash
        let board1 = Board::initial();
        let board2 = Board::initial();
        assert_eq!(board1.zobrist_hash(), board2.zobrist_hash());
    }

    #[test]
    fn test_zobrist_hash_different_positions() {
        let board1 = Board::initial();
        let fen2 = "1k7/9/9/9/9/9/9/9/1n2R4/5K3 w - - 0 1";
        let board2 = Board::from_fen(fen2).unwrap();
        assert_ne!(board1.zobrist_hash(), board2.zobrist_hash());
    }

    #[test]
    fn test_zobrist_hash_make_undo_restores() {
        let board = Board::initial();
        let mut board = board;
        let hash_before = board.zobrist_hash();

        // Make a move
        let m = crate::pieces::Move::new(
            crate::board::Position::new(1, 7),
            crate::board::Position::new(4, 7),
        );
        let captured = board.make_move(m);
        let hash_after = board.zobrist_hash();
        assert_ne!(hash_before, hash_after, "Hash should change after a move");

        // Undo
        board.undo_move(m, captured);
        assert_eq!(board.zobrist_hash(), hash_before, "Hash should be restored after undo");
    }

    #[test]
    fn test_zobrist_hash_side_to_move() {
        // Same piece placement but different side to move should have different hashes
        let fen_red = "4k4/9/9/9/9/9/9/9/9/4K4 w - - 0 1";
        let fen_black = "4k4/9/9/9/9/9/9/9/9/4K4 b - - 0 1";
        let board_red = Board::from_fen(fen_red).unwrap();
        let board_black = Board::from_fen(fen_black).unwrap();
        assert_ne!(board_red.zobrist_hash(), board_black.zobrist_hash());
        // Difference should be exactly SIDE_KEY
        assert_eq!(
            board_red.zobrist_hash() ^ board_black.zobrist_hash(),
            SIDE_KEY,
        );
    }

    #[test]
    fn test_zobrist_hash_multiple_make_undo() {
        let board = Board::initial();
        let mut board = board;
        let hash_before = board.zobrist_hash();

        // Make several moves and undo them all
        let moves = [
            crate::pieces::Move::new(crate::board::Position::new(1, 7), crate::board::Position::new(4, 7)),
            crate::pieces::Move::new(crate::board::Position::new(1, 0), crate::board::Position::new(2, 2)),
        ];

        let mut history = Vec::new();
        for &m in &moves {
            let captured = board.make_move(m);
            history.push((m, captured));
        }

        // Hash should be different after moves
        assert_ne!(board.zobrist_hash(), hash_before);

        // Undo in reverse
        for (m, captured) in history.into_iter().rev() {
            board.undo_move(m, captured);
        }

        assert_eq!(board.zobrist_hash(), hash_before, "Hash should be restored after all undos");
    }

    #[test]
    fn test_zobrist_hash_capture_and_undo() {
        // Position where a capture happens
        let fen = "1k5R1/9/9/9/9/9/9/9/9/4K4 w - - 0 1";
        let board = Board::from_fen(fen).unwrap();
        let mut board = board;
        let hash_before = board.zobrist_hash();

        // Rook captures king at (1,0)
        let m = crate::pieces::Move::new(crate::board::Position::new(7, 0), crate::board::Position::new(1, 0));
        let captured = board.make_move(m);

        // Hash should change
        assert_ne!(board.zobrist_hash(), hash_before);

        // Undo
        board.undo_move(m, captured);
        assert_eq!(board.zobrist_hash(), hash_before, "Hash should be restored after capture undo");
    }
}
