use anyhow::Result;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::db::models::{Game, GameEvent, UserInfo};

const INITIAL_FEN: &str = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";

#[derive(Clone)]
pub struct GameRepository {
    pool: PgPool,
}

impl GameRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Expose the underlying pool for transaction-based operations
    /// (e.g., atomic game finish + Elo updates in elo_service).
    pub fn pool(&self) -> &PgPool {
        &self.pool
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
        // Atomically fill whichever player slot is vacant in a single UPDATE.
        // This avoids the race condition of two separate UPDATEs where the first
        // sets status='playing' and the second's WHERE status='waiting' no longer matches.
        //
        // Logic: If red_player_id IS NULL, fill red and set status='playing'.
        //        Else if black_player_id IS NULL, fill black and set status='playing'.
        //        Else return no rows (game is full).
        let game = sqlx::query_as::<_, Game>(
            "UPDATE games SET \
                red_player_id = CASE WHEN red_player_id IS NULL THEN $1 ELSE red_player_id END, \
                black_player_id = CASE WHEN red_player_id IS NULL THEN black_player_id ELSE $1 END, \
                status = 'playing', \
                started_at = NOW() \
             WHERE id = $2 AND status = 'waiting' AND (red_player_id IS NULL OR black_player_id IS NULL) \
             RETURNING *"
        )
        .bind(joining_player_id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        match game {
            Some(g) => Ok(g),
            None => Err(anyhow::anyhow!("Game is already full or not found")),
        }
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
        // Cap page_size to prevent excessive queries
        let page_size = page_size.min(100);
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

    /// List games with player info in a single query (avoids N+1).
    /// Returns a list of (Game, Option<UserInfo>, Option<UserInfo>) tuples.
    /// Uses column aliases to avoid name collisions between games and users tables.
    pub async fn list_with_players(&self, status: Option<&str>, page: i64, page_size: i64) -> Result<Vec<(Game, Option<UserInfo>, Option<UserInfo>)>> {
        let offset = (page - 1) * page_size;
        let page_size = page_size.min(100);
        let rows = match status {
            Some(s) => sqlx::query(
                "SELECT g.id, g.red_player_id, g.black_player_id, g.status, g.result, g.end_reason, \
                 g.fen, g.move_history, g.initial_fen, g.time_control, g.move_time_limit, g.byoyomi, \
                 g.red_time, g.black_time, g.created_at, g.started_at, g.finished_at, g.last_tick_at, \
                 ru.id as ru_id, ru.username as ru_username, ru.display_name as ru_display_name, \
                 ru.rating as ru_rating, ru.wins as ru_wins, ru.losses as ru_losses, ru.draws as ru_draws, \
                 bu.id as bu_id, bu.username as bu_username, bu.display_name as bu_display_name, \
                 bu.rating as bu_rating, bu.wins as bu_wins, bu.losses as bu_losses, bu.draws as bu_draws \
                 FROM games g \
                 LEFT JOIN users ru ON g.red_player_id = ru.id \
                 LEFT JOIN users bu ON g.black_player_id = bu.id \
                 WHERE g.status = $1 \
                 ORDER BY g.created_at DESC \
                 LIMIT $2 OFFSET $3"
            )
            .bind(s)
            .bind(page_size)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?,
            None => sqlx::query(
                "SELECT g.id, g.red_player_id, g.black_player_id, g.status, g.result, g.end_reason, \
                 g.fen, g.move_history, g.initial_fen, g.time_control, g.move_time_limit, g.byoyomi, \
                 g.red_time, g.black_time, g.created_at, g.started_at, g.finished_at, g.last_tick_at, \
                 ru.id as ru_id, ru.username as ru_username, ru.display_name as ru_display_name, \
                 ru.rating as ru_rating, ru.wins as ru_wins, ru.losses as ru_losses, ru.draws as ru_draws, \
                 bu.id as bu_id, bu.username as bu_username, bu.display_name as bu_display_name, \
                 bu.rating as bu_rating, bu.wins as bu_wins, bu.losses as bu_losses, bu.draws as bu_draws \
                 FROM games g \
                 LEFT JOIN users ru ON g.red_player_id = ru.id \
                 LEFT JOIN users bu ON g.black_player_id = bu.id \
                 ORDER BY g.created_at DESC \
                 LIMIT $1 OFFSET $2"
            )
            .bind(page_size)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?,
        };
        Ok(rows.iter().map(|row| row_to_game_with_players(row)).collect())
    }

    /// Find a single game with player info in one query (avoids N+1).
    /// Returns (Game, Option<UserInfo>, Option<UserInfo>) or None if not found.
    pub async fn find_with_players(&self, id: Uuid) -> Result<Option<(Game, Option<UserInfo>, Option<UserInfo>)>> {
        let row = sqlx::query(
            "SELECT g.id, g.red_player_id, g.black_player_id, g.status, g.result, g.end_reason, \
             g.fen, g.move_history, g.initial_fen, g.time_control, g.move_time_limit, g.byoyomi, \
             g.red_time, g.black_time, g.created_at, g.started_at, g.finished_at, g.last_tick_at, \
             ru.id as ru_id, ru.username as ru_username, ru.display_name as ru_display_name, \
             ru.rating as ru_rating, ru.wins as ru_wins, ru.losses as ru_losses, ru.draws as ru_draws, \
             bu.id as bu_id, bu.username as bu_username, bu.display_name as bu_display_name, \
             bu.rating as bu_rating, bu.wins as bu_wins, bu.losses as bu_losses, bu.draws as bu_draws \
             FROM games g \
             LEFT JOIN users ru ON g.red_player_id = ru.id \
             LEFT JOIN users bu ON g.black_player_id = bu.id \
             WHERE g.id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|row| row_to_game_with_players(&row)))
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

/// Parse a joined game+players row into (Game, Option<UserInfo>, Option<UserInfo>).
/// Uses column aliases (ru_*, bu_*) to avoid name collisions.
fn row_to_game_with_players(row: &sqlx::postgres::PgRow) -> (Game, Option<UserInfo>, Option<UserInfo>) {
    use chrono::{DateTime, Utc};

    let game = Game {
        id: row.get("id"),
        red_player_id: row.get("red_player_id"),
        black_player_id: row.get("black_player_id"),
        status: row.get("status"),
        result: row.get("result"),
        end_reason: row.get("end_reason"),
        fen: row.get("fen"),
        move_history: row.get("move_history"),
        initial_fen: row.get("initial_fen"),
        time_control: row.get("time_control"),
        move_time_limit: row.get("move_time_limit"),
        byoyomi: row.get("byoyomi"),
        red_time: row.get("red_time"),
        black_time: row.get("black_time"),
        created_at: row.get::<DateTime<Utc>, _>("created_at"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
        last_tick_at: row.get("last_tick_at"),
    };

    let red_player = row.get::<Option<Uuid>, _>("ru_id").map(|_| UserInfo {
        id: row.get("ru_id"),
        username: row.get("ru_username"),
        display_name: row.get("ru_display_name"),
        rating: row.get("ru_rating"),
        wins: row.get("ru_wins"),
        losses: row.get("ru_losses"),
        draws: row.get("ru_draws"),
    });

    let black_player = row.get::<Option<Uuid>, _>("bu_id").map(|_| UserInfo {
        id: row.get("bu_id"),
        username: row.get("bu_username"),
        display_name: row.get("bu_display_name"),
        rating: row.get("bu_rating"),
        wins: row.get("bu_wins"),
        losses: row.get("bu_losses"),
        draws: row.get("bu_draws"),
    });

    (game, red_player, black_player)
}
