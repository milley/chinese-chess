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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[test]
    fn test_client_new() {
        let user_id = Uuid::new_v4();
        let username = "test_user".to_string();
        let (tx, _rx) = mpsc::unbounded_channel();
        let client = Client::new(user_id, username.clone(), tx);
        assert_eq!(client.user_id, user_id);
        assert_eq!(client.username, username);
    }

    #[test]
    fn test_client_send_success() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let client = Client::new(Uuid::new_v4(), "test_user".to_string(), tx);
        let result = client.send("hello");
        assert!(result);
        let msg = rx.try_recv().unwrap();
        assert_eq!(msg, "hello");
    }

    #[test]
    fn test_client_send_channel_closed() {
        let (tx, rx) = mpsc::unbounded_channel();
        let client = Client::new(Uuid::new_v4(), "test_user".to_string(), tx);
        drop(rx);
        let result = client.send("hello");
        assert!(!result);
    }
}
