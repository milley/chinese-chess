use axum::extract::{Path, State};
use axum::Json;
use uuid::Uuid;

use crate::db::models::*;
use crate::error::AppError;
use crate::middleware::auth::AuthUser;
use crate::AppState;

/// POST /api/games/{id}/move — 走棋
pub async fn make_move(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(data): Json<MakeMoveRequest>,
) -> Result<Json<MakeMoveResponse>, AppError> {
    let game = state.game_repo.find_by_id(id).await?
        .ok_or(AppError::NotFound("Game not found".into()))?;

    if game.status != "playing" {
        return Err(AppError::GameError("Game is not in progress".into()));
    }

    // 验证玩家身份
    let player_color = if game.red_player_id == Some(auth.user_id) {
        chess_engine::Color::Red
    } else if game.black_player_id == Some(auth.user_id) {
        chess_engine::Color::Black
    } else {
        return Err(AppError::Forbidden("You are not a player in this game".into()));
    };

    // 通过 RoomManager 走棋
    let result = state.room_manager.make_move(
        id,
        auth.user_id,
        player_color,
        &data.from,
        &data.to,
    ).await.map_err(|e| AppError::GameError(e))?;

    // 更新数据库
    if result.is_game_over {
        let (result_str, reason_str) = match result.result.as_deref() {
            Some("red_win") => ("red_win", result.end_reason.as_deref().unwrap_or("checkmate")),
            Some("black_win") => ("black_win", result.end_reason.as_deref().unwrap_or("checkmate")),
            Some("draw") => ("draw", result.end_reason.as_deref().unwrap_or("draw")),
            _ => ("draw", "unknown"),
        };
        state.game_repo.finish_game(id, result_str, reason_str, &result.fen, "[]").await?;

        // 更新 Elo 评分
        if let (Some(red_id), Some(black_id)) = (game.red_player_id, game.black_player_id) {
            let red_user = state.user_repo.find_by_id(red_id).await?;
            let black_user = state.user_repo.find_by_id(black_id).await?;
            if let (Some(ru), Some(bu)) = (red_user, black_user) {
                let (red_score, black_score) = match result_str {
                    "red_win" => (1.0, 0.0),
                    "black_win" => (0.0, 1.0),
                    _ => (0.5, 0.5),
                };
                let new_red = crate::db::repositories::user_repo::calculate_new_rating(ru.rating, bu.rating, red_score);
                let new_black = crate::db::repositories::user_repo::calculate_new_rating(bu.rating, ru.rating, black_score);
                if let Err(e) = state.user_repo.update_rating(red_id, new_red, red_score > 0.5, red_score == 0.5).await {
                    tracing::error!("Failed to update Elo rating for red player {}: {}", red_id, e);
                }
                if let Err(e) = state.user_repo.update_rating(black_id, new_black, black_score > 0.5, black_score == 0.5).await {
                    tracing::error!("Failed to update Elo rating for black player {}: {}", black_id, e);
                }
            }
        }
    } else {
        let moves_json = serde_json::to_string(&Vec::<String>::new()).unwrap_or("[]".into());
        state.game_repo.update_fen(id, &result.fen, &moves_json).await?;
    }

    Ok(Json(MakeMoveResponse {
        fen: result.fen,
        is_check: result.is_check,
        is_game_over: result.is_game_over,
        result: result.result,
        end_reason: result.end_reason,
    }))
}
