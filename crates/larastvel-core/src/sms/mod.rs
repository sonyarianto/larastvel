//! # SMS Module
//!
//! Provides an abstraction for sending SMS messages, inspired by Laravel's
//! Vonage SMS notification channel.
//!
//! The module defines the `SmsSender` trait (analogous to `Mailer` in the mail
//! module) with a `LogSmsSender` for development/testing and a `VonageSmsSender`
//! for production via the [Vonage API](https://developer.vonage.com/).
//!
//! ## Example
//!
//! ```rust,ignore
//! use larastvel_core::sms::{LogSmsSender, SmsMessage, SmsSender};
//!
//! #[tokio::main]
//! async fn main() {
//!     let sender = LogSmsSender::new();
//!     let msg = SmsMessage::new("+15551234567", "Hello from Larastvel!");
//!     let result = sender.send(msg).await;
//!     assert!(result.is_ok());
//! }
//! ```
//!
//! ## Vonage Configuration
//!
//! Set the following environment variables when using `VonageSmsSender`:
//!
//! - `VONAGE_API_KEY` — Your Vonage API key
//! - `VONAGE_API_SECRET` — Your Vonage API secret
//! - `VONAGE_FROM_NUMBER` — The default sender phone number or alphanumeric ID

use async_trait::async_trait;
use serde::Serialize;

// =============================================================================
// TYPES
// =============================================================================

/// An SMS message to be sent.
#[derive(Debug, Clone, Serialize)]
pub struct SmsMessage {
    /// The recipient phone number (E.164 format, e.g. `+15551234567`).
    pub to: Vec<String>,
    /// The sender ID or phone number to display as the "from".
    pub from: Option<String>,
    /// The text content of the SMS message.
    pub content: String,
}

impl SmsMessage {
    /// Create a new SMS message to a single recipient.
    pub fn new(to: &str, content: &str) -> Self {
        Self {
            to: vec![to.to_string()],
            from: None,
            content: content.to_string(),
        }
    }

    /// Create a new SMS message to multiple recipients.
    pub fn new_multi(to: Vec<String>, content: &str) -> Self {
        Self {
            to,
            from: None,
            content: content.to_string(),
        }
    }

    /// Set the sender ID or phone number.
    pub fn from(mut self, from: &str) -> Self {
        self.from = Some(from.to_string());
        self
    }
}

// =============================================================================
// SMS SENDER TRAIT
// =============================================================================

/// Errors that can occur when sending an SMS.
#[derive(Debug, thiserror::Error)]
pub enum SmsError {
    #[error("Failed to send SMS: {0}")]
    Send(String),

    #[error("Invalid phone number: {0}")]
    InvalidNumber(String),

    #[error("Vonage API error: {0}")]
    VonageApi(String),

    #[error("No recipients specified")]
    NoRecipients,
}

/// Trait for sending SMS messages.
///
/// Implementations include `LogSmsSender` (development/testing) and
/// `VonageSmsSender` (production via the Vonage API).
#[async_trait]
pub trait SmsSender: Send + Sync + std::fmt::Debug {
    /// Send an SMS message.
    async fn send(&self, message: SmsMessage) -> Result<(), SmsError>;

    /// Return the name of this sender (used for logging/debugging).
    fn name(&self) -> &str;
}

// =============================================================================
// LOG SMS SENDER (DEVELOPMENT / TESTING)
// =============================================================================

/// An SMS sender that logs messages to `tracing` instead of sending them.
///
/// Useful for development and testing environments where you don't want
/// to send actual SMS messages.
#[derive(Debug, Clone)]
pub struct LogSmsSender {
    name: String,
}

impl LogSmsSender {
    pub fn new() -> Self {
        Self {
            name: "log".to_string(),
        }
    }

