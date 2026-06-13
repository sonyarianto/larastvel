//! # Notifications Module
//!
//! A notification system inspired by Laravel, supporting delivery via:
//!
//! - **Mail** — transactional emails via the mail module's `Mailer` trait
//! - **Broadcast** — real-time WebSocket events via the broadcasting module's `Broadcaster` trait
//! - **Database** — persistent notifications stored in a `notifications` table
//! - **Webhook** — HTTP POST requests via a `reqwest::Client`
//! - **SMS** — text messages via an `SmsSender` trait (LogSmsSender, VonageSmsSender)
//!
//! ## Example
//!
//! ```rust
//! use larastvel_core::notifications::{
//!     Notification, NotificationChannel, NotificationSender, Notifiable,
//! };
//!
//! // 1. Define a notification
//! #[derive(Debug, Clone)]
//! struct OrderShipped {
//!     order_id: String,
//!     customer_name: String,
//! }
//!
//! impl Notification for OrderShipped {
//!     fn via(&self) -> Vec<NotificationChannel> {
//!         vec![
//!             NotificationChannel::Mail,
//!             NotificationChannel::Broadcast,
//!         ]
//!     }
//!
//!     fn to_mail(&self) -> Option<larastvel_core::mail::Mailable> {
//!         Some(larastvel_core::mail::Mailable::html(
//!             vec![],  // recipient set by the sender based on notifiable
//!             &format!("Order #{} Shipped!", self.order_id),
//!             &format!("<h1>Hi {}</h1><p>Your order has shipped!</p>", self.customer_name),
//!         ).from("orders@example.com"))
//!     }
//!
//!     fn to_broadcast(&self) -> Option<larastvel_core::notifications::BroadcastPayload> {
//!         Some(larastvel_core::notifications::BroadcastPayload {
//!             event: "order.shipped".to_string(),
//!             data: serde_json::json!({ "order_id": self.order_id }),
//!         })
//!     }
//! }
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde::Serialize;
use uuid::Uuid;

use crate::broadcasting::{BroadcastError, BroadcastMessage, Broadcaster};
use crate::mail::{MailError, Mailable, Mailer};
use crate::sms::{SmsError, SmsMessage, SmsSender};

// =============================================================================
// TYPES
// =============================================================================

/// Channels a notification can be sent through.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NotificationChannel {
    /// Send via the mail system. The recipient's email is resolved from the
    /// `Notifiable` implementation.
    Mail,
    /// Send via WebSocket broadcast.
    Broadcast,
    /// Persist to the `notifications` database table.
    Database,
    /// Send an HTTP POST request to a webhook URL. The URL is resolved from
    /// the `Notifiable::route_webhook_url()` method, and the JSON body from
    /// `Notification::to_webhook()`.
    Webhook,
    /// Send an SMS message via an `SmsSender`. The recipient phone number is
    /// resolved from `Notifiable::route_phone()`, and the message content from
    /// `Notification::to_sms()`.
    Sms,
}

/// Payload returned by `Notification::to_broadcast()`.
#[derive(Debug, Clone, Serialize)]
pub struct BroadcastPayload {
    /// The event name broadcast to clients (e.g. `"order.shipped"`).
    pub event: String,
    /// The JSON data payload sent with the event.
    pub data: serde_json::Value,
}

/// A record stored in the notifications database table.
#[derive(Debug, Clone, Serialize)]
pub struct DatabaseNotification {
    pub id: String,
    pub notifiable_id: String,
    pub notifiable_type: String,
    pub notification_type: String,
    pub data: serde_json::Value,
    pub read_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

// =============================================================================
// TRAITS
// =============================================================================

/// Trait for notifiable entities (e.g. User, Admin).
///
/// Implement this on your model to tell the notification system how to
/// reach the entity through each channel.
pub trait Notifiable: Send + Sync {
    /// The unique identifier for this notifiable (e.g. user ID).
    fn notification_id(&self) -> String;

    /// The type name used in the database `notifiable_type` column.
    fn notification_type_name(&self) -> String {
        "user".to_string()
    }

    /// Email address for the Mail channel. Return `None` to skip mail delivery.
    fn route_email(&self) -> Option<String> {
        None
    }

    /// Broadcast channels the notifiable should listen on.
    fn route_broadcast_channels(&self) -> Vec<String> {
        vec![]
    }

    /// Webhook URL for the Webhook channel. Return `None` to skip webhook delivery.
    fn route_webhook_url(&self) -> Option<String> {
        None
    }

    /// Phone number for the SMS channel. Return `None` to skip SMS delivery.
    /// Should be in E.164 format (e.g. `+15551234567`).
    fn route_phone(&self) -> Option<String> {
        None
    }
}

/// Trait for notifications.
///
/// Implement this on your notification struct (e.g. `OrderShipped`,
/// `WelcomeEmail`, `InvoicePaid`).
pub trait Notification: Send + Sync + std::fmt::Debug {
    /// The channels this notification should be delivered through.
    fn via(&self) -> Vec<NotificationChannel>;

    /// Return the mail representation of the notification.
    ///
    /// The `to` field of the returned `Mailable` may be left empty; the
    /// `NotificationSender` will populate it from the notifiable's
    /// `route_email()`.
    fn to_mail(&self) -> Option<Mailable> {
        None
    }

    /// Return the broadcast representation of the notification.
    fn to_broadcast(&self) -> Option<BroadcastPayload> {
        None
    }

    /// Return the database representation of the notification (JSON data).
    fn to_database(&self) -> Option<serde_json::Value> {
        None
    }

    /// Return the webhook payload for the Webhook channel.
    ///
    /// The returned JSON value is POSTed as the request body to the URL
    /// provided by `Notifiable::route_webhook_url()`. A `Content-Type:
    /// application/json` header is set automatically.
    fn to_webhook(&self) -> Option<serde_json::Value> {
        None
    }

