use crate::pieces::Color;

/// Whether a player is in their main game time or in byoyomi.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimePhase {
    /// Normal game time is still remaining.
    Main,
    /// Game time expired; player is now in byoyomi per-move countdown.
    Byoyomi,
}

/// Per-player time state.
#[derive(Clone, Debug)]
pub struct PlayerTime {
    /// Remaining main game time in seconds (0 means expired).
    pub remaining: i32,
    /// Which phase the player is in.
    pub phase: TimePhase,
    /// Seconds consumed on the current move (reset each move).
    pub move_elapsed: i32,
}

/// Result of a time tick (1-second elapse check).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TickResult {
    /// No timeout occurred. Returns current remaining times and active phase.
    Ok {
        red_remaining: i32,
        black_remaining: i32,
        active_phase: TimePhase,
    },
    /// A player timed out. Contains the color that lost.
    Timeout(Color),
}

/// Encapsulates all server-side time control logic.
///
/// Time control rules:
/// - **局时 (game time)**: Each player has a total time bank. It counts down only on their turn.
///   When it reaches 0, the player enters byoyomi (if configured) or loses immediately.
/// - **步时 (move time limit)**: Per-move time limit. If a player exceeds this on any single move,
///   they lose. This is independent of game time and takes priority.
/// - **读秒 (byoyomi)**: After game time expires, each move must be completed within the byoyomi
///   period. The byoyomi timer resets at the start of each move. If a move is not made within
///   byoyomi seconds, the player loses.
#[derive(Clone, Debug)]
pub struct TimeControl {
    /// Total game time in seconds per player. None = no game time limit.
    pub time_control: Option<i32>,
    /// Per-move time limit in seconds. None = no per-move limit.
    pub move_time_limit: Option<i32>,
    /// Byoyomi period in seconds. None = no byoyomi.
    pub byoyomi: Option<i32>,
    /// Red's time state.
    pub red: PlayerTime,
    /// Black's time state.
    pub black: PlayerTime,
    /// Whether time control is active (game has started and is in progress).
    pub active: bool,
}

impl TimeControl {
    /// Create a new TimeControl with both players starting at full time.
    pub fn new(time_control: Option<i32>, move_time_limit: Option<i32>, byoyomi: Option<i32>) -> Self {
        let remaining = time_control.unwrap_or(0);
        let phase = if time_control.is_none() && byoyomi.is_some() {
            // Unusual config: no main time, start directly in byoyomi
            TimePhase::Byoyomi
        } else {
            TimePhase::Main
        };

        Self {
            time_control,
            move_time_limit,
            byoyomi,
            red: PlayerTime {
                remaining,
                phase,
                move_elapsed: 0,
            },
            black: PlayerTime {
                remaining,
                phase,
                move_elapsed: 0,
            },
            active: false,
        }
    }

    /// Create a TimeControl restoring from persisted DB state (for room reload after server restart).
    pub fn new_with_state(
        time_control: Option<i32>,
        move_time_limit: Option<i32>,
        byoyomi: Option<i32>,
        red_remaining: i32,
        black_remaining: i32,
    ) -> Self {
        let red_phase = if red_remaining > 0 {
            TimePhase::Main
        } else if byoyomi.is_some() {
            TimePhase::Byoyomi
        } else {
            TimePhase::Main // no byoyomi, already at 0 — will timeout on next tick
        };

        let black_phase = if black_remaining > 0 {
            TimePhase::Main
        } else if byoyomi.is_some() {
            TimePhase::Byoyomi
        } else {
            TimePhase::Main
        };

        Self {
            time_control,
            move_time_limit,
            byoyomi,
            red: PlayerTime {
                remaining: red_remaining,
                phase: red_phase,
                move_elapsed: 0,
            },
            black: PlayerTime {
                remaining: black_remaining,
                phase: black_phase,
                move_elapsed: 0,
            },
            active: false,
        }
    }

    /// Activate time control (call when game transitions to "playing").
    pub fn activate(&mut self) {
        self.active = true;
    }

