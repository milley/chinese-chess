use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::{Game, GameEvent};

const INITIAL_FEN: &str = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";

#[derive(Clone)]
pub struct GameRepository {
    pool: PgPool,
}

impl GameRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, creator_id: Uuid, creator_color: &str, time_control: Option<i32>, move_time_limit: Option<i32>, byoyomi: Option<i32>) -> Result<Game> {
        let red_time = time_control;
        let black_time = time_control;
        let (red_player_id, black_player_id) = if creator_color == "black" {
            (None, Some(creator_id))
        } else {
            (Some(creator_id), None)
        };
        let game = sqlx::query_as::<_, Game>(
            "INSERT INTO games (red_player_id, black_player_id, fen, initial_fen, time_control, move_time_limit, byoyomi, red_time, black_time, status) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'waiting') RETURNING *"
        )
        .bind(red_player_id)
        .bind(black_player_id)
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

    pub async fn join_game(&self, id: Uuid, joining_player_id: Uuid) -> Result<Game> {
        // Try to fill the red slot first, then black slot.
        // Each UPDATE includes WHERE ... IS NULL to prevent double-assignment
        // from concurrent join_game calls.
        let red_result = sqlx::query_as::<_, Game>(
            "UPDATE games SET red_player_id = $1, status = 'playing', started_at = NOW() WHERE id = $2 AND status = 'waiting' AND red_player_id IS NULL RETURNING *"
        )
        .bind(joining_player_id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(game) = red_result {
            return Ok(game);
        }

        let black_result = sqlx::query_as::<_, Game>(
            "UPDATE games SET black_player_id = $1, status = 'playing', started_at = NOW() WHERE id = $2 AND status = 'waiting' AND black_player_id IS NULL RETURNING *"
        )
        .bind(joining_player_id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(game) = black_result {
            return Ok(game);
        }

        Err(anyhow::anyhow!("Game is already full or not found"))
    }

    pub async fn finish_game(&self, id: Uuid, result: &str, end_reason: &str, fen: &str, move_history: &str) -> Result<Option<Game>> {
        let game = sqlx::query_as::<_, Game>(
            "UPDATE games SET status = 'finished', result = $1, end_reason = $2, fen = $3, move_history = $4, finished_at = NOW() WHERE id = $5 AND status != 'finished' RETURNING *"
        )
        .bind(result)
        .bind(end_reason)
        .bind(fen)
        .bind(move_history)
        .bind(id)
        .fetch_optional(&self.pool)
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

    pub async fn append_event(&self, game_id: Uuid, event_type: &str, actor_id: Option<Uuid>, data: serde_json::Value) -> Result<GameEvent> {
        // Use an explicit transaction with pg_advisory_xact_lock to serialize
        // concurrent inserts for the same game_id. The advisory lock is
        // transaction-scoped: automatically released on commit/rollback.
        // This prevents the TOCTOU race between reading MAX(seq_num) and
        // inserting, which could produce duplicate seq_num values under
        // concurrent writes.
        let lock_key1 = game_id.as_u128() as i32;
        let lock_key2 = (game_id.as_u128() >> 32) as i32;

        let mut tx = self.pool.begin().await?;

        // Acquire advisory lock within the transaction
        sqlx::query("SELECT pg_advisory_xact_lock($1, $2)")
            .bind(lock_key1)
            .bind(lock_key2)
            .execute(&mut *tx)
            .await?;

        let event = sqlx::query_as::<_, GameEvent>(
            "INSERT INTO game_events (game_id, seq_num, event_type, actor_id, data) \
             SELECT $1, COALESCE(MAX(seq_num), 0) + 1, $2, $3, $4 \
             FROM game_events WHERE game_id = $1 \
             RETURNING *"
        )
        .bind(game_id)
        .bind(event_type)
        .bind(actor_id)
        .bind(data)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(event)
    }

    pub async fn list_events(&self, game_id: Uuid) -> Result<Vec<GameEvent>> {
        let events = sqlx::query_as::<_, GameEvent>(
            "SELECT * FROM game_events WHERE game_id = $1 ORDER BY seq_num ASC"
        )
        .bind(game_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(events)
    }

    /// Update the last_tick_at timestamp for a game.
    /// Called by the timeout checker every tick to record when time was last processed.
    /// On server restart, this timestamp is used to deduct elapsed downtime.
    pub async fn update_last_tick(&self, id: Uuid) -> Result<()> {
        sqlx::query("UPDATE games SET last_tick_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
