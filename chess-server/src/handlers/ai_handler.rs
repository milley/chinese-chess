use axum::Json;

use crate::db::models::*;
use crate::error::AppError;

/// POST /api/ai/move — AI 推荐走法
pub async fn get_ai_move(
    Json(data): Json<AiMoveRequest>,
) -> Result<Json<AiMoveResponse>, AppError> {
    let state = chess_engine::GameState::from_fen(&data.fen)
        .map_err(|e| AppError::BadRequest(format!("Invalid FEN: {}", e)))?;

    let depth = data.depth.unwrap_or(4);

    // Use spawn_blocking to avoid blocking the async runtime
    let best = tokio::task::spawn_blocking(move || {
        chess_engine::find_best_move(&state, depth)
    }).await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("AI task failed: {}", e)))?;

    let best_move = best.map(|(m, _)| m.to_uci());

    Ok(Json(AiMoveResponse {
        best_move,
        depth,
    }))
}
