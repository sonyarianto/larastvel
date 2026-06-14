use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::Extension;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, RwLock};

use super::{
    BroadcastError, BroadcastMessage, Broadcaster, ChannelAuthCallback, PresenceChannelData,
};

// ---------------------------------------------------------------------------
// Shared subscriber registry
// ---------------------------------------------------------------------------

type Sender = mpsc::UnboundedSender<String>;

/// Maps channel names to connected WebSocket senders.
#[derive(Debug, Clone)]
pub struct SubscriberRegistry {
    channels: Arc<RwLock<HashMap<String, Vec<Sender>>>>,
}

impl SubscriberRegistry {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Subscribe a sender to a channel.
    pub async fn subscribe(&self, channel: &str, sender: Sender) {
        let mut map = self.channels.write().await;
        map.entry(channel.to_string()).or_default().push(sender);
    }

    /// Unsubscribe a sender from a channel.
    pub async fn unsubscribe(&self, channel: &str, sender: &Sender) {
        let mut map = self.channels.write().await;
        if let Some(senders) = map.get_mut(channel) {
            senders.retain(|s| !s.same_channel(sender));
            if senders.is_empty() {
                map.remove(channel);
            }
        }
    }

    /// Remove a sender from all channels it belongs to.
    pub async fn remove_sender(&self, sender: &Sender) {
        let mut map = self.channels.write().await;
        map.retain(|_, senders| {
            senders.retain(|s| !s.same_channel(sender));
            !senders.is_empty()
        });
    }

    /// Send a message to all subscribers of the given channels.
    pub async fn broadcast_to_channels(&self, channels: &[String], message: &str) {
        let map = self.channels.read().await;
        for channel in channels {
            if let Some(senders) = map.get(channel) {
                for sender in senders {
                    let _ = sender.send(message.to_string());
                }
            }
        }
    }

    /// Number of channels with subscribers.
    pub async fn channel_count(&self) -> usize {
        self.channels.read().await.len()
    }

    /// Total number of subscribers across all channels.
    pub async fn subscriber_count(&self) -> usize {
        self.channels.read().await.values().map(|v| v.len()).sum()
    }
}

impl Default for SubscriberRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// NativeBroadcaster
// ---------------------------------------------------------------------------

/// A self-hosted WebSocket broadcaster.
///
/// Clients connect via WebSocket to a provided endpoint, subscribe to
/// channels, and receive real-time events when the application broadcasts.
///
/// This replaces the need for third-party services like Pusher or Ably
/// in development and small-scale deployments.
#[derive(Debug, Clone)]
pub struct NativeBroadcaster {
    name: String,
    registry: SubscriberRegistry,
}

impl NativeBroadcaster {
    /// Create a new native broadcaster.
    pub fn new(name: &str, registry: SubscriberRegistry) -> Self {
        Self {
            name: name.to_string(),
            registry,
        }
    }

    /// Returns a reference to the shared subscriber registry.
    ///
    /// Used to mount the WebSocket upgrade handler.
    pub fn registry(&self) -> &SubscriberRegistry {
        &self.registry
    }
}

#[async_trait]
impl Broadcaster for NativeBroadcaster {
    async fn broadcast(&self, message: BroadcastMessage) -> Result<(), BroadcastError> {
        let channels = message.channels.clone();
        let payload = serde_json::json!({
            "event": message.event,
            "data": message.data,
            "channel": channels.first().map(|s| s.as_str()),
        });
        let text = serde_json::to_string(&payload)
            .map_err(|e| BroadcastError::Failed(format!("Serialization error: {}", e)))?;
        self.registry.broadcast_to_channels(&channels, &text).await;
        Ok(())
    }

    async fn authenticate(
        &self,
        _channel_name: &str,
        _socket_id: &str,
        _callback: Option<&ChannelAuthCallback>,
    ) -> Result<String, BroadcastError> {
        let response = serde_json::json!({
            "auth": format!("{}:native-auth", self.name),
        });
        serde_json::to_string(&response)
            .map_err(|e| BroadcastError::AuthError(format!("Serialization error: {}", e)))
    }

