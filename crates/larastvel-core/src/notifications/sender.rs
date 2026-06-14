use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use uuid::Uuid;

use crate::broadcasting::{BroadcastMessage, Broadcaster};
use crate::mail::Mailer;
use crate::sms::SmsSender;

use super::types::{Notifiable, Notification, NotificationChannel};
use super::NotificationError;

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
    pub(crate) mailer: Option<Arc<dyn Mailer>>,
    pub(crate) broadcaster: Option<Arc<dyn Broadcaster>>,
    pub(crate) database: Option<sea_orm::DatabaseConnection>,
    pub(crate) webhook_client: Option<reqwest::Client>,
    pub(crate) sms_sender: Option<Arc<dyn SmsSender>>,
    pub(crate) from_address: String,
    pub(crate) app_url: String,
    pub(crate) app_name: String,
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
    pub async fn send<N: Notification>(
        &self,
        notifiable: &dyn Notifiable,
        notification: N,
    ) -> HashMap<NotificationChannel, Result<(), NotificationError>> {
        let channels = notification.via();
        let mut results = HashMap::new();

        for channel in &channels {
            let result = match channel {
                NotificationChannel::Mail => self.send_mail(notifiable, &notification).await,
                NotificationChannel::Broadcast => {
                    self.send_broadcast(notifiable, &notification).await
                }
                NotificationChannel::Database => {
                    self.send_database(notifiable, &notification).await
                }
                NotificationChannel::Webhook => self.send_webhook(notifiable, &notification).await,
                NotificationChannel::Sms => self.send_sms(notifiable, &notification).await,
            };
            results.insert(channel.clone(), result);
        }

        results
    }

    /// Send a notification and return `Ok(())` only if ALL channels succeed.
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

        mailable.to = vec![email];

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
        let broadcaster =
            self.broadcaster
                .as_ref()
                .ok_or(NotificationError::ChannelNotConfigured(
                    "broadcast".to_string(),
                ))?;

        let payload = notification
            .to_broadcast()
            .ok_or(NotificationError::ChannelNotSupported(
                "broadcast".to_string(),
            ))?;

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
    pub async fn ensure_notifications_table(&self) -> Result<(), NotificationError> {
        let db = self
            .database
            .as_ref()
            .ok_or(NotificationError::ChannelNotConfigured(
                "database".to_string(),
            ))?;

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

        db.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            sql.to_string(),
        ))
        .await
        .map_err(|e| {
            NotificationError::Database(format!("Failed to create notifications table: {}", e))
        })?;

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

        sms.to = vec![phone];

        sender.send(sms).await.map_err(NotificationError::Sms)?;

        Ok(())
    }

    async fn send_webhook<N: Notification>(
        &self,
        notifiable: &dyn Notifiable,
        notification: &N,
    ) -> Result<(), NotificationError> {
        let client =
            self.webhook_client
                .as_ref()
                .ok_or(NotificationError::ChannelNotConfigured(
                    "webhook".to_string(),
                ))?;

        let url = notifiable
            .route_webhook_url()
            .ok_or_else(|| NotificationError::InvalidRecipient("No webhook URL".to_string()))?;

        let payload = notification
            .to_webhook()
            .ok_or(NotificationError::ChannelNotSupported(
                "webhook".to_string(),
            ))?;

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
        self.ensure_notifications_table().await?;

        let db = self
            .database
            .as_ref()
            .ok_or(NotificationError::ChannelNotConfigured(
                "database".to_string(),
            ))?;

        let data = notification
            .to_database()
            .ok_or(NotificationError::ChannelNotSupported(
                "database".to_string(),
            ))?;

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
        let data_json = serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string());

        let sql = "INSERT INTO notifications \
             (id, notifiable_id, notifiable_type, notification_type, data, \
              read_at, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?6)";

        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            sql,
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
pub async fn send<N: Notification>(
    notifiable: &dyn Notifiable,
    notification: N,
) -> HashMap<NotificationChannel, Result<(), NotificationError>> {
    NotificationSender::new()
        .send(notifiable, notification)
        .await
}

/// Send a notification using a configured sender.
pub async fn send_via<N: Notification>(
    sender: &NotificationSender,
    notifiable: &dyn Notifiable,
    notification: N,
) -> HashMap<NotificationChannel, Result<(), NotificationError>> {
    sender.send(notifiable, notification).await
}
