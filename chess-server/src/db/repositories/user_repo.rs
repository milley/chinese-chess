use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::User;

#[derive(Clone)]
pub struct UserRepository {
    pool: PgPool,
}

impl UserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, username: &str, password_hash: &str, display_name: Option<&str>) -> Result<User> {
        let user = sqlx::query_as::<_, User>(
            "INSERT INTO users (username, password_hash, display_name) VALUES ($1, $2, $3) RETURNING *"
        )
        .bind(username)
        .bind(password_hash)
        .bind(display_name)
        .fetch_one(&self.pool)
        .await?;
        Ok(user)
    }

    pub async fn find_by_username(&self, username: &str) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = $1")
            .bind(username)
            .fetch_optional(&self.pool)
            .await?;
        Ok(user)
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(user)
    }

    /// Find a user by ID within a transaction, using `SELECT ... FOR UPDATE` to
    /// acquire a row-level lock. This prevents concurrent transactions from
    /// reading/modifying the same user row until the current transaction commits
    /// or rolls back, eliminating stale-read race conditions in Elo calculations.
    ///
    /// This is an associated function (not a method) because it takes a generic
    /// executor (transaction reference) rather than using `&self.pool`.
    pub async fn find_by_id_for_update<'a, E>(executor: E, id: Uuid) -> Result<Option<User>>
    where
        E: sqlx::Executor<'a, Database = sqlx::Postgres>,
    {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(executor)
            .await?;
        Ok(user)
    }

    pub async fn update(&self, id: Uuid, display_name: Option<&str>) -> Result<User> {
        let user = sqlx::query_as::<_, User>(
            "UPDATE users SET display_name = $1, updated_at = NOW() WHERE id = $2 RETURNING *"
        )
        .bind(display_name)
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        Ok(user)
    }

    pub async fn delete(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list(&self, page: i64, page_size: i64) -> Result<Vec<User>> {
        let offset = (page - 1) * page_size;
        let users = sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY created_at DESC LIMIT $1 OFFSET $2")
            .bind(page_size)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        Ok(users)
    }
}

/// Minimum Elo rating (floor). Prevents ratings from going negative
/// even after many consecutive losses.
const RATING_FLOOR: i32 = 100;

/// Calculate new Elo rating with variable K-factor and rating floor.
///
/// K-factor varies by player experience:
/// - K=40 for new players (fewer than 30 games) — faster convergence to true rating
/// - K=20 for established players (30+ games) — more stable ratings
///
/// The rating floor ensures no player drops below RATING_FLOOR (100).
pub fn calculate_new_rating(rating: i32, opponent_rating: i32, score: f64, total_games: i32) -> i32 {
    let expected = 1.0 / (1.0 + 10_f64.powi((opponent_rating - rating) / 400));
    let k = if total_games < 30 { 40.0 } else { 20.0 };
    let new_rating = (rating as f64 + k * (score - expected)).round() as i32;
    new_rating.max(RATING_FLOOR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_new_rating_win() {
        // Equal ratings, winner: rating should increase
        let new_rating = calculate_new_rating(1500, 1500, 1.0, 30);
        assert!(new_rating > 1500, "Winning against equal rating should increase rating, got {}", new_rating);
        // K=20, expected=0.5, gain = 20*(1-0.5) = 10
        assert_eq!(new_rating, 1510);
    }

    #[test]
    fn test_calculate_new_rating_win_new_player() {
        // New player (K=40) gets bigger gain
        let new_rating = calculate_new_rating(1500, 1500, 1.0, 5);
        assert!(new_rating > 1500);
        // K=40, gain = 40*(1-0.5) = 20
        assert_eq!(new_rating, 1520);
    }

    #[test]
    fn test_calculate_new_rating_loss() {
        // Equal ratings, loser: rating should decrease
        let new_rating = calculate_new_rating(1500, 1500, 0.0, 30);
        assert!(new_rating < 1500, "Losing against equal rating should decrease rating, got {}", new_rating);
        // K=20, loss = 20*(0-0.5) = -10
        assert_eq!(new_rating, 1490);
    }

    #[test]
    fn test_calculate_new_rating_draw() {
        // Equal ratings, draw: rating should stay the same
        let new_rating = calculate_new_rating(1500, 1500, 0.5, 30);
        assert_eq!(new_rating, 1500, "Drawing against equal rating should not change rating");
    }

    #[test]
    fn test_calculate_new_rating_upset_win() {
        // Low-rated player beating high-rated player: big gain
        let new_rating = calculate_new_rating(1200, 1800, 1.0, 30);
        let gain = new_rating - 1200;
        assert!(gain > 10, "Upset win should have large gain, got {}", gain);
    }

    #[test]
    fn test_calculate_new_rating_expected_win() {
        // High-rated player beating low-rated player: small gain
        let new_rating = calculate_new_rating(1800, 1200, 1.0, 30);
        let gain = new_rating - 1800;
        assert!(gain < 10, "Expected win should have small gain, got {}", gain);
    }

    #[test]
    fn test_rating_floor_prevents_below_minimum() {
        // Even against a very low opponent, many losses should not drop below floor.
        // Simulate by starting near the floor and losing against a near-floor opponent
        // where the expected score is ~0.5, giving maximum loss per game.
        let new_rating = calculate_new_rating(RATING_FLOOR, RATING_FLOOR, 0.0, 5);
        // At RATING_FLOOR vs RATING_FLOOR, expected=0.5, loss = K*0.5 = 20 (K=40)
        // 100 - 20 = 80, but floor should clamp to 100
        assert_eq!(new_rating, RATING_FLOOR, "Rating should not go below floor");
    }

    #[test]
    fn test_rating_floor_clamps_large_loss() {
        // A player at 105 losing against equal rating with K=40
        // loss = 40 * (0 - 0.5) = -20, so 105 - 20 = 85, clamped to 100
        let new_rating = calculate_new_rating(105, 105, 0.0, 5);
        assert_eq!(new_rating, RATING_FLOOR, "Rating should be clamped to floor after large loss");
    }

    #[test]
    fn test_rating_floor_exact() {
        // A player exactly at floor losing should stay at floor
        let new_rating = calculate_new_rating(RATING_FLOOR, 1500, 0.0, 30);
        assert_eq!(new_rating, RATING_FLOOR);
    }

    #[test]
    fn test_k_factor_decreases_with_experience() {
        // New player (5 games) gets K=40, experienced (30 games) gets K=20
        let new_player_gain = calculate_new_rating(1500, 1500, 1.0, 5) - 1500;
        let experienced_gain = calculate_new_rating(1500, 1500, 1.0, 30) - 1500;
        assert!(new_player_gain > experienced_gain,
            "New player gain ({}) should be > experienced gain ({})", new_player_gain, experienced_gain);
        assert_eq!(new_player_gain, 20); // K=40 * 0.5
        assert_eq!(experienced_gain, 10); // K=20 * 0.5
    }

    #[test]
    fn test_k_factor_transition_at_30_games() {
        // 29 games → K=40, 30 games → K=20
        let gain_29 = calculate_new_rating(1500, 1500, 1.0, 29) - 1500;
        let gain_30 = calculate_new_rating(1500, 1500, 1.0, 30) - 1500;
        assert_eq!(gain_29, 20); // K=40
        assert_eq!(gain_30, 10); // K=20
    }
}
