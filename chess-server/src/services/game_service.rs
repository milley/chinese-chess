use uuid::Uuid;

use crate::db::models::*;
use crate::db::repositories::game_repo::GameRepository;
use crate::error::AppError;

/// 创建对局
pub async fn create_game(
    game_repo: &GameRepository,
    creator_id: Uuid,
    data: &CreateGameRequest,
) -> Result<(Game, String), AppError> {
    let color = data.player_color.as_deref().unwrap_or("red");
    if color != "red" && color != "black" {
        return Err(AppError::BadRequest("player_color must be 'red' or 'black'".into()));
    }
    let game = game_repo.create(
        creator_id,
        color,
        data.time_control,
        data.move_time_limit,
        data.byoyomi,
    ).await?;
    Ok((game, color.to_string()))
}

/// 加入对局
pub async fn join_game(
    game_repo: &GameRepository,
    user_repo: &crate::db::repositories::user_repo::UserRepository,
    game_id: Uuid,
    joining_user_id: Uuid,
    _joining_username: String,
) -> Result<GameInfo, AppError> {
    let game = game_repo.find_by_id(game_id).await?
        .ok_or(AppError::NotFound("Game not found".into()))?;

    if game.status != "waiting" {
        return Err(AppError::BadRequest("Game is not waiting for players".into()));
    }

    // Check if user is already in this game
    if game.red_player_id == Some(joining_user_id) || game.black_player_id == Some(joining_user_id) {
        return Err(AppError::BadRequest("You are already in this game".into()));
    }

    // Check if both slots are filled
    if game.red_player_id.is_some() && game.black_player_id.is_some() {
        return Err(AppError::BadRequest("Game is already full".into()));
    }

    let game = game_repo.join_game(game_id, joining_user_id).await?;

    // Build response
    let red_player = match game.red_player_id {
        Some(pid) => user_repo.find_by_id(pid).await?.map(UserInfo::from),
        None => None,
    };
    let black_player = match game.black_player_id {
        Some(pid) => user_repo.find_by_id(pid).await?.map(UserInfo::from),
        None => None,
    };

    Ok(GameInfo {
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
    })
}

/// 删除对局 (仅玩家可删除，且仅限等待中或已结束的对局)
pub async fn delete_game(
    game_repo: &GameRepository,
    game_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    let game = game_repo.find_by_id(game_id).await?
        .ok_or(AppError::NotFound("Game not found".into()))?;

    let is_player = game.red_player_id == Some(user_id) || game.black_player_id == Some(user_id);
    if !is_player {
        return Err(AppError::Forbidden("Only players in this game can delete it".into()));
    }

    // Prevent deletion of games that are currently in progress
    if game.status == "playing" {
        return Err(AppError::BadRequest("Cannot delete a game that is in progress. Resign or wait for it to finish.".into()));
    }

    game_repo.delete(game_id).await?;
    Ok(())
}
