pub mod state;
pub mod time_control;

pub use state::{GameState, GameResult, GameEndReason};
pub use time_control::{TimeControl, TickResult, TimePhase, PlayerTime};