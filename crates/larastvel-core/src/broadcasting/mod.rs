pub mod ably;
pub mod log;
pub mod pusher;

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum BroadcastError {
    #[error("Broadcast failed: {0}")]
    Failed(String),
    #[error("Invalid channel: {0}")]
    InvalidChannel(String),
    #[error("Auth error: {0}")]
    AuthError(String),
    #[error("Broadcaster [{0}] not configured")]
    NotConfigured(String),
}

/// The payload that gets sent to connected clients.
#[derive(Debug, Clone, Serialize)]
pub struct BroadcastMessage {
    pub event: String,
    pub data: serde_json::Value,
    pub channels: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub socket_id: Option<String>,
}

impl BroadcastMessage {
    pub fn new(event: &str, data: serde_json::Value, channels: Vec<String>) -> Self {
        Self {
            event: event.to_string(),
            data,
            channels,
            socket_id: None,
        }
    }

    pub fn with_socket_id(mut self, socket_id: &str) -> Self {
        self.socket_id = Some(socket_id.to_string());
        self
    }
}

/// A channel that clients can subscribe to.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Channel {
    /// Anyone can subscribe without authentication.
    Public(String),
    /// Requires authentication via the application.
    Private(String),
    /// Requires authentication and tracks connected users.
    Presence {
        name: String,
        #[serde(skip)]
        channel_data: Option<PresenceChannelData>,
    },
}

impl Channel {
    pub fn name(&self) -> &str {
        match self {
            Channel::Public(name) => name,
            Channel::Private(name) => name,
            Channel::Presence { name, .. } => name,
        }
    }

    pub fn is_public(&self) -> bool {
        matches!(self, Channel::Public(_))
    }

    pub fn is_private(&self) -> bool {
        matches!(self, Channel::Private(_))
    }

    pub fn is_presence(&self) -> bool {
        matches!(self, Channel::Presence { .. })
    }

    pub fn to_channel_name(&self) -> String {
        match self {
            Channel::Public(name) => name.clone(),
            Channel::Private(name) => format!("private-{}", name),
            Channel::Presence { name, .. } => format!("presence-{}", name),
        }
    }

    /// Create a public channel.
    pub fn public(name: &str) -> Self {
        Channel::Public(name.to_string())
    }

    /// Create a private channel that requires auth.
    pub fn private(name: &str) -> Self {
        Channel::Private(name.to_string())
    }

    /// Create a presence channel that tracks users.
    pub fn presence(name: &str, user_id: &str, user_info: Option<serde_json::Value>) -> Self {
        let channel_data = PresenceChannelData {
            user_id: user_id.to_string(),
            user_info,
        };
        Channel::Presence {
            name: name.to_string(),
            channel_data: Some(channel_data),
        }
    }

