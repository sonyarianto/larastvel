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

mod error;
mod sender;
mod types;

pub use error::*;
pub use sender::*;
pub use types::*;

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broadcasting::Broadcaster;
    use crate::mail::{LogMailer, Mailable, Mailer};
    use crate::sms::SmsMessage;
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
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
            vec![NotificationChannel::Mail, NotificationChannel::Broadcast]
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
        let sender = NotificationSender::new().with_mailer(Arc::new(LogMailer::new("log")));

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

        let sender = NotificationSender::new().with_mailer(Arc::new(LogMailer::new("log")));

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
        assert!(
            mail_result.is_ok(),
            "Mail should succeed: {:?}",
            mail_result
        );

        let broadcast_result = results.get(&NotificationChannel::Broadcast).unwrap();
        assert!(
            broadcast_result.is_ok(),
            "Broadcast should succeed: {:?}",
            broadcast_result
        );
    }

    #[tokio::test]
    async fn test_send_all_succeeds() {
        let sender = NotificationSender::new().with_mailer(Arc::new(LogMailer::new("log")));

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

        let sender =
            NotificationSender::new().with_broadcaster(Arc::new(LogBroadcaster::new("log")));

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

        let sender =
            NotificationSender::new().with_broadcaster(Arc::new(LogBroadcaster::new("log")));

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
        NotificationSender::new().with_database(db)
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

        let result = sender
            .send(
                &user,
                WelcomeDbNotification {
                    user_name: "Alice".to_string(),
                },
            )
            .await;

        let db_result = result.get(&NotificationChannel::Database).unwrap();
        assert!(
            db_result.is_ok(),
            "Database insert should succeed: {:?}",
            db_result
        );

        // Verify the notification was stored
        let sql = "SELECT id, notifiable_id, notification_type, data FROM notifications WHERE notifiable_id = ?1";
        let row = sender
            .database
            .as_ref()
            .unwrap()
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
            let result = sender
                .send(
                    &user,
                    SimpleNotification {
                        msg: format!("notification-{}", i),
                    },
                )
                .await;
            assert!(result.get(&NotificationChannel::Database).unwrap().is_ok());
        }

        // Verify all 3 were stored
        let sql = "SELECT COUNT(*) as cnt FROM notifications WHERE notifiable_id = ?1";
        let row = sender
            .database
            .as_ref()
            .unwrap()
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
        let sender = NotificationSender::new().with_mailer(Arc::new(LogMailer::new("log")));

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

        let results = sender
            .send(
                &WebhookUser,
                WebhookTestNotification {
                    event: "test".to_string(),
                    data: "hello".to_string(),
                },
            )
            .await;

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
        let sender = NotificationSender::new().with_webhook_client(reqwest::Client::new());

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

        let results = sender
            .send(
                &NoUrlUser,
                WebhookTestNotification {
                    event: "ping".to_string(),
                    data: "pong".to_string(),
                },
            )
            .await;

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

        let sender = NotificationSender::new().with_webhook_client(reqwest::Client::new());

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
                    .unwrap(),
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
        assert!(results
            .get(&NotificationChannel::Broadcast)
            .unwrap()
            .is_ok());

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

        let results = sender
            .send(
                &SmsUser,
                SmsTestNotification {
                    content: "Hello!".to_string(),
                },
            )
            .await;

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

        let sender = NotificationSender::new().with_sms_sender(Arc::new(LogSmsSender::new()));

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

        let results = sender
            .send(
                &NoPhoneUser,
                SmsTestNotification {
                    content: "Ping".to_string(),
                },
            )
            .await;

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

        let sender = NotificationSender::new().with_sms_sender(Arc::new(LogSmsSender::new()));

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

        let sender = NotificationSender::new().with_sms_sender(Arc::new(LogSmsSender::new()));

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

        let results = sender
            .send(
                &SmsUser,
                SmsTestNotification {
                    content: "Hello from Larastvel!".to_string(),
                },
            )
            .await;

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
        assert!(results
            .get(&NotificationChannel::Broadcast)
            .unwrap()
            .is_ok());
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