    /// Return the SMS representation of the notification.
    ///
    /// The returned `SmsMessage` should include the content; the `to` field
    /// may be left empty (it will be populated from the notifiable's
    /// `route_phone()`). The `from` field can be set here or left for the
    /// sender's default.
    fn to_sms(&self) -> Option<SmsMessage> {
        None
    }
}

// =============================================================================
// NOTIFICATION SENDER
// =============================================================================

/// Sends notifications through configured channels.
///
/// # Example
///
/// ```rust
/// use std::sync::Arc;
/// use larastvel_core::mail::LogMailer;
/// use larastvel_core::notifications::NotificationSender;
///
/// let sender = NotificationSender::new()
///     .with_mailer(Arc::new(LogMailer::new("log")));
/// ```
#[derive(Debug, Clone)]
pub struct NotificationSender {
    mailer: Option<Arc<dyn Mailer>>,
    broadcaster: Option<Arc<dyn Broadcaster>>,
    database: Option<sea_orm::DatabaseConnection>,
    webhook_client: Option<reqwest::Client>,
    sms_sender: Option<Arc<dyn SmsSender>>,
    from_address: String,
    app_url: String,
    app_name: String,
}

impl Default for NotificationSender {
    fn default() -> Self {
        Self {
            mailer: None,
            broadcaster: None,
            database: None,
            webhook_client: None,
            sms_sender: None,
            from_address: "noreply@example.com".to_string(),
            app_url: "http://localhost:8080".to_string(),
            app_name: "Larastvel".to_string(),
        }
    }
}

impl NotificationSender {
    /// Create a new `NotificationSender` with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the mailer used for the Mail channel.
    pub fn with_mailer(mut self, mailer: Arc<dyn Mailer>) -> Self {
        self.mailer = Some(mailer);
        self
    }

    /// Set the broadcaster used for the Broadcast channel.
    pub fn with_broadcaster(mut self, broadcaster: Arc<dyn Broadcaster>) -> Self {
        self.broadcaster = Some(broadcaster);
        self
    }

    /// Set the default "From" address for mail notifications.
    pub fn with_from(mut self, from: &str) -> Self {
        self.from_address = from.to_string();
        self
    }

    /// Set the application URL (used in notification templates).
    pub fn with_app_url(mut self, url: &str) -> Self {
        self.app_url = url.to_string();
        self
    }

    /// Set the database connection used for the Database channel.
    pub fn with_database(mut self, db: sea_orm::DatabaseConnection) -> Self {
        self.database = Some(db);
        self
    }

    /// Set the application name (used in notification templates).
    pub fn with_app_name(mut self, name: &str) -> Self {
        self.app_name = name.to_string();
        self
    }

    /// Set the HTTP client used for the Webhook channel.
    ///
    /// If no client is configured and a notification specifies `Webhook` in
    /// its `via()` list, the sender returns a `ChannelNotConfigured` error.
    pub fn with_webhook_client(mut self, client: reqwest::Client) -> Self {
        self.webhook_client = Some(client);
        self
    }

    /// Set the SMS sender used for the SMS channel.
    pub fn with_sms_sender(mut self, sender: Arc<dyn SmsSender>) -> Self {
        self.sms_sender = Some(sender);
        self
    }

    /// Send a notification to a notifiable entity through all configured channels.
    ///
    /// Returns a map of channel → result for each attempted delivery.
    pub async fn send<N: Notification>(
        &self,
        notifiable: &dyn Notifiable,
        notification: N,
    ) -> HashMap<NotificationChannel, Result<(), NotificationError>> {
        let channels = notification.via();
        let mut results = HashMap::new();

        for channel in &channels {
            let result = match channel {
                NotificationChannel::Mail => {
                    self.send_mail(notifiable, &notification).await
                }
                NotificationChannel::Broadcast => {
                    self.send_broadcast(notifiable, &notification).await
                }
                NotificationChannel::Database => {
                    self.send_database(notifiable, &notification).await
                }
                NotificationChannel::Webhook => {
                    self.send_webhook(notifiable, &notification).await
                }
                NotificationChannel::Sms => {
                    self.send_sms(notifiable, &notification).await
                }
            };
            results.insert(channel.clone(), result);
        }

        results
    }

    /// Send a notification and return `Ok(())` only if ALL channels succeed.
    ///
    /// Errors are collected into a single `NotificationError::PartialDelivery` if
    /// some channels fail, or the specific error if only one channel was attempted.
    pub async fn send_all<N: Notification>(
        &self,
        notifiable: &dyn Notifiable,
        notification: N,
    ) -> Result<(), NotificationError> {
        let results = self.send(notifiable, notification).await;

        let errors: Vec<(NotificationChannel, NotificationError)> = results
            .into_iter()
            .filter_map(|(ch, res)| res.err().map(|e| (ch, e)))
            .collect();

        if errors.is_empty() {
            Ok(())
        } else if errors.len() == 1 {
            Err(errors.into_iter().next().unwrap().1)
        } else {
            Err(NotificationError::PartialDelivery(errors))
        }
    }

    // -------------------------------------------------------------------------
    // Channel implementations
    // -------------------------------------------------------------------------

    async fn send_mail<N: Notification>(
        &self,
        notifiable: &dyn Notifiable,
        notification: &N,
    ) -> Result<(), NotificationError> {
        let mailer = self
            .mailer
            .as_ref()
            .ok_or(NotificationError::ChannelNotConfigured("mail".to_string()))?;

        let email = notifiable
            .route_email()
            .ok_or_else(|| NotificationError::InvalidRecipient("No email route".to_string()))?;

        let mut mailable = notification
            .to_mail()
            .ok_or(NotificationError::ChannelNotSupported("mail".to_string()))?;

        // Set the recipient from the notifiable
        mailable.to = vec![email];

        // Set a default from address if none was set
        if mailable.from.is_none() {
            mailable.from = Some(self.from_address.clone());
        }

        mailer
            .send(mailable)
            .await
            .map_err(NotificationError::Mail)?;

        Ok(())
    }

