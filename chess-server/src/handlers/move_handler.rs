use axum::Json;

use crate::db::models::*;
use crate::error::AppError;

/// POST /api/moves/valid — 查询合法走法
pub async fn get_valid_moves(
    Json(data): Json<ValidMovesRequest>,
) -> Result<Json<ValidMovesResponse>, AppError> {
    let board = chess_engine::Board::from_fen(&data.fen)
        .map_err(|e| AppError::BadRequest(format!("Invalid FEN: {}", e)))?;

    let from_pos = chess_engine::Position::from_uci(&data.from)
        .ok_or(AppError::BadRequest("Invalid from position".into()))?;

    let piece = board.piece_at(from_pos)
        .ok_or(AppError::BadRequest("No piece at from position".into()))?;

    let legal_moves = board.generate_legal_moves(piece.color);
    let valid_targets: Vec<String> = legal_moves
        .iter()
        .filter(|m| m.from == from_pos)
        .map(|m| m.to.to_uci())
        .collect();

    Ok(Json(ValidMovesResponse { moves: valid_targets }))
}
