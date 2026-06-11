pub mod check;
pub mod validator;

pub use check::{is_in_check, is_checkmate, is_stalemate, find_king};
pub use validator::{validate_move, MoveError};
