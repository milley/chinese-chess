use axum::Json;

use crate::db::models::*;
use crate::error::AppError;
use crate::middleware::auth::AuthUser;

/// Maximum allowed AI search depth to prevent CPU abuse.
const MAX_AI_DEPTH: u8 = 6;

/// POST /api/ai/move — AI 推荐走法 (requires authentication)
pub async fn get_ai_move(
    _auth: AuthUser,  // Require authentication to prevent anonymous CPU abuse
    Json(data): Json<AiMoveRequest>,
) -> Result<Json<AiMoveResponse>, AppError> {
    let state = chess_engine::GameState::from_fen(&data.fen)
        .map_err(|e| AppError::BadRequest(format!("Invalid FEN: {}", e)))?;

    // Clamp depth to prevent excessive CPU usage
    let requested_depth = data.depth.unwrap_or(4);
    let depth = requested_depth.min(MAX_AI_DEPTH);

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
