use tokio::sync::mpsc;
use uuid::Uuid;

/// 封装 WebSocket 连接和用户信息
#[derive(Debug)]
pub struct Client {
    pub user_id: Uuid,
    pub username: String,
    pub sender: mpsc::UnboundedSender<String>,
}

impl Client {
    pub fn new(user_id: Uuid, username: String, sender: mpsc::UnboundedSender<String>) -> Self {
        Self { user_id, username, sender }
    }

    pub fn send(&self, msg: &str) -> bool {
        self.sender.send(msg.to_string()).is_ok()
    }
}
