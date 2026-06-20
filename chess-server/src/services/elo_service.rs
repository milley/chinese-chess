use uuid::Uuid;

use crate::db::models::Game;
use crate::db::repositories::game_repo::GameRepository;
use crate::db::repositories::user_repo::UserRepository;
use crate::error::AppError;

/// 对局结束处理: 更新结果、Elo 评分、胜负平统计
/// Returns Ok(true) if game was finished (first call), Ok(false) if already finished (duplicate call).
///
/// Note: The game finish (status + result) and Elo updates are not wrapped in a single DB
/// transaction due to the current repository design (separate pools). Instead, we rely on
/// the idempotent `finish_game` (WHERE status != 'finished') to ensure only one call
/// proceeds to Elo updates. If Elo updates partially fail (one player updated, other not),
/// the error is logged but not propagated — the game result is still correctly persisted.
/// This is acceptable because:
/// 1. The game result is authoritative (persisted first).
/// 2. Elo is an approximate rating — a missed update is not critical.
/// 3. A future migration can wrap both in a transaction.
#[allow(clippy::too_many_arguments)]
pub async fn finish_game_with_elo(
    game_repo: &GameRepository,
    user_repo: &UserRepository,
    game_id: Uuid,
    game: &Game,
    result_str: &str,
    reason_str: &str,
    fen: &str,
    move_history: &str,
) -> Result<bool, AppError> {
    // 更新对局结果 (idempotent: WHERE status != 'finished')
    let finished = game_repo.finish_game(game_id, result_str, reason_str, fen, move_history).await?;
    if finished.is_none() {
        // Game was already finished by another concurrent call — skip Elo update
        return Ok(false);
    }

    // 更新 Elo 评分
    if let (Some(red_id), Some(black_id)) = (game.red_player_id, game.black_player_id) {
        let red_user = user_repo.find_by_id(red_id).await?;
        let black_user = user_repo.find_by_id(black_id).await?;
        if let (Some(ru), Some(bu)) = (red_user, black_user) {
            let (red_score, black_score) = match result_str {
                "red_win" => (1.0, 0.0),
                "black_win" => (0.0, 1.0),
                _ => (0.5, 0.5),
            };
            let new_red = crate::db::repositories::user_repo::calculate_new_rating(
                ru.rating, bu.rating, red_score,
                ru.wins + ru.losses + ru.draws,
            );
            let new_black = crate::db::repositories::user_repo::calculate_new_rating(
                bu.rating, ru.rating, black_score,
                bu.wins + bu.losses + bu.draws,
            );

            // Update both players' ratings. If one fails, log but don't roll back the other.
            // The game result is already persisted and is the source of truth.
            let red_result = user_repo.update_rating(red_id, new_red, red_score > 0.5, red_score == 0.5).await;
            if let Err(e) = red_result {
                tracing::error!("Failed to update Elo rating for red player {}: {}", red_id, e);
            }

            let black_result = user_repo.update_rating(black_id, new_black, black_score > 0.5, black_score == 0.5).await;
            if let Err(e) = black_result {
                tracing::error!("Failed to update Elo rating for black player {}: {}", black_id, e);
            }
        }
    }

    Ok(true)
}
