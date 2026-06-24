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
    let (game, red_player, black_player) = game_repo.find_with_players(game_id).await?
        .ok_or(AppError::NotFound("Game not found".into()))?;

    // If user is already in this game, return current state (idempotent).
    // This handles double-clicks, page refresh, and reconnect scenarios.
    if game.red_player_id == Some(joining_user_id) || game.black_player_id == Some(joining_user_id) {
        return Ok(GameInfo {
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
        });
    }

    if game.status != "waiting" {
        return Err(AppError::BadRequest("Game is not waiting for players".into()));
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

/// 再来一局 (创建新对局，交换颜色，相同时间设置)
pub async fn create_rematch(
    game_repo: &GameRepository,
    original_game_id: Uuid,
    requester_id: Uuid,
) -> Result<(Uuid, String), AppError> {
    // Load the original game
    let (game, _red_player, _black_player) = game_repo.find_with_players(original_game_id).await?
        .ok_or(AppError::NotFound("Game not found".into()))?;

    // Game must be finished
    if game.status != "finished" {
        return Err(AppError::BadRequest("Can only rematch finished games".into()));
    }

    // Requester must be a player
    let was_red = game.red_player_id == Some(requester_id);
    let was_black = game.black_player_id == Some(requester_id);
    if !was_red && !was_black {
        return Err(AppError::Forbidden("Only players can request rematch".into()));
    }

    // Swap colors: if requester was red, they are now black (and vice versa)
    let requester_color = if was_red { "black" } else { "red" };

    // Create new game with same time control, requester in swapped color
    let new_game = game_repo.create(
        requester_id,
        requester_color,
        game.time_control,
        game.move_time_limit,
        game.byoyomi,
    ).await?;

    Ok((new_game.id, requester_color.to_string()))
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
