use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::db::models::*;
use crate::error::AppError;
use crate::middleware::auth::AuthUser;
use crate::services::game_service;
use crate::websocket::room::MoveEntry;
use crate::AppState;
/// POST /api/games — 创建对局
pub async fn create_game(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(data): Json<CreateGameRequest>,
) -> Result<Json<CreateGameResponse>, AppError> {
    let (game, color) = game_service::create_game(&state.game_repo, auth.user_id, &data).await?;
    Ok(Json(CreateGameResponse {
        game_id: game.id,
        color,
    }))
}

/// POST /api/games/{id}/join — 加入对局
pub async fn join_game(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<GameInfo>, AppError> {
    let game_info = game_service::join_game(
        &state.game_repo,
        id,
        auth.user_id,
    ).await?;
    Ok(Json(game_info))
}

/// GET /api/games/{id}
pub async fn get_game(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<GameInfo>, AppError> {
    let (game, red_player, black_player) = state.game_repo.find_with_players(id).await?
        .ok_or(AppError::NotFound("Game not found".into()))?;

    Ok(Json(GameInfo {
        id: game.id,
        red_player,
        black_player,
        status: game.status,
        result: game.result,
        end_reason: game.end_reason,
        fen: game.fen,
        initial_fen: game.initial_fen,
        time_control: game.time_control,
        move_time_limit: game.move_time_limit,
        byoyomi: game.byoyomi,
        red_time: game.red_time,
        black_time: game.black_time,
        created_at: game.created_at,
    }))
}

#[derive(Deserialize)]
pub struct ListGamesQuery {
    status: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
}

/// GET /api/games
pub async fn list_games(
    Query(q): Query<ListGamesQuery>,
    State(state): State<AppState>,
) -> Result<Json<Vec<GameInfo>>, AppError> {
    let page = q.page.unwrap_or(1);
    let page_size = q.page_size.unwrap_or(20).min(100);
    let rows = state.game_repo.list_with_players(q.status.as_deref(), page, page_size).await?;

    let result: Vec<GameInfo> = rows.into_iter().map(|(game, red_player, black_player)| {
        GameInfo {
            id: game.id,
            red_player,
            black_player,
            status: game.status,
            result: game.result,
            end_reason: game.end_reason,
            fen: game.fen,
            initial_fen: game.initial_fen,
            time_control: game.time_control,
            move_time_limit: game.move_time_limit,
            byoyomi: game.byoyomi,
            red_time: game.red_time,
            black_time: game.black_time,
            created_at: game.created_at,
        }
    }).collect();
    Ok(Json(result))
}

/// DELETE /api/games/{id}
pub async fn delete_game(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    game_service::delete_game(&state.game_repo, id, auth.user_id).await?;
    // Clean up in-memory room to prevent zombie rooms
    state.room_manager.remove_room(id).await;
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/games/{id}/rematch — 再来一局
pub async fn rematch(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<RematchResponse>, AppError> {
    let (new_game_id, color) = game_service::create_rematch(
        &state.game_repo,
        id,
        auth.user_id,
    ).await?;
    Ok(Json(RematchResponse {
        game_id: new_game_id,
        color,
    }))
}

/// GET /api/games/{id}/moves — 返回结构化走法记录 (用于调试回溯)
pub async fn get_game_moves(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Vec<MoveEntry>>, AppError> {
    let game = state.game_repo.find_by_id(id).await?
        .ok_or(AppError::NotFound("Game not found".into()))?;
    let moves: Vec<MoveEntry> = game.move_history
        .and_then(|h| serde_json::from_str(&h).ok())
        .unwrap_or_default();
    Ok(Json(moves))
}

/// GET /api/games/{id}/events — 返回对局事件记录 (完整可追溯)
pub async fn get_game_events(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Vec<GameEvent>>, AppError> {
    // Verify game exists
    state.game_repo.find_by_id(id).await?
        .ok_or(AppError::NotFound("Game not found".into()))?;
    let events = state.game_repo.list_events(id).await?;
    Ok(Json(events))
}