    /// Process a 1-second tick. Called by the timeout checker every second.
    ///
    /// Returns `TickResult::Timeout(color)` if a player has lost on time,
    /// or `TickResult::Ok` with current remaining times.
    pub fn tick(&mut self, side_to_move: Color) -> TickResult {
        if !self.active {
            return TickResult::Ok {
                red_remaining: self.red.remaining,
                black_remaining: self.black.remaining,
                active_phase: self.phase(side_to_move),
            };
        }

        let player = match side_to_move {
            Color::Red => &mut self.red,
            Color::Black => &mut self.black,
        };

        player.move_elapsed += 1;

        // Check move_time_limit FIRST — it takes priority over game time
        if let Some(mtl) = self.move_time_limit {
            if player.move_elapsed >= mtl {
                return TickResult::Timeout(side_to_move);
            }
        }

        // Handle game time / byoyomi
        match player.phase {
            TimePhase::Main => {
                // Only decrement main time if game time is configured
                if self.time_control.is_some() {
                    player.remaining -= 1;
                    if player.remaining <= 0 {
                        if self.byoyomi.is_some() {
                            // Transition to byoyomi
                            player.phase = TimePhase::Byoyomi;
                            player.remaining = 0;
                            player.move_elapsed = 0;
                        } else {
                            // No byoyomi — immediate timeout
                            return TickResult::Timeout(side_to_move);
                        }
                    }
                }
            }
            TimePhase::Byoyomi => {
                if let Some(byo) = self.byoyomi {
                    if player.move_elapsed >= byo {
                        return TickResult::Timeout(side_to_move);
                    }
                }
            }
        }

        TickResult::Ok {
            red_remaining: self.red.remaining,
            black_remaining: self.black.remaining,
            active_phase: self.phase(side_to_move),
        }
    }

    /// Called after a successful move. Resets the moving player's move_elapsed.
    /// In byoyomi phase, this effectively resets the byoyomi countdown for the next move.
    pub fn on_move_made(&mut self, side_that_moved: Color) {
        if !self.active {
            return;
        }
        let player = match side_that_moved {
            Color::Red => &mut self.red,
            Color::Black => &mut self.black,
        };
        player.move_elapsed = 0;
    }

    /// Get the effective remaining time for a player (for display).
    /// In main phase, returns `remaining`. In byoyomi phase, returns `byoyomi - move_elapsed`.
    pub fn remaining(&self, color: Color) -> i32 {
        let player = match color {
            Color::Red => &self.red,
            Color::Black => &self.black,
        };
        match player.phase {
            TimePhase::Main => player.remaining,
            TimePhase::Byoyomi => {
                self.byoyomi
                    .map(|byo| (byo - player.move_elapsed).max(0))
                    .unwrap_or(0)
            }
        }
    }

    /// Get the current phase for a player.
    pub fn phase(&self, color: Color) -> TimePhase {
        match color {
            Color::Red => self.red.phase,
            Color::Black => self.black.phase,
        }
    }