    /// Create a presence channel without explicit user data (for auth response).
    pub fn presence_channel(name: &str) -> Self {
        Channel::Presence {
            name: name.to_string(),
            channel_data: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PresenceChannelData {
    pub user_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_info: Option<serde_json::Value>,
}

/// Trait for events that can be broadcast over WebSockets.
#[async_trait]
pub trait BroadcastEvent: Send + Sync + std::fmt::Debug + serde::Serialize {
    /// The event name broadcast to clients (e.g. "new-message").
    fn broadcast_event_name(&self) -> &str;

    /// The channels this event should be broadcast on.
    fn broadcast_channels(&self) -> Vec<Channel>;

    /// The data payload sent to clients.
    fn broadcast_data(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }

    /// Whether this event should be broadcast to others (excluding sender).
    fn broadcast_via(&self) -> Option<String> {
        None
    }
}

// Auto-implement Serialize requirement for BroadcastEvent implementors.
// We use a blanket impl approach.

/// Driver trait for broadcasting messages to clients.
#[async_trait]
pub trait Broadcaster: Send + Sync + std::fmt::Debug {
    /// Broadcast a message to one or more channels.
    async fn broadcast(&self, message: BroadcastMessage) -> Result<(), BroadcastError>;

    /// Authenticate a private channel subscription.
    async fn authenticate(
        &self,
        channel_name: &str,
        socket_id: &str,
        callback: Option<&ChannelAuthCallback>,
    ) -> Result<String, BroadcastError>;

    /// Authenticate a presence channel subscription.
    async fn authenticate_presence(
        &self,
        channel_name: &str,
        socket_id: &str,
        channel_data: &PresenceChannelData,
        callback: Option<&ChannelAuthCallback>,
    ) -> Result<String, BroadcastError>;

    fn name(&self) -> &str;
}

/// Callback for authorizing private/presence channel access.
/// Takes (user_id, channel_name) and returns Ok(()) if authorized.
pub type ChannelAuthCallback = dyn Fn(&str, &str) -> Result<(), BroadcastError> + Send + Sync;

/// Manager for multiple broadcast drivers.
#[derive(Debug, Clone)]
pub struct BroadcastManager {
    broadcasters: HashMap<String, Arc<dyn Broadcaster>>,
    default: String,
}

impl BroadcastManager {
    pub fn new(default: &str) -> Self {
        Self {
            broadcasters: HashMap::new(),
            default: default.to_string(),
        }
    }

    pub fn register<B: Broadcaster + 'static>(&mut self, name: &str, broadcaster: B) {
        self.broadcasters
            .insert(name.to_string(), Arc::new(broadcaster));
    }

    pub fn broadcaster(&self, name: &str) -> Result<Arc<dyn Broadcaster>, BroadcastError> {
        self.broadcasters
            .get(name)
            .cloned()
            .ok_or_else(|| BroadcastError::NotConfigured(name.to_string()))
    }

    pub fn default_broadcaster(&self) -> Result<Arc<dyn Broadcaster>, BroadcastError> {
        self.broadcaster(&self.default)
    }

    pub fn set_default(&mut self, name: &str) {
        self.default = name.to_string();
    }

    pub fn default_name(&self) -> &str {
        &self.default
    }

    pub fn broadcaster_names(&self) -> Vec<String> {
        self.broadcasters.keys().cloned().collect()
    }

    /// Convenience: broadcast an event using the default broadcaster.
    pub async fn broadcast<E: BroadcastEvent>(&self, event: &E) -> Result<(), BroadcastError> {
        let message = event_to_message(event);
        let broadcaster = self.default_broadcaster()?;
        broadcaster.broadcast(message).await
    }

    /// Convenience: broadcast using a named broadcaster.
    pub async fn broadcast_via<E: BroadcastEvent>(
        &self,
        driver: &str,
        event: &E,
    ) -> Result<(), BroadcastError> {
        let message = event_to_message(event);
        let broadcaster = self.broadcaster(driver)?;
        broadcaster.broadcast(message).await
    }
}

fn event_to_message<E: BroadcastEvent>(event: &E) -> BroadcastMessage {
    let channels: Vec<String> = event
        .broadcast_channels()
        .iter()
        .map(|c| c.to_channel_name())
        .collect();
    let mut message = BroadcastMessage::new(
        event.broadcast_event_name(),
        event.broadcast_data(),
        channels,
    );
    if let Some(socket_id) = event.broadcast_via() {
        message = message.with_socket_id(&socket_id);
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[derive(Debug, serde::Serialize)]
    struct TestEvent {
        pub message: String,
    }

    #[async_trait]
    impl BroadcastEvent for TestEvent {
        fn broadcast_event_name(&self) -> &str {
            "test-event"
        }

        fn broadcast_channels(&self) -> Vec<Channel> {
            vec![Channel::public("test-channel")]
        }

        fn broadcast_data(&self) -> serde_json::Value {
            serde_json::json!({ "message": self.message })
        }
    }

    #[derive(Debug)]
    struct TestBroadcaster {
        name: String,
        called: Arc<AtomicBool>,
    }

    #[async_trait]
    impl Broadcaster for TestBroadcaster {
        async fn broadcast(&self, _message: BroadcastMessage) -> Result<(), BroadcastError> {
            self.called.store(true, Ordering::SeqCst);
            Ok(())
        }

        async fn authenticate(
            &self,
            channel_name: &str,
            _socket_id: &str,
            _callback: Option<&ChannelAuthCallback>,
        ) -> Result<String, BroadcastError> {
            Ok(format!("auth-{}", channel_name))
        }

        async fn authenticate_presence(
            &self,
            channel_name: &str,
            _socket_id: &str,
            _channel_data: &PresenceChannelData,
            _callback: Option<&ChannelAuthCallback>,
        ) -> Result<String, BroadcastError> {
            Ok(format!("presence-auth-{}", channel_name))
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    #[tokio::test]
    async fn test_channel_public() {
        let ch = Channel::public("chat");
        assert!(ch.is_public());
        assert!(!ch.is_private());
        assert_eq!(ch.name(), "chat");
        assert_eq!(ch.to_channel_name(), "chat");
    }

    #[tokio::test]
    async fn test_channel_private() {
        let ch = Channel::private("chat");
        assert!(ch.is_private());
        assert!(!ch.is_public());
        assert_eq!(ch.name(), "chat");
        assert_eq!(ch.to_channel_name(), "private-chat");
    }

    #[tokio::test]
    async fn test_channel_presence() {
        let ch = Channel::presence("chat", "user-1", Some(serde_json::json!({"name": "Alice"})));
        assert!(ch.is_presence());
        assert_eq!(ch.name(), "chat");
        assert_eq!(ch.to_channel_name(), "presence-chat");
    }

    #[tokio::test]
    async fn test_broadcast_message() {
        let msg = BroadcastMessage::new(
            "new-message",
            serde_json::json!({"text": "hello"}),
            vec!["chat".to_string()],
        );
        assert_eq!(msg.event, "new-message");
        assert_eq!(msg.channels, vec!["chat"]);
        assert!(msg.socket_id.is_none());
    }

    #[tokio::test]
    async fn test_broadcast_message_with_socket_id() {
        let msg =
            BroadcastMessage::new("e", serde_json::json!({}), vec![]).with_socket_id("123.456");
        assert_eq!(msg.socket_id, Some("123.456".to_string()));
    }

    #[tokio::test]
    async fn test_broadcaster_called() {
        let called = Arc::new(AtomicBool::new(false));
        let broadcaster = TestBroadcaster {
            name: "test".to_string(),
            called: called.clone(),
        };

        let event = TestEvent {
            message: "hello".to_string(),
        };

        let mut manager = BroadcastManager::new("test");
        manager.register("test", broadcaster);

        manager.broadcast(&event).await.unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_broadcast_via_named() {
        let called = Arc::new(AtomicBool::new(false));
        let broadcaster = TestBroadcaster {
            name: "pusher".to_string(),
            called: called.clone(),
        };

        let event = TestEvent {
            message: "via pusher".to_string(),
        };

        let mut manager = BroadcastManager::new("log");
        manager.register("pusher", broadcaster);

        manager.broadcast_via("pusher", &event).await.unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_broadcast_manager_default() {
        let mut manager = BroadcastManager::new("log");
        manager.register(
            "log",
            TestBroadcaster {
                name: "log".to_string(),
                called: Arc::new(AtomicBool::new(false)),
            },
        );
        manager.register(
            "pusher",
            TestBroadcaster {
                name: "pusher".to_string(),
                called: Arc::new(AtomicBool::new(false)),
            },
        );

        let default = manager.default_broadcaster().unwrap();
        assert_eq!(default.name(), "log");

        let named = manager.broadcaster("pusher").unwrap();
        assert_eq!(named.name(), "pusher");
    }

    #[tokio::test]
    async fn test_broadcast_manager_missing() {
        let manager = BroadcastManager::new("default");
        let result = manager.broadcaster("nonexistent");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_broadcast_manager_names() {
        let mut manager = BroadcastManager::new("log");
        manager.register(
            "log",
            TestBroadcaster {
                name: "log".to_string(),
                called: Arc::new(AtomicBool::new(false)),
            },
        );
        manager.register(
            "pusher",
            TestBroadcaster {
                name: "pusher".to_string(),
                called: Arc::new(AtomicBool::new(false)),
            },
        );
        let mut names = manager.broadcaster_names();
        names.sort();
        assert_eq!(names, vec!["log", "pusher"]);
    }

    #[tokio::test]
    async fn test_broadcast_manager_set_default() {
        let mut manager = BroadcastManager::new("first");
        manager.register(
            "first",
            TestBroadcaster {
                name: "first".to_string(),
                called: Arc::new(AtomicBool::new(false)),
            },
        );
        manager.register(
            "second",
            TestBroadcaster {
                name: "second".to_string(),
                called: Arc::new(AtomicBool::new(false)),
            },
        );
        manager.set_default("second");
        assert_eq!(manager.default_name(), "second");
    }

    #[tokio::test]
    async fn test_authenticate() {
        let broadcaster = TestBroadcaster {
            name: "test".to_string(),
            called: Arc::new(AtomicBool::new(false)),
        };
        let auth = broadcaster
            .authenticate("private-chat", "123.456", None)
            .await
            .unwrap();
        assert_eq!(auth, "auth-private-chat");
    }

    #[tokio::test]
    async fn test_authenticate_presence() {
        let broadcaster = TestBroadcaster {
            name: "test".to_string(),
            called: Arc::new(AtomicBool::new(false)),
        };
        let channel_data = PresenceChannelData {
            user_id: "user-1".to_string(),
            user_info: None,
        };
        let auth = broadcaster
            .authenticate_presence("presence-chat", "123.456", &channel_data, None)
            .await
            .unwrap();
        assert_eq!(auth, "presence-auth-presence-chat");
    }

    #[tokio::test]
    async fn test_broadcast_error_messages() {
        let err = BroadcastError::Failed("network error".to_string());
        assert_eq!(err.to_string(), "Broadcast failed: network error");

        let err = BroadcastError::NotConfigured("pusher".to_string());
        assert_eq!(err.to_string(), "Broadcaster [pusher] not configured");
    }

    #[tokio::test]
    async fn test_channel_serde() {
        let ch = Channel::public("chat");
        let json = serde_json::to_value(&ch).unwrap();
        assert_eq!(json, serde_json::json!("chat"));

        let ch = Channel::private("chat");
        let json = serde_json::to_value(&ch).unwrap();
        // With #[serde(untagged)], private channels serialize as the inner string
        assert_eq!(json, serde_json::json!("chat"));
        // Use to_channel_name() for the prefixed name
        assert_eq!(ch.to_channel_name(), "private-chat");
    }
}
