use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::db::models::*;
use crate::error::AppError;
use crate::middleware::auth::AuthUser;
use crate::services::game_service;
use crate::websocket::message::LobbyGameInfo;
use crate::websocket::room::MoveEntry;
use crate::AppState;

/// Build a lobby game list for broadcast to WS subscribers.
async fn build_lobby_games(state: &AppState) -> Result<Vec<LobbyGameInfo>, AppError> {
    let rows = state.game_repo.list_with_players(None, 1, 100).await?;
    let games: Vec<LobbyGameInfo> = rows.into_iter().map(|(game, red_player, black_player)| {
        LobbyGameInfo {
            id: game.id.to_string(),
            red_player,
            black_player,
            status: game.status,
            time_control: game.time_control,
            move_time_limit: game.move_time_limit,
            byoyomi: game.byoyomi,
            created_at: game.created_at.to_rfc3339(),
        }
    }).collect();
    Ok(games)
}

/// POST /api/games — 创建对局
pub async fn create_game(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(data): Json<CreateGameRequest>,
) -> Result<Json<CreateGameResponse>, AppError> {
    let (game, color) = game_service::create_game(&state.game_repo, auth.user_id, &data).await?;

    // Notify lobby subscribers
    if let Ok(games) = build_lobby_games(&state).await {
        state.room_manager.broadcast_lobby_update(games).await;
    }

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

    // Notify lobby subscribers (game status changed from "waiting" to "playing")
    if let Ok(games) = build_lobby_games(&state).await {
        state.room_manager.broadcast_lobby_update(games).await;
    }

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
) -> Result<Json<PaginatedResponse<GameInfo>>, AppError> {
    let page = q.page.unwrap_or(1).max(1);
    let page_size = q.page_size.unwrap_or(20).clamp(1, 100);
    let total = state.game_repo.count(q.status.as_deref()).await?;
    let rows = state.game_repo.list_with_players(q.status.as_deref(), page, page_size).await?;

    let items: Vec<GameInfo> = rows.into_iter().map(|(game, red_player, black_player)| {
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
    Ok(Json(PaginatedResponse { items, total, page, page_size }))
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

/// GET /api/games/{id}/moves — 返回结构化走法记录 (reconstructed from game_events)
pub async fn get_game_moves(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Vec<MoveEntry>>, AppError> {
    state.game_repo.find_by_id(id).await?
        .ok_or(AppError::NotFound("Game not found".into()))?;
    let moves = state.game_repo.get_move_history_from_events(id).await?;
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
