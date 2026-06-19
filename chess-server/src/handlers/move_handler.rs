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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::ValidMovesRequest;

    const INITIAL_FEN: &str = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";

    #[tokio::test]
    async fn test_get_valid_moves_initial_position_cannon() {
        // Red cannon at b7 in the initial position (1C5C1 on row 7)
        let request = ValidMovesRequest {
            fen: INITIAL_FEN.to_string(),
            from: "b7".to_string(),
        };
        let result = get_valid_moves(Json(request)).await;
        match result {
            Ok(response) => {
                // Red cannon on b7 can move horizontally along rank 7 and vertically
                // Horizontally: a7, c7, d7, e7, f7, g7, h7 (blocked by own cannon at h7)
                // Vertically: b8, b9 (blocked by own horse at b9)
                assert!(response.moves.contains(&"a7".to_string()), "Should include a7");
                assert!(response.moves.contains(&"c7".to_string()), "Should include c7");
                assert!(response.moves.contains(&"b8".to_string()), "Should include b8");
            }
            Err(_) => panic!("Expected Ok for valid cannon position"),
        }
    }

    #[tokio::test]
    async fn test_get_valid_moves_invalid_fen() {
        let request = ValidMovesRequest {
            fen: "not-a-valid-fen".to_string(),
            from: "b9".to_string(),
        };
        let result = get_valid_moves(Json(request)).await;
        match result {
            Err(AppError::BadRequest(msg)) => {
                assert!(msg.contains("Invalid FEN"), "Expected 'Invalid FEN' in message, got: {}", msg);
            }
            Err(_other) => panic!("Expected BadRequest, got a different error variant"),
            Ok(_) => panic!("Expected error for invalid FEN"),
        }
    }

    #[tokio::test]
    async fn test_get_valid_moves_no_piece_at_position() {
        // e5 is an empty square in the initial position
        let request = ValidMovesRequest {
            fen: INITIAL_FEN.to_string(),
            from: "e5".to_string(),
        };
        let result = get_valid_moves(Json(request)).await;
        match result {
            Err(AppError::BadRequest(msg)) => {
                assert!(msg.contains("No piece at from position"), "Expected 'No piece at from position', got: {}", msg);
            }
            Err(_other) => panic!("Expected BadRequest, got a different error variant"),
            Ok(_) => panic!("Expected error for empty square"),
        }
    }
}