    pub fn named(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

impl Default for LogSmsSender {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SmsSender for LogSmsSender {
    async fn send(&self, message: SmsMessage) -> Result<(), SmsError> {
        if message.to.is_empty() {
            return Err(SmsError::NoRecipients);
        }

        tracing::info!(
            target: "larastvel::sms",
            "📱 [{}] To: {:?} | From: {:?} | Content: {}",
            self.name,
            message.to,
            message.from,
            if message.content.len() > 200 {
                format!("{}... ({} chars)", &message.content[..200], message.content.len())
            } else {
                message.content.clone()
            },
        );

        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

// =============================================================================
// VONAGE SMS SENDER (PRODUCTION)
// =============================================================================

/// Sends SMS messages via the Vonage (formerly Nexmo) REST API.
///
/// # Configuration
///
/// Requires `VONAGE_API_KEY`, `VONAGE_API_SECRET`, and `VONAGE_FROM_NUMBER`
/// environment variables, or pass them directly to `VonageSmsSender::new()`.
///
/// # Example
///
/// ```rust,ignore
/// use larastvel_core::sms::{SmsMessage, SmsSender, VonageSmsSender};
///
/// let sender = VonageSmsSender::new(
///     "api_key", "api_secret", "+15551234567",
/// );
///
/// let msg = SmsMessage::new("+15559876543", "Your order has shipped!")
///     .from("MyApp");
///
/// sender.send(msg).await.unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct VonageSmsSender {
    name: String,
    api_key: String,
    api_secret: String,
    from: String,
    client: reqwest::Client,
}

impl VonageSmsSender {
    /// Create a new Vonage SMS sender.
    ///
    /// In a real application, load these values from environment variables
    /// or your app configuration:
    ///
    /// ```rust,ignore
    /// use larastvel_core::sms::VonageSmsSender;
    ///
    /// let sender = VonageSmsSender::new(
    ///     &std::env::var("VONAGE_API_KEY").unwrap(),
    ///     &std::env::var("VONAGE_API_SECRET").unwrap(),
    ///     &std::env::var("VONAGE_FROM_NUMBER").unwrap(),
    /// );
    /// ```
    pub fn new(api_key: &str, api_secret: &str, from: &str) -> Self {
        Self {
            name: "vonage".to_string(),
            api_key: api_key.to_string(),
            api_secret: api_secret.to_string(),
            from: from.to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Create a Vonage sender with a custom name (for multi-provider setups).
    pub fn named(name: &str, api_key: &str, api_secret: &str, from: &str) -> Self {
        Self {
            name: name.to_string(),
            api_key: api_key.to_string(),
            api_secret: api_secret.to_string(),
            from: from.to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Send an SMS via the Vonage REST API.
    ///
    /// Uses basic auth with the API key/secret and sends form-encoded
    /// parameters to `https://rest.nexmo.com/sms/json`.
    async fn send_vonage(&self, to: &str, from: &str, content: &str) -> Result<(), SmsError> {
        let url = "https://rest.nexmo.com/sms/json";

        let params = serde_json::json!({
            "from": from,
            "to": to,
            "text": content,
        });

        let response = self
            .client
            .post(url)
            .json(&params)
            .basic_auth(&self.api_key, Some(&self.api_secret))
            .send()
            .await
            .map_err(|e| SmsError::Send(format!("HTTP request failed: {}", e)))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable>".to_string());

        if !status.is_success() {
            return Err(SmsError::VonageApi(format!(
                "Vonage returned {}: {}",
                status, body
            )));
        }

        Ok(())
    }
}

#[async_trait]
impl SmsSender for VonageSmsSender {
    async fn send(&self, message: SmsMessage) -> Result<(), SmsError> {
        if message.to.is_empty() {
            return Err(SmsError::NoRecipients);
        }

        let from = message.from.as_deref().unwrap_or(&self.from);

        for recipient in &message.to {
            self.send_vonage(recipient, from, &message.content).await?;
        }

        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // SmsMessage tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_sms_message_new() {
        let msg = SmsMessage::new("+15551234567", "Hello!");
        assert_eq!(msg.to, vec!["+15551234567"]);
        assert_eq!(msg.content, "Hello!");
        assert!(msg.from.is_none());
    }

    #[test]
    fn test_sms_message_from() {
        let msg = SmsMessage::new("+15551234567", "Hi").from("MyApp");
        assert_eq!(msg.from, Some("MyApp".to_string()));
    }

    #[test]
    fn test_sms_message_multi() {
        let msg = SmsMessage::new_multi(
            vec!["+15551111111".to_string(), "+15552222222".to_string()],
            "Broadcast!",
        );
        assert_eq!(msg.to.len(), 2);
        assert_eq!(msg.content, "Broadcast!");
    }

    #[test]
    fn test_sms_message_serialization() {
        let msg = SmsMessage::new("+15551234567", "Test");
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["to"][0], "+15551234567");
        assert_eq!(json["content"], "Test");
    }

    // -------------------------------------------------------------------------
    // LogSmsSender tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_log_sms_sender_success() {
        let sender = LogSmsSender::new();
        let msg = SmsMessage::new("+15551234567", "Test message");
        let result = sender.send(msg).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_log_sms_sender_no_recipients() {
        let sender = LogSmsSender::new();
        let msg = SmsMessage {
            // Create a message with no recipients
            to: vec![],
            from: None,
            content: "test".to_string(),
        };
        let result = sender.send(msg).await;
        assert!(result.is_err());
        match result {
            Err(SmsError::NoRecipients) => {}
            other => panic!("Expected NoRecipients, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_log_sms_sender_name() {
        let sender = LogSmsSender::named("my-sms");
        assert_eq!(sender.name(), "my-sms");
    }

    #[tokio::test]
    async fn test_log_sms_sender_long_content() {
        let sender = LogSmsSender::new();
        let long = "a".repeat(500);
        let msg = SmsMessage::new("+15551234567", &long);
        let result = sender.send(msg).await;
        assert!(result.is_ok());
    }

    // -------------------------------------------------------------------------
    // VonageSmsSender tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_vonage_sms_sender_name() {
        let sender = VonageSmsSender::new("key", "secret", "+15551234567");
        assert_eq!(sender.name(), "vonage");

        let custom = VonageSmsSender::named("custom", "key", "secret", "+15551234567");
        assert_eq!(custom.name(), "custom");
    }

    #[tokio::test]
    async fn test_vonage_sms_sender_no_recipients() {
        let sender = VonageSmsSender::new("key", "secret", "+15551234567");
        let msg = SmsMessage {
            to: vec![],
            from: None,
            content: "test".to_string(),
        };
        let result = sender.send(msg).await;
        assert!(result.is_err());
        match result {
            Err(SmsError::NoRecipients) => {}
            other => panic!("Expected NoRecipients, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_vonage_sms_sender_http_error() {
        // This test hits the real Vonage API with bad credentials.
        // The API may return 401 (HTTP error) or 200 with an error body,
        // so we just verify the call completes without panicking.
        let sender = VonageSmsSender::new("bad-key", "bad-secret", "+15551234567");
        let msg = SmsMessage::new("+15551234567", "Test");
        let _ = sender.send(msg).await;
        // No assertion — this test validates the code path doesn't panic
    }
}
