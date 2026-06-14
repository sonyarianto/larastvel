use thiserror::Error;

use super::types::NotificationChannel;

/// Errors that can occur during notification delivery.
#[derive(Debug, Error)]
pub enum NotificationError {
    /// The channel is not supported by the notification.
    #[error("Channel {0} is not supported by this notification")]
    ChannelNotSupported(String),

    /// The channel is not configured on the sender.
    #[error("Channel {0} is not configured on the sender")]
    ChannelNotConfigured(String),

    /// The recipient did not provide the required route for a channel.
    #[error("Invalid recipient: {0}")]
    InvalidRecipient(String),

    /// Mail delivery failed.
    #[error(transparent)]
    Mail(#[from] crate::mail::MailError),

    /// Broadcast delivery failed.
    #[error(transparent)]
    Broadcast(#[from] crate::broadcasting::BroadcastError),

    /// Database storage failed.
    #[error("Database error: {0}")]
    Database(String),

    /// Webhook delivery failed.
    #[error("Webhook error: {0}")]
    Webhook(String),

    /// SMS delivery failed.
    #[error(transparent)]
    Sms(#[from] crate::sms::SmsError),

    /// Some channels succeeded but others failed.
    #[error("Partial delivery: {0:?}")]
    PartialDelivery(Vec<(NotificationChannel, NotificationError)>),
}