    async fn authenticate_presence(
        &self,
        _channel_name: &str,
        _socket_id: &str,
        channel_data: &PresenceChannelData,
        _callback: Option<&ChannelAuthCallback>,
    ) -> Result<String, BroadcastError> {
        let channel_data_json = serde_json::to_string(channel_data)
            .map_err(|e| BroadcastError::AuthError(format!("Serialization error: {}", e)))?;
        let response = serde_json::json!({
            "auth": format!("{}:native-auth", self.name),
            "channel_data": channel_data_json,
        });
        serde_json::to_string(&response)
            .map_err(|e| BroadcastError::AuthError(format!("Serialization error: {}", e)))
    }

    fn name(&self) -> &str {
        &self.name
    }
}

// ---------------------------------------------------------------------------
// WebSocket handler
// ---------------------------------------------------------------------------

/// Axum handler that upgrades an HTTP connection to a WebSocket.
///
/// Clients connect to this endpoint, then send JSON control messages
/// to subscribe/unsubscribe from channels:
///
/// ```json
/// {"type": "subscribe", "channel": "chat"}
/// {"type": "unsubscribe", "channel": "chat"}
/// {"type": "ping"}
/// ```
///
/// The server responds with:
/// ```json
/// {"type": "subscribed", "channel": "chat"}
/// {"type": "unsubscribed", "channel": "chat"}
/// {"type": "pong"}
/// ```
///
/// Broadcast events are pushed as:
/// ```json
/// {"event": "new-message", "data": {...}, "channel": "chat"}
/// ```
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(registry): Extension<SubscriberRegistry>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, registry))
}

