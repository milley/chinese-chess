pub mod board;
pub mod pieces;
pub mod rules;
pub mod game;
pub mod ai;

// 重导出常用类型
pub use board::{Board, Position};
pub use pieces::{Color, Piece, PieceType, Move};
pub use game::{GameState, GameResult, GameEndReason};
pub use rules::{is_checkmate, is_stalemate, is_in_check};
pub use rules::validator::{validate_move, MoveError};
pub use ai::search::find_best_move;