    /// Check if any time control is configured.
    pub fn is_configured(&self) -> bool {
        self.time_control.is_some() || self.move_time_limit.is_some() || self.byoyomi.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_time_control() {
        let mut tc = TimeControl::new(None, None, None);
        tc.activate();
        // Tick should always return Ok with no timeout
        for _ in 0..100 {
            let result = tc.tick(Color::Red);
            assert!(matches!(result, TickResult::Ok { .. }));
        }
    }

    #[test]
    fn test_game_time_expires_no_byoyomi() {
        let mut tc = TimeControl::new(Some(3), None, None);
        tc.activate();

        // Tick 2 times — still ok
        assert!(matches!(tc.tick(Color::Red), TickResult::Ok { .. }));
        assert!(matches!(tc.tick(Color::Red), TickResult::Ok { .. }));

        // 3rd tick — Red's time reaches 0, no byoyomi → timeout
        let result = tc.tick(Color::Red);
        assert_eq!(result, TickResult::Timeout(Color::Red));
    }

    #[test]
    fn test_game_time_expires_enters_byoyomi() {
        let mut tc = TimeControl::new(Some(3), None, Some(5));
        tc.activate();

        // Tick 3 times — Red's main time expires, enters byoyomi
        for _ in 0..3 {
            let result = tc.tick(Color::Red);
            assert!(matches!(result, TickResult::Ok { .. }), "Should not timeout during main time with byoyomi");
        }

        // Red should now be in byoyomi phase
        assert_eq!(tc.phase(Color::Red), TimePhase::Byoyomi);
        assert_eq!(tc.red.remaining, 0);
        assert_eq!(tc.red.move_elapsed, 0); // reset on entering byoyomi

        // Tick 4 more times in byoyomi (move_elapsed goes 1..4)
        for i in 1..=4 {
            let result = tc.tick(Color::Red);
            assert!(matches!(result, TickResult::Ok { .. }), "Tick {} should be ok", i);
        }

        // 5th byoyomi tick (move_elapsed=5 >= byoyomi=5) → timeout
        let result = tc.tick(Color::Red);
        assert_eq!(result, TickResult::Timeout(Color::Red));
    }

    #[test]
    fn test_byoyomi_resets_on_move() {
        let mut tc = TimeControl::new(Some(1), None, Some(5));
        tc.activate();

        // Tick 1 time — Red's main time expires, enters byoyomi
        let result = tc.tick(Color::Red);
        assert!(matches!(result, TickResult::Ok { .. }));
        assert_eq!(tc.phase(Color::Red), TimePhase::Byoyomi);

        // Tick 3 more times in byoyomi (move_elapsed = 3)
        for _ in 0..3 {
            let result = tc.tick(Color::Red);
            assert!(matches!(result, TickResult::Ok { .. }));
        }
        assert_eq!(tc.red.move_elapsed, 3);

        // Red makes a move — byoyomi resets
        tc.on_move_made(Color::Red);
        assert_eq!(tc.red.move_elapsed, 0);

        // Now it's Black's turn. Tick Black a few times.
        for _ in 0..2 {
            let result = tc.tick(Color::Black);
            assert!(matches!(result, TickResult::Ok { .. }));
        }

        // Back to Red's turn. Red has fresh byoyomi (5 seconds).
        // Tick 4 times — still ok
        for _ in 0..4 {
            let result = tc.tick(Color::Red);
            assert!(matches!(result, TickResult::Ok { .. }));
        }

        // 5th tick in byoyomi → timeout
        let result = tc.tick(Color::Red);
        assert_eq!(result, TickResult::Timeout(Color::Red));
    }

    #[test]
    fn test_move_time_limit_timeout() {
        let mut tc = TimeControl::new(None, Some(5), None);
        tc.activate();

        // Tick 4 times — still ok
        for _ in 0..4 {
            let result = tc.tick(Color::Red);
            assert!(matches!(result, TickResult::Ok { .. }));
        }

        // 5th tick — move_time_limit exceeded
        let result = tc.tick(Color::Red);
        assert_eq!(result, TickResult::Timeout(Color::Red));
    }

    #[test]
    fn test_move_time_limit_takes_priority() {
        // Player has 600s game time but only 3s per move
        let mut tc = TimeControl::new(Some(600), Some(3), None);
        tc.activate();

        // Tick 2 times — still ok
        assert!(matches!(tc.tick(Color::Red), TickResult::Ok { .. }));
        assert!(matches!(tc.tick(Color::Red), TickResult::Ok { .. }));

        // 3rd tick — move_time_limit exceeded, even though game time is far from zero
        let result = tc.tick(Color::Red);
        assert_eq!(result, TickResult::Timeout(Color::Red));
    }

    #[test]
    fn test_time_only_ticks_on_active_side() {
        let mut tc = TimeControl::new(Some(5), None, None);
        tc.activate();

        // Tick 3 times for Red
        for _ in 0..3 {
            tc.tick(Color::Red);
        }

        // Red should have 2 seconds left, Black should still have 5
        assert_eq!(tc.red.remaining, 2);
        assert_eq!(tc.black.remaining, 5);

        // Simulate Red making a move, now it's Black's turn
        tc.on_move_made(Color::Red);

        // Tick 3 times for Black
        for _ in 0..3 {
            tc.tick(Color::Black);
        }

        // Red still has 2, Black now has 2
        assert_eq!(tc.red.remaining, 2);
        assert_eq!(tc.black.remaining, 2);
    }

    #[test]
    fn test_on_move_made_resets_move_elapsed() {
        let mut tc = TimeControl::new(Some(100), None, None);
        tc.activate();

        // Tick 5 times
        for _ in 0..5 {
            tc.tick(Color::Red);
        }
        assert_eq!(tc.red.move_elapsed, 5);

        // Move made resets move_elapsed
        tc.on_move_made(Color::Red);
        assert_eq!(tc.red.move_elapsed, 0);
    }

    #[test]
    fn test_inactive_time_control() {
        let mut tc = TimeControl::new(Some(3), None, None);
        // NOT activated

        // Tick should not decrement time
        for _ in 0..10 {
            let result = tc.tick(Color::Red);
            assert!(matches!(result, TickResult::Ok { .. }));
        }
        assert_eq!(tc.red.remaining, 3);
    }

    #[test]
    fn test_new_with_state_restores_from_db() {
        let tc = TimeControl::new_with_state(Some(600), None, None, 300, 500);

        assert_eq!(tc.red.remaining, 300);
        assert_eq!(tc.black.remaining, 500);
        assert_eq!(tc.red.phase, TimePhase::Main);
        assert_eq!(tc.black.phase, TimePhase::Main);
    }

    #[test]
    fn test_new_with_state_zero_remaining_with_byoyomi() {
        let tc = TimeControl::new_with_state(Some(600), None, Some(30), 0, 500);

        // Red at 0 with byoyomi → should be in Byoyomi phase
        assert_eq!(tc.red.remaining, 0);
        assert_eq!(tc.red.phase, TimePhase::Byoyomi);
        // Black still has time → Main phase
        assert_eq!(tc.black.remaining, 500);
        assert_eq!(tc.black.phase, TimePhase::Main);
    }

    #[test]
    fn test_effective_time_main_phase() {
        let tc = TimeControl::new(Some(600), None, None);
        // In main phase, remaining() returns the main time
        assert_eq!(tc.remaining(Color::Red), 600);
        assert_eq!(tc.remaining(Color::Black), 600);
    }

    #[test]
    fn test_effective_time_byoyomi_phase() {
        let mut tc = TimeControl::new(Some(1), None, Some(10));
        tc.activate();

        // Tick once to enter byoyomi
        tc.tick(Color::Red);
        assert_eq!(tc.phase(Color::Red), TimePhase::Byoyomi);

        // In byoyomi, remaining() returns byoyomi - move_elapsed
        assert_eq!(tc.remaining(Color::Red), 10); // move_elapsed was reset to 0

        // Tick 3 more times
        for _ in 0..3 {
            tc.tick(Color::Red);
        }
        assert_eq!(tc.remaining(Color::Red), 7); // 10 - 3 = 7
    }

    #[test]
    fn test_is_configured() {
        assert!(!TimeControl::new(None, None, None).is_configured());
        assert!(TimeControl::new(Some(600), None, None).is_configured());
        assert!(TimeControl::new(None, Some(30), None).is_configured());
        assert!(TimeControl::new(None, None, Some(10)).is_configured());
    }

    #[test]
    fn test_game_time_with_byoyomi_and_move_time_limit() {
        // Complex scenario: 5s game time, 3s per move, 10s byoyomi
        let mut tc = TimeControl::new(Some(5), Some(3), Some(10));
        tc.activate();

        // Tick 3 times for Red — move_time_limit hit first
        for _ in 0..3 {
            let result = tc.tick(Color::Red);
            // First 2 ticks are ok, 3rd should timeout due to move_time_limit
            if result == TickResult::Timeout(Color::Red) {
                // move_time_limit triggered
                return;
            }
        }
        // If we get here, the 3rd tick should have been a timeout
        panic!("Expected Timeout from move_time_limit after 3 ticks");
    }

    #[test]
    fn test_byoyomi_with_move_time_limit() {
        // After entering byoyomi, move_time_limit still applies
        let mut tc = TimeControl::new(Some(1), Some(8), Some(10));
        tc.activate();

        // Tick 1 — Red enters byoyomi
        let result = tc.tick(Color::Red);
        assert!(matches!(result, TickResult::Ok { .. }));
        assert_eq!(tc.phase(Color::Red), TimePhase::Byoyomi);

        // Tick 7 more — move_elapsed = 7, still under move_time_limit (8)
        for _ in 0..7 {
            let result = tc.tick(Color::Red);
            assert!(matches!(result, TickResult::Ok { .. }));
        }

        // 8th tick in byoyomi — move_time_limit exceeded
        let result = tc.tick(Color::Red);
        assert_eq!(result, TickResult::Timeout(Color::Red));
    }

    #[test]
    fn test_tick_result_ok_carries_correct_values() {
        let mut tc = TimeControl::new(Some(600), None, None);
        tc.activate();

        // Tick Red 3 times
        for _ in 0..3 {
            let result = tc.tick(Color::Red);
            match result {
                TickResult::Ok { red_remaining, black_remaining, active_phase } => {
                    assert_eq!(black_remaining, 600);
                    assert!(red_remaining <= 600);
                    assert_eq!(active_phase, TimePhase::Main);
                }
                TickResult::Timeout(_) => panic!("Should not timeout"),
            }
        }
        assert_eq!(tc.red.remaining, 597);
    }

    #[test]
    fn test_alternating_sides_tick() {
        // Simulate a game with alternating Red and Black moves
        let mut tc = TimeControl::new(Some(10), None, None);
        tc.activate();

        // Red move: tick 2 times, then Red makes a move
        for _ in 0..2 {
            tc.tick(Color::Red);
        }
        tc.on_move_made(Color::Red);

        // Black move: tick 3 times, then Black makes a move
        for _ in 0..3 {
            tc.tick(Color::Black);
        }
        tc.on_move_made(Color::Black);

        // Red: 10-2=8, Black: 10-3=7
        assert_eq!(tc.red.remaining, 8);
        assert_eq!(tc.black.remaining, 7);
    }

    #[test]
    fn test_remaining_in_byoyomi_decreases_each_tick() {
        let mut tc = TimeControl::new(Some(1), None, Some(5));
        tc.activate();

        // Tick once to enter byoyomi
        tc.tick(Color::Red);
        assert_eq!(tc.phase(Color::Red), TimePhase::Byoyomi);
        assert_eq!(tc.remaining(Color::Red), 5); // fresh byoyomi

        // Each tick should decrease the effective remaining
        tc.tick(Color::Red);
        assert_eq!(tc.remaining(Color::Red), 4);
        tc.tick(Color::Red);
        assert_eq!(tc.remaining(Color::Red), 3);
        tc.tick(Color::Red);
        assert_eq!(tc.remaining(Color::Red), 2);
        tc.tick(Color::Red);
        assert_eq!(tc.remaining(Color::Red), 1);

        // 5th byoyomi tick → timeout
        let result = tc.tick(Color::Red);
        assert_eq!(result, TickResult::Timeout(Color::Red));
    }

    #[test]
    fn test_byoyomi_only_no_main_time() {
        // Unusual config: no main time, start directly in byoyomi
        let mut tc = TimeControl::new(None, None, Some(3));
        tc.activate();

        assert_eq!(tc.phase(Color::Red), TimePhase::Byoyomi);
        assert_eq!(tc.remaining(Color::Red), 3);

        // Tick 2 times — still ok
        for _ in 0..2 {
            let result = tc.tick(Color::Red);
            assert!(matches!(result, TickResult::Ok { .. }));
        }
        assert_eq!(tc.remaining(Color::Red), 1);

        // 3rd tick → timeout
        let result = tc.tick(Color::Red);
        assert_eq!(result, TickResult::Timeout(Color::Red));
    }

    #[test]
    fn test_on_move_made_inactive_no_effect() {
        let mut tc = TimeControl::new(Some(600), None, None);
        // NOT activated — on_move_made should be a no-op
        tc.red.move_elapsed = 5;
        tc.on_move_made(Color::Red);
        // move_elapsed should NOT reset because time control is not active
        assert_eq!(tc.red.move_elapsed, 5);
    }

    #[test]
    fn test_new_with_state_zero_remaining_no_byoyomi() {
        // Red at 0 with no byoyomi — should still be in Main phase (will timeout on next tick)
        let tc = TimeControl::new_with_state(Some(600), None, None, 0, 300);
        assert_eq!(tc.red.remaining, 0);
        assert_eq!(tc.red.phase, TimePhase::Main); // no byoyomi, stays Main

        let mut tc = tc;
        tc.activate();
        // Next tick should timeout because remaining is 0 and no byoyomi
        let result = tc.tick(Color::Red);
        assert_eq!(result, TickResult::Timeout(Color::Red));
    }

    #[test]
    fn test_game_time_exactly_reaches_zero_with_byoyomi() {
        // When remaining goes to exactly 0, should enter byoyomi (not timeout)
        let mut tc = TimeControl::new(Some(5), None, Some(10));
        tc.activate();

        // Tick 5 times — remaining should go 4, 3, 2, 1, 0
        for _ in 0..5 {
            let result = tc.tick(Color::Red);
            assert!(matches!(result, TickResult::Ok { .. }), "Should enter byoyomi, not timeout");
        }

        // Red should be in byoyomi with remaining=0 and move_elapsed=0
        assert_eq!(tc.phase(Color::Red), TimePhase::Byoyomi);
        assert_eq!(tc.red.remaining, 0);
        assert_eq!(tc.red.move_elapsed, 0);
    }

    #[test]
    fn test_time_control_clone_preserves_state() {
        let mut tc = TimeControl::new(Some(600), Some(30), Some(10));
        tc.activate();
        tc.tick(Color::Red);
        tc.tick(Color::Red);

        let clone = tc.clone();
        assert_eq!(clone.red.remaining, tc.red.remaining);
        assert_eq!(clone.red.move_elapsed, tc.red.move_elapsed);
        assert_eq!(clone.red.phase, tc.red.phase);
        assert_eq!(clone.active, tc.active);
    }
}