    async fn send_broadcast<N: Notification>(
        &self,
        notifiable: &dyn Notifiable,
        notification: &N,
    ) -> Result<(), NotificationError> {
        let broadcaster = self
            .broadcaster
            .as_ref()
            .ok_or(NotificationError::ChannelNotConfigured("broadcast".to_string()))?;

        let payload = notification
            .to_broadcast()
            .ok_or(NotificationError::ChannelNotSupported("broadcast".to_string()))?;

        let channels: Vec<String> = notifiable
            .route_broadcast_channels()
            .into_iter()
            .map(|ch| {
                if ch.starts_with("private-") || ch.starts_with("presence-") {
                    ch
                } else {
                    format!("private-{}", ch)
                }
            })
            .collect();

        if channels.is_empty() {
            return Err(NotificationError::InvalidRecipient(
                "No broadcast channels configured".to_string(),
            ));
        }

        let message = BroadcastMessage::new(&payload.event, payload.data, channels);

        broadcaster
            .broadcast(message)
            .await
            .map_err(NotificationError::Broadcast)?;

        Ok(())
    }

    /// Ensure the `notifications` table exists.
    ///
    /// This is called automatically by `send_database()` on the first use.
    /// You can also call it explicitly during application boot to ensure
    /// the table is ready before any notifications are sent.
    pub async fn ensure_notifications_table(&self) -> Result<(), NotificationError> {
        let db = self
            .database
            .as_ref()
            .ok_or(NotificationError::ChannelNotConfigured("database".to_string()))?;

        let sql = "CREATE TABLE IF NOT EXISTS notifications (
            id TEXT PRIMARY KEY,
            notifiable_id TEXT NOT NULL,
            notifiable_type TEXT NOT NULL,
            notification_type TEXT NOT NULL,
            data TEXT NOT NULL,
            read_at INTEGER,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )";

        db.execute(Statement::from_string(DatabaseBackend::Sqlite, sql))
            .await
            .map_err(|e| NotificationError::Database(format!("Failed to create notifications table: {}", e)))?;

        Ok(())
    }

    async fn send_sms<N: Notification>(
        &self,
        notifiable: &dyn Notifiable,
        notification: &N,
    ) -> Result<(), NotificationError> {
        let sender = self
            .sms_sender
            .as_ref()
            .ok_or(NotificationError::ChannelNotConfigured("sms".to_string()))?;

        let phone = notifiable
            .route_phone()
            .ok_or_else(|| NotificationError::InvalidRecipient("No phone route".to_string()))?;

        let mut sms = notification
            .to_sms()
            .ok_or(NotificationError::ChannelNotSupported("sms".to_string()))?;

        // Set the recipient from the notifiable
        sms.to = vec![phone];

        sender
            .send(sms)
            .await
            .map_err(NotificationError::Sms)?;

        Ok(())
    }

    async fn send_webhook<N: Notification>(
        &self,
        notifiable: &dyn Notifiable,
        notification: &N,
    ) -> Result<(), NotificationError> {
        let client = self
            .webhook_client
            .as_ref()
            .ok_or(NotificationError::ChannelNotConfigured("webhook".to_string()))?;

        let url = notifiable
            .route_webhook_url()
            .ok_or_else(|| NotificationError::InvalidRecipient("No webhook URL".to_string()))?;

        let payload = notification
            .to_webhook()
            .ok_or(NotificationError::ChannelNotSupported("webhook".to_string()))?;

        let response = client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| NotificationError::Webhook(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable>".to_string());
            return Err(NotificationError::Webhook(format!(
                "Webhook returned {}: {}",
                status, body
            )));
        }

        Ok(())
    }

    async fn send_database<N: Notification>(
        &self,
        notifiable: &dyn Notifiable,
        notification: &N,
    ) -> Result<(), NotificationError> {
        // Auto-create the notifications table on first use
        self.ensure_notifications_table().await?;

        let db = self
            .database
            .as_ref()
            .ok_or(NotificationError::ChannelNotConfigured("database".to_string()))?;

        let data = notification
            .to_database()
            .ok_or(NotificationError::ChannelNotSupported("database".to_string()))?;

        let id = Uuid::new_v4().to_string();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let notification_type = std::any::type_name::<N>()
            .split("::")
            .last()
            .unwrap_or("unknown")
            .to_string();
        let data_json = serde_json::to_string(&data)
            .unwrap_or_else(|_| "{}".to_string());

        let sql = format!(
            "INSERT INTO notifications \
             (id, notifiable_id, notifiable_type, notification_type, data, \
              read_at, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?6)"
        );

        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            &sql,
            [
                id.into(),
                notifiable.notification_id().into(),
                notifiable.notification_type_name().into(),
                notification_type.into(),
                data_json.into(),
                now.into(),
            ],
        ))
        .await
        .map_err(|e| NotificationError::Database(format!("Failed to store notification: {}", e)))?;

        Ok(())
    }
}

// =============================================================================
// CONVENIENCE FUNCTIONS
// =============================================================================

/// Send a notification using the default sender configuration.
///
/// Convenience wrapper for `NotificationSender::new().send(notifiable, notification).await`.
pub async fn send<N: Notification>(
    notifiable: &dyn Notifiable,
    notification: N,
) -> HashMap<NotificationChannel, Result<(), NotificationError>> {
    NotificationSender::new().send(notifiable, notification).await
}

/// Send a notification using a configured sender.
pub async fn send_via<N: Notification>(
    sender: &NotificationSender,
    notifiable: &dyn Notifiable,
    notification: N,
) -> HashMap<NotificationChannel, Result<(), NotificationError>> {
    sender.send(notifiable, notification).await
}

// =============================================================================
// ERRORS
// =============================================================================

