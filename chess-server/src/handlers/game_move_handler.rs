use axum::extract::{Path, State};
use axum::Json;
use uuid::Uuid;

use crate::db::models::*;
use crate::error::AppError;
use crate::middleware::auth::AuthUser;
use crate::services::elo_service;
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
    ).await.map_err(AppError::GameError)?;

    // 更新数据库
    if result.is_game_over {
        let (result_str, reason_str) = match result.result.as_deref() {
            Some("red_win") => ("red_win", result.end_reason.as_deref().unwrap_or("checkmate")),
            Some("black_win") => ("black_win", result.end_reason.as_deref().unwrap_or("checkmate")),
            Some("draw") => ("draw", result.end_reason.as_deref().unwrap_or("draw")),
            _ => ("draw", "unknown"),
        };
        let moves_json = serde_json::to_string(&result.move_history_uci).unwrap_or("[]".into());

        elo_service::finish_game_with_elo(
            &state.game_repo,
            &state.user_repo,
            id,
            &game,
            result_str,
            reason_str,
            &result.fen,
            &moves_json,
        ).await?;
    } else {
        let moves_json = serde_json::to_string(&result.move_history_uci).unwrap_or("[]".into());
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
