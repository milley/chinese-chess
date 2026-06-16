use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::db::models::*;
use crate::error::AppError;
use crate::middleware::auth::AuthUser;
use crate::AppState;

/// POST /api/games — 创建对局
pub async fn create_game(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(data): Json<CreateGameRequest>,
) -> Result<Json<CreateGameResponse>, AppError> {
    let color = data.player_color.as_deref().unwrap_or("red");
    let game = state.game_repo.create(
        auth.user_id,
        data.time_control,
        data.move_time_limit,
        data.byoyomi,
    ).await?;

    Ok(Json(CreateGameResponse {
        game_id: game.id,
        color: color.to_string(),
    }))
}

/// POST /api/games/{id}/join — 加入对局
pub async fn join_game(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<GameInfo>, AppError> {
    let game = state.game_repo.find_by_id(id).await?
        .ok_or(AppError::NotFound("Game not found".into()))?;

    if game.status != "waiting" {
        return Err(AppError::BadRequest("Game is not waiting for players".into()));
    }

    if game.red_player_id == Some(auth.user_id) {
        return Err(AppError::BadRequest("You are already in this game".into()));
    }

    let game = state.game_repo.join_game(id, auth.user_id).await?;

    // Get player info for response
    let red_player = match game.red_player_id {
        Some(pid) => state.user_repo.find_by_id(pid).await?.map(UserInfo::from),
        None => None,
    };
    let black_player = Some(UserInfo {
        id: auth.user_id,
        username: auth.username,
        display_name: None,
        rating: 0,
        wins: 0,
        losses: 0,
        draws: 0,
    });

    Ok(Json(GameInfo {
        id: game.id,
        red_player,
        black_player,
        status: game.status,
        result: game.result,
        end_reason: game.end_reason,
        fen: game.fen,
        time_control: game.time_control,
        move_time_limit: game.move_time_limit,
        byoyomi: game.byoyomi,
        red_time: game.red_time,
        black_time: game.black_time,
        created_at: game.created_at,
    }))
}

/// GET /api/games/{id}
pub async fn get_game(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<GameInfo>, AppError> {
    let game = state.game_repo.find_by_id(id).await?
        .ok_or(AppError::NotFound("Game not found".into()))?;

    let red_player = match game.red_player_id {
        Some(pid) => state.user_repo.find_by_id(pid).await?.map(UserInfo::from),
        None => None,
    };
    let black_player = match game.black_player_id {
        Some(pid) => state.user_repo.find_by_id(pid).await?.map(UserInfo::from),
        None => None,
    };

    Ok(Json(GameInfo {
        id: game.id,
        red_player,
        black_player,
        status: game.status,
        result: game.result,
        end_reason: game.end_reason,
        fen: game.fen,
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
    let page_size = q.page_size.unwrap_or(20);
    let games = state.game_repo.list(q.status.as_deref(), page, page_size).await?;

    let mut result = Vec::new();
    for game in games {
        let red_player = match game.red_player_id {
            Some(pid) => state.user_repo.find_by_id(pid).await?.map(UserInfo::from),
            None => None,
        };
        let black_player = match game.black_player_id {
            Some(pid) => state.user_repo.find_by_id(pid).await?.map(UserInfo::from),
            None => None,
        };
        result.push(GameInfo {
            id: game.id,
            red_player,
            black_player,
            status: game.status,
            result: game.result,
            end_reason: game.end_reason,
            fen: game.fen,
            time_control: game.time_control,
            move_time_limit: game.move_time_limit,
            byoyomi: game.byoyomi,
            red_time: game.red_time,
            black_time: game.black_time,
            created_at: game.created_at,
        });
    }

    Ok(Json(result))
}

/// DELETE /api/games/{id}
pub async fn delete_game(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let game = state.game_repo.find_by_id(id).await?
        .ok_or(AppError::NotFound("Game not found".into()))?;

    // Only players in the game can delete it
    let is_player = game.red_player_id == Some(auth.user_id)
        || game.black_player_id == Some(auth.user_id);
    if !is_player {
        return Err(AppError::Forbidden("Only players in this game can delete it".into()));
    }

    state.game_repo.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
