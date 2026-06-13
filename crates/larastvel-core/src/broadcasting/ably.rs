use async_trait::async_trait;

use super::{
    BroadcastError, BroadcastMessage, Broadcaster, ChannelAuthCallback, PresenceChannelData,
};

/// Ably-compatible broadcaster using the Ably REST API.
///
/// Sends events to the Ably HTTP API at `https://{environment}.realtime.ably.net`.
/// Supports public channels. Private/presence channel auth can use Ably's
/// token-based authentication.
#[derive(Debug, Clone)]
pub struct AblyBroadcaster {
    name: String,
    api_key: String,
    environment: String,
    client: reqwest::Client,
}

impl AblyBroadcaster {
    /// Create a new Ably broadcaster.
    ///
    /// `api_key` should be in the format `{APP_ID}:{API_KEY}`.
    pub fn new(name: &str, api_key: &str) -> Self {
        Self {
            name: name.to_string(),
            api_key: api_key.to_string(),
            environment: "main".to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Set the Ably environment (default: "main").
    pub fn with_environment(mut self, env: &str) -> Self {
        self.environment = env.to_string();
        self
    }

    fn base_url(&self) -> String {
        format!("https://{}.realtime.ably.net", self.environment)
    }

    fn auth_header_value(&self) -> String {
        format!(
            "Basic {}",
            base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                self.api_key.as_bytes()
            )
        )
    }

    /// Generate a basic auth token for private channel subscriptions.
    /// Ably uses Capability-based tokens — for simplicity we generate
    /// a capability token with full access using the API key directly.
    pub fn generate_private_auth(
        &self,
        channel_name: &str,
        _socket_id: &str,
    ) -> Result<String, BroadcastError> {
        // Ably doesn't use HMAC signing like Pusher for channel auth.
        // Instead, clients authenticate via token or basic auth.
        // For a dev/demo scenario, we return a capability token string.
        // In production, you would use Ably's TokenRequest API.
        let capability = serde_json::json!({
            channel_name: ["subscribe", "publish"]
        });
        let claims = serde_json::json!({
            "capability": capability.to_string(),
            "clientId": "server",
        });
        serde_json::to_string(&claims)
            .map_err(|e| BroadcastError::AuthError(format!("Serialization error: {}", e)))
    }
}

#[async_trait]
impl Broadcaster for AblyBroadcaster {
    async fn broadcast(&self, message: BroadcastMessage) -> Result<(), BroadcastError> {
        // Ably REST API: POST /channels/{channel}/messages
        // Auth: Authorization: Basic <base64(api_key)>
        //
        // We send one request per channel since Ably's batch endpoint
        // has a different format.
        for channel in &message.channels {
            let payload = serde_json::json!({
                "name": message.event,
                "data": message.data,
            });

            let body = serde_json::to_string(&payload)
                .map_err(|e| BroadcastError::Failed(format!("Serialization error: {}", e)))?;

            let url = format!("{}/channels/{}/messages", self.base_url(), channel);

            let response = self
                .client
                .post(&url)
                .header("Content-Type", "application/json")
                .header("Authorization", &self.auth_header_value())
                .body(body)
                .send()
                .await
                .map_err(|e| BroadcastError::Failed(format!("HTTP request failed: {}", e)))?;

            if !response.status().is_success() {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                return Err(BroadcastError::Failed(format!(
                    "Ably API returned {}: {}",
                    status, text
                )));
            }
        }

        Ok(())
    }

    async fn authenticate(
        &self,
        channel_name: &str,
        _socket_id: &str,
        _callback: Option<&ChannelAuthCallback>,
    ) -> Result<String, BroadcastError> {
        let token = self.generate_private_auth(channel_name, _socket_id)?;
        Ok(token)
    }

    async fn authenticate_presence(
        &self,
        channel_name: &str,
        _socket_id: &str,
        channel_data: &PresenceChannelData,
        _callback: Option<&ChannelAuthCallback>,
    ) -> Result<String, BroadcastError> {
        // For presence channels, include the channel data in the capability
        let capability = serde_json::json!({
            channel_name: ["subscribe", "publish", "presence"]
        });
        let claims = serde_json::json!({
            "capability": capability.to_string(),
            "clientId": channel_data.user_id,
            "data": channel_data,
        });
        serde_json::to_string(&claims)
            .map_err(|e| BroadcastError::AuthError(format!("Serialization error: {}", e)))
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_broadcaster_name() {
        let broadcaster = AblyBroadcaster::new("ably", "test-key:test-secret");
        assert_eq!(broadcaster.name(), "ably");
    }

    #[test]
    fn test_base_url_default() {
        let broadcaster = AblyBroadcaster::new("ably", "key:secret");
        assert_eq!(broadcaster.base_url(), "https://main.realtime.ably.net");
    }

    #[test]
    fn test_base_url_custom_environment() {
        let broadcaster = AblyBroadcaster::new("ably", "key:secret").with_environment("sandbox");
        assert_eq!(broadcaster.base_url(), "https://sandbox.realtime.ably.net");
    }

    #[test]
    fn test_auth_header() {
        let broadcaster = AblyBroadcaster::new("ably", "test-key:test-secret");
        let header = broadcaster.auth_header_value();
        assert!(header.starts_with("Basic "));
    }

    #[test]
    fn test_generate_private_auth() {
        let broadcaster = AblyBroadcaster::new("ably", "key:secret");
        let result = broadcaster.generate_private_auth("private-chat", "123.456");
        assert!(result.is_ok());
        let auth = result.unwrap();
        assert!(auth.contains("capability"));
        assert!(auth.contains("private-chat"));
    }

    #[tokio::test]
    async fn test_authenticate() {
        let broadcaster = AblyBroadcaster::new("ably", "key:secret");
        let result = broadcaster
            .authenticate("private-chat", "123.456", None)
            .await
            .unwrap();
        assert!(result.contains("capability"));
    }

    #[tokio::test]
    async fn test_authenticate_presence() {
        let broadcaster = AblyBroadcaster::new("ably", "key:secret");
        let channel_data = PresenceChannelData {
            user_id: "user-1".to_string(),
            user_info: None,
        };
        let result = broadcaster
            .authenticate_presence("presence-chat", "123.456", &channel_data, None)
            .await
            .unwrap();
        assert!(result.contains("user-1"));
        assert!(result.contains("presence-chat"));
    }
}
