use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::Game;

const INITIAL_FEN: &str = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";

#[derive(Clone)]
pub struct GameRepository {
    pool: PgPool,
}

impl GameRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, red_player_id: Uuid, time_control: Option<i32>, move_time_limit: Option<i32>, byoyomi: Option<i32>) -> Result<Game> {
        let red_time = time_control;
        let black_time = time_control;
        let game = sqlx::query_as::<_, Game>(
            "INSERT INTO games (red_player_id, fen, initial_fen, time_control, move_time_limit, byoyomi, red_time, black_time, status) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'waiting') RETURNING *"
        )
        .bind(red_player_id)
        .bind(INITIAL_FEN)
        .bind(INITIAL_FEN)
        .bind(time_control)
        .bind(move_time_limit)
        .bind(byoyomi)
        .bind(red_time)
        .bind(black_time)
        .fetch_one(&self.pool)
        .await?;
        Ok(game)
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Game>> {
        let game = sqlx::query_as::<_, Game>("SELECT * FROM games WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(game)
    }

    pub async fn join_game(&self, id: Uuid, black_player_id: Uuid) -> Result<Game> {
        let game = sqlx::query_as::<_, Game>(
            "UPDATE games SET black_player_id = $1, status = 'playing', started_at = NOW() WHERE id = $2 AND status = 'waiting' RETURNING *"
        )
        .bind(black_player_id)
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        Ok(game)
    }

    pub async fn finish_game(&self, id: Uuid, result: &str, end_reason: &str, fen: &str, move_history: &str) -> Result<Game> {
        let game = sqlx::query_as::<_, Game>(
            "UPDATE games SET status = 'finished', result = $1, end_reason = $2, fen = $3, move_history = $4, finished_at = NOW() WHERE id = $5 RETURNING *"
        )
        .bind(result)
        .bind(end_reason)
        .bind(fen)
        .bind(move_history)
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        Ok(game)
    }

    pub async fn update_fen(&self, id: Uuid, fen: &str, move_history: &str) -> Result<()> {
        sqlx::query("UPDATE games SET fen = $1, move_history = $2 WHERE id = $3")
            .bind(fen)
            .bind(move_history)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_time(&self, id: Uuid, red_time: i32, black_time: i32) -> Result<()> {
        sqlx::query("UPDATE games SET red_time = $1, black_time = $2 WHERE id = $3")
            .bind(red_time)
            .bind(black_time)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list(&self, status: Option<&str>, page: i64, page_size: i64) -> Result<Vec<Game>> {
        let offset = (page - 1) * page_size;
        let games = match status {
            Some(s) => sqlx::query_as::<_, Game>("SELECT * FROM games WHERE status = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3")
                .bind(s)
                .bind(page_size)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?,
            None => sqlx::query_as::<_, Game>("SELECT * FROM games ORDER BY created_at DESC LIMIT $1 OFFSET $2")
                .bind(page_size)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?,
        };
        Ok(games)
    }

    pub async fn delete(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM games WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