/// Errors that can occur during notification delivery.
#[derive(Debug, thiserror::Error)]
pub enum NotificationError {
    #[error("Mail delivery failed: {0}")]
    Mail(#[from] MailError),

    #[error("Broadcast delivery failed: {0}")]
    Broadcast(#[from] BroadcastError),

    #[error("Database storage failed: {0}")]
    Database(String),

    #[error("Webhook delivery failed: {0}")]
    Webhook(String),

    #[error("SMS delivery failed: {0}")]
    Sms(#[from] SmsError),

    #[error("Channel [{0}] is not configured on the sender")]
    ChannelNotConfigured(String),

    #[error("Notification does not support channel [{0}]")]
    ChannelNotSupported(String),

    #[error("Invalid recipient for notification: {0}")]
    InvalidRecipient(String),

    #[error("Partial delivery: {0:?}")]
    PartialDelivery(Vec<(NotificationChannel, NotificationError)>),
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broadcasting::{BroadcastEvent, BroadcastManager};
    use crate::mail::LogMailer;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    // -------------------------------------------------------------------------
    // Test fixtures
    // -------------------------------------------------------------------------

    #[derive(Debug)]
    struct TestUser {
        id: String,
        email: String,
        broadcast_channels: Vec<String>,
    }

    impl Notifiable for TestUser {
        fn notification_id(&self) -> String {
            self.id.clone()
        }

        fn route_email(&self) -> Option<String> {
            Some(self.email.clone())
        }

        fn route_broadcast_channels(&self) -> Vec<String> {
            self.broadcast_channels.clone()
        }
    }

    #[derive(Debug, Clone)]
    struct WelcomeNotification {
        user_name: String,
    }

    impl Notification for WelcomeNotification {
        fn via(&self) -> Vec<NotificationChannel> {
            vec![NotificationChannel::Mail]
        }

        fn to_mail(&self) -> Option<Mailable> {
            Some(
                Mailable::html(
                    vec![],
                    &format!("Welcome, {}!", self.user_name),
                    &format!("<h1>Hi {}</h1><p>Welcome aboard!</p>", self.user_name),
                )
                .from("welcome@example.com"),
            )
        }
    }

    #[derive(Debug, Clone)]
    struct OrderNotification {
        order_id: String,
    }

    impl Notification for OrderNotification {
        fn via(&self) -> Vec<NotificationChannel> {
            vec![
                NotificationChannel::Mail,
                NotificationChannel::Broadcast,
            ]
        }

        fn to_mail(&self) -> Option<Mailable> {
            Some(
                Mailable::new(
                    vec![],
                    &format!("Order #{} shipped!", self.order_id),
                    &format!("Your order {} has shipped.", self.order_id),
                )
                .from("orders@example.com"),
            )
        }

        fn to_broadcast(&self) -> Option<BroadcastPayload> {
            Some(BroadcastPayload {
                event: "order.shipped".to_string(),
                data: serde_json::json!({ "order_id": self.order_id }),
            })
        }
    }

    #[derive(Debug, Clone)]
    struct EmptyNotification;

    impl Notification for EmptyNotification {
        fn via(&self) -> Vec<NotificationChannel> {
            vec![]
        }
    }

    // -------------------------------------------------------------------------
    // Notifiable trait tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_notifiable_defaults() {
        struct MinimalUser {
            id: String,
        }

        impl Notifiable for MinimalUser {
            fn notification_id(&self) -> String {
                self.id.clone()
            }
        }

        let user = MinimalUser {
            id: "42".to_string(),
        };
        assert_eq!(user.notification_id(), "42");
        assert_eq!(user.notification_type_name(), "user");
        assert_eq!(user.route_email(), None);
        assert!(user.route_broadcast_channels().is_empty());
    }

    #[test]
    fn test_notifiable_full() {
        let user = TestUser {
            id: "1".to_string(),
            email: "user@example.com".to_string(),
            broadcast_channels: vec!["user.1".to_string()],
        };
        assert_eq!(user.notification_id(), "1");
        assert_eq!(user.route_email(), Some("user@example.com".to_string()));
        assert_eq!(user.route_broadcast_channels(), vec!["user.1"]);
    }

    // -------------------------------------------------------------------------
    // Notification trait tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_notification_channels() {
        let welcome = WelcomeNotification {
            user_name: "Alice".to_string(),
        };
        assert_eq!(welcome.via(), vec![NotificationChannel::Mail]);

        let order = OrderNotification {
            order_id: "ORD-42".to_string(),
        };
        assert_eq!(
            order.via(),
            vec![NotificationChannel::Mail, NotificationChannel::Broadcast]
        );

        let empty = EmptyNotification;
        assert!(empty.via().is_empty());
    }

    #[test]
    fn test_notification_to_mail() {
        let welcome = WelcomeNotification {
            user_name: "Bob".to_string(),
        };
        let mailable = welcome.to_mail().unwrap();
        assert_eq!(mailable.subject, "Welcome, Bob!");
        assert_eq!(mailable.from, Some("welcome@example.com".to_string()));
        assert!(mailable.body.contains("Bob"));
    }

    #[test]
    fn test_notification_to_broadcast() {
        let order = OrderNotification {
            order_id: "ORD-99".to_string(),
        };
        let payload = order.to_broadcast().unwrap();
        assert_eq!(payload.event, "order.shipped");
        assert_eq!(payload.data["order_id"], "ORD-99");
    }

    #[test]
    fn test_notification_unsupported_channels_return_none() {
        let welcome = WelcomeNotification {
            user_name: "Test".to_string(),
        };
        assert!(welcome.to_broadcast().is_none());
        assert!(welcome.to_database().is_none());
    }

    // -------------------------------------------------------------------------
    // NotificationSender tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_send_mail_channel() {
        let sender = NotificationSender::new()
            .with_mailer(Arc::new(LogMailer::new("log")));

        let user = TestUser {
            id: "42".to_string(),
            email: "alice@example.com".to_string(),
            broadcast_channels: vec![],
        };

        let notification = WelcomeNotification {
            user_name: "Alice".to_string(),
        };

        let results = sender.send(&user, notification).await;
        let mail_result = results.get(&NotificationChannel::Mail);
        assert!(mail_result.is_some());
        assert!(mail_result.unwrap().is_ok());
    }

