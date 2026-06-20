use uuid::Uuid;

use crate::db::models::*;
use crate::db::repositories::game_repo::GameRepository;
use crate::error::AppError;

/// Validate player_color from a CreateGameRequest.
/// Returns the validated color string.
pub fn validate_player_color(color: Option<&str>) -> Result<&str, AppError> {
    let color = color.unwrap_or("red");
    if color != "red" && color != "black" {
        return Err(AppError::BadRequest("player_color must be 'red' or 'black'".into()));
    }
    Ok(color)
}

/// 创建对局
pub async fn create_game(
    game_repo: &GameRepository,
    creator_id: Uuid,
    data: &CreateGameRequest,
) -> Result<(Game, String), AppError> {
    let color = validate_player_color(data.player_color.as_deref())?;
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
    game_id: Uuid,
    joining_user_id: Uuid,
) -> Result<GameInfo, AppError> {
    let (game, _red_player, _black_player) = game_repo.find_with_players(game_id).await?
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

    // Join the game (updates the DB row)
    game_repo.join_game(game_id, joining_user_id).await?;

    // Re-fetch with player info in a single query to build the response
    let (game, red_player, black_player) = game_repo.find_with_players(game_id).await?
        .ok_or(AppError::NotFound("Game not found".into()))?;

    Ok(GameInfo {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_player_color_red() {
        assert_eq!(validate_player_color(Some("red")).unwrap(), "red");
    }

    #[test]
    fn test_validate_player_color_black() {
        assert_eq!(validate_player_color(Some("black")).unwrap(), "black");
    }

    #[test]
    fn test_validate_player_color_invalid() {
        let result = validate_player_color(Some("green"));
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::BadRequest(msg) => assert!(msg.contains("player_color")),
            _ => panic!("Expected BadRequest error"),
        }
    }

    #[test]
    fn test_validate_player_color_none_default() {
        assert_eq!(validate_player_color(None).unwrap(), "red");
    }
}
