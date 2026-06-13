//! # Multi-Channel Notification Example
//!
//! Demonstrates how to send a single notification through **Mail**, **Database**,
//! and **Broadcast** channels simultaneously using the `NotificationSender`.
//!
//! Unlike the per-controller examples (MailController, NotificationController),
//! this example focuses purely on the notification system itself — defining a
//! notification with multiple channels, configuring a sender, inspecting
//! per-channel results, and handling partial delivery.
//!
//! ## Key Concepts
//!
//! - **`via()` returns multiple channels** — Mail + Database + Broadcast
//! - **`Notifiable` supplies routing info** — email for Mail, channels for Broadcast
//! - **`send()` returns per-channel results** — inspect each channel individually
//! - **`send_all()` requires all channels** — fails if any one channel fails
//! - **Channels are independent** — a failure in Broadcast doesn't affect Mail/Database
//!
//! ## Integration
//!
//! ```ignore
//! use std::sync::Arc;
//! use larastvel_core::mail::LogMailer;
//! use larastvel_core::broadcasting::log::LogBroadcaster;
//! use larastvel_core::notifications::NotificationSender;
//!
//! let sender = NotificationSender::new()
//!     .with_mailer(Arc::new(LogMailer::new("log")))
//!     .with_broadcaster(Arc::new(LogBroadcaster::new("log")))
//!     .with_database(db)
//!     .with_from("notifications@example.com");
//! ```

#![allow(unused_imports, dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use larastvel_core::broadcasting::log::LogBroadcaster;
use larastvel_core::mail::LogMailer;
use larastvel_core::notifications::{
    BroadcastPayload, Notification, NotificationChannel, NotificationError, NotificationSender,
    Notifiable,
};
use larastvel_core::sea_orm::{self, ConnectionTrait, DatabaseBackend, Statement};
use larastvel_core::serde_json::{self, json};

// =============================================================================
// NOTIFICATION — ORDER SHIPPED
// =============================================================================

/// A notification that fires when an order ships.
///
/// Delivers through **Mail** (email to the customer), **Database** (persistent
/// record in the notifications table), and **Broadcast** (real-time WebSocket
/// event on the user's private channel).
#[derive(Debug, Clone)]
pub struct OrderShippedNotification {
    pub order_id: String,
    pub customer_name: String,
    pub total: String,
}

impl Notification for OrderShippedNotification {
    /// Deliver through all three channels simultaneously.
    fn via(&self) -> Vec<NotificationChannel> {
        vec![
            NotificationChannel::Mail,
            NotificationChannel::Database,
            NotificationChannel::Broadcast,
        ]
    }

    /// Mail representation: styled HTML email with order summary.
    fn to_mail(&self) -> Option<larastvel_core::mail::Mailable> {
        let body = format!(
            r#"<div style="font-family:sans-serif;max-width:600px;margin:0 auto;">
                <div style="background:linear-gradient(135deg,#059669,#10b981);padding:32px;text-align:center;border-radius:12px 12px 0 0;">
                    <h1 style="color:#fff;margin:0;font-size:24px;">🚚 Order Shipped!</h1>
                </div>
                <div style="background:#fff;padding:32px;border:1px solid #e2e8f0;border-radius:0 0 12px 12px;">
                    <p style="font-size:16px;color:#334155;">Hi <strong>{}</strong>,</p>
                    <p style="font-size:16px;color:#475569;">Your order <strong>#{}</strong> is on its way!</p>
                    <div style="background:#f0fdf4;border:1px solid #bbf7d0;border-radius:8px;padding:20px;margin:24px 0;">
                        <table style="width:100%;border-collapse:collapse;">
                            <tr>
                                <td style="color:#64748b;font-size:14px;padding:8px 0;">Order ID</td>
                                <td style="text-align:right;font-weight:600;font-size:14px;padding:8px 0;">#{}</td>
                            </tr>
                            <tr>
                                <td style="color:#64748b;font-size:14px;padding:8px 0;border-top:1px solid #e2e8f0;">Total</td>
                                <td style="text-align:right;font-weight:700;font-size:18px;padding:8px 0;border-top:1px solid #e2e8f0;color:#059669;">{}</td>
                            </tr>
                        </table>
                    </div>
                    <p style="font-size:14px;color:#94a3b8;">You can track your order in real-time on your dashboard.</p>
                    <hr style="border:none;border-top:1px solid #e2e8f0;margin:24px 0;">
                    <p style="font-size:14px;color:#94a3b8;">Regards,<br>The Shop Team</p>
                </div>
            </div>"#,
            self.customer_name, self.order_id, self.order_id, self.total,
        );

        Some(
            larastvel_core::mail::Mailable::html(vec![], "Your Order Has Shipped! 🚚", &body)
                .from("orders@example.com")
                .reply_to("support@example.com"),
        )
    }