    #[tokio::test]
    async fn test_send_mail_without_email_fails() {
        struct NoEmailUser {
            id: String,
        }

        impl Notifiable for NoEmailUser {
            fn notification_id(&self) -> String {
                self.id.clone()
            }
            fn route_email(&self) -> Option<String> {
                None
            }
        }

        let sender = NotificationSender::new()
            .with_mailer(Arc::new(LogMailer::new("log")));

        let user = NoEmailUser {
            id: "1".to_string(),
        };

        let notification = WelcomeNotification {
            user_name: "Bob".to_string(),
        };

        let results = sender.send(&user, notification).await;
        let mail_result = results.get(&NotificationChannel::Mail).unwrap();
        assert!(mail_result.is_err());
        match mail_result {
            Err(NotificationError::InvalidRecipient(msg)) => {
                assert!(msg.contains("email"));
            }
            other => panic!("Expected InvalidRecipient, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_send_without_mailer_returns_error() {
        let sender = NotificationSender::new(); // no mailer configured

        let user = TestUser {
            id: "1".to_string(),
            email: "test@example.com".to_string(),
            broadcast_channels: vec![],
        };

        let notification = WelcomeNotification {
            user_name: "Test".to_string(),
        };

        let results = sender.send(&user, notification).await;
        let mail_result = results.get(&NotificationChannel::Mail).unwrap();
        assert!(mail_result.is_err());
        match mail_result {
            Err(NotificationError::ChannelNotConfigured(name)) => {
                assert_eq!(name, "mail");
            }
            other => panic!("Expected ChannelNotConfigured, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_send_multi_channel() {
        use crate::broadcasting::log::LogBroadcaster;

        let mailer: Arc<dyn Mailer> = Arc::new(LogMailer::new("log"));
        let broadcaster: Arc<dyn Broadcaster> = Arc::new(LogBroadcaster::new("log"));

        let sender = NotificationSender::new()
            .with_mailer(mailer)
            .with_broadcaster(broadcaster);

        let user = TestUser {
            id: "42".to_string(),
            email: "alice@example.com".to_string(),
            broadcast_channels: vec!["user.42".to_string()],
        };

        let notification = OrderNotification {
            order_id: "ORD-2024".to_string(),
        };

        let results = sender.send(&user, notification).await;

        let mail_result = results.get(&NotificationChannel::Mail).unwrap();
        assert!(mail_result.is_ok(), "Mail should succeed: {:?}", mail_result);

        let broadcast_result = results.get(&NotificationChannel::Broadcast).unwrap();
        assert!(
            broadcast_result.is_ok(),
            "Broadcast should succeed: {:?}",
            broadcast_result
        );
    }

    #[tokio::test]
    async fn test_send_all_succeeds() {
        let sender = NotificationSender::new()
            .with_mailer(Arc::new(LogMailer::new("log")));

        let user = TestUser {
            id: "99".to_string(),
            email: "user@example.com".to_string(),
            broadcast_channels: vec![],
        };

        let notification = WelcomeNotification {
            user_name: "User".to_string(),
        };

        let result = sender.send_all(&user, notification).await;
        assert!(result.is_ok(), "send_all should succeed: {:?}", result);
    }

    #[tokio::test]
    async fn test_send_all_with_failure() {
        let sender = NotificationSender::new(); // no services configured

        let user = TestUser {
            id: "1".to_string(),
            email: "test@example.com".to_string(),
            broadcast_channels: vec![],
        };

        let notification = WelcomeNotification {
            user_name: "Test".to_string(),
        };

        let result = sender.send_all(&user, notification).await;
        assert!(result.is_err());
    }

    // -------------------------------------------------------------------------
    // Broadcast channel tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_broadcast_without_channels_fails() {
        use crate::broadcasting::log::LogBroadcaster;

        let sender = NotificationSender::new()
            .with_broadcaster(Arc::new(LogBroadcaster::new("log")));

        let user = TestUser {
            id: "1".to_string(),
            email: "test@example.com".to_string(),
            broadcast_channels: vec![], // no channels
        };

        let notification = OrderNotification {
            order_id: "ORD-1".to_string(),
        };

        let results = sender.send(&user, notification).await;
        let broadcast_result = results.get(&NotificationChannel::Broadcast).unwrap();
        assert!(broadcast_result.is_err());
    }

    #[tokio::test]
    async fn test_broadcast_private_channel_prefixing() {
        use crate::broadcasting::log::LogBroadcaster;

        let sender = NotificationSender::new()
            .with_broadcaster(Arc::new(LogBroadcaster::new("log")));

        let user = TestUser {
            id: "1".to_string(),
            email: "test@example.com".to_string(),
            broadcast_channels: vec!["private-user.1".to_string()],
        };

        let notification = OrderNotification {
            order_id: "ORD-1".to_string(),
        };

        let results = sender.send(&user, notification).await;
        // With a pre-prefixed "private-" channel, it shouldn't double-prefix
        let broadcast_result = results.get(&NotificationChannel::Broadcast).unwrap();
        assert!(broadcast_result.is_ok());
    }

    // -------------------------------------------------------------------------
    // Database channel tests
    // -------------------------------------------------------------------------

    /// Build an in-memory SQLite sender for database channel tests.
    async fn db_sender() -> NotificationSender {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory SQLite");
        NotificationSender::new()
            .with_database(db)
    }

    #[tokio::test]
    async fn test_database_channel_without_db_returns_error() {
        let sender = NotificationSender::new(); // no db configured

        #[derive(Debug, Clone)]
        struct DbNotification;

        impl Notification for DbNotification {
            fn via(&self) -> Vec<NotificationChannel> {
                vec![NotificationChannel::Database]
            }
            fn to_database(&self) -> Option<serde_json::Value> {
                Some(serde_json::json!({"message": "test"}))
            }
        }

        struct MinimalNotifiable;

        impl Notifiable for MinimalNotifiable {
            fn notification_id(&self) -> String {
                "1".to_string()
            }
        }

        let result = sender.send(&MinimalNotifiable, DbNotification).await;
        let db_result = result.get(&NotificationChannel::Database).unwrap();
        assert!(db_result.is_err());
        match db_result {
            Err(NotificationError::ChannelNotConfigured(name)) => {
                assert_eq!(name, "database");
            }
            other => panic!("Expected ChannelNotConfigured, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_database_channel_stores_notification() {
        let sender = db_sender().await;

        #[derive(Debug, Clone)]
        struct WelcomeDbNotification {
            user_name: String,
        }

        impl Notification for WelcomeDbNotification {
            fn via(&self) -> Vec<NotificationChannel> {
                vec![NotificationChannel::Database]
            }
            fn to_database(&self) -> Option<serde_json::Value> {
                Some(serde_json::json!({
                    "title": "Welcome!",
                    "body": format!("Hi {}, welcome aboard!", self.user_name),
                }))
            }
        }

        let user = TestUser {
            id: "user-42".to_string(),
            email: "test@example.com".to_string(),
            broadcast_channels: vec![],
        };

        let result = sender.send(&user, WelcomeDbNotification {
            user_name: "Alice".to_string(),
        }).await;

        let db_result = result.get(&NotificationChannel::Database).unwrap();
        assert!(db_result.is_ok(), "Database insert should succeed: {:?}", db_result);

        // Verify the notification was stored
        let sql = "SELECT id, notifiable_id, notification_type, data FROM notifications WHERE notifiable_id = ?1";
        let row = sender.database.as_ref().unwrap()
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                sql,
                ["user-42".into()],
            ))
            .await
            .unwrap()
            .expect("Notification should exist in DB");

        let notification_type: String = row.try_get_by_index(2).unwrap();
        assert!(notification_type.contains("WelcomeDbNotification"));

        let data_json: String = row.try_get_by_index(3).unwrap();
        let data: serde_json::Value = serde_json::from_str(&data_json).unwrap();
        assert_eq!(data["title"], "Welcome!");
        assert_eq!(data["body"], "Hi Alice, welcome aboard!");
    }

    #[tokio::test]
    async fn test_database_channel_multi_notifications() {
        let sender = db_sender().await;

        #[derive(Debug, Clone)]
        struct SimpleNotification {
            msg: String,
        }

        impl Notification for SimpleNotification {
            fn via(&self) -> Vec<NotificationChannel> {
                vec![NotificationChannel::Database]
            }
            fn to_database(&self) -> Option<serde_json::Value> {
                Some(serde_json::json!({"msg": self.msg}))
            }
        }

        let user = TestUser {
            id: "multi-user".to_string(),
            email: "multi@example.com".to_string(),
            broadcast_channels: vec![],
        };

        // Send 3 notifications
        for i in 0..3 {
            let result = sender.send(&user, SimpleNotification {
                msg: format!("notification-{}", i),
            }).await;
            assert!(result.get(&NotificationChannel::Database).unwrap().is_ok());
        }

        // Verify all 3 were stored
        let sql = "SELECT COUNT(*) as cnt FROM notifications WHERE notifiable_id = ?1";
        let row = sender.database.as_ref().unwrap()
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                sql,
                ["multi-user".into()],
            ))
            .await
            .unwrap()
            .expect("Should have results");

        let count: i64 = row.try_get_by_index::<i64>(0).unwrap_or(0);
        assert_eq!(count, 3, "Should have 3 notifications stored");
    }

    // -------------------------------------------------------------------------
    // Empty notification tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_empty_notification_no_channels() {
        let sender = NotificationSender::new()
            .with_mailer(Arc::new(LogMailer::new("log")));

        let user = TestUser {
            id: "1".to_string(),
            email: "test@example.com".to_string(),
            broadcast_channels: vec![],
        };

        let results = sender.send(&user, EmptyNotification).await;
        assert!(results.is_empty(), "No channels should mean no results");
    }

