use tokio::sync::mpsc;
use uuid::Uuid;

/// Maximum number of queued outbound messages per client.
/// If a client falls this far behind, they're likely dead and will be disconnected.
const WS_CHANNEL_CAPACITY: usize = 64;

/// 封装 WebSocket 连接和用户信息
#[derive(Debug)]
pub struct Client {
    pub user_id: Uuid,
    pub username: String,
    sender: mpsc::Sender<String>,
}

impl Client {
    pub fn new(user_id: Uuid, username: String, sender: mpsc::Sender<String>) -> Self {
        Self { user_id, username, sender }
    }

    /// Create a new bounded channel pair for WebSocket communication.
    pub fn create_channel() -> (mpsc::Sender<String>, mpsc::Receiver<String>) {
        mpsc::channel(WS_CHANNEL_CAPACITY)
    }

    /// Send a message to the client. Returns false if the channel is closed or full.
    /// Uses `try_send` to avoid blocking — if the channel is full, the message is dropped.
    pub fn send(&self, msg: &str) -> bool {
        self.sender.try_send(msg.to_string()).is_ok()
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
        let (tx, _rx) = mpsc::channel(WS_CHANNEL_CAPACITY);
        let client = Client::new(user_id, username.clone(), tx);
        assert_eq!(client.user_id, user_id);
        assert_eq!(client.username, username);
    }

    #[test]
    fn test_client_send_success() {
        let (tx, mut rx) = mpsc::channel(WS_CHANNEL_CAPACITY);
        let client = Client::new(Uuid::new_v4(), "test_user".to_string(), tx);
        let result = client.send("hello");
        assert!(result);
        let msg = rx.try_recv().unwrap();
        assert_eq!(msg, "hello");
    }

    #[test]
    fn test_client_send_channel_closed() {
        let (tx, rx) = mpsc::channel(WS_CHANNEL_CAPACITY);
        let client = Client::new(Uuid::new_v4(), "test_user".to_string(), tx);
        drop(rx);
        let result = client.send("hello");
        assert!(!result);
    }

    #[test]
    fn test_client_send_channel_full() {
        let (tx, _rx) = mpsc::channel(2);
        let client = Client::new(Uuid::new_v4(), "test_user".to_string(), tx);
        assert!(client.send("msg1"));
        assert!(client.send("msg2"));
        // Channel is full — next send should fail (message dropped)
        assert!(!client.send("msg3"));
    }
}