    /// Broadcast representation: real-time event on the user's private channel.
    fn to_broadcast(&self) -> Option<BroadcastPayload> {
        Some(BroadcastPayload {
            event: "order.shipped".to_string(),
            data: json!({
                "order_id": self.order_id,
                "customer_name": self.customer_name,
                "total": self.total,
                "status": "shipped",
            }),
        })
    }

    /// Database representation: JSON data stored in the notifications table.
    fn to_database(&self) -> Option<serde_json::Value> {
        Some(json!({
            "type": "order.shipped",
            "order_id": self.order_id,
            "customer_name": self.customer_name,
            "total": self.total,
            "message": format!("Order #{} has shipped!", self.order_id),
        }))
    }
}

// =============================================================================
// NOTIFIABLE — SHOP USER
// =============================================================================

/// A customer who receives notifications via email, broadcast, and DB.
#[derive(Debug)]
pub struct ShopUser {
    pub id: String,
    pub email: String,
}

impl Notifiable for ShopUser {
    fn notification_id(&self) -> String {
        self.id.clone()
    }

    fn notification_type_name(&self) -> String {
        "customer".to_string()
    }

    fn route_email(&self) -> Option<String> {
        Some(self.email.clone())
    }

    fn route_broadcast_channels(&self) -> Vec<String> {
        vec![format!("user.{}", self.id)]
    }
}

// =============================================================================
// NOTIFICATION — WELCOME (MAIL + DATABASE ONLY)
// =============================================================================

/// A welcome notification that sends via Mail and Database (no broadcast).
#[derive(Debug, Clone)]
pub struct WelcomeNotification {
    pub name: String,
}

impl Notification for WelcomeNotification {
    fn via(&self) -> Vec<NotificationChannel> {
        vec![NotificationChannel::Mail, NotificationChannel::Database]
    }

    fn to_mail(&self) -> Option<larastvel_core::mail::Mailable> {
        Some(
            larastvel_core::mail::Mailable::new(
                vec![],
                &format!("Welcome to Shop, {}!", self.name),
                &format!("Hi {},\n\nThanks for joining! We're happy to have you.\n\n-The Shop Team", self.name),
            )
            .from("welcome@example.com"),
        )
    }

    fn to_database(&self) -> Option<serde_json::Value> {
        Some(json!({
            "type": "user.welcome",
            "user_name": self.name,
            "message": format!("Welcome, {}!", self.name),
        }))
    }
}

// =============================================================================
// ENTRY POINT
// =============================================================================

