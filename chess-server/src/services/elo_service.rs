use uuid::Uuid;

use crate::db::models::Game;
use crate::db::repositories::game_repo::GameRepository;
use crate::db::repositories::user_repo::{UserRepository, calculate_new_rating};
use crate::error::AppError;

/// 对局结束处理: 更新结果、Elo 评分、胜负平统计
/// Returns Ok(true) if game was finished (first call), Ok(false) if already finished (duplicate call).
///
/// All operations (game finish + both Elo updates) are wrapped in a single DB transaction.
/// Player rows are read with `SELECT ... FOR UPDATE` to acquire row-level locks,
/// preventing concurrent transactions from reading stale ratings for the same player.
/// This ensures atomicity: either all updates succeed or none do, preventing the case
/// where one player's rating is updated but the other's is not.
#[allow(clippy::too_many_arguments)]
pub async fn finish_game_with_elo(
    game_repo: &GameRepository,
    _user_repo: &UserRepository,
    game_id: Uuid,
    game: &Game,
    result_str: &str,
    reason_str: &str,
    fen: &str,
    move_history: &str,
) -> Result<bool, AppError> {
    // Start a transaction for atomic game finish + Elo updates
    let mut tx = game_repo.pool().begin().await.map_err(|e| AppError::Internal(e.into()))?;

    // 更新对局结果 (idempotent: WHERE status != 'finished')
    let finished = sqlx::query_as::<_, Game>(
        "UPDATE games SET status = 'finished', result = $1, end_reason = $2, fen = $3, move_history = $4, finished_at = NOW() \
         WHERE id = $5 AND status != 'finished' RETURNING *"
    )
    .bind(result_str)
    .bind(reason_str)
    .bind(fen)
    .bind(move_history)
    .bind(game_id)
    .fetch_optional(&mut *tx)
    .await.map_err(|e| AppError::Internal(e.into()))?;

    if finished.is_none() {
        // Game was already finished by another concurrent call — skip Elo update
        tx.rollback().await.map_err(|e| AppError::Internal(e.into()))?;
        return Ok(false);
    }

    // 更新 Elo 评分 (within the same transaction)
    // Read player data with FOR UPDATE to lock rows and prevent stale reads
    if let (Some(red_id), Some(black_id)) = (game.red_player_id, game.black_player_id) {
        let red_user = UserRepository::find_by_id_for_update(&mut *tx, red_id).await?;
        let black_user = UserRepository::find_by_id_for_update(&mut *tx, black_id).await?;
        if let (Some(ru), Some(bu)) = (red_user, black_user) {
            let (red_score, black_score) = match result_str {
                "red_win" => (1.0, 0.0),
                "black_win" => (0.0, 1.0),
                _ => (0.5, 0.5),
            };
            let new_red = calculate_new_rating(
                ru.rating, bu.rating, red_score,
                ru.wins + ru.losses + ru.draws,
            );
            let new_black = calculate_new_rating(
                bu.rating, ru.rating, black_score,
                bu.wins + bu.losses + bu.draws,
            );

            // Update red player's rating within the transaction
            let red_wins_change = if red_score > 0.5 { 1 } else { 0 };
            let red_losses_change = if red_score < 0.5 { 1 } else { 0 };
            let red_draws_change = if red_score == 0.5 { 1 } else { 0 };
            sqlx::query(
                "UPDATE users SET rating = $1, wins = wins + $2, losses = losses + $3, draws = draws + $4, updated_at = NOW() WHERE id = $5"
            )
            .bind(new_red)
            .bind(red_wins_change)
            .bind(red_losses_change)
            .bind(red_draws_change)
            .bind(red_id)
            .execute(&mut *tx)
            .await.map_err(|e| AppError::Internal(e.into()))?;

            // Update black player's rating within the transaction
            let black_wins_change = if black_score > 0.5 { 1 } else { 0 };
            let black_losses_change = if black_score < 0.5 { 1 } else { 0 };
            let black_draws_change = if black_score == 0.5 { 1 } else { 0 };
            sqlx::query(
                "UPDATE users SET rating = $1, wins = wins + $2, losses = losses + $3, draws = draws + $4, updated_at = NOW() WHERE id = $5"
            )
            .bind(new_black)
            .bind(black_wins_change)
            .bind(black_losses_change)
            .bind(black_draws_change)
            .bind(black_id)
            .execute(&mut *tx)
            .await.map_err(|e| AppError::Internal(e.into()))?;
        }
    }

    tx.commit().await.map_err(|e| AppError::Internal(e.into()))?;
    Ok(true)
}