async fn handle_socket(socket: WebSocket, registry: SubscriberRegistry) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    // Spawn a task to forward messages from rx to the WebSocket sender.
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    let mut subscribed_channels: Vec<String> = Vec::new();

    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
                if let Ok(cmd) = serde_json::from_str::<ClientCommand>(&text) {
                    match cmd.r#type.as_str() {
                        "subscribe" => {
                            if let Some(ch) = cmd.channel {
                                registry.subscribe(&ch, tx.clone()).await;
                                subscribed_channels.push(ch.clone());
                                let _ = tx
                                    .send(format!(r#"{{"type":"subscribed","channel":"{}"}}"#, ch));
                            }
                        }
                        "unsubscribe" => {
                            if let Some(ch) = cmd.channel {
                                registry.unsubscribe(&ch, &tx).await;
                                subscribed_channels.retain(|c| c != &ch);
                                let _ = tx.send(format!(
                                    r#"{{"type":"unsubscribed","channel":"{}"}}"#,
                                    ch
                                ));
                            }
                        }
                        "ping" => {
                            let _ = tx.send(r#"{"type":"pong"}"#.to_string());
                        }
                        _ => {}
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    // Clean up: remove this sender from all subscribed channels.
    for ch in &subscribed_channels {
        registry.unsubscribe(ch, &tx).await;
    }
    registry.remove_sender(&tx).await;
    send_task.abort();
}

#[derive(serde::Deserialize)]
struct ClientCommand {
    r#type: String,
    channel: Option<String>,
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_subscriber_registry_subscribe() {
        let registry = SubscriberRegistry::new();
        let (tx, _rx) = mpsc::unbounded_channel();

        registry.subscribe("chat", tx).await;
        assert_eq!(registry.channel_count().await, 1);
        assert_eq!(registry.subscriber_count().await, 1);
    }

    #[tokio::test]
    async fn test_subscriber_registry_unsubscribe() {
        let registry = SubscriberRegistry::new();
        let (tx, _rx) = mpsc::unbounded_channel();

        registry.subscribe("chat", tx.clone()).await;
        registry.subscribe("chat", tx.clone()).await;
        assert_eq!(registry.subscriber_count().await, 2);

        registry.unsubscribe("chat", &tx).await;
        // Both senders are the same channel, so both get removed
        assert_eq!(registry.channel_count().await, 0);
    }

    #[tokio::test]
    async fn test_subscriber_registry_remove_sender() {
        let registry = SubscriberRegistry::new();
        let (tx, _rx) = mpsc::unbounded_channel();

        registry.subscribe("chat", tx.clone()).await;
        registry.subscribe("alerts", tx.clone()).await;
        assert_eq!(registry.channel_count().await, 2);

        registry.remove_sender(&tx).await;
        assert_eq!(registry.channel_count().await, 0);
    }

    #[tokio::test]
    async fn test_subscriber_registry_broadcast_to_channels() {
        let registry = SubscriberRegistry::new();
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();

        registry.subscribe("chat", tx.clone()).await;
        registry
            .broadcast_to_channels(&["chat".to_string()], "hello")
            .await;

        let received = rx.recv().await;
        assert_eq!(received, Some("hello".to_string()));
    }

    #[tokio::test]
    async fn test_subscriber_registry_broadcast_no_subscribers() {
        let registry = SubscriberRegistry::new();
        // Should not panic
        registry
            .broadcast_to_channels(&["empty".to_string()], "msg")
            .await;
    }

    #[tokio::test]
    async fn test_native_broadcaster_broadcast() {
        let registry = SubscriberRegistry::new();
        let broadcaster = NativeBroadcaster::new("native", registry.clone());

        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        registry.subscribe("chat", tx).await;

        let message = BroadcastMessage::new(
            "test-event",
            serde_json::json!({"key": "value"}),
            vec!["chat".to_string()],
        );
        broadcaster.broadcast(message).await.unwrap();

        let received = rx.recv().await;
        assert!(received.is_some());
        let parsed: serde_json::Value = serde_json::from_str(&received.unwrap()).unwrap();
        assert_eq!(parsed["event"], "test-event");
        assert_eq!(parsed["data"]["key"], "value");
    }

    #[tokio::test]
    async fn test_native_broadcaster_broadcast_multiple_channels() {
        let registry = SubscriberRegistry::new();
        let broadcaster = NativeBroadcaster::new("native", registry.clone());

        let (tx1, mut rx1) = mpsc::unbounded_channel();
        let (tx2, mut rx2) = mpsc::unbounded_channel();
        registry.subscribe("chat", tx1).await;
        registry.subscribe("alerts", tx2).await;

        let message = BroadcastMessage::new(
            "multi",
            serde_json::json!({"n": 1}),
            vec!["chat".to_string(), "alerts".to_string()],
        );
        broadcaster.broadcast(message).await.unwrap();

        assert!(rx1.recv().await.is_some());
        assert!(rx2.recv().await.is_some());
    }

    #[tokio::test]
    async fn test_native_broadcaster_authenticate() {
        let registry = SubscriberRegistry::new();
        let broadcaster = NativeBroadcaster::new("native", registry);

        let result = broadcaster
            .authenticate("private-chat", "123.456", None)
            .await
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed["auth"].is_string());
    }

    #[tokio::test]
    async fn test_native_broadcaster_authenticate_presence() {
        let registry = SubscriberRegistry::new();
        let broadcaster = NativeBroadcaster::new("native", registry);

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

    #[test]
    fn test_native_broadcaster_name() {
        let registry = SubscriberRegistry::new();
        let broadcaster = NativeBroadcaster::new("my-native", registry);
        assert_eq!(broadcaster.name(), "my-native");
    }

    #[tokio::test]
    async fn test_registry_default_is_empty() {
        let registry = SubscriberRegistry::default();
        assert_eq!(registry.channel_count().await, 0);
        assert_eq!(registry.subscriber_count().await, 0);
    }

    #[tokio::test]
    async fn test_registry_multiple_channels() {
        let registry = SubscriberRegistry::new();
        let (tx, _rx) = mpsc::unbounded_channel();

        registry.subscribe("chat", tx.clone()).await;
        registry.subscribe("alerts", tx.clone()).await;
        registry.subscribe("system", tx.clone()).await;

        assert_eq!(registry.channel_count().await, 3);
        assert_eq!(registry.subscriber_count().await, 3);
    }
}
