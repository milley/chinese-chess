use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use uuid::Uuid;

use crate::db::repositories::game_repo::GameRepository;
use crate::websocket::room::{GameRoom, MoveResult};

/// 房间管理器
pub struct RoomManager {
    /// 活跃房间映射
    rooms: Arc<RwLock<HashMap<Uuid, Arc<GameRoom>>>>,
    /// 数据库仓库
    game_repo: GameRepository,
}

impl RoomManager {
    pub fn with_game_repo(game_repo: GameRepository) -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
            game_repo,
        }
    }

    /// 获取或创建房间 (懒加载)
    pub async fn get_or_create_room(&self, game_id: Uuid) -> Result<Arc<GameRoom>, String> {
        let rooms = self.rooms.read().await;
        if let Some(room) = rooms.get(&game_id) {
            return Ok(room.clone());
        }
        drop(rooms);

        // 从数据库加载
        let game = self.game_repo.find_by_id(game_id).await
            .map_err(|e| format!("DB error: {}", e))?
            .ok_or("Game not found".to_string())?;

        let room = Arc::new(GameRoom::new(game_id, &game.fen, self.game_repo.clone()));

        let mut rooms = self.rooms.write().await;
        rooms.insert(game_id, room.clone());

        Ok(room)
    }

    /// 执行走法 (REST 和 WS 统一入口)
    pub async fn make_move(
        &self,
        game_id: Uuid,
        user_id: Uuid,
        player_color: chess_engine::Color,
        from: &str,
        to: &str,
    ) -> Result<MoveResult, String> {
        let room = self.get_or_create_room(game_id).await?;
        room.make_move(user_id, player_color, from, to).await
    }

    /// 启动超时检查器
    pub fn start_timeout_checker(&self) {
        let rooms = self.rooms.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                let rooms_guard = rooms.read().await;
                for (_, _room) in rooms_guard.iter() {
                    // TODO: Check timeout for each room
                    // For now, timeout checking is a placeholder
                }
            }
        });
    }
}

impl Clone for RoomManager {
    fn clone(&self) -> Self {
        Self {
            rooms: self.rooms.clone(),
            game_repo: self.game_repo.clone(),
        }
    }
}
