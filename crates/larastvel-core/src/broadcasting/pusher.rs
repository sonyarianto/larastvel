use async_trait::async_trait;
use hmac::{Hmac, Mac};
use md5::{Digest, Md5};
use sha2::Sha256;

use super::{
    BroadcastError, BroadcastMessage, Broadcaster, ChannelAuthCallback, PresenceChannelData,
};

type HmacSha256 = Hmac<Sha256>;

/// Pusher-compatible broadcaster using the Pusher HTTP REST API.
///
/// Sends events to the Pusher HTTP API at `https://api-{cluster}.pusher.com`.
/// Supports public, private, and presence channels.
#[derive(Debug, Clone)]
pub struct PusherBroadcaster {
    name: String,
    app_id: String,
    key: String,
    secret: String,
    cluster: String,
    encrypted: bool,
    client: reqwest::Client,
}

impl PusherBroadcaster {
    /// Create a new Pusher broadcaster.
    pub fn new(name: &str, app_id: &str, key: &str, secret: &str, cluster: &str) -> Self {
        Self {
            name: name.to_string(),
            app_id: app_id.to_string(),
            key: key.to_string(),
            secret: secret.to_string(),
            cluster: cluster.to_string(),
            encrypted: true,
            client: reqwest::Client::new(),
        }
    }

    /// Set whether to use HTTPS (default: true).
    pub fn with_encrypted(mut self, encrypted: bool) -> Self {
        self.encrypted = encrypted;
        self
    }

    fn scheme(&self) -> &str {
        if self.encrypted {
            "https"
        } else {
            "http"
        }
    }

    fn api_url(&self) -> String {
        format!(
            "{}://api-{}.pusher.com/apps/{}/events",
            self.scheme(),
            self.cluster,
            self.app_id
        )
    }

    /// Generate the Pusher auth signature for a private channel subscription.
    pub fn generate_private_auth(&self, socket_id: &str, channel_name: &str) -> String {
        let string_to_sign = format!("{}:{}", socket_id, channel_name);
        let signature = self.sign(&string_to_sign);
        format!("{}:{}", self.key, signature)
    }

    /// Generate the Pusher auth signature for a presence channel subscription.
    pub fn generate_presence_auth(
        &self,
        socket_id: &str,
        channel_name: &str,
        channel_data: &PresenceChannelData,
    ) -> Result<String, BroadcastError> {
        let data_json = serde_json::to_string(channel_data)
            .map_err(|e| BroadcastError::AuthError(format!("Serialization error: {}", e)))?;
        let string_to_sign = format!("{}:{}:{}", socket_id, channel_name, data_json);
        let signature = self.sign(&string_to_sign);
        Ok(format!("{}:{}", self.key, signature))
    }

    /// Sign a string with HMAC-SHA256 using the Pusher secret.
    fn sign(&self, string_to_sign: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(self.secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(string_to_sign.as_bytes());
        let result = mac.finalize();
        let code_bytes = result.into_bytes();
        hex::encode(code_bytes)
    }

    /// Build the Pusher REST API query string for auth.
    fn auth_query(&self, body_md5: &str, timestamp: &str) -> String {
        format!(
            "auth_key={}&auth_timestamp={}&auth_version=1.0&body_md5={}",
            self.key, timestamp, body_md5
        )
    }

    /// Sign the full request string for the Pusher REST API.
    /// Sign a request for the Pusher REST API.
    /// The string to sign is: "METHOD\nPATH\nQUERY_STRING\nBODY_MD5"
    fn sign_request(&self, method: &str, path: &str, query: &str, body_md5: &str) -> String {
        let string_to_sign = format!("{}\n{}\n{}\n{}", method, path, query, body_md5);
        self.sign(&string_to_sign)
    }
}

#[async_trait]
impl Broadcaster for PusherBroadcaster {
    async fn broadcast(&self, message: BroadcastMessage) -> Result<(), BroadcastError> {
        let payload = serde_json::json!({
            "name": message.event,
            "channels": message.channels,
            "data": message.data.to_string(),
        });

        let body = serde_json::to_string(&payload)
            .map_err(|e| BroadcastError::Failed(format!("Serialization error: {}", e)))?;

        let body_md5 = hex::encode(Md5::digest(body.as_bytes()));
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();

        let path = format!("/apps/{}/events", self.app_id);
        let query = self.auth_query(&body_md5, &timestamp);
        let signature = self.sign_request("POST", &path, &query, &body_md5);

        let url = format!("{}?{}&auth_signature={}", self.api_url(), query, signature);

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| BroadcastError::Failed(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(BroadcastError::Failed(format!(
                "Pusher API returned {}: {}",
                status, text
            )));
        }

        Ok(())
    }

