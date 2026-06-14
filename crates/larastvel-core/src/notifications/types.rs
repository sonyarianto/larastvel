use serde::Serialize;

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
    fn to_mail(&self) -> Option<crate::mail::Mailable> {
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
    fn to_webhook(&self) -> Option<serde_json::Value> {
        None
    }

    /// Return the SMS representation of the notification.
    fn to_sms(&self) -> Option<crate::sms::SmsMessage> {
        None
    }
}