    // -------------------------------------------------------------------------
    // Webhook channel tests
    // -------------------------------------------------------------------------

    /// Minimal webhook test: client configured + URL on notifiable = Webhook
    /// attempt. We validate error cases (no client, no URL) and the code path
    /// that calls the webhook.

    #[derive(Debug, Clone)]
    struct WebhookTestNotification {
        event: String,
        data: String,
    }

    impl Notification for WebhookTestNotification {
        fn via(&self) -> Vec<NotificationChannel> {
            vec![NotificationChannel::Webhook]
        }
        fn to_webhook(&self) -> Option<serde_json::Value> {
            Some(serde_json::json!({
                "event": self.event,
                "data": self.data,
            }))
        }
    }

    #[tokio::test]
    async fn test_webhook_without_client_fails() {
        let sender = NotificationSender::new(); // no webhook client

        #[derive(Debug)]
        struct WebhookUser;

        impl Notifiable for WebhookUser {
            fn notification_id(&self) -> String {
                "wh-user".to_string()
            }
            fn route_webhook_url(&self) -> Option<String> {
                Some("http://localhost:9999/hook".to_string())
            }
        }

        let results = sender.send(
            &WebhookUser,
            WebhookTestNotification {
                event: "test".to_string(),
                data: "hello".to_string(),
            },
        ).await;

        let wh_result = results.get(&NotificationChannel::Webhook).unwrap();
        assert!(wh_result.is_err());
        match wh_result {
            Err(NotificationError::ChannelNotConfigured(name)) => {
                assert_eq!(name, "webhook");
            }
            other => panic!("Expected ChannelNotConfigured, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_webhook_without_url_fails() {
        let sender = NotificationSender::new()
            .with_webhook_client(reqwest::Client::new());

        #[derive(Debug)]
        struct NoUrlUser;

        impl Notifiable for NoUrlUser {
            fn notification_id(&self) -> String {
                "no-url".to_string()
            }
            fn route_webhook_url(&self) -> Option<String> {
                None // no webhook URL
            }
        }

        let results = sender.send(
            &NoUrlUser,
            WebhookTestNotification {
                event: "ping".to_string(),
                data: "pong".to_string(),
            },
        ).await;

        let wh_result = results.get(&NotificationChannel::Webhook).unwrap();
        assert!(wh_result.is_err());
        match wh_result {
            Err(NotificationError::InvalidRecipient(msg)) => {
                assert!(msg.contains("webhook URL"));
            }
            other => panic!("Expected InvalidRecipient, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_webhook_without_payload_fails() {
        #[derive(Debug, Clone)]
        struct NoPayloadNotification;

        impl Notification for NoPayloadNotification {
            fn via(&self) -> Vec<NotificationChannel> {
                vec![NotificationChannel::Webhook]
            }
            fn to_webhook(&self) -> Option<serde_json::Value> {
                None // no payload
            }
        }

        let sender = NotificationSender::new()
            .with_webhook_client(reqwest::Client::new());

        #[derive(Debug)]
        struct PayloadlessUser;

        impl Notifiable for PayloadlessUser {
            fn notification_id(&self) -> String {
                "no-payload".to_string()
            }
            fn route_webhook_url(&self) -> Option<String> {
                Some("http://localhost:9999/hook".to_string())
            }
        }

        let results = sender.send(&PayloadlessUser, NoPayloadNotification).await;
        let wh_result = results.get(&NotificationChannel::Webhook).unwrap();
        assert!(wh_result.is_err());
        match wh_result {
            Err(NotificationError::ChannelNotSupported(name)) => {
                assert_eq!(name, "webhook");
            }
            other => panic!("Expected ChannelNotSupported, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_webhook_with_multi_channel_send() {
        use crate::broadcasting::log::LogBroadcaster;

        let sender = NotificationSender::new()
            .with_mailer(Arc::new(LogMailer::new("log")))
            .with_broadcaster(Arc::new(LogBroadcaster::new("log")))
            .with_webhook_client(
                reqwest::Client::builder()
                    .connect_timeout(std::time::Duration::from_millis(100))
                    .build()
                    .unwrap()
            );

        #[derive(Debug)]
        struct MultiChannelUser;

        impl Notifiable for MultiChannelUser {
            fn notification_id(&self) -> String {
                "multi-wh".to_string()
            }
            fn route_email(&self) -> Option<String> {
                Some("multi@example.com".to_string())
            }
            fn route_broadcast_channels(&self) -> Vec<String> {
                vec!["user.multi-wh".to_string()]
            }
            fn route_webhook_url(&self) -> Option<String> {
                Some("http://localhost:9999/hook".to_string())
            }
        }

        #[derive(Debug, Clone)]
        struct MultiNotification;

        impl Notification for MultiNotification {
            fn via(&self) -> Vec<NotificationChannel> {
                vec![
                    NotificationChannel::Mail,
                    NotificationChannel::Broadcast,
                    NotificationChannel::Webhook,
                ]
            }
            fn to_mail(&self) -> Option<Mailable> {
                Some(Mailable::new(vec![], "Test", "Body").from("t@t.com"))
            }
            fn to_broadcast(&self) -> Option<BroadcastPayload> {
                Some(BroadcastPayload {
                    event: "test".to_string(),
                    data: serde_json::json!({}),
                })
            }
            fn to_webhook(&self) -> Option<serde_json::Value> {
                Some(serde_json::json!({"msg": "hello"}))
            }
        }

        let results = sender.send(&MultiChannelUser, MultiNotification).await;

        // Mail and Broadcast should succeed
        assert!(results.get(&NotificationChannel::Mail).unwrap().is_ok());
        assert!(results.get(&NotificationChannel::Broadcast).unwrap().is_ok());

        // Webhook should fail (no actual server listening on localhost:9999)
        let wh_result = results.get(&NotificationChannel::Webhook).unwrap();
        assert!(wh_result.is_err());
        match wh_result {
            Err(NotificationError::Webhook(msg)) => {
                assert!(msg.contains("HTTP request failed") || msg.contains("Webhook returned"));
            }
            other => panic!("Expected Webhook error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_webhook_notifiable_default() {
        // Verify the default implementation returns None
        #[derive(Debug)]
        struct DefaultUser;

        impl Notifiable for DefaultUser {
            fn notification_id(&self) -> String {
                "default".to_string()
            }
        }

        let user = DefaultUser;
        assert_eq!(user.route_webhook_url(), None);
    }

    // -------------------------------------------------------------------------
    // SMS channel tests
    // -------------------------------------------------------------------------

    #[derive(Debug, Clone)]
    struct SmsTestNotification {
        content: String,
    }

    impl Notification for SmsTestNotification {
        fn via(&self) -> Vec<NotificationChannel> {
            vec![NotificationChannel::Sms]
        }
        fn to_sms(&self) -> Option<SmsMessage> {
            Some(SmsMessage::new("", &self.content).from("TestApp"))
        }
    }

    #[tokio::test]
    async fn test_sms_without_sender_fails() {
        let sender = NotificationSender::new(); // no sms sender

        #[derive(Debug)]
        struct SmsUser;

        impl Notifiable for SmsUser {
            fn notification_id(&self) -> String {
                "sms-user".to_string()
            }
            fn route_phone(&self) -> Option<String> {
                Some("+15551234567".to_string())
            }
        }

        let results = sender.send(
            &SmsUser,
            SmsTestNotification {
                content: "Hello!".to_string(),
            },
        ).await;

        let sms_result = results.get(&NotificationChannel::Sms).unwrap();
        assert!(sms_result.is_err());
        match sms_result {
            Err(NotificationError::ChannelNotConfigured(name)) => {
                assert_eq!(name, "sms");
            }
            other => panic!("Expected ChannelNotConfigured, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_sms_without_phone_fails() {
        use crate::sms::LogSmsSender;

        let sender = NotificationSender::new()
            .with_sms_sender(Arc::new(LogSmsSender::new()));

        #[derive(Debug)]
        struct NoPhoneUser;

        impl Notifiable for NoPhoneUser {
            fn notification_id(&self) -> String {
                "no-phone".to_string()
            }
            fn route_phone(&self) -> Option<String> {
                None // no phone number
            }
        }

        let results = sender.send(
            &NoPhoneUser,
            SmsTestNotification {
                content: "Ping".to_string(),
            },
        ).await;

        let sms_result = results.get(&NotificationChannel::Sms).unwrap();
        assert!(sms_result.is_err());
        match sms_result {
            Err(NotificationError::InvalidRecipient(msg)) => {
                assert!(msg.contains("phone"));
            }
            other => panic!("Expected InvalidRecipient, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_sms_without_payload_fails() {
        use crate::sms::LogSmsSender;

        #[derive(Debug, Clone)]
        struct NoPayloadSms;

        impl Notification for NoPayloadSms {
            fn via(&self) -> Vec<NotificationChannel> {
                vec![NotificationChannel::Sms]
            }
            fn to_sms(&self) -> Option<SmsMessage> {
                None // no payload
            }
        }

        let sender = NotificationSender::new()
            .with_sms_sender(Arc::new(LogSmsSender::new()));

        #[derive(Debug)]
        struct PayloadlessUser;

        impl Notifiable for PayloadlessUser {
            fn notification_id(&self) -> String {
                "no-sms-payload".to_string()
            }
            fn route_phone(&self) -> Option<String> {
                Some("+15551234567".to_string())
            }
        }

        let results = sender.send(&PayloadlessUser, NoPayloadSms).await;
        let sms_result = results.get(&NotificationChannel::Sms).unwrap();
        assert!(sms_result.is_err());
        match sms_result {
            Err(NotificationError::ChannelNotSupported(name)) => {
                assert_eq!(name, "sms");
            }
            other => panic!("Expected ChannelNotSupported, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_sms_channel_success() {
        use crate::sms::LogSmsSender;

        let sender = NotificationSender::new()
            .with_sms_sender(Arc::new(LogSmsSender::new()));

        #[derive(Debug)]
        struct SmsUser;

        impl Notifiable for SmsUser {
            fn notification_id(&self) -> String {
                "sms-success".to_string()
            }
            fn route_phone(&self) -> Option<String> {
                Some("+15551234567".to_string())
            }
        }

        let results = sender.send(
            &SmsUser,
            SmsTestNotification {
                content: "Hello from Larastvel!".to_string(),
            },
        ).await;

        let sms_result = results.get(&NotificationChannel::Sms).unwrap();
        assert!(sms_result.is_ok(), "SMS should succeed: {:?}", sms_result);
    }

    #[tokio::test]
    async fn test_sms_with_multi_channel() {
        use crate::broadcasting::log::LogBroadcaster;
        use crate::mail::LogMailer;
        use crate::sms::LogSmsSender;

        let sender = NotificationSender::new()
            .with_mailer(Arc::new(LogMailer::new("log")))
            .with_broadcaster(Arc::new(LogBroadcaster::new("log")))
            .with_sms_sender(Arc::new(LogSmsSender::new()));

        #[derive(Debug)]
        struct MultiUser;

        impl Notifiable for MultiUser {
            fn notification_id(&self) -> String {
                "multi-sms".to_string()
            }
            fn route_email(&self) -> Option<String> {
                Some("multi@example.com".to_string())
            }
            fn route_broadcast_channels(&self) -> Vec<String> {
                vec!["user.multi-sms".to_string()]
            }
            fn route_phone(&self) -> Option<String> {
                Some("+15551234567".to_string())
            }
        }

        #[derive(Debug, Clone)]
        struct AllChannelsNotification;

        impl Notification for AllChannelsNotification {
            fn via(&self) -> Vec<NotificationChannel> {
                vec![
                    NotificationChannel::Mail,
                    NotificationChannel::Broadcast,
                    NotificationChannel::Sms,
                ]
            }
            fn to_mail(&self) -> Option<Mailable> {
                Some(Mailable::new(vec![], "Multi", "Body").from("t@t.com"))
            }
            fn to_broadcast(&self) -> Option<BroadcastPayload> {
                Some(BroadcastPayload {
                    event: "test".to_string(),
                    data: serde_json::json!({}),
                })
            }
            fn to_sms(&self) -> Option<SmsMessage> {
                Some(SmsMessage::new("", "Multi-channel SMS!").from("TestApp"))
            }
        }

        let results = sender.send(&MultiUser, AllChannelsNotification).await;

        assert_eq!(results.len(), 3);
        assert!(results.get(&NotificationChannel::Mail).unwrap().is_ok());
        assert!(results.get(&NotificationChannel::Broadcast).unwrap().is_ok());
        assert!(results.get(&NotificationChannel::Sms).unwrap().is_ok());
    }

    #[tokio::test]
    async fn test_sms_notifiable_default() {
        #[derive(Debug)]
        struct DefaultUser;

        impl Notifiable for DefaultUser {
            fn notification_id(&self) -> String {
                "default".to_string()
            }
        }

        let user = DefaultUser;
        assert_eq!(user.route_phone(), None);
    }

    // -------------------------------------------------------------------------
    // BroadcastPayload serialization tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_broadcast_payload_serialization() {
        let payload = BroadcastPayload {
            event: "test.event".to_string(),
            data: serde_json::json!({"key": "value"}),
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["event"], "test.event");
        assert_eq!(json["data"]["key"], "value");
    }

    // -------------------------------------------------------------------------
    // Mailer sets default from address
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_mail_default_from_address() {
        #[derive(Debug, Clone)]
        struct NoFromNotification;

        impl Notification for NoFromNotification {
            fn via(&self) -> Vec<NotificationChannel> {
                vec![NotificationChannel::Mail]
            }
            fn to_mail(&self) -> Option<Mailable> {
                // No .from() set — sender should add the default
                Some(Mailable::new(vec![], "Test", "Body"))
            }
        }

        let user = TestUser {
            id: "1".to_string(),
            email: "user@example.com".to_string(),
            broadcast_channels: vec![],
        };

        let sender = NotificationSender::new()
            .with_mailer(Arc::new(LogMailer::new("log")))
            .with_from("default@example.com");

        let results = sender.send(&user, NoFromNotification).await;
        let mail_result = results.get(&NotificationChannel::Mail).unwrap();
        assert!(mail_result.is_ok());
    }

    // -------------------------------------------------------------------------
    // Send via convenience function tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_send_convenience_function() {
        let _ = super::send(
            &TestUser {
                id: "1".to_string(),
                email: "test@example.com".to_string(),
                broadcast_channels: vec![],
            },
            WelcomeNotification {
                user_name: "Test".to_string(),
            },
        )
        .await;
        // Default sender has no services configured, so results are errors.
        // The function exists for ergonomics when a default sender is appropriate.
    }
}