fn main() {
    println!("Multi-Channel Notification example — see the source code.");
    println!();
    println!("Notifications defined:");
    println!("  OrderShippedNotification — Mail + Database + Broadcast");
    println!("  WelcomeNotification      — Mail + Database (no broadcast)");
    println!();
    println!("Key patterns:");
    println!("  1. Notification::via() returns multiple channels");
    println!("  2. Notifiable supplies routing info (email, channels)");
    println!("  3. NotificationSender with .with_mailer(), .with_broadcaster(), .with_database()");
    println!("  4. send() returns HashMap<NotificationChannel, Result>");
    println!("  5. send_all() returns Ok(()) only if ALL channels succeed");
    println!();
    println!("Run the tests: cargo test --example multi_channel_notification");
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use larastvel_core::broadcasting::Broadcaster;
    use larastvel_core::mail::Mailer;

    /// Build a sender with all three channels configured (in-memory SQLite
    /// for the database channel, LogMailer for mail, LogBroadcaster for
    /// broadcast). The notifications table is auto-created on first send.
    async fn full_sender() -> (NotificationSender, sea_orm::DatabaseConnection) {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory SQLite");

        let sender = NotificationSender::new()
            .with_mailer(Arc::new(LogMailer::new("log")))
            .with_broadcaster(Arc::new(LogBroadcaster::new("log")))
            .with_database(db.clone())
            .with_from("notifications@example.com");

        (sender, db)
    }

    // -------------------------------------------------------------------------
    // All three channels: send + inspect per-channel results
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_order_shipped_all_three_channels() {
        let (sender, db) = full_sender().await;

        let user = ShopUser {
            id: "cust-42".to_string(),
            email: "alice@example.com".to_string(),
        };

        let notification = OrderShippedNotification {
            order_id: "ORD-2024-1234".to_string(),
            customer_name: "Alice".to_string(),
            total: "$79.99".to_string(),
        };

        let results = sender.send(&user, notification).await;

        // Should have exactly 3 results (Mail, Database, Broadcast)
        assert_eq!(results.len(), 3, "All 3 channels should be attempted");

        // Each channel should succeed
        for (channel, result) in &results {
            assert!(
                result.is_ok(),
                "Channel {:?} should succeed: {:?}",
                channel,
                result
            );
        }

        // Verify the notification was persisted in the database
        let sql = "SELECT notification_type, data FROM notifications WHERE notifiable_id = ?1";
        let row = db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                sql,
                ["cust-42".into()],
            ))
            .await
            .unwrap()
            .expect("Notification should exist in database");

        let notification_type: String = row.try_get_by_index(0).unwrap();
        assert!(
            notification_type.contains("OrderShippedNotification"),
            "Notification type should contain OrderShippedNotification, got: {}",
            notification_type
        );

        let data_json: String = row.try_get_by_index(1).unwrap();
        let data: serde_json::Value = serde_json::from_str(&data_json).unwrap();
        assert_eq!(data["order_id"], "ORD-2024-1234");
        assert_eq!(data["customer_name"], "Alice");
        assert_eq!(data["total"], "$79.99");
    }

    // -------------------------------------------------------------------------
    // Two channels (Mail + Database): inspect results + verify DB insert
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_welcome_mail_and_database() {
        let (sender, db) = full_sender().await;

        let user = ShopUser {
            id: "cust-1".to_string(),
            email: "bob@example.com".to_string(),
        };

        let notification = WelcomeNotification {
            name: "Bob".to_string(),
        };

        let results = sender.send(&user, notification).await;

        assert_eq!(results.len(), 2, "Should have 2 channel results");

        let mail_result = results.get(&NotificationChannel::Mail).unwrap();
        assert!(mail_result.is_ok());

        let db_result = results.get(&NotificationChannel::Database).unwrap();
        assert!(db_result.is_ok());

        // Verify DB content
        let sql = "SELECT data FROM notifications WHERE notifiable_id = ?1";
        let row = db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                sql,
                ["cust-1".into()],
            ))
            .await
            .unwrap()
            .expect("Welcome notification should exist");
        let data_json: String = row.try_get_by_index(0).unwrap();
        let data: serde_json::Value = serde_json::from_str(&data_json).unwrap();
        assert_eq!(data["type"], "user.welcome");
        assert_eq!(data["user_name"], "Bob");
    }

    // -------------------------------------------------------------------------
    // send_all(): all channels succeed
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_send_all_succeeds() {
        let (sender, _db) = full_sender().await;

        let user = ShopUser {
            id: "cust-99".to_string(),
            email: "carol@example.com".to_string(),
        };

        let notification = OrderShippedNotification {
            order_id: "ORD-SUCCESS".to_string(),
            customer_name: "Carol".to_string(),
            total: "$199.99".to_string(),
        };

        let result = sender.send_all(&user, notification).await;
        assert!(result.is_ok(), "send_all should succeed: {:?}", result);
    }

    // -------------------------------------------------------------------------
    // send_all(): Mail missing from sender (should fail)
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_send_all_fails_when_mailer_missing() {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .unwrap();

        // No mailer configured — only database and broadcast
        let sender = NotificationSender::new()
            .with_broadcaster(Arc::new(LogBroadcaster::new("log")))
            .with_database(db);

        let user = ShopUser {
            id: "cust-77".to_string(),
            email: "dave@example.com".to_string(),
        };

        let notification = OrderShippedNotification {
            order_id: "ORD-NO-MAIL".to_string(),
            customer_name: "Dave".to_string(),
            total: "$49.99".to_string(),
        };

        // send_all should fail because mail channel is not configured
        let result = sender.send_all(&user, notification).await;
        assert!(result.is_err(), "send_all should fail without mailer");
    }

    // -------------------------------------------------------------------------
    // Partial delivery: mail succeeds, database unconfigured, broadcast succeeds
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_partial_delivery_db_missing() {
        let sender = NotificationSender::new()
            .with_mailer(Arc::new(LogMailer::new("log")))
            .with_broadcaster(Arc::new(LogBroadcaster::new("log")));
        // No database configured

        let user = ShopUser {
            id: "cust-55".to_string(),
            email: "eve@example.com".to_string(),
        };

        let notification = OrderShippedNotification {
            order_id: "ORD-PARTIAL".to_string(),
            customer_name: "Eve".to_string(),
            total: "$149.99".to_string(),
        };

        let results = sender.send(&user, notification).await;

        // Should have 3 results
        assert_eq!(results.len(), 3);

        // Mail should succeed
        let mail_result = results.get(&NotificationChannel::Mail).unwrap();
        assert!(mail_result.is_ok());

        // Database should fail (not configured)
        let db_result = results.get(&NotificationChannel::Database).unwrap();
        assert!(db_result.is_err());
        match db_result {
            Err(NotificationError::ChannelNotConfigured(name)) => {
                assert_eq!(name, "database");
            }
            other => panic!("Expected ChannelNotConfigured for database, got {:?}", other),
        }

        // Broadcast should succeed
        let broadcast_result = results.get(&NotificationChannel::Broadcast).unwrap();
        assert!(broadcast_result.is_ok());
    }

    // -------------------------------------------------------------------------
    // No broadcast channels configured on notifiable
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_broadcast_fails_without_channels() {
        let sender = NotificationSender::new()
            .with_mailer(Arc::new(LogMailer::new("log")))
            .with_broadcaster(Arc::new(LogBroadcaster::new("log")))
            .with_database(
                sea_orm::Database::connect("sqlite::memory:")
                    .await
                    .unwrap(),
            );

        // User with no broadcast channels configured
        struct NoBroadcastUser {
            id: String,
            email: String,
        }

        impl Notifiable for NoBroadcastUser {
            fn notification_id(&self) -> String {
                self.id.clone()
            }
            fn route_email(&self) -> Option<String> {
                Some(self.email.clone())
            }
            fn route_broadcast_channels(&self) -> Vec<String> {
                vec![] // no broadcast channels
            }
        }

        let user = NoBroadcastUser {
            id: "cust-nobc".to_string(),
            email: "frank@example.com".to_string(),
        };

        let notification = OrderShippedNotification {
            order_id: "ORD-NO-BC".to_string(),
            customer_name: "Frank".to_string(),
            total: "$29.99".to_string(),
        };

        let results = sender.send(&user, notification).await;

        // Mail should succeed
        let mail_result = results.get(&NotificationChannel::Mail).unwrap();
        assert!(mail_result.is_ok());

        // Database should succeed
        let db_result = results.get(&NotificationChannel::Database).unwrap();
        assert!(db_result.is_ok());

        // Broadcast should fail (no channels on notifiable)
        let broadcast_result = results.get(&NotificationChannel::Broadcast).unwrap();
        assert!(broadcast_result.is_err());
        match broadcast_result {
            Err(NotificationError::InvalidRecipient(msg)) => {
                assert!(msg.contains("broadcast channels"));
            }
            other => panic!("Expected InvalidRecipient for broadcast, got {:?}", other),
        }
    }

    // -------------------------------------------------------------------------
    // Multiple notifications to the same notifiable
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_multiple_notifications_same_user() {
        let (sender, db) = full_sender().await;

        let user = ShopUser {
            id: "cust-multi".to_string(),
            email: "grace@example.com".to_string(),
        };

        // Send 3 order notifications
        for i in 0..3 {
            let notification = OrderShippedNotification {
                order_id: format!("ORD-MULTI-{}", i),
                customer_name: "Grace".to_string(),
                total: format!("${}.99", (i + 1) * 10),
            };

            let results = sender.send(&user, notification).await;
            assert_eq!(results.len(), 3);
            for (channel, result) in &results {
                assert!(result.is_ok(), "Channel {:?} failed: {:?}", channel, result);
            }
        }

        // Verify all 3 were stored in the database
        let sql = "SELECT COUNT(*) as cnt FROM notifications WHERE notifiable_id = ?1";
        let row = db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                sql,
                ["cust-multi".into()],
            ))
            .await
            .unwrap()
            .expect("Should have results");
        let count: i64 = row.try_get_by_index(0).unwrap_or(0);
        assert_eq!(count, 3, "Should have 3 notifications stored");
    }

    // -------------------------------------------------------------------------
    // Missing channel configuration on the sender
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_sender_reports_missing_configuration() {
        // Sender only has a broadcaster — no mailer, no database.
        // WelcomeNotification.via() returns [Mail, Database], so when
        // the sender iterates those channels, neither is configured and
        // both return ChannelNotConfigured errors.
        let sender = NotificationSender::new()
            .with_broadcaster(Arc::new(LogBroadcaster::new("log")));

        let user = ShopUser {
            id: "cust-nosup".to_string(),
            email: "hank@example.com".to_string(),
        };

        let notification = WelcomeNotification {
            name: "Hank".to_string(),
        };

        let results = sender.send(&user, notification).await;
        assert_eq!(results.len(), 2);

        for (channel, result) in &results {
            match channel {
                NotificationChannel::Mail => {
                    assert!(result.is_err());
                    match result {
                        Err(NotificationError::ChannelNotConfigured(name)) => {
                            assert_eq!(name, "mail");
                        }
                        other => panic!("Expected ChannelNotConfigured, got {:?}", other),
                    }
                }
                NotificationChannel::Database => {
                    assert!(result.is_err());
                    match result {
                        Err(NotificationError::ChannelNotConfigured(name)) => {
                            assert_eq!(name, "database");
                        }
                        other => panic!("Expected ChannelNotConfigured, got {:?}", other),
                    }
                }
                _ => unreachable!(),
            }
        }
    }

    // -------------------------------------------------------------------------
    // Broadcast channel prefixing
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_broadcast_channel_prefixing() {
        let sender = NotificationSender::new()
            .with_mailer(Arc::new(LogMailer::new("log")))
            .with_broadcaster(Arc::new(LogBroadcaster::new("log")))
            .with_database(
                sea_orm::Database::connect("sqlite::memory:")
                    .await
                    .unwrap(),
            );

        struct PrefixedUser;

        impl Notifiable for PrefixedUser {
            fn notification_id(&self) -> String {
                "prefixed-user".to_string()
            }
            fn route_email(&self) -> Option<String> {
                Some("prefixed@example.com".to_string())
            }
            fn route_broadcast_channels(&self) -> Vec<String> {
                // One already-prefixed, one not
                vec![
                    "private-user.1".to_string(),
                    "user.2".to_string(), // should get private- prefix
                ]
            }
        }

        let notification = OrderShippedNotification {
            order_id: "ORD-PREFIX".to_string(),
            customer_name: "Ivy".to_string(),
            total: "$39.99".to_string(),
        };

        let results = sender.send(&PrefixedUser, notification).await;

        // All three should succeed
        assert_eq!(results.len(), 3);
        for (channel, result) in &results {
            assert!(result.is_ok(), "Channel {:?} should succeed: {:?}", channel, result);
        }
    }

    // -------------------------------------------------------------------------
    // Custom from address on sender is applied to mailable without .from()
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_custom_from_address() {
        #[derive(Debug, Clone)]
        struct NoFromNotification;

        impl Notification for NoFromNotification {
            fn via(&self) -> Vec<NotificationChannel> {
                vec![NotificationChannel::Mail]
            }
            fn to_mail(&self) -> Option<larastvel_core::mail::Mailable> {
                // No .from() set — sender should apply the default
                Some(larastvel_core::mail::Mailable::new(
                    vec![],
                    "No From Set",
                    "Body",
                ))
            }
        }

        let sender = NotificationSender::new()
            .with_mailer(Arc::new(LogMailer::new("log")))
            .with_from("custom@example.com");

        let user = ShopUser {
            id: "custom-from".to_string(),
            email: "julia@example.com".to_string(),
        };

        let results = sender.send(&user, NoFromNotification).await;
        let mail_result = results.get(&NotificationChannel::Mail).unwrap();
        assert!(mail_result.is_ok());
    }

    // -------------------------------------------------------------------------
    // Empty notification (no channels) returns empty results
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_empty_notification_no_channels() {
        #[derive(Debug, Clone)]
        struct EmptyNotification;

        impl Notification for EmptyNotification {
            fn via(&self) -> Vec<NotificationChannel> {
                vec![] // no channels!
            }
        }

        let sender = NotificationSender::new()
            .with_mailer(Arc::new(LogMailer::new("log")))
            .with_broadcaster(Arc::new(LogBroadcaster::new("log")))
            .with_database(
                sea_orm::Database::connect("sqlite::memory:")
                    .await
                    .unwrap(),
            );

        let user = ShopUser {
            id: "empty-user".to_string(),
            email: "empty@example.com".to_string(),
        };

        let results = sender.send(&user, EmptyNotification).await;
        assert!(results.is_empty(), "No channels should mean no results");
    }
}
