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

    /// Update Elo rating after a game ends
    pub async fn update_rating(&self, id: Uuid, new_rating: i32, is_win: bool, is_draw: bool) -> Result<()> {
        let wins_change = if is_win && !is_draw { 1 } else { 0 };
        let losses_change = if !is_win && !is_draw { 1 } else { 0 };
        let draws_change = if is_draw { 1 } else { 0 };

        sqlx::query(
            "UPDATE users SET rating = $1, wins = wins + $2, losses = losses + $3, draws = draws + $4, updated_at = NOW() WHERE id = $5"
        )
        .bind(new_rating)
        .bind(wins_change)
        .bind(losses_change)
        .bind(draws_change)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

/// Calculate new Elo rating (K=32)
pub fn calculate_new_rating(rating: i32, opponent_rating: i32, score: f64) -> i32 {
    let expected = 1.0 / (1.0 + 10_f64.powi((opponent_rating - rating) / 400));
    let k = 32.0;
    (rating as f64 + k * (score - expected)).round() as i32
}
