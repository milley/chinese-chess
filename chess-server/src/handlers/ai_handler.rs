use axum::Json;

use crate::db::models::*;
use crate::error::AppError;
use crate::middleware::auth::AuthUser;

/// Maximum allowed AI search depth to prevent CPU abuse.
const MAX_AI_DEPTH: u8 = 6;

/// Clamp the requested depth: default to 4, never exceed MAX_AI_DEPTH.
fn clamp_depth(depth: Option<u8>) -> u8 {
    depth.unwrap_or(4).min(MAX_AI_DEPTH)
}

/// POST /api/ai/move — AI 推荐走法 (requires authentication)
pub async fn get_ai_move(
    _auth: AuthUser,  // Require authentication to prevent anonymous CPU abuse
    Json(data): Json<AiMoveRequest>,
) -> Result<Json<AiMoveResponse>, AppError> {
    let state = chess_engine::GameState::from_fen(&data.fen)
        .map_err(|e| AppError::BadRequest(format!("Invalid FEN: {}", e)))?;

    // Clamp depth to prevent excessive CPU usage
    let depth = clamp_depth(data.depth);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clamp_depth_within_limit() {
        assert_eq!(clamp_depth(Some(4)), 4);
    }

    #[test]
    fn test_clamp_depth_exceeds_limit() {
        assert_eq!(clamp_depth(Some(10)), 6);
    }

    #[test]
    fn test_clamp_depth_none_defaults() {
        assert_eq!(clamp_depth(None), 4);
    }
}