    async fn authenticate(
        &self,
        channel_name: &str,
        socket_id: &str,
        _callback: Option<&ChannelAuthCallback>,
    ) -> Result<String, BroadcastError> {
        let auth = self.generate_private_auth(socket_id, channel_name);
        let response = serde_json::json!({
            "auth": auth,
        });
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
        let auth = self.generate_presence_auth(socket_id, channel_name, channel_data)?;
        let channel_data_json = serde_json::to_string(channel_data)
            .map_err(|e| BroadcastError::AuthError(format!("Serialization error: {}", e)))?;
        let response = serde_json::json!({
            "auth": auth,
            "channel_data": channel_data_json,
        });
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

    #[test]
    fn test_signature_generation() {
        let broadcaster =
            PusherBroadcaster::new("pusher", "123456", "app-key", "app-secret", "mt1");

        let socket_id = "123.456";
        let channel = "private-chat";
        let auth = broadcaster.generate_private_auth(socket_id, channel);
        assert!(auth.starts_with("app-key:"));
        assert!(auth.len() > 20);
    }

    #[test]
    fn test_presence_auth_generation() {
        let broadcaster =
            PusherBroadcaster::new("pusher", "123456", "app-key", "app-secret", "mt1");

        let channel_data = PresenceChannelData {
            user_id: "user-1".to_string(),
            user_info: Some(serde_json::json!({"name": "Alice"})),
        };

        let auth = broadcaster.generate_presence_auth("123.456", "presence-chat", &channel_data);
        assert!(auth.is_ok());
        let auth = auth.unwrap();
        assert!(auth.starts_with("app-key:"));
    }

    #[test]
    fn test_sign_consistency() {
        let broadcaster = PusherBroadcaster::new("pusher", "123456", "key", "secret", "mt1");

        // Same input should produce same signature
        let a = broadcaster.sign("test");
        let b = broadcaster.sign("test");
        assert_eq!(a, b);

        // Different input should produce different signature
        let c = broadcaster.sign("different");
        assert_ne!(a, c);
    }

    #[test]
    fn test_broadcaster_name() {
        let broadcaster = PusherBroadcaster::new("pusher", "123", "key", "secret", "mt1");
        assert_eq!(broadcaster.name(), "pusher");
    }

    #[test]
    fn test_api_url() {
        let broadcaster = PusherBroadcaster::new("pusher", "123456", "key", "secret", "mt1");
        assert_eq!(
            broadcaster.api_url(),
            "https://api-mt1.pusher.com/apps/123456/events"
        );
    }

    #[test]
    fn test_api_url_unencrypted() {
        let broadcaster =
            PusherBroadcaster::new("pusher", "123", "key", "secret", "us2").with_encrypted(false);
        assert_eq!(
            broadcaster.api_url(),
            "http://api-us2.pusher.com/apps/123/events"
        );
    }

    #[test]
    fn test_private_auth_format() {
        let broadcaster = PusherBroadcaster::new("pusher", "app-id", "key", "secret", "mt1");
        let auth = broadcaster.generate_private_auth("123.456", "private-chat");
        // Format should be "key:signature"
        let parts: Vec<&str> = auth.split(':').collect();
        assert_eq!(parts[0], "key");
        assert_eq!(parts.len(), 2);
        // Signature should be hex (64 chars for HMAC-SHA256)
        assert_eq!(parts[1].len(), 64);
    }

    #[test]
    fn test_presence_auth_serialization() {
        let broadcaster = PusherBroadcaster::new("pusher", "app-id", "key", "secret", "mt1");

        let channel_data = PresenceChannelData {
            user_id: "user-1".to_string(),
            user_info: Some(serde_json::json!({"name": "Alice", "avatar": "/avatars/1.png"})),
        };

        let auth = broadcaster
            .generate_presence_auth("123.456", "presence-chat", &channel_data)
            .unwrap();

        let parts: Vec<&str> = auth.split(':').collect();
        assert_eq!(parts[0], "key");
        assert_eq!(parts.len(), 2);
    }

    #[tokio::test]
    async fn test_authenticate_response() {
        let broadcaster = PusherBroadcaster::new("pusher", "app-id", "key", "secret", "mt1");

        let result = broadcaster
            .authenticate("private-chat", "123.456", None)
            .await
            .unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed["auth"].is_string());
        assert!(parsed["auth"].as_str().unwrap().starts_with("key:"));
    }

    #[tokio::test]
    async fn test_authenticate_presence_response() {
        let broadcaster = PusherBroadcaster::new("pusher", "app-id", "key", "secret", "mt1");

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
}
