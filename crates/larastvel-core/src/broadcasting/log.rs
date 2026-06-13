use async_trait::async_trait;

use super::{
    BroadcastError, BroadcastMessage, Broadcaster, ChannelAuthCallback, PresenceChannelData,
};

/// A broadcaster that logs messages instead of sending them over the wire.
///
/// Useful for development and testing environments where you don't have
/// a real broadcasting service configured.
#[derive(Debug, Clone)]
pub struct LogBroadcaster {
    name: String,
}

impl LogBroadcaster {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

#[async_trait]
impl Broadcaster for LogBroadcaster {
    async fn broadcast(&self, message: BroadcastMessage) -> Result<(), BroadcastError> {
        tracing::info!(
            target: "larastvel::broadcasting",
            "📡 [{}] Event: {} | Channels: {:?} | Data: {}",
            self.name,
            message.event,
            message.channels,
            serde_json::to_string(&message.data).unwrap_or_default(),
        );
        Ok(())
    }

    async fn authenticate(
        &self,
        channel_name: &str,
        socket_id: &str,
        _callback: Option<&ChannelAuthCallback>,
    ) -> Result<String, BroadcastError> {
        // In dev mode, allow all private channel subscriptions
        let response = serde_json::json!({
            "auth": format!("{}:dev-auth-token", self.name),
        });
        tracing::debug!(
            target: "larastvel::broadcasting",
            "🔐 [{}] Auth granted for {} (socket: {})",
            self.name,
            channel_name,
            socket_id,
        );
        serde_json::to_string(&response)
            .map_err(|e| BroadcastError::AuthError(format!("Serialization error: {}", e)))
    }

    async fn authenticate_presence(
        &self,
        channel_name: &str,
        socket_id: &str,
        channel_data: &PresenceChannelData,
        _callback: Option<&ChannelAuthCallback>,
    ) -> Result<String, BroadcastError> {
        let channel_data_json = serde_json::to_string(channel_data)
            .map_err(|e| BroadcastError::AuthError(format!("Serialization error: {}", e)))?;
        let response = serde_json::json!({
            "auth": format!("{}:dev-auth-token", self.name),
            "channel_data": channel_data_json,
        });
        tracing::debug!(
            target: "larastvel::broadcasting",
            "🔐 [{}] Presence auth granted for {} (user: {}, socket: {})",
            self.name,
            channel_name,
            channel_data.user_id,
            socket_id,
        );
        serde_json::to_string(&response)
            .map_err(|e| BroadcastError::AuthError(format!("Serialization error: {}", e)))
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_log_broadcaster_broadcast() {
        let broadcaster = LogBroadcaster::new("log");
        let message = BroadcastMessage::new(
            "test-event",
            serde_json::json!({"key": "value"}),
            vec!["public-channel".to_string()],
        );
        let result = broadcaster.broadcast(message).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_log_broadcaster_name() {
        let broadcaster = LogBroadcaster::new("my-broadcaster");
        assert_eq!(broadcaster.name(), "my-broadcaster");
    }

    #[tokio::test]
    async fn test_log_broadcaster_authenticate() {
        let broadcaster = LogBroadcaster::new("log");
        let result = broadcaster
            .authenticate("private-chat", "123.456", None)
            .await
            .unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed["auth"].is_string());
    }

    #[tokio::test]
    async fn test_log_broadcaster_authenticate_presence() {
        let broadcaster = LogBroadcaster::new("log");
        let channel_data = PresenceChannelData {
            user_id: "user-1".to_string(),
            user_info: None,
        };
        let result = broadcaster
            .authenticate_presence("presence-chat", "123.456", &channel_data, None)
            .await
            .unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed["auth"].is_string());
        assert!(parsed["channel_data"].is_string());
    }

    #[tokio::test]
    async fn test_log_broadcaster_multiple_messages() {
        let broadcaster = LogBroadcaster::new("log");
        for i in 0..3 {
            let msg = BroadcastMessage::new(
                &format!("event-{}", i),
                serde_json::json!({"i": i}),
                vec!["ch".to_string()],
            );
            assert!(broadcaster.broadcast(msg).await.is_ok());
        }
    }
}
